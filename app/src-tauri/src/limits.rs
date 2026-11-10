//! When an account hits its usage limit: remember until when, and why, so the bot leaves that account alone until the
//! quota comes back (and says so on screen), then carries on by itself.
//!
//! The time comes from the AI's own message when it gives one ("try again in 4 days 2 hours", "try again at Nov 9th,
//! 2026 3:12 AM", "resets at 3:45 PM"), else from the quota reading of that account (the 5-hour or weekly reset), else
//! a short pause. Kept in one setting (`acct_limits`) so a restart does not forget it.
use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::collections::BTreeMap;

use crate::db;
use crate::usage::Snapshot;

const MIN_WAIT: i64 = 120;
const MAX_WAIT: i64 = 8 * 86400;
const FALLBACK_WAIT: i64 = 20 * 60;

fn load(c: &Connection) -> BTreeMap<String, (i64, String)> {
    db::get_setting(c, "acct_limits").and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save(c: &Connection, m: &BTreeMap<String, (i64, String)>) {
    if let Ok(s) = serde_json::to_string(m) {
        let _ = db::set_setting(c, "acct_limits", &s);
    }
}

pub fn set(c: &Connection, account: &str, until: i64, why: &str) {
    let mut m = load(c);
    m.insert(account.to_string(), (until, why.to_string()));
    save(c, &m);
}

pub fn clear(c: &Connection, account: &str) {
    let mut m = load(c);
    if m.remove(account).is_some() {
        save(c, &m);
    }
}

/// (until, why) while the account is still limited; an elapsed one is dropped.
pub fn get(c: &Connection, account: &str, now: i64) -> Option<(i64, String)> {
    let m = load(c);
    let (until, why) = m.get(account)?.clone();
    if until > now {
        Some((until, why))
    } else {
        clear(c, account);
        None
    }
}

/// For the screen: { account: { until, why } } of every account still limited.
pub fn all_json(c: &Connection, now: i64) -> Value {
    let m: BTreeMap<String, Value> = load(c).into_iter().filter(|(_, (u, _))| *u > now).map(|(k, (u, w))| (k, json!({ "until": u, "why": w }))).collect();
    json!(m)
}

/// "HH:MM" today, or "d MMM HH:MM" when it is another day.
pub fn when_text(until: i64, now: i64) -> String {
    let Some(t) = Local.timestamp_opt(until, 0).single() else { return String::new() };
    let today = Local.timestamp_opt(now, 0).single().map(|n| n.date_naive());
    if Some(t.date_naive()) == today {
        t.format("%H:%M").to_string()
    } else {
        t.format("%d/%m %H:%M").to_string()
    }
}

const UNITS: [(&str, i64); 4] = [("day", 86400), ("hour", 3600), ("min", 60), ("sec", 1)];

fn unit_secs(word: &str) -> Option<i64> {
    let w = word.trim_matches(|c: char| !c.is_ascii_alphabetic());
    if w.is_empty() {
        return None;
    }
    for (name, s) in UNITS {
        if w.starts_with(name) {
            return Some(s);
        }
    }
    match w {
        "d" => Some(86400),
        "h" | "hr" | "hrs" => Some(3600),
        "m" => Some(60),
        "s" => Some(1),
        _ => None,
    }
}

/// "4 days 2 hours 3 minutes", "2h 30m", "45 minutes" → seconds, from the text that follows "in".
fn parse_relative(rest: &str) -> Option<i64> {
    let mut total = 0;
    let mut found = false;
    let mut toks = rest.split(|c: char| c.is_whitespace() || c == ',').filter(|t| !t.is_empty()).peekable();
    while let Some(t) = toks.next() {
        if t == "and" {
            continue;
        }
        // "2h" glued
        let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            break;
        }
        let n: i64 = digits.parse().ok()?;
        let glued = &t[digits.len()..];
        let unit = if !glued.is_empty() { unit_secs(glued) } else { toks.next().and_then(unit_secs) };
        match unit {
            Some(u) => {
                total += n * u;
                found = true;
            }
            None => break,
        }
    }
    found.then_some(total)
}

