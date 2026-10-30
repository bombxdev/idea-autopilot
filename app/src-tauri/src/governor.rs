//! Decides whether the bot may start (or keep) work right now.
//!
//! bot_budget = 100 − weekly% − reserve for the user's remaining work hours − safety.
//! That budget is spread across the remaining "bot days" (days off weigh double), except in
//! the last 12 hours before the weekly reset, when whatever is left is fair game.
use chrono::{DateTime, Duration, Local, TimeZone};
use serde::Serialize;

use crate::calendar::{Calendar, Slot};
use crate::usage::Snapshot;

pub const SAFETY_PCT: f64 = 3.0;
pub const DEFAULT_RESERVE_PER_WORK_HOUR: f64 = 1.5; // 12% per 8-hour day
const STALE_SECS: i64 = 20 * 60;

#[derive(Debug, Clone)]
pub struct Inputs<'a> {
    pub now: DateTime<Local>,
    pub snap: Option<&'a Snapshot>,
    pub user_active: bool,
    pub paused: bool,
    pub turbo: bool,
    pub reserve_per_hour: f64,
    /// weekly % the bot used since the current bot day started (08:00)
    pub bot_used_today: f64,
    pub cal: &'a Calendar,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Decision {
    pub allow: bool,
    pub reason: String,
    /// short code for statistics: running | off | work | sprint | turbo | paused | no_data | stale |
    /// user_active | five_cap | no_budget | daily_cap
    pub code: String,
    pub slot: String,
    pub week_pct: f64,
    pub five_pct: f64,
    pub reserve: f64,
    pub bot_budget: f64,
    pub today_allowance: f64,
    pub bot_used_today: f64,
    pub five_cap: f64,
    pub sprint: bool,
    pub turbo: bool,
    pub paused: bool,
    pub hours_to_reset: f64,
    pub next_work_start: Option<i64>,
}

fn to_local(ts: i64) -> DateTime<Local> {
    Local.timestamp_opt(ts, 0).single().unwrap_or_else(Local::now)
}

