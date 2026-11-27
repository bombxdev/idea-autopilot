//! Backup accounts when an idea's account hits its limit (ISSUE-176) — for the bot's runs and for chats alike.
//!
//! Setting `account_switch`:
//! - "auto" (default): carry on with the backups in order, then with any account that still has quota
//! - "list": only the backups the owner set, in order; when none can work, wait for the idea's own account
//! - "wait": never switch; wait until the idea's account has quota again
//! Backups: the idea's own list first (`IdeaAi::backups`, "+ เพิ่มโมเดลสำรอง" on the idea), then the list for every
//! idea (setting `backup_accounts`, comma-separated, in order).

use crate::state::AppState;
use rusqlite::Connection;

pub fn mode(c: &Connection) -> String {
    match crate::db::get_setting(c, "account_switch").as_deref() {
        Some("wait") => "wait".into(),
        Some("list") => "list".into(),
        _ => "auto".into(),
    }
}

pub fn global_backups(c: &Connection) -> Vec<String> {
    crate::db::get_setting(c, "backup_accounts").map(|v| v.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or_default()
}

/// The accounts to try for an idea, in order: its own (or `primary`), its backups, then the backups for every idea.
pub fn chain(c: &Connection, idea_id: i64, primary: &str) -> Vec<String> {
    let cfg = crate::idea_ai::get(c, idea_id).unwrap_or_default();
    let usable: Vec<String> = crate::accounts::usable(c).into_iter().map(|a| a.id).collect();
    let mut out: Vec<String> = vec![];
    let first = if primary.is_empty() { cfg.account.clone() } else { primary.to_string() };
    for a in std::iter::once(first).chain(cfg.backups.into_iter()).chain(global_backups(c)) {
        if !a.is_empty() && usable.contains(&a) && !out.contains(&a) {
            out.push(a);
        }
    }
    out
}

/// Can this account work now: not at its limit, not held for a sign-in problem, not at 100% of its quota.
pub fn available(st: &AppState, c: &Connection, account: &str) -> bool {
    let now = crate::db::now();
    crate::limits::get(c, account, now).is_none()
        && crate::connect::parked(c, account, now).is_none()
        && st.rt.lock().acct_snaps.get(account).is_none_or(|s| s.week_pct < 100.0 && s.five_pct < 100.0)
}

/// The account to use instead of `primary` for this idea, when `primary` cannot work now (None = keep / wait).
/// `same_kind`: only an account of this program (a chat that cannot change program mid-way).
pub fn replacement(st: &AppState, idea_id: i64, primary: &str, same_kind: Option<&str>) -> Option<String> {
    let c = st.db.lock();
    let m = mode(&c);
    if m == "wait" || available(st, &c, primary) {
        return None;
    }
    let kind_ok = |id: &str| same_kind.is_none_or(|k| crate::accounts::find(&c, id).is_some_and(|a| a.kind == k));
    let ch = chain(&c, idea_id, primary);
    if let Some(a) = ch.iter().skip(1).find(|a| kind_ok(a) && available(st, &c, a)) {
        return Some(a.clone());
    }
    if m == "auto" {
        return crate::accounts::usable(&c).into_iter().map(|a| a.id).find(|a| a != primary && kind_ok(a) && available(st, &c, a));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_idea_backups_come_before_the_ones_for_every_idea() {
        let dir = std::env::temp_dir().join(format!("ia_fb_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let c = crate::db::open(&dir.join("t.db")).unwrap();
        crate::db::set_setting(&c, "backup_accounts", "g1, b2").unwrap();
        assert_eq!(mode(&c), "auto");
        crate::db::set_setting(&c, "account_switch", "list").unwrap();
        assert_eq!(mode(&c), "list");
        assert_eq!(global_backups(&c), vec!["g1".to_string(), "b2".to_string()]);
        // accounts that are not set up on this machine are left out of the chain
        assert!(chain(&c, 9, "nope").is_empty());
    }
}
