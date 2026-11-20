//! Keeps the model routing current as models improve — from run statistics only, no tokens.
//!
//! - Each agent (Claude, Codex, …) is learned on its own: a tier means a different model on each, so results never mix.
//! - Runs are tagged with the concrete model id (e.g. `claude-sonnet-5-5`, or the agent's default). Only runs on the
//!   newest version of each tier count, so a new release starts with fresh evidence.
//! - Every 5th run of a stage on an agent "probes" one tier cheaper than the current choice.
//! - A cheaper tier is adopted for a stage once it has ≥5 recent runs whose quality is within 5
//!   points of the tier above. Evidence windows are rolling, so a slipping tier falls back up.
//! - Quality of a run = finished OK, wrote its status file, and the user did not ask to
//!   "dig deeper" / "try another way" / "retry" that idea within 24 hours afterwards.
use rusqlite::Connection;
use serde::Serialize;
use std::collections::BTreeMap;

pub const TIERS: [&str; 3] = ["fast", "standard", "strong"];
const WINDOW: usize = 20;
const MIN_EVIDENCE: usize = 5;
const PROBE_EVERY: i64 = 5;

#[derive(Debug, Clone, Serialize, Default)]
pub struct TierStat {
    pub runs: usize,
    pub good: usize,
}

impl TierStat {
    pub fn rate(&self) -> f64 {
        if self.runs == 0 { 0.0 } else { self.good as f64 / self.runs as f64 }
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct StagePolicy {
    pub base: String,
    pub chosen: String,
    /// tier steps relative to the rule table (0 or negative)
    pub offset: i32,
    pub stats: BTreeMap<String, TierStat>,
    pub note: String,
}

/// What was learned for one agent.
#[derive(Debug, Clone, Serialize, Default)]
pub struct AgentPolicy {
    pub stages: BTreeMap<String, StagePolicy>,
    /// newest concrete model id seen per tier
    pub versions: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Policy {
    /// per agent (claude, codex, mistral, …)
    pub agents: BTreeMap<String, AgentPolicy>,
    pub learned_at: i64,
}

impl Policy {
    pub fn offset(&self, provider: &str, stage: &str) -> i32 {
        self.agents.get(agent_of(provider)).and_then(|a| a.stages.get(stage)).map(|s| s.offset).unwrap_or(0)
    }
    /// The newest model id this agent has run for a tier.
    pub fn version(&self, provider: &str, tier: usize) -> Option<String> {
        self.agents.get(agent_of(provider)).and_then(|a| a.versions.get(TIERS[tier])).cloned()
    }
}

/// Runs recorded before the agent was stored were all Claude's.
pub fn agent_of(provider: &str) -> &str {
    if provider.is_empty() { "claude" } else { provider }
}

/// Runs recorded before tiers were agent-neutral carry Claude names (haiku/sonnet/opus).
pub fn tier_index(model: &str) -> Option<usize> {
    crate::router::tier_of(model)
}

struct RunRow {
    provider: String,
    stage: String,
    tier: usize,
    model_id: Option<String>,
    good: bool,
}

pub fn learn(c: &Connection, base_tier: impl Fn(&str) -> usize) -> rusqlite::Result<Policy> {
    // negative feedback: steering toward "deeper"/"another way" or a retry, per idea, with time
    let mut neg: Vec<(i64, i64)> = Vec::new();
    {
        let mut st = c.prepare(
            "SELECT idea_id, ts FROM events WHERE idea_id IS NOT NULL AND
             (kind='retry' OR (kind='steer' AND (text LIKE '%ขุดลึก%' OR text LIKE '%ลองอีกแนว%')))",
        )?;
        for r in st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))? {
            neg.push(r?);
        }
    }
    let mut st = c.prepare(
        "SELECT stage, model, model_id, status, summary IS NOT NULL, idea_id, COALESCE(ended, started), COALESCE(provider,'')
         FROM runs WHERE status != 'running' AND status != 'cancelled' AND power=0 ORDER BY id DESC LIMIT 4000",
    )?;
    let mut rows = Vec::new();
    let mut versions: BTreeMap<(String, usize), String> = BTreeMap::new();
    for r in st.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, bool>(4)?,
            r.get::<_, i64>(5)?,
            r.get::<_, i64>(6)?,
            r.get::<_, String>(7)?,
        ))
    })? {
        let (stage, model, model_id, status, has_summary, idea, ended, provider) = r?;
        let Some(tier) = tier_index(&model) else { continue };
        let provider = agent_of(&provider).to_string();
        let model_id = model_id.filter(|m| !m.is_empty());
        if let Some(id) = &model_id {
            versions.entry((provider.clone(), tier)).or_insert_with(|| id.clone()); // newest first
        }
        let steered = neg.iter().any(|(i, t)| *i == idea && *t >= ended && *t - ended < 86400);
        rows.push(RunRow { provider, stage, tier, model_id, good: status == "ok" && has_summary && !steered });
    }

    let mut policy = Policy { learned_at: crate::db::now(), ..Default::default() };
    for ((p, t), id) in &versions {
        policy.agents.entry(p.clone()).or_default().versions.insert(TIERS[*t].to_string(), id.clone());
    }
    let mut pairs: Vec<(String, String)> = rows.iter().map(|r| (r.provider.clone(), r.stage.clone())).collect();
    pairs.sort();
    pairs.dedup();
    for (provider, stage) in pairs {
        let base = base_tier(&stage);
        let mut stats: BTreeMap<String, TierStat> = BTreeMap::new();
        for t in 0..TIERS.len() {
            let newest = versions.get(&(provider.clone(), t));
            let s = rows
                .iter()
                // only the newest version of a tier counts; untagged old runs count until one exists
                .filter(|r| r.provider == provider && r.stage == stage && r.tier == t && (newest.is_none() || r.model_id.as_ref() == newest))
                .take(WINDOW)
                .fold(TierStat::default(), |mut a, r| {
                    a.runs += 1;
                    a.good += r.good as usize;
                    a
                });
            if s.runs > 0 {
                stats.insert(TIERS[t].to_string(), s);
            }
        }
        // walk down while the cheaper tier is about as good as the one above
        let mut chosen = base;
        while chosen > 0 {
            let lower = stats.get(TIERS[chosen - 1]).cloned().unwrap_or_default();
            let upper = stats.get(TIERS[chosen]).cloned().unwrap_or_default();
            let bar = if upper.runs >= 3 { upper.rate() - 0.05 } else { 0.85 };
            if lower.runs >= MIN_EVIDENCE && lower.rate() >= bar {
                chosen -= 1;
            } else {
                break;
            }
        }
        let note = if chosen < base {
            format!("{} ทำได้ดีพอๆ กัน → ใช้ตัวถูกกว่า", TIERS[chosen])
        } else {
            "ใช้ตามตารางพื้นฐาน".into()
        };
        policy.agents.entry(provider).or_default().stages.insert(
            stage,
            StagePolicy {
                base: TIERS[base].into(),
                chosen: TIERS[chosen].into(),
                offset: chosen as i32 - base as i32,
                stats,
                note,
            },
        );
    }
    Ok(policy)
}

