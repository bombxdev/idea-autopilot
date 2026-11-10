//! The usage limit each provider really has. Only a Claude or ChatGPT (Codex) sign-in has a 5-hour window and a weekly
//! window; the others are something else, and the app should meter them the way they really work:
//!
//! - `Window`: 5-hour + weekly windows (Claude Pro/Max, ChatGPT plans) — read for real.
//! - `Daily`:  a free tier with a number of requests per day (Gemini with a Google sign-in, OpenCode Zen's free models,
//!   Groq, Cerebras). No hours budget from the app: the bot uses it until the provider says "limit", then waits for the
//!   provider's daily reset.
//! - `Paid`:   pay per token from a balance or a bill (API keys). There is no window to reset, so the app keeps only its
//!   own weekly hours cap as a spending guard, and shows the balance where the provider lets a key read it.
//!
//! Figures as documented by each provider (checked 2026-11): Gemini CLI 1,000 requests/day with a Google sign-in, 250/day
//! on a free key; OpenRouter `:free` models 50 requests/day (1,000 after buying ≥ 10 credits), 20/min, reset 00:00 UTC;
//! OpenCode Zen free models are limited per IP per day (no number published); OpenCode Go has 5-hour/weekly/monthly
//! caps shown only on its website; Mistral, DeepSeek, Kimi, xAI… are per-token with per-minute rate limits.
use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use serde_json::{json, Value};

use crate::accounts::Account;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Window,
    Daily,
    Paid,
}

impl Kind {
    pub fn id(self) -> &'static str {
        match self {
            Kind::Window => "window",
            Kind::Daily => "daily",
            Kind::Paid => "paid",
        }
    }
}

/// When the provider's daily count starts again.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reset {
    UtcMidnight,
    PacificMidnight,
    Unknown,
}

pub struct Info {
    pub kind: Kind,
    pub reset: Reset,
    pub text: &'static str,
}

const fn i(kind: Kind, reset: Reset, text: &'static str) -> Info {
    Info { kind, reset, text }
}

