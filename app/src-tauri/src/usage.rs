//! Reads plan quota (5-hour window and weekly) the same way `/usage` does.
//!
//! Order of preference:
//! 1. `~/.claude/usage-cache.json` when it is fresh (the usage widget refreshes it every 5 min,
//!    so we don't hit the API twice).
//! 2. `GET https://api.anthropic.com/api/oauth/usage` with the access token from
//!    `~/.claude/.credentials.json` (read-only; we never refresh tokens ourselves).
//! 3. A stale cache — returned with its real timestamp so the governor can refuse to act on it.
//!
//! This endpoint is not a documented public API, so all parsing lives in `parse()`.
//!
//! Agents without a readable quota (Codex, Gemini, OpenCode, custom) use `budget_snapshot`:
//! the app meters itself — hours the bot actually ran against a weekly budget the user sets —
//! and reports it in the same shape, so the governor and calendar rules work unchanged.
use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const FRESH_SECS: i64 = 8 * 60;

#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    /// unix seconds when the numbers were fetched
    pub ts: i64,
    pub five_pct: f64,
    pub five_reset: Option<i64>,
    pub week_pct: f64,
    pub week_reset: Option<i64>,
    pub source: String,
}

pub fn claude_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".claude")
}

fn ts(v: &Value) -> Option<i64> {
    v.as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.timestamp())
}

pub fn parse(v: &Value, fetched_at: i64, source: &str) -> Result<Snapshot> {
    let five = v.get("five_hour").ok_or_else(|| anyhow!("no five_hour"))?;
    let week = v.get("seven_day").ok_or_else(|| anyhow!("no seven_day"))?;
    Ok(Snapshot {
        ts: fetched_at,
        five_pct: five.get("utilization").and_then(Value::as_f64).unwrap_or(0.0),
        five_reset: five.get("resets_at").and_then(ts),
        week_pct: week
            .get("utilization")
            .and_then(Value::as_f64)
            .ok_or_else(|| anyhow!("no weekly utilization"))?,
        week_reset: week.get("resets_at").and_then(ts),
        source: source.into(),
    })
}

fn read_cache(dir: &std::path::Path) -> Result<Snapshot> {
    let raw = std::fs::read_to_string(dir.join("usage-cache.json")).context("read usage-cache.json")?;
    let v: Value = serde_json::from_str(&raw)?;
    let fetched = v
        .get("_fetched_at")
        .and_then(ts)
        .or_else(|| {
            std::fs::metadata(dir.join("usage-cache.json"))
                .and_then(|m| m.modified())
                .ok()
                .map(|t| chrono::DateTime::<chrono::Utc>::from(t).timestamp())
        })
        .unwrap_or(0);
    parse(&v, fetched, "cache")
}

fn access_token(dir: &std::path::Path) -> Result<String> {
    let raw = std::fs::read_to_string(dir.join(".credentials.json"))?;
    let v: Value = serde_json::from_str(&raw)?;
    let o = v.get("claudeAiOauth").unwrap_or(&v);
    let exp = o.get("expiresAt").and_then(Value::as_i64).unwrap_or(0);
    if exp > 0 && exp / 1000 < crate::db::now() + 60 {
        return Err(anyhow!("access token expired"));
    }
    o.get("accessToken")
        .and_then(Value::as_str)
        .map(String::from)
        .ok_or_else(|| anyhow!("no access token"))
}

async fn fetch_api(dir: &std::path::Path) -> Result<Snapshot> {
    let token = access_token(dir)?;
    let resp = reqwest::Client::new()
        .get(USAGE_URL)
        .bearer_auth(token)
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("User-Agent", "idea-autopilot/0.1")
        .header("Accept", "application/json")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(anyhow!("usage api http {}", resp.status()));
    }
    let v: Value = resp.json().await?;
    parse(&v, crate::db::now(), "api")
}

/// Best available snapshot plus a note when something went wrong.
pub async fn fetch() -> (Option<Snapshot>, Option<String>) {
    fetch_in(&claude_dir()).await
}

/// Same for a Claude login kept in its own folder (an account with its own `home`).
pub async fn fetch_in(dir: &std::path::Path) -> (Option<Snapshot>, Option<String>) {
    let cache = read_cache(dir);
    if let Ok(s) = &cache {
        if crate::db::now() - s.ts < FRESH_SECS {
            return (Some(s.clone()), None);
        }
    }
    match fetch_api(dir).await {
        Ok(s) => (Some(s), None),
        Err(e) => match cache {
            Ok(s) => (Some(s), Some(format!("ใช้ค่าเก่าจาก cache ({e})"))),
            Err(c) => (None, Some(format!("อ่านโควต้าไม่ได้: {e}; cache: {c}"))),
        },
    }
}