fn month_num(m: &str) -> Option<u32> {
    let m = &m[..m.len().min(3)];
    ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"].iter().position(|x| *x == m).map(|i| i as u32 + 1)
}

/// "3:45 pm", "15:45", "3 am" → time of day
fn parse_clock(s: &str) -> Option<NaiveTime> {
    let s = s.trim().trim_end_matches(|c: char| c == '.' || c == ',');
    let (body, pm) = if let Some(b) = s.strip_suffix("pm") {
        (b.trim(), Some(true))
    } else if let Some(b) = s.strip_suffix("am") {
        (b.trim(), Some(false))
    } else {
        (s, None)
    };
    let (h, m) = match body.split_once(':') {
        Some((h, m)) => (h.parse::<u32>().ok()?, m.get(..2).unwrap_or(m).parse::<u32>().ok()?),
        None => (body.parse::<u32>().ok()?, 0),
    };
    let h = match pm {
        Some(true) if h < 12 => h + 12,
        Some(false) if h == 12 => 0,
        _ => h,
    };
    NaiveTime::from_hms_opt(h, m, 0)
}

/// When the AI's message says the quota is back (unix seconds), if it says.
pub fn parse_reset(text: &str, now: i64) -> Option<i64> {
    let lower = text.to_lowercase();
    // the HTTP header, passed on by the agent: "retry-after: 30" / "retry_after_seconds\":30" (seconds)
    for key in ["retry-after:", "retry-after\":", "retry_after_seconds\":", "retry_after\":", "retryafter\":"] {
        if let Some(i) = lower.find(key) {
            let digits: String = lower[i + key.len()..].trim_start().trim_start_matches('"').chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = digits.parse::<i64>() {
                return Some(now + n);
            }
        }
    }
    // relative: "try again in 4 days 2 hours", "resets in 45 minutes"
    for key in ["again in ", "resets in ", "reset in ", "available in ", "retry in ", "retry after ", "try again after "] {
        if let Some(i) = lower.find(key) {
            if let Some(s) = parse_relative(&lower[i + key.len()..]) {
                return Some(now + s);
            }
        }
    }
    // absolute: "try again at Nov 9th, 2026 3:12 AM" / "resets at 3:45 PM"
    for key in ["again at ", "resets at ", "reset at ", "available at ", "resets on ", "again on "] {
        let Some(i) = lower.find(key) else { continue };
        let rest: Vec<&str> = lower[i + key.len()..].split_whitespace().take(6).collect();
        // with a date: month day[, year] time [am|pm]
        if let Some(mon) = rest.first().and_then(|m| month_num(m)) {
            let day: u32 = rest.get(1)?.trim_matches(|c: char| !c.is_ascii_digit()).parse().ok()?;
            let (year, ti) = match rest.get(2).map(|y| y.trim_matches(|c: char| !c.is_ascii_digit())).and_then(|y| y.parse::<i32>().ok()).filter(|y| *y > 1999) {
                Some(y) => (y, 3),
                None => (Local.timestamp_opt(now, 0).single()?.year(), 2),
            };
            let clock = format!("{}{}", rest.get(ti)?, rest.get(ti + 1).filter(|x| x.starts_with("am") || x.starts_with("pm")).map(|x| format!(" {}", &x[..2])).unwrap_or_default());
            let clock = clock.replace(' ', "");
            let t = parse_clock(&clock)?;
            let d = NaiveDate::from_ymd_opt(year, mon, day)?;
            return Local.from_local_datetime(&NaiveDateTime::new(d, t)).single().map(|x| x.timestamp());
        }
        // clock only: today, or tomorrow when that time has passed
        let clock = format!("{}{}", rest.first()?, rest.get(1).filter(|x| x.starts_with("am") || x.starts_with("pm")).map(|x| &x[..2]).unwrap_or(""));
        let t = parse_clock(&clock)?;
        let n = Local.timestamp_opt(now, 0).single()?;
        let mut at = Local.from_local_datetime(&NaiveDateTime::new(n.date_naive(), t)).single()?;
        if at.timestamp() <= now {
            at += chrono::Duration::days(1);
        }
        return Some(at.timestamp());
    }
    None
}