/// What limit this account really has.
pub fn info(a: &Account) -> Info {
    use Kind::*;
    use Reset::*;
    let key = a.uses_key();
    match a.vendor_id().as_str() {
        "anthropic" if !key => i(Window, Unknown, "บัญชี Claude (Pro/Max): โควต้ารอบ 5 ชม. และรอบสัปดาห์ — แอปอ่านค่าจริงจาก Claude"),
        "openai" if !key => i(Window, Unknown, "บัญชี ChatGPT: โควต้ารอบ 5 ชม. และรอบสัปดาห์ — แอปอ่านค่าจริงจาก log ของ Codex (หลังรันครั้งแรก)"),
        "anthropic" | "openai" => i(Paid, Unknown, "API key: จ่ายตามโทเคน ไม่มีรอบ 5 ชม./สัปดาห์ — จำกัดแค่ความเร็วต่อนาทีตาม tier ของบัญชี"),
        "google" if !key => i(Daily, PacificMidnight, "ล็อกอิน Google: ฟรีราว 1,000 คำขอ/วัน (60 คำขอ/นาที) ไม่มีรอบ 5 ชม./สัปดาห์ — เริ่มนับใหม่เที่ยงคืนเวลาแปซิฟิก"),
        "google" => i(Paid, PacificMidnight, "API key: แบบฟรีได้ราว 250 คำขอ/วัน (เฉพาะรุ่น Flash) ถ้าเปิด billing คิดตามโทเคน — ไม่มีรอบ 5 ชม./สัปดาห์"),
        "mistral" => i(Paid, Unknown, "คิดตามโทเคน จำกัดความเร็วต่อนาทีตาม tier ขององค์กร ไม่มีรอบ 5 ชม./สัปดาห์ (ถ้าเป็น key ของแพ็กเกจ Vibe จะหักจากงบรายเดือนของแพ็กเกจ)"),
        "deepseek" | "moonshot" => i(Paid, Unknown, "จ่ายตามโทเคนจากยอดเงินที่เติมไว้ ไม่มีรอบ 5 ชม./สัปดาห์ — เงินหมดแล้วจึงหยุด"),
        "zhipu" => i(Paid, Unknown, "key แบบจ่ายตามโทเคน ไม่มีรอบ 5 ชม./สัปดาห์ (ถ้าเป็น key ของ GLM Coding Plan จะมีรอบ 5 ชม. ซึ่งดูได้ที่ z.ai เท่านั้น)"),
        "openrouter" => i(Paid, UtcMidnight, "โมเดล :free ได้ 50 คำขอ/วัน (1,000/วัน ถ้าเคยเติมเครดิต ≥ 10) และ 20 คำขอ/นาที เริ่มนับใหม่เที่ยงคืน UTC · โมเดลเสียเงินหักเครดิต — ไม่มีรอบ 5 ชม./สัปดาห์"),
        "groq" | "cerebras" => i(Daily, Unknown, "แบบฟรีจำกัดคำขอและโทเคนต่อนาที/ต่อวันตามโมเดล ไม่มีรอบ 5 ชม./สัปดาห์"),
        "xai" | "together" | "fireworks" => i(Paid, Unknown, "จ่ายตามโทเคน จำกัดความเร็วต่อนาที ไม่มีรอบ 5 ชม./สัปดาห์"),
        "omniroute" => i(Daily, Unknown, "เกตเวย์ในเครื่อง: ลิมิตจริงเป็นของแต่ละค่ายที่เชื่อมไว้ใน OmniRoute (ส่วนใหญ่เป็นแบบฟรีรายวัน) และ OmniRoute สลับไปค่ายอื่นให้เองเมื่อค่ายหนึ่งเต็ม — ดูที่เหลือได้ใน dashboard ของ OmniRoute หน้า Free tiers"),
        "opencode" => i(Daily, UtcMidnight, "โมเดลฟรีของ OpenCode Zen จำกัดรายวันต่อเครือข่าย (ไม่ประกาศตัวเลข) เริ่มนับใหม่ราวเที่ยงคืน UTC · โมเดลเสียเงินหักยอด Zen · แพ็กเกจ OpenCode Go มีรอบ 5 ชม./สัปดาห์/เดือน แต่ดูได้ในเว็บเท่านั้น"),
        _ => i(Paid, Unknown, "ไม่ทราบลิมิตของผู้ให้บริการนี้ — แอปใช้เพดานชั่วโมงต่อสัปดาห์ของตัวเองกันไว้"),
    }
}

fn nth_sunday(y: i32, m: u32, n: u32) -> NaiveDate {
    let first = NaiveDate::from_ymd_opt(y, m, 1).unwrap_or_default();
    let off = (7 - first.weekday().num_days_from_sunday()) % 7;
    first + chrono::Duration::days((off + 7 * (n - 1)) as i64)
}

/// US Pacific daylight time: 2nd Sunday of March 02:00 PST (10:00 UTC) to 1st Sunday of November 02:00 PDT (09:00 UTC).
fn pacific_dst(now: i64) -> bool {
    let Some(t) = Utc.timestamp_opt(now, 0).single() else { return false };
    let y = t.year();
    let start = nth_sunday(y, 3, 2).and_hms_opt(10, 0, 0).map(|d| d.and_utc().timestamp()).unwrap_or(0);
    let end = nth_sunday(y, 11, 1).and_hms_opt(9, 0, 0).map(|d| d.and_utc().timestamp()).unwrap_or(0);
    now >= start && now < end
}

/// The next time the provider's daily count starts again (unix seconds), when it is known.
pub fn next_reset(r: Reset, now: i64) -> Option<i64> {
    match r {
        Reset::UtcMidnight => Some((now.div_euclid(86400) + 1) * 86400),
        Reset::PacificMidnight => {
            let off = if pacific_dst(now) { 7 * 3600 } else { 8 * 3600 };
            Some(((now - off).div_euclid(86400) + 1) * 86400 + off)
        }
        Reset::Unknown => None,
    }
}

