//! Picks the model, effort and tool set for a stage with fixed rules — no tokens spent deciding.
//!
//! "Don't hire a CEO to sweep the floor": bulk work (reading the web, writing code) goes to the
//! standard tier, judgement calls (verdict, plan) to the strong tier, bookkeeping (replan) to the
//! fast tier. Each agent maps tiers to its own models (providers.rs): Claude = Haiku / Sonnet / Opus.
//! Then three adjustments:
//! - trouble (failures, timeouts, a build that keeps repeating) → one step up
//! - surplus (quota that would expire unused) → upgrade quality; unused quota is worth nothing
//! - scarcity (little bot budget left) → one step down on effort, keep the model
use serde::Serialize;

use crate::db::Idea;
use crate::governor::Decision;

const MODELS: [&str; 3] = ["fast", "standard", "strong"];

/// Old settings/runs used Claude family names; they map onto the same three tiers.
pub fn tier_of(name: &str) -> Option<usize> {
    match name {
        "fast" | "haiku" => Some(0),
        "standard" | "sonnet" => Some(1),
        "strong" | "opus" => Some(2),
        _ => None,
    }
}
const EFFORTS: [&str; 4] = ["low", "medium", "high", "xhigh"];

const BASE_TOOLS: &str = "Bash,Read,Write,Edit,Glob,Grep,TodoWrite";
const WEB_TOOLS: &str = "WebSearch,WebFetch";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Choice {
    pub model: String,
    pub effort: String,
    pub tools: String,
    pub reason: String,
}

/// Full-power mode: strongest model, highest effort, whatever the thrift rules would say. Bookkeeping stays small.
pub fn choose_power(stage: &str, forced_model: Option<&str>) -> Choice {
    let small = stage == "replan";
    let model = match forced_model.map(str::trim).filter(|f| !f.is_empty() && *f != "auto") {
        Some(f) => tier_of(f).map(|t| MODELS[t].to_string()).unwrap_or_else(|| f.to_string()),
        None => MODELS[if small { 0 } else { 2 }].to_string(),
    };
    let tools = if small { BASE_TOOLS.to_string() } else { format!("{BASE_TOOLS},{WEB_TOOLS}") };
    Choice { model, effort: EFFORTS[if small { 0 } else { 3 }].to_string(), tools, reason: "โหมดเต็มที่ → โมเดลแรงสุด + effort สูงสุด".into() }
}

/// An idea that has already stumbled: the router moves it up a step, and cost tuning leaves it alone.
pub fn in_trouble(idea: &Idea, stage: &str) -> bool {
    idea.fail_count > 0 || (stage == "build" && idea.repeat_count >= 3) || (stage != "build" && idea.repeat_count >= 1)
}

pub fn base(stage: &str) -> (usize, usize, &'static str) {
    // (model tier, effort level, why)
    match stage {
        "replan" => (0, 0, "งานจัดคิวเล็กๆ"),
        "clarify" => (1, 1, "ตีความสั้นๆ"),
        "research" => (1, 1, "อ่านเว็บปริมาณมาก"),
        "learn" => (1, 1, "เรียบเรียงจากข้อมูลที่มี"),
        "build" => (1, 1, "เขียนโค้ดตามแผน"),
        "verify" => (1, 1, "รันและแก้ตามผล"),
        "experiment" => (1, 2, "ทดลองต้องคิดละเอียด"),
        "compare" => (1, 2, "เทียบทางเลือก"),
        "verdict" => (2, 1, "ตัดสินใจสำคัญ"),
        "plan" => (2, 1, "ออกแบบระบบ"),
        _ => (1, 1, "ค่าเริ่มต้น"),
    }
}

/// Is weekly quota likely to expire unused? Compare what the bot may still use with how much
/// it could realistically burn in the remaining time.
fn surplus(d: &Decision) -> bool {
    d.sprint || (d.hours_to_reset > 0.0 && d.bot_budget / d.hours_to_reset > 1.0)
}

fn scarce(d: &Decision) -> bool {
    !d.sprint && !d.turbo && d.bot_budget < 5.0
}

/// What the tuner learned for this stage (see tuner.rs).
#[derive(Debug, Clone, Copy, Default)]
pub struct Learned {
    /// 0 or negative: tiers cheaper than the rule table that proved good enough
    pub offset: i32,
    /// try one tier cheaper this time to keep collecting evidence
    pub probe: bool,
}