pub fn decide(i: &Inputs) -> Decision {
    let slot = i.cal.slot_at(i.now);
    let mut d = Decision {
        slot: format!("{:?}", slot).to_lowercase(),
        paused: i.paused,
        turbo: i.turbo,
        bot_used_today: i.bot_used_today,
        ..Default::default()
    };
    let deny = |mut d: Decision, code: &str, why: &str| {
        d.allow = false;
        d.code = code.into();
        d.reason = why.into();
        d
    };

    let Some(s) = i.snap else { return deny(d, "no_data", "ยังอ่านโควต้าไม่ได้ — บอทหยุดไว้ก่อนเพื่อความปลอดภัย") };
    d.week_pct = s.week_pct;
    d.five_pct = s.five_pct;
    if i.now.timestamp() - s.ts > STALE_SECS {
        return deny(d, "stale", "ข้อมูลโควต้าเก่าเกิน 20 นาที — รอข้อมูลใหม่");
    }

    let week_reset = s.week_reset.map(to_local).unwrap_or(i.now + Duration::days(7));
    d.hours_to_reset = ((week_reset - i.now).num_minutes() as f64 / 60.0).max(0.0);
    let next_work = i.cal.next_work_start(i.now);
    d.next_work_start = next_work.map(|t| t.timestamp());

    d.reserve = if i.turbo { 0.0 } else { i.cal.work_hours_between(i.now, week_reset) * i.reserve_per_hour };
    d.bot_budget = 100.0 - s.week_pct - d.reserve - SAFETY_PCT;
    d.sprint = d.hours_to_reset <= 12.0 && slot != Slot::Work;

    // spread the budget over the remaining bot days
    let today = i.cal.bot_day_start(i.now);
    let mut weights = 0.0;
    let mut day = today;
    while day < week_reset {
        weights += i.cal.bot_day_weight(day);
        day += Duration::days(1);
    }
    let w0 = i.cal.bot_day_weight(today);
    d.today_allowance = if weights > 0.0 { (d.bot_budget + i.bot_used_today) * w0 / weights } else { d.bot_budget };

    // 5-hour window cap
    d.five_cap = match slot {
        Slot::Work => 40.0,
        Slot::Lunch => 50.0,
        Slot::Off => 95.0,
    };
    if slot == Slot::Off {
        if let Some(nw) = next_work {
            if nw - i.now < Duration::hours(5) {
                // "window priming": if the current/new 5h window would still be open when work
                // starts, keep most of it for the user's morning.
                let window_end = s
                    .five_reset
                    .map(to_local)
                    .filter(|r| *r > i.now)
                    .unwrap_or(i.now + Duration::hours(5));
                if window_end > nw {
                    d.five_cap = 30.0;
                }
            }
        }
        if i.user_active {
            d.five_cap = d.five_cap.min(60.0);
        }
    }
    if i.turbo {
        d.five_cap = 95.0;
    }

    if i.paused {
        return deny(d, "paused", "หยุดชั่วคราว (Pause)");
    }
    if !i.turbo && slot != Slot::Off && i.user_active {
        return deny(d, "user_active", "คุณกำลังใช้ Claude ทำงานอยู่ — บอทรอ");
    }
    if s.five_pct >= d.five_cap {
        let why = format!("โควต้า 5 ชม. {:.0}% ถึงเพดาน {:.0}% ของช่วงนี้แล้ว", s.five_pct, d.five_cap);
        return deny(d, "five_cap", &why);
    }
    if d.bot_budget <= 0.0 {
        return deny(d, "no_budget", "โควต้าที่เหลือกันไว้ให้งานของคุณทั้งหมดแล้ว");
    }
    if !i.turbo && !d.sprint && i.bot_used_today >= d.today_allowance {
        let why = format!("วันนี้บอทใช้ครบโควต้ารายวัน {:.1}% แล้ว", d.today_allowance);
        return deny(d, "daily_cap", &why);
    }
    d.allow = true;
    d.code = if i.turbo {
        "turbo"
    } else if d.sprint {
        "sprint"
    } else if slot == Slot::Off {
        "off"
    } else {
        "work"
    }
    .into();
    d.reason = if i.turbo {
        "Turbo — ใช้เต็มที่ตอนนี้".into()
    } else if d.sprint {
        "ช่วงโค้งสุดท้ายก่อนรีเซ็ต — ใช้ที่เหลือให้หมด".into()
    } else {
        match slot {
            Slot::Off => "นอกเวลางาน — บอททำงานได้".into(),
            _ => "ช่วงงานแต่คุณไม่ได้ใช้ — บอทรันเบาๆ".into(),
        }
    };
    d
}

/// Stop a running stage mid-way only in clear-cut cases.
pub fn should_kill(d: &Decision) -> Option<&'static str> {
    if d.paused {
        return Some("ผู้ใช้กด Pause");
    }
    if d.five_pct >= 98.0 {
        return Some("โควต้า 5 ชม. ใกล้หมด");
    }
    if d.slot == "work" && !d.turbo && d.five_pct >= 60.0 {
        return Some("เข้าเวลางานและโควต้า 5 ชม. เหลือน้อย");
    }
    None
}

