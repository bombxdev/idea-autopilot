//! Work calendar: which hours belong to the user's day job and which are free for the bot.
//!
//! The schedule is configurable (setting `schedule`, JSON) so anyone can use the app.
//! Default for a new install: Mon–Fri 09–12 + 13–18 work, 12–13 lunch, weekend off.
//! `Schedule::legacy()` is the original owner's rota (Saturdays by week-of-month); it is kept so
//! an install that existed before the setting did keeps behaving the same.
//! Per-date overrides ("off", "work", "half") win over the schedule.
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone, Timelike, Weekday};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DayKind {
    Full,
    Half,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Slot {
    Work,
    Lunch,
    Off,
}

/// A weekday is either always the same kind, or depends on its week-of-month ("1".."5").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DayRule {
    Same(DayKind),
    ByWeek(BTreeMap<String, DayKind>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Schedule {
    /// hours as decimals, e.g. 8.5 = 08:30
    pub start: f64,
    pub lunch_start: f64,
    pub lunch_end: f64,
    pub end: f64,
    /// a half day runs from `start` to `half_end`
    pub half_end: f64,
    /// keys: mon tue wed thu fri sat sun
    pub days: BTreeMap<String, DayRule>,
}

impl Default for Schedule {
    fn default() -> Self {
        let mut days = BTreeMap::new();
        for d in ["mon", "tue", "wed", "thu", "fri"] {
            days.insert(d.into(), DayRule::Same(DayKind::Full));
        }
        days.insert("sat".into(), DayRule::Same(DayKind::Off));
        days.insert("sun".into(), DayRule::Same(DayKind::Off));
        Schedule { start: 9.0, lunch_start: 12.0, lunch_end: 13.0, end: 18.0, half_end: 12.0, days }
    }
}

impl Schedule {
    /// 08–17 Mon–Fri, Saturdays by week of the month (2nd and 4th full, 5th half day).
    pub fn legacy() -> Self {
        let mut days = BTreeMap::new();
        for d in ["mon", "tue", "wed", "thu", "fri"] {
            days.insert(d.into(), DayRule::Same(DayKind::Full));
        }
        days.insert(
            "sat".into(),
            DayRule::ByWeek(BTreeMap::from([
                ("2".into(), DayKind::Full),
                ("4".into(), DayKind::Full),
                ("5".into(), DayKind::Half),
            ])),
        );
        days.insert("sun".into(), DayRule::Same(DayKind::Off));
        Schedule { start: 8.0, lunch_start: 12.0, lunch_end: 13.0, end: 17.0, half_end: 12.0, days }
    }
}

fn day_key(w: Weekday) -> &'static str {
    match w {
        Weekday::Mon => "mon",
        Weekday::Tue => "tue",
        Weekday::Wed => "wed",
        Weekday::Thu => "thu",
        Weekday::Fri => "fri",
        Weekday::Sat => "sat",
        Weekday::Sun => "sun",
    }
}

#[derive(Debug, Clone, Default)]
pub struct Calendar {
    pub sched: Schedule,
    /// date -> kind override
    pub overrides: HashMap<NaiveDate, DayKind>,
}

impl Calendar {
    pub fn new(sched: Schedule, rows: &[(String, String, String)]) -> Self {
        let mut overrides = HashMap::new();
        for (d, k, _) in rows {
            if let Ok(date) = NaiveDate::parse_from_str(d, "%Y-%m-%d") {
                let kind = match k.as_str() {
                    "work" => DayKind::Full,
                    "half" => DayKind::Half,
                    _ => DayKind::Off,
                };
                overrides.insert(date, kind);
            }
        }
        Calendar { sched, overrides }
    }

    #[cfg(test)]
    pub fn from_rows(rows: &[(String, String, String)]) -> Self {
        Self::new(Schedule::legacy(), rows)
    }

    pub fn default_kind(&self, date: NaiveDate) -> DayKind {
        match self.sched.days.get(day_key(date.weekday())) {
            Some(DayRule::Same(k)) => *k,
            Some(DayRule::ByWeek(m)) => {
                let nth = ((date.day() - 1) / 7 + 1).to_string();
                m.get(&nth).copied().unwrap_or(DayKind::Off)
            }
            None => DayKind::Off,
        }
    }

    pub fn day_kind(&self, date: NaiveDate) -> DayKind {
        self.overrides.get(&date).copied().unwrap_or_else(|| self.default_kind(date))
    }

    pub fn slot_at(&self, t: DateTime<Local>) -> Slot {
        let h = t.hour() as f64 + t.minute() as f64 / 60.0;
        let s = &self.sched;
        let within = |a: f64, b: f64| h >= a && h < b;
        match self.day_kind(t.date_naive()) {
            DayKind::Off => Slot::Off,
            DayKind::Half => {
                if within(s.start, s.half_end) { Slot::Work } else { Slot::Off }
            }
            DayKind::Full => {
                if within(s.lunch_start, s.lunch_end) {
                    Slot::Lunch
                } else if within(s.start, s.end) {
                    Slot::Work
                } else {
                    Slot::Off
                }
            }
        }
    }

    /// Slot for each hour of a day (for the UI timeline).
    pub fn day_slots(&self, date: NaiveDate) -> Vec<Slot> {
        (0..24)
            .map(|h| {
                Local
                    .with_ymd_and_hms(date.year(), date.month(), date.day(), h, 30, 0)
                    .single()
                    .map(|t| self.slot_at(t))
                    .unwrap_or(Slot::Off)
            })
            .collect()
    }

    /// Work hours between `from` and `to` (15-minute resolution).
    pub fn work_hours_between(&self, from: DateTime<Local>, to: DateTime<Local>) -> f64 {
        let mut t = from;
        let mut quarters = 0;
        while t < to {
            if self.slot_at(t) == Slot::Work {
                quarters += 1;
            }
            t += Duration::minutes(15);
        }
        quarters as f64 / 4.0
    }

    /// Start of the next work block strictly after `from` (within 10 days).
    pub fn next_work_start(&self, from: DateTime<Local>) -> Option<DateTime<Local>> {
        let mut t = from + Duration::minutes(15 - (from.minute() as i64 % 15));
        let mut prev = self.slot_at(from);
        let end = from + Duration::days(10);
        while t < end {
            let s = self.slot_at(t);
            if s == Slot::Work && prev != Slot::Work {
                return Some(t);
            }
            prev = s;
            t += Duration::minutes(15);
        }
        None
    }

    /// "Bot day" boundary: days start when work starts, so a night belongs to the evening before.
    pub fn bot_day_start(&self, t: DateTime<Local>) -> DateTime<Local> {
        let h = self.sched.start.floor() as u32;
        let m = ((self.sched.start - self.sched.start.floor()) * 60.0) as u32;
        let base = if (t.hour(), t.minute()) < (h, m) { t - Duration::days(1) } else { t };
        Local.with_ymd_and_hms(base.year(), base.month(), base.day(), h, m, 0).single().unwrap_or(t)
    }

    /// Weight of a bot day for spreading the budget: days off count double.
    pub fn bot_day_weight(&self, day_start: DateTime<Local>) -> f64 {
        match self.day_kind(day_start.date_naive()) {
            DayKind::Off => 2.0,
            DayKind::Half => 1.5,
            DayKind::Full => 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(y: i32, m: u32, d: u32, h: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(y, m, d, h, 0, 0).unwrap()
    }

    #[test]
    fn saturdays_follow_rules() {
        // October 2026: Saturdays are 3, 10, 17, 24, 31
        let c = Calendar::new(Schedule::legacy(), &[]);
        assert_eq!(c.day_kind(NaiveDate::from_ymd_opt(2026, 10, 3).unwrap()), DayKind::Off);
        assert_eq!(c.day_kind(NaiveDate::from_ymd_opt(2026, 10, 10).unwrap()), DayKind::Full);
        assert_eq!(c.day_kind(NaiveDate::from_ymd_opt(2026, 10, 17).unwrap()), DayKind::Off);
        assert_eq!(c.day_kind(NaiveDate::from_ymd_opt(2026, 10, 24).unwrap()), DayKind::Full);
        assert_eq!(c.day_kind(NaiveDate::from_ymd_opt(2026, 10, 31).unwrap()), DayKind::Half);
        assert_eq!(c.slot_at(at(2026, 10, 31, 10)), Slot::Work);
        assert_eq!(c.slot_at(at(2026, 10, 31, 14)), Slot::Off);
    }

    #[test]
    fn weekday_slots() {
        let c = Calendar::new(Schedule::legacy(), &[]);
        assert_eq!(c.slot_at(at(2026, 10, 13, 7)), Slot::Off);
        assert_eq!(c.slot_at(at(2026, 10, 13, 9)), Slot::Work);
        assert_eq!(c.slot_at(at(2026, 10, 13, 12)), Slot::Lunch);
        assert_eq!(c.slot_at(at(2026, 10, 13, 16)), Slot::Work);
        assert_eq!(c.slot_at(at(2026, 10, 13, 17)), Slot::Off);
        assert_eq!(c.slot_at(at(2026, 10, 11, 10)), Slot::Off); // Sunday
    }

    #[test]
    fn override_wins() {
        let c = Calendar::from_rows(&[("2026-10-13".into(), "off".into(), "ลา".into())]);
        assert_eq!(c.slot_at(at(2026, 10, 13, 9)), Slot::Off);
    }

    #[test]
    fn work_hours_and_next_start() {
        let c = Calendar::new(Schedule::legacy(), &[]);
        assert_eq!(c.work_hours_between(at(2026, 10, 13, 0), at(2026, 10, 14, 0)), 8.0);
        assert_eq!(c.next_work_start(at(2026, 10, 13, 2)), Some(at(2026, 10, 13, 8)));
        assert_eq!(c.next_work_start(at(2026, 10, 13, 12)), Some(at(2026, 10, 13, 13)));
        assert_eq!(c.next_work_start(at(2026, 10, 9, 18)), Some(at(2026, 10, 10, 8)));
    }

    #[test]
    fn custom_schedule_from_json() {
        // a friend who works 09:30–18:30 Mon–Sat, no lunch block
        let s: Schedule = serde_json::from_value(serde_json::json!({
            "start": 9.5, "lunch_start": 0, "lunch_end": 0, "end": 18.5, "half_end": 13,
            "days": {"mon":"full","tue":"full","wed":"full","thu":"full","fri":"full","sat":"full","sun":"off"}
        }))
        .unwrap();
        let c = Calendar::new(s, &[]);
        assert_eq!(c.slot_at(at(2026, 10, 3, 9)), Slot::Off);
        assert_eq!(c.slot_at(at(2026, 10, 3, 12)), Slot::Work);
        assert_eq!(c.slot_at(at(2026, 10, 3, 18)), Slot::Work);
        assert_eq!(c.bot_day_start(at(2026, 10, 3, 9)).hour(), 9);
    }
}