/// The app's own meter for an account without a readable quota, shaped like the provider's real limit:
/// `Window` (not read yet) keeps the 5-hour + weekly hours budget; `Paid` only the weekly cap (no 5-hour window exists);
/// `Daily` none at all — the provider's own daily limit stops it.
pub fn meter(kind: Kind, spans: &[(i64, Option<i64>)], now: chrono::DateTime<chrono::Local>, cfg: crate::usage::BudgetCfg) -> crate::usage::Snapshot {
    let mut s = crate::usage::budget_snapshot(spans, now, cfg);
    match kind {
        Kind::Window => {}
        Kind::Paid => {
            s.five_pct = 0.0;
            s.five_reset = None;
        }
        Kind::Daily => {
            s.five_pct = 0.0;
            s.five_reset = None;
            s.week_pct = 0.0;
            s.week_reset = None;
            s.source = "provider".into();
        }
    }
    s
}

/// Until when an account that hit a limit should wait: the time the AI's own message gives, else the provider's daily
/// reset (a free daily tier), else the usual guess from its quota reading.
pub fn limit_until(a: Option<&Account>, text: &str, snap: Option<&crate::usage::Snapshot>, now: i64) -> i64 {
    if crate::limits::parse_reset(text, now).is_none() {
        if let Some(r) = a.and_then(|a| next_reset(info(a).reset, now)) {
            return r.clamp(now + 120, now + 2 * 86400);
        }
    }
    crate::limits::wait_until(text, snap, now)
}

/// For the screen: the account's real limit, the next daily reset and the balance read (if any).
pub fn json(a: &Account, balance: Option<&String>, now: i64) -> Value {
    let inf = info(a);
    json!({ "kind": inf.kind.id(), "text": inf.text, "reset": next_reset(inf.reset, now), "balance": balance })
}

/// Can this account's key read its balance?
pub fn has_balance(a: &Account) -> bool {
    a.uses_key() && matches!(a.vendor_id().as_str(), "deepseek" | "moonshot" | "openrouter")
}

fn money(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
}