/// Runs of this stage on this agent so far (full-power runs left out): the counter both probes go by.
pub fn stage_runs(c: &Connection, provider: &str, stage: &str) -> i64 {
    let sql = if agent_of(provider) == "claude" {
        "SELECT COUNT(*) FROM runs WHERE stage=?1 AND power=0 AND (provider='claude' OR COALESCE(provider,'')='')"
    } else {
        "SELECT COUNT(*) FROM runs WHERE stage=?1 AND power=0 AND provider=?2"
    };
    let n = if sql.contains("?2") { c.query_row(sql, [stage, provider], |r| r.get(0)) } else { c.query_row(sql, [stage], |r| r.get(0)) };
    n.unwrap_or(0)
}

/// Every PROBE_EVERY-th run of a stage on an agent tries one tier cheaper to keep collecting evidence.
pub fn should_probe(n: i64) -> bool {
    n % PROBE_EVERY == PROBE_EVERY - 1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(stage: &str) -> usize {
        if stage == "verdict" { 2 } else { 1 }
    }

    fn add_run(c: &Connection, idea: i64, stage: &str, model: &str, id: &str, ok: bool) {
        add_on(c, "", idea, stage, model, id, ok)
    }

    fn add_on(c: &Connection, provider: &str, idea: i64, stage: &str, model: &str, id: &str, ok: bool) {
        c.execute(
            "INSERT INTO runs(idea_id,stage,model,model_id,started,ended,status,summary,provider) VALUES(?1,?2,?3,?4,1000,1100,?5,?6,?7)",
            rusqlite::params![idea, stage, model, id, if ok { "ok" } else { "error" }, if ok { Some("s") } else { None }, provider],
        )
        .unwrap();
    }

    fn db() -> Connection {
        let p = std::env::temp_dir().join(format!("ia-tuner-{}-{:?}.db", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_file(&p);
        let c = crate::db::open(&p).unwrap();
        crate::db::insert_idea(&c, "i1", "t", "", 0, false).unwrap();
        c
    }

    #[test]
    fn adopts_cheaper_tier_with_evidence_and_resets_on_new_version() {
        let c = db();
        for _ in 0..5 {
            add_run(&c, 1, "verdict", "opus", "claude-opus-5-5", true);
        }
        for _ in 0..4 {
            add_run(&c, 1, "verdict", "sonnet", "claude-sonnet-5-5", true);
        }
        let p = learn(&c, base).unwrap();
        assert_eq!(p.offset("claude", "verdict"), 0, "4 runs is not enough evidence");
        add_run(&c, 1, "verdict", "sonnet", "claude-sonnet-5-5", true);
        let p = learn(&c, base).unwrap();
        assert_eq!(p.agents["claude"].stages["verdict"].chosen, "standard");
        // a new sonnet release: old evidence no longer counts
        add_run(&c, 1, "verdict", "sonnet", "claude-sonnet-6", true);
        let p = learn(&c, base).unwrap();
        assert_eq!(p.offset("claude", "verdict"), 0);
        assert_eq!(p.version("claude", 1).as_deref(), Some("claude-sonnet-6"));
    }

    #[test]
    fn agents_are_learned_apart() {
        let c = db();
        for _ in 0..5 {
            add_on(&c, "claude", 1, "verdict", "strong", "claude-opus-5-5", true);
            // Codex does well on its cheaper tier; that says nothing about Claude's
            add_on(&c, "codex", 1, "verdict", "standard", "gpt-6.1-sol", true);
        }
        add_run(&c, 1, "verdict", "standard", "claude-sonnet-5-5", false);
        let p = learn(&c, base).unwrap();
        assert_eq!(p.offset("codex", "verdict"), -1);
        assert_eq!(p.offset("claude", "verdict"), 0);
        assert_eq!(p.version("claude", 1).as_deref(), Some("claude-sonnet-5-5"));
        assert_eq!(p.version("codex", 1).as_deref(), Some("gpt-6.1-sol"));
        // runs from before the agent was stored were Claude's
        assert_eq!(stage_runs(&c, "claude", "verdict"), 6);
        assert_eq!(stage_runs(&c, "codex", "verdict"), 5);
    }

    #[test]
    fn worse_cheaper_tier_is_not_adopted() {
        let c = db();
        for _ in 0..6 {
            add_run(&c, 1, "research", "sonnet", "s1", true);
        }
        for i in 0..6 {
            add_run(&c, 1, "research", "haiku", "h1", i % 2 == 0);
        }
        assert_eq!(learn(&c, base).unwrap().offset("claude", "research"), 0);
    }
}