// ---------- Codex: the real quota, read from its own session logs ----------

/// Where a Codex login lives: the account's own folder, else `CODEX_HOME`, else `~/.codex`.
pub fn codex_home(account_home: &str) -> PathBuf {
    if !account_home.is_empty() {
        return PathBuf::from(account_home);
    }
    std::env::var_os("CODEX_HOME").map(PathBuf::from).unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".codex"))
}

fn rollout_files(dir: &std::path::Path, out: &mut Vec<(std::time::SystemTime, PathBuf)>, depth: u8) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() && depth < 5 {
            rollout_files(&p, out, depth + 1);
        } else if p.extension().is_some_and(|x| x == "jsonl") {
            if let Ok(t) = e.metadata().and_then(|m| m.modified()) {
                out.push((t, p));
            }
        }
    }
}

/// Codex writes its plan limits into every session log (`token_count` events carry `rate_limits`: the 5-hour window
/// as `primary`, the weekly one as `secondary`). The newest such event is what `/status` in Codex would show, so
/// the real quota is read from there. A window whose reset time has passed counts as empty again.
pub fn codex_snapshot(home: &std::path::Path, now: i64) -> Option<Snapshot> {
    let mut files = Vec::new();
    rollout_files(&home.join("sessions"), &mut files, 0);
    files.sort_by_key(|f| std::cmp::Reverse(f.0));
    for (_, path) in files.into_iter().take(8) {
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        for line in text.lines().rev() {
            if !line.contains("rate_limits") {
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
            let Some(rl) = v.pointer("/payload/rate_limits") else { continue };
            let win = |k: &str| {
                let w = rl.get(k)?;
                Some((w.get("used_percent").and_then(Value::as_f64)?, w.get("window_minutes").and_then(Value::as_i64).unwrap_or(0), w.get("resets_at").and_then(Value::as_i64)))
            };
            let (Some(a), Some(b)) = (win("primary"), win("secondary")) else { continue };
            // the shorter window is the 5-hour one
            let (five, week) = if a.1 <= b.1 { (a, b) } else { (b, a) };
            let live = |w: (f64, i64, Option<i64>)| match w.2 {
                Some(r) if r <= now => (0.0, None),
                r => (w.0, r),
            };
            let (five_pct, five_reset) = live(five);
            let (week_pct, week_reset) = live(week);
            return Some(Snapshot { ts: now, five_pct, five_reset, week_pct, week_reset, source: "codex".into() });
        }
    }
    None
}

/// The user's self-metered budget (settings `budget_hours_week`, `budget_hours_5h`, `budget_reset_day`).
#[derive(Debug, Clone, Copy)]
pub struct BudgetCfg {
    pub hours_week: f64,
    pub hours_5h: f64,
    /// 0 = Monday … 6 = Sunday; the week budget starts again at 00:00 that day
    pub reset_day: u32,
}

impl Default for BudgetCfg {
    fn default() -> Self {
        BudgetCfg { hours_week: 10.0, hours_5h: 3.0, reset_day: 0 }
    }
}

impl BudgetCfg {
    pub fn load(c: &rusqlite::Connection) -> Self {
        let d = Self::default();
        let num = |k: &str, dv: f64| crate::db::get_setting(c, k).and_then(|v| v.trim().parse::<f64>().ok()).filter(|v| *v > 0.0).unwrap_or(dv);
        BudgetCfg {
            hours_week: num("budget_hours_week", d.hours_week),
            hours_5h: num("budget_hours_5h", d.hours_5h),
            reset_day: crate::db::get_setting(c, "budget_reset_day").and_then(|v| v.trim().parse::<u32>().ok()).filter(|v| *v < 7).unwrap_or(d.reset_day),
        }
    }
}

fn overlap(span: (i64, Option<i64>), lo: i64, hi: i64, now: i64) -> i64 {
    let (a, b) = (span.0.max(lo), span.1.unwrap_or(now).min(hi));
    (b - a).max(0)
}

/// Start of the current budget week: the latest `reset_day` at 00:00 local time.
pub fn week_start(now: chrono::DateTime<chrono::Local>, reset_day: u32) -> chrono::DateTime<chrono::Local> {
    use chrono::{Datelike, Duration, TimeZone};
    let back = (now.weekday().num_days_from_monday() + 7 - reset_day) % 7;
    let date = now.date_naive() - Duration::days(back as i64);
    chrono::Local.from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap()).earliest().unwrap_or(now)
}