/// Learn the user's weekly-% use per work hour (p80 of hourly totals over recent weeks).
pub fn learn_reserve(deltas: &[crate::db::Delta], cal: &Calendar) -> Option<f64> {
    use std::collections::BTreeMap;
    let mut hours: BTreeMap<i64, f64> = BTreeMap::new();
    for d in deltas {
        let t = to_local(d.ts);
        if cal.slot_at(t) == Slot::Work {
            *hours.entry(d.ts / 3600).or_default() += d.user;
        }
    }
    // count every work hour we have data for, including zero-use hours
    if hours.len() < 24 {
        return None;
    }
    let mut v: Vec<f64> = hours.values().copied().collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p80 = v[((v.len() as f64 * 0.8) as usize).min(v.len() - 1)];
    Some(p80.clamp(0.3, 5.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(d: u32, h: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, d, h, 0, 0).unwrap()
    }

    fn snap(now: DateTime<Local>, week: f64, five: f64, five_reset: Option<DateTime<Local>>) -> Snapshot {
        Snapshot {
            ts: now.timestamp(),
            five_pct: five,
            five_reset: five_reset.map(|t| t.timestamp()),
            week_pct: week,
            week_reset: Some(at(14, 20).timestamp()), // Wed 14 Oct 20:00
            source: "test".into(),
        }
    }

    fn inputs<'a>(now: DateTime<Local>, s: &'a Snapshot, cal: &'a Calendar) -> Inputs<'a> {
        Inputs {
            now,
            snap: Some(s),
            user_active: false,
            paused: false,
            turbo: false,
            reserve_per_hour: DEFAULT_RESERVE_PER_WORK_HOUR,
            bot_used_today: 0.0,
            cal,
        }
    }

    #[test]
    fn friday_night_allows_and_reserves_work_hours() {
        let cal = Calendar::new(crate::calendar::Schedule::legacy(), &[]);
        let now = at(9, 22);
        let s = snap(now, 34.0, 0.0, None);
        let d = decide(&inputs(now, &s, &cal));
        assert!(d.allow, "{}", d.reason);
        // remaining work hours: Sat 10th (8h) + Mon 12 (8) + Tue 13 (8) + Wed 14 (8) = 32h * 1.5
        assert!((d.reserve - 48.0).abs() < 0.01, "reserve {}", d.reserve);
        assert!(d.today_allowance > 0.0 && d.today_allowance < d.bot_budget);
    }

    #[test]
    fn work_hours_block_when_user_active() {
        let cal = Calendar::new(crate::calendar::Schedule::legacy(), &[]);
        let now = at(13, 10);
        let s = snap(now, 50.0, 10.0, None);
        let mut i = inputs(now, &s, &cal);
        i.user_active = true;
        assert!(!decide(&i).allow);
        i.user_active = false;
        assert!(decide(&i).allow);
    }

    #[test]
    fn window_priming_protects_morning() {
        let cal = Calendar::new(crate::calendar::Schedule::legacy(), &[]);
        // Tue 13 Oct 05:00, no active window -> new window would end 10:00, after 08:00 start
        let now = at(13, 5);
        let s = snap(now, 50.0, 35.0, Some(at(13, 9)));
        let d = decide(&inputs(now, &s, &cal));
        assert_eq!(d.five_cap, 30.0);
        assert!(!d.allow);
        // 02:00 with window ending 07:00 -> full cap
        let now = at(13, 2);
        let s = snap(now, 50.0, 35.0, Some(at(13, 7)));
        assert!(decide(&inputs(now, &s, &cal)).allow);
    }

    #[test]
    fn sprint_ignores_daily_cap_before_reset() {
        let cal = Calendar::new(crate::calendar::Schedule::legacy(), &[]);
        let now = at(14, 18); // Wed 18:00, reset 20:00
        let s = snap(now, 60.0, 10.0, None);
        let mut i = inputs(now, &s, &cal);
        i.bot_used_today = 50.0;
        let d = decide(&i);
        assert!(d.sprint);
        assert!(d.allow, "{}", d.reason);
    }

    #[test]
    fn stale_or_missing_data_denies() {
        let cal = Calendar::new(crate::calendar::Schedule::legacy(), &[]);
        let now = at(10, 22);
        let mut s = snap(now, 10.0, 0.0, None);
        s.ts -= 3600;
        assert!(!decide(&inputs(now, &s, &cal)).allow);
        let mut i = inputs(now, &s, &cal);
        i.snap = None;
        assert!(!decide(&i).allow);
    }
}