/// The first sentence of the AI's message that talks about the limit, short enough for the screen.
pub fn why(text: &str) -> String {
    let flat = text.replace('\r', "");
    let hit = flat
        .split(['.', '!', '\n'])
        .map(str::trim)
        .find(|s| {
            let l = s.to_lowercase();
            ["limit", "quota", "429", "too many requests", "exhausted"].iter().any(|w| l.contains(w))
        })
        .unwrap_or(flat.trim());
    hit.chars().take(200).collect()
}

/// Until when to leave the account alone: the AI's own time, else this account's own quota reading, else a short pause.
pub fn wait_until(text: &str, snap: Option<&Snapshot>, now: i64) -> i64 {
    let at = parse_reset(text, now).or_else(|| {
        let s = snap?;
        // the window that is (nearly) full decides; the weekly one wins when both are
        if s.week_pct >= 95.0 {
            s.week_reset
        } else if s.five_pct >= 90.0 {
            s.five_reset
        } else {
            None
        }
    });
    at.unwrap_or(now + FALLBACK_WAIT).clamp(now + MIN_WAIT, now + MAX_WAIT)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_793_000_000;

    #[test]
    fn relative_times_are_read() {
        assert_eq!(parse_reset("You've hit your usage limit. Try again in 4 days 2 hours 3 minutes.", NOW), Some(NOW + 4 * 86400 + 2 * 3600 + 3 * 60));
        assert_eq!(parse_reset("rate limit, resets in 45 minutes", NOW), Some(NOW + 45 * 60));
        assert_eq!(parse_reset("retry after 2h 30m", NOW), Some(NOW + 9000));
        assert_eq!(parse_reset("quota exceeded", NOW), None);
        assert_eq!(parse_reset("429 Too Many Requests\nRetry-After: 30", NOW), Some(NOW + 30));
        assert_eq!(parse_reset("{\"error\":{\"retry_after_seconds\":45}}", NOW), Some(NOW + 45));
    }

    #[test]
    fn absolute_times_are_read() {
        let t = parse_reset("You've hit your usage limit. Try again at Nov 9th, 2026 3:12 AM.", NOW).unwrap();
        let d = Local.timestamp_opt(t, 0).single().unwrap();
        assert_eq!((d.year(), d.month(), d.day(), d.format("%H:%M").to_string()), (2026, 11, 9, "03:12".to_string()));
        let c = parse_reset("limit reached, resets at 3:45 PM", NOW).unwrap();
        assert!(c > NOW && c <= NOW + 86400);
        assert_eq!(Local.timestamp_opt(c, 0).single().unwrap().format("%H:%M").to_string(), "15:45");
    }

    #[test]
    fn without_a_time_the_accounts_own_reading_then_a_short_pause_decide() {
        let snap = Snapshot { ts: NOW, five_pct: 100.0, five_reset: Some(NOW + 3000), week_pct: 40.0, week_reset: Some(NOW + 400_000), source: "codex".into() };
        assert_eq!(wait_until("usage limit", Some(&snap), NOW), NOW + 3000);
        let weekly = Snapshot { week_pct: 99.0, ..snap.clone() };
        assert_eq!(wait_until("usage limit", Some(&weekly), NOW), NOW + 400_000);
        assert_eq!(wait_until("usage limit", None, NOW), NOW + FALLBACK_WAIT);
        // never absurd
        assert_eq!(wait_until("try again in 90 days", None, NOW), NOW + MAX_WAIT);
    }

    #[test]
    fn the_reason_is_the_sentence_about_the_limit() {
        assert_eq!(why("Error: something\nYou've hit your usage limit. Upgrade to Pro. Try again in 2 hours."), "You've hit your usage limit");
    }

    #[test]
    fn a_limit_is_remembered_until_it_ends() {
        let path = std::env::temp_dir().join(format!("ia-limits-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let c = db::open(&path).unwrap();
        set(&c, "codex01", NOW + 1000, "limit");
        assert_eq!(get(&c, "codex01", NOW).map(|x| x.0), Some(NOW + 1000));
        assert!(get(&c, "codex01", NOW + 2000).is_none());
        assert!(get(&c, "codex01", NOW).is_none(), "an ended limit is dropped");
    }
}