/// Bot runtime as a percentage of the budget. `spans` are (started, ended) of bot runs; a run
/// that has not ended yet counts up to `now`.
pub fn budget_snapshot(spans: &[(i64, Option<i64>)], now: chrono::DateTime<chrono::Local>, cfg: BudgetCfg) -> Snapshot {
    let n = now.timestamp();
    let ws = week_start(now, cfg.reset_day);
    let week_reset = (ws + chrono::Duration::days(7)).timestamp();
    let used_week: i64 = spans.iter().map(|s| overlap(*s, ws.timestamp(), n, n)).sum();
    let lo5 = n - 5 * 3600;
    let used_5h: i64 = spans.iter().map(|s| overlap(*s, lo5, n, n)).sum();
    // the 5-hour window frees up when the oldest run inside it ages out
    let five_reset = spans.iter().filter(|s| overlap(**s, lo5, n, n) > 0).map(|s| s.0.max(lo5) + 5 * 3600).min();
    Snapshot {
        ts: n,
        five_pct: (used_5h as f64 / 3600.0 / cfg.hours_5h * 100.0).min(100.0),
        five_reset,
        week_pct: (used_week as f64 / 3600.0 / cfg.hours_week * 100.0).min(100.0),
        week_reset: Some(week_reset),
        source: "budget".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_counts_bot_hours_and_resets_weekly() {
        use chrono::TimeZone;
        // Wednesday 2026-10-14 12:00 local; week resets Monday
        let now = chrono::Local.with_ymd_and_hms(2026, 10, 14, 12, 0, 0).unwrap();
        let n = now.timestamp();
        let cfg = BudgetCfg { hours_week: 10.0, hours_5h: 2.0, reset_day: 0 };
        // 1 h ended 3 h ago, 30 min still running, 2 h run last week (must not count)
        let spans = [(n - 4 * 3600, Some(n - 3 * 3600)), (n - 1800, None), (n - 9 * 86400, Some(n - 9 * 86400 + 7200))];
        let s = budget_snapshot(&spans, now, cfg);
        assert!((s.week_pct - 15.0).abs() < 0.01, "1.5h of 10h, got {}", s.week_pct);
        assert!((s.five_pct - 75.0).abs() < 0.01, "1.5h of 2h, got {}", s.five_pct);
        assert_eq!(s.source, "budget");
        let reset = chrono::Local.timestamp_opt(s.week_reset.unwrap(), 0).unwrap();
        assert_eq!(reset.format("%Y-%m-%d %H:%M").to_string(), "2026-10-19 00:00");
        assert!(s.five_reset.unwrap() > n);
    }

    #[test]
    fn parses_widget_cache_shape() {
        let v: Value = serde_json::json!({
            "five_hour": {"utilization": 3.0, "resets_at": "2026-10-09T12:59:59.712504+00:00"},
            "seven_day": {"utilization": 34.0, "resets_at": "2026-10-14T12:59:59.712538+00:00"},
            "_fetched_at": "2026-10-09T16:19:59+07:00"
        });
        let s = parse(&v, 1, "cache").unwrap();
        assert_eq!(s.week_pct, 34.0);
        assert_eq!(s.five_pct, 3.0);
        assert!(s.week_reset.unwrap() > s.five_reset.unwrap());
    }

    #[test]
    fn codex_quota_comes_from_the_newest_session_log() {
        let dir = std::env::temp_dir().join(format!("ia-codex-{}", std::process::id()));
        let day = dir.join("sessions").join("2026").join("11").join("04");
        std::fs::create_dir_all(&day).unwrap();
        let ev = |five: f64, week: f64| format!(
            r#"{{"timestamp":"t","payload":{{"type":"token_count","rate_limits":{{"primary":{{"used_percent":{five},"window_minutes":300,"resets_at":2000}},"secondary":{{"used_percent":{week},"window_minutes":10080,"resets_at":9000}}}}}}}}"#
        );
        std::fs::write(day.join("rollout-a.jsonl"), format!("{}
{}
{{\"x\":1}}
", ev(10.0, 20.0), ev(16.0, 42.0))).unwrap();
        let s = codex_snapshot(&dir, 1000).unwrap();
        assert_eq!((s.five_pct, s.week_pct, s.source.as_str()), (16.0, 42.0, "codex"));
        assert_eq!((s.five_reset, s.week_reset), (Some(2000), Some(9000)));
        // the 5-hour window has reset since: empty again, the weekly one still counts
        let s = codex_snapshot(&dir, 3000).unwrap();
        assert_eq!((s.five_pct, s.five_reset, s.week_pct), (0.0, None, 42.0));
        assert!(codex_snapshot(&dir.join("nothing"), 1000).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