/// The balance (and what it means for the limits), read with the account's own key.
pub async fn balance(a: &Account, key: &str) -> Option<String> {
    let c = reqwest::Client::builder().timeout(std::time::Duration::from_secs(15)).build().ok()?;
    let get = |url: String| {
        let c = c.clone();
        let key = key.to_string();
        async move {
            let r = c.get(url).bearer_auth(key).send().await.ok()?;
            if !r.status().is_success() {
                return None;
            }
            r.json::<Value>().await.ok()
        }
    };
    match a.vendor_id().as_str() {
        "deepseek" => {
            let v = get("https://api.deepseek.com/user/balance".into()).await?;
            let b = v["balance_infos"].as_array()?.first()?.clone();
            Some(format!("ยอดเงินคงเหลือ {:.2} {}", money(&b["total_balance"])?, b["currency"].as_str().unwrap_or("")))
        }
        "moonshot" => {
            let cn = a.base_url.contains("moonshot.cn");
            let host = if cn { "https://api.moonshot.cn" } else { "https://api.moonshot.ai" };
            let v = get(format!("{host}/v1/users/me/balance")).await?;
            let n = money(&v["data"]["available_balance"])?;
            Some(if cn { format!("ยอดเงินคงเหลือ ¥{n:.2}") } else { format!("ยอดเงินคงเหลือ ${n:.2}") })
        }
        "openrouter" => {
            let k = get("https://openrouter.ai/api/v1/key".into()).await?;
            let d = &k["data"];
            let mut parts = vec![];
            if let Some(cr) = get("https://openrouter.ai/api/v1/credits".into()).await {
                if let (Some(t), Some(u)) = (money(&cr["data"]["total_credits"]), money(&cr["data"]["total_usage"])) {
                    parts.push(format!("เครดิตคงเหลือ ${:.2}", t - u));
                }
            }
            if let Some(rem) = money(&d["limit_remaining"]) {
                parts.push(format!("key นี้ใช้ได้อีก ${rem:.2}"));
            }
            if let Some(day) = money(&d["usage_daily"]) {
                parts.push(format!("วันนี้ใช้ไป ${day:.2}"));
            }
            match d["is_free_tier"].as_bool() {
                Some(true) => parts.push("ยังไม่เคยเติมเครดิต → โมเดล :free ได้ 50 คำขอ/วัน".into()),
                Some(false) => parts.push("เคยเติมเครดิตแล้ว → โมเดล :free ได้ 1,000 คำขอ/วัน".into()),
                None => {}
            }
            (!parts.is_empty()).then(|| parts.join(" · "))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acct(vendor: &str, auth: &str) -> Account {
        Account { id: "x".into(), kind: "claude".into(), label: String::new(), enabled: true, home: String::new(), model: "auto".into(), effort: "auto".into(), vendor: vendor.into(), auth: auth.into(), base_url: String::new() }
    }

    #[test]
    fn only_claude_and_chatgpt_sign_ins_have_windows() {
        assert_eq!(info(&acct("anthropic", "login")).kind, Kind::Window);
        assert_eq!(info(&acct("openai", "login")).kind, Kind::Window);
        assert_eq!(info(&acct("anthropic", "key")).kind, Kind::Paid);
        assert_eq!(info(&acct("google", "login")).kind, Kind::Daily);
        assert_eq!(info(&acct("opencode", "login")).kind, Kind::Daily);
        assert_eq!(info(&acct("openrouter", "key")).kind, Kind::Paid);
        assert_eq!(info(&acct("mistral", "key")).kind, Kind::Paid);
    }

    #[test]
    fn daily_resets() {
        // 2026-11-06 12:00 UTC → next UTC midnight 2026-11-07 00:00
        let now = NaiveDate::from_ymd_opt(2026, 11, 6).unwrap().and_hms_opt(12, 0, 0).unwrap().and_utc().timestamp();
        let utc = NaiveDate::from_ymd_opt(2026, 11, 7).unwrap().and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
        assert_eq!(next_reset(Reset::UtcMidnight, now), Some(utc));
        // November 6 is after DST ended (Nov 1) → PST, midnight Pacific = 08:00 UTC
        assert_eq!(next_reset(Reset::PacificMidnight, now), Some(utc + 8 * 3600));
        // in July (PDT) → 07:00 UTC
        let jul = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap().and_hms_opt(12, 0, 0).unwrap().and_utc().timestamp();
        let jul2 = NaiveDate::from_ymd_opt(2026, 7, 2).unwrap().and_hms_opt(7, 0, 0).unwrap().and_utc().timestamp();
        assert_eq!(next_reset(Reset::PacificMidnight, jul), Some(jul2));
    }

    #[test]
    fn meters_follow_the_real_limit() {
        let now = chrono::Local::now();
        let n = now.timestamp();
        let spans = vec![(n - 3600, Some(n))];
        let cfg = crate::usage::BudgetCfg { hours_week: 10.0, hours_5h: 2.0, reset_day: 0 };
        let w = meter(Kind::Window, &spans, now, cfg);
        assert!(w.five_pct > 40.0);
        let p = meter(Kind::Paid, &spans, now, cfg);
        assert_eq!(p.five_pct, 0.0);
        assert!(p.week_pct > 0.0);
        let d = meter(Kind::Daily, &spans, now, cfg);
        assert_eq!((d.five_pct, d.week_pct), (0.0, 0.0));
    }

    #[test]
    fn a_free_daily_limit_waits_for_the_reset() {
        let a = acct("opencode", "login");
        let now = NaiveDate::from_ymd_opt(2026, 11, 6).unwrap().and_hms_opt(20, 0, 0).unwrap().and_utc().timestamp();
        assert_eq!(limit_until(Some(&a), "FreeUsageLimitError", None, now), now + 4 * 3600);
        // the AI's own time wins
        assert_eq!(limit_until(Some(&a), "rate limit, resets in 45 minutes", None, now), now + 45 * 60);
    }
}
