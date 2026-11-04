//! Full-power mode: for the ideas it covers the bot uses the strongest model at the highest effort, and may
//! work in the owner's work hours (it is the owner who wants the answer now, e.g. to think a problem through).
//!
//! Scope (setting `power_mode`): `off` | `ideas` (only the ideas marked 🚀, `power_ideas`) | `all`.
//! It lifts the thrift rules — the reserve kept for the owner's own work, the daily allowance, the cheaper-model
//! tuning — and nothing else: a paused bot stays paused, and the hard limits (a weekly total near 100%,
//! a 5-hour window near its end) still stop it.
use rusqlite::Connection;
use std::collections::BTreeSet;

use crate::db;

pub fn mode(c: &Connection) -> String {
    match db::get_setting(c, "power_mode").as_deref() {
        Some("ideas") => "ideas".into(),
        Some("all") => "all".into(),
        _ => "off".into(),
    }
}

pub fn ideas(c: &Connection) -> BTreeSet<i64> {
    db::get_setting(c, "power_ideas")
        .unwrap_or_default()
        .split(',')
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .collect()
}

pub fn is_on(c: &Connection, idea_id: i64) -> bool {
    match mode(c).as_str() {
        "all" => true,
        "ideas" => ideas(c).contains(&idea_id),
        _ => false,
    }
}

/// Mark / unmark one idea (turns the mode on for marked ideas if it was off).
pub fn set_idea(c: &Connection, idea_id: i64, on: bool) -> Result<(), String> {
    let mut set = ideas(c);
    if on {
        set.insert(idea_id);
    } else {
        set.remove(&idea_id);
    }
    let csv = set.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
    db::set_setting(c, "power_ideas", &csv).map_err(|e| e.to_string())?;
    if on && mode(c) == "off" {
        db::set_setting(c, "power_mode", "ideas").map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        let p = std::env::temp_dir().join(format!("ia-power-{}-{:?}.db", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_file(&p);
        db::open(&p).unwrap()
    }

    #[test]
    fn scope_is_all_none_or_the_marked_ideas() {
        let c = conn();
        assert!(!is_on(&c, 1));
        set_idea(&c, 3, true).unwrap();
        assert_eq!(mode(&c), "ideas");
        assert!(is_on(&c, 3) && !is_on(&c, 4));
        db::set_setting(&c, "power_mode", "all").unwrap();
        assert!(is_on(&c, 4));
        db::set_setting(&c, "power_mode", "off").unwrap();
        assert!(!is_on(&c, 3));
        set_idea(&c, 3, false).unwrap();
        assert!(ideas(&c).is_empty());
    }
}