pub fn choose(stage: &str, idea: &Idea, d: &Decision, forced_model: Option<&str>, learned: Learned) -> Choice {
    let (mut m, mut e, why) = base(stage);
    let mut notes = vec![why.to_string()];
    if learned.offset != 0 {
        m = (m as i32 + learned.offset).clamp(0, 2) as usize;
        notes.push(format!("สถิติจริงบอกว่า {} ทำได้ดีพอ", MODELS[m]));
    }

    if in_trouble(idea, stage) {
        m = (m + 1).min(2);
        e = (e + 1).min(3);
        notes.push("เคยติดขัด → ขยับขึ้น".into());
    } else if surplus(d) && matches!(stage, "research" | "learn" | "verify" | "experiment" | "compare") {
        m = 2;
        notes.push("โควต้าเหลือจะหมดอายุ → ใช้ตัวเก่ง".into());
    } else {
        if learned.probe && m > 0 {
            m -= 1;
            notes.push(format!("ทดลอง {} เก็บสถิติ", MODELS[m]));
        }
        if scarce(d) && stage != "replan" {
            e = e.saturating_sub(1);
            notes.push("โควต้าใกล้หมด → ลด effort".into());
        }
    }

    // "auto" or empty = the rules above decide; a tier name or an exact model name is the user's call
    let model = match forced_model.map(str::trim).filter(|f| !f.is_empty() && *f != "auto") {
        Some(f) => {
            notes.push("ตั้งค่าบังคับไว้".into());
            tier_of(f).map(|t| MODELS[t].to_string()).unwrap_or_else(|| f.to_string())
        }
        None => MODELS[m].to_string(),
    };
    let tools = if stage == "replan" { BASE_TOOLS.to_string() } else { format!("{BASE_TOOLS},{WEB_TOOLS}") };
    Choice { model, effort: EFFORTS[e].to_string(), tools, reason: notes.join(" · ") }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idea() -> Idea {
        serde_json::from_value(serde_json::json!({
            "id": 1, "slug": "x", "title": "t", "body": "", "intents": [], "recipe": [], "stage_idx": 0,
            "status": "active", "priority": 0, "research_only": false, "summary": "", "verdict": null,
            "run_cmd": null, "knowledge": false, "needs": [], "repeat_count": 0, "fail_count": 0,
            "sort": 0.0, "created": 0, "updated": 0
        }))
        .unwrap()
    }

    fn normal() -> Decision {
        Decision { bot_budget: 20.0, hours_to_reset: 90.0, ..Default::default() }
    }

    #[test]
    fn bulk_work_goes_to_sonnet_judgement_to_opus() {
        assert_eq!(choose("research", &idea(), &normal(), None, Learned::default()).model, "standard");
        assert_eq!(choose("build", &idea(), &normal(), None, Learned::default()).model, "standard");
        assert_eq!(choose("verdict", &idea(), &normal(), None, Learned::default()).model, "strong");
        assert_eq!(choose("replan", &idea(), &normal(), None, Learned::default()).model, "fast");
        assert!(!choose("replan", &idea(), &normal(), None, Learned::default()).tools.contains("WebSearch"));
    }

    #[test]
    fn trouble_escalates() {
        let mut i = idea();
        i.repeat_count = 3;
        let c = choose("build", &i, &normal(), None, Learned::default());
        assert_eq!((c.model.as_str(), c.effort.as_str()), ("strong", "high"));
    }

    #[test]
    fn surplus_upgrades_and_scarcity_lowers_effort() {
        let d = Decision { sprint: true, bot_budget: 30.0, hours_to_reset: 5.0, ..Default::default() };
        assert_eq!(choose("research", &idea(), &d, None, Learned::default()).model, "strong");
        let d = Decision { bot_budget: 2.0, hours_to_reset: 50.0, ..Default::default() };
        let c = choose("build", &idea(), &d, None, Learned::default());
        assert_eq!((c.model.as_str(), c.effort.as_str()), ("standard", "low"));
    }

    #[test]
    fn full_power_is_strongest_and_highest_effort_but_keeps_bookkeeping_small() {
        let c = choose_power("research", None);
        assert_eq!((c.model.as_str(), c.effort.as_str()), ("strong", "xhigh"));
        assert!(c.tools.contains("WebSearch"));
        let r = choose_power("replan", None);
        assert_eq!((r.model.as_str(), r.effort.as_str()), ("fast", "low"));
        assert_eq!(choose_power("build", Some("my-model")).model, "my-model");
    }

    #[test]
    fn learned_offset_and_probe() {
        let l = Learned { offset: -1, probe: false };
        assert_eq!(choose("verdict", &idea(), &normal(), None, l).model, "standard");
        let p = Learned { offset: 0, probe: true };
        assert_eq!(choose("research", &idea(), &normal(), None, p).model, "fast");
        // never probe an idea that is already in trouble
        let mut i = idea();
        i.fail_count = 1;
        assert_eq!(choose("research", &i, &normal(), None, p).model, "strong");
    }

    #[test]
    fn forced_model_wins() {
        assert_eq!(choose("build", &idea(), &normal(), Some("opus"), Learned::default()).model, "strong");
        assert_eq!(choose("build", &idea(), &normal(), Some("strong"), Learned::default()).model, "strong");
        assert_eq!(choose("build", &idea(), &normal(), Some("auto"), Learned::default()).model, "standard");
    }
}
