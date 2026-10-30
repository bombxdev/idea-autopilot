//! Keeps the model routing current as models improve — from run statistics only, no tokens.
//!
//! - Runs are tagged with the concrete model id (e.g. `claude-sonnet-5-5`, or the agent's default). Only runs on the
//!   newest version of each tier count, so a new release starts with fresh evidence.
//! - Every 5th run of a stage "probes" one tier cheaper than the current choice.
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

#[derive(Debug, Clone, Serialize, Default)]
pub struct Policy {
    pub stages: BTreeMap<String, StagePolicy>,
    /// newest concrete model id seen per tier
    pub versions: BTreeMap<String, String>,
    pub learned_at: i64,
}

impl Policy {
    pub fn offset(&self, stage: &str) -> i32 {
        self.stages.get(stage).map(|s| s.offset).unwrap_or(0)
    }
}

/// Runs recorded before tiers were agent-neutral carry Claude names (haiku/sonnet/opus).
pub fn tier_index(model: &str) -> Option<usize> {
    crate::router::tier_of(model)
}

struct RunRow {
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
        "SELECT stage, model, model_id, status, summary IS NOT NULL, idea_id, COALESCE(ended, started)
         FROM runs WHERE status != 'running' AND status != 'cancelled' ORDER BY id DESC LIMIT 2000",
    )?;
    let mut rows = Vec::new();
    let mut versions: BTreeMap<String, String> = BTreeMap::new();
    for r in st.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, bool>(4)?,
            r.get::<_, i64>(5)?,
            r.get::<_, i64>(6)?,
        ))
    })? {
        let (stage, model, model_id, status, has_summary, idea, ended) = r?;
        let Some(tier) = tier_index(&model) else { continue };
        if let Some(id) = &model_id {
            versions.entry(TIERS[tier].to_string()).or_insert_with(|| id.clone()); // newest first
        }
        let steered = neg.iter().any(|(i, t)| *i == idea && *t >= ended && *t - ended < 86400);
        rows.push(RunRow { stage, tier, model_id, good: status == "ok" && has_summary && !steered });
    }

    let mut policy = Policy { versions: versions.clone(), learned_at: crate::db::now(), ..Default::default() };
    let stages: Vec<String> = {
        let mut v: Vec<String> = rows.iter().map(|r| r.stage.clone()).collect();
        v.sort();
        v.dedup();
        v
    };
    for stage in stages {
        let base = base_tier(&stage);
        let mut stats: BTreeMap<String, TierStat> = BTreeMap::new();
        for t in 0..TIERS.len() {
            let newest = versions.get(TIERS[t]);
            let s = rows
                .iter()
                // only the newest version of a tier counts; untagged old runs count until one exists
                .filter(|r| r.stage == stage && r.tier == t && (newest.is_none() || r.model_id.as_ref() == newest))
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
        policy.stages.insert(
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

/// Every PROBE_EVERY-th run of a stage tries one tier cheaper to keep collecting evidence.
pub fn should_probe(c: &Connection, stage: &str) -> bool {
    let n: i64 = c.query_row("SELECT COUNT(*) FROM runs WHERE stage=?1", [stage], |r| r.get(0)).unwrap_or(0);
    n % PROBE_EVERY == PROBE_EVERY - 1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(stage: &str) -> usize {
        if stage == "verdict" { 2 } else { 1 }
    }

    fn add_run(c: &Connection, idea: i64, stage: &str, model: &str, id: &str, ok: bool) {
        c.execute(
            "INSERT INTO runs(idea_id,stage,model,model_id,started,ended,status,summary) VALUES(?1,?2,?3,?4,1000,1100,?5,?6)",
            rusqlite::params![idea, stage, model, id, if ok { "ok" } else { "error" }, if ok { Some("s") } else { None }],
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
        assert_eq!(p.offset("verdict"), 0, "4 runs is not enough evidence");
        add_run(&c, 1, "verdict", "sonnet", "claude-sonnet-5-5", true);
        let p = learn(&c, base).unwrap();
        assert_eq!(p.stages["verdict"].chosen, "standard");
        // a new sonnet release: old evidence no longer counts
        add_run(&c, 1, "verdict", "sonnet", "claude-sonnet-6", true);
        let p = learn(&c, base).unwrap();
        assert_eq!(p.offset("verdict"), 0);
        assert_eq!(p.versions["standard"], "claude-sonnet-6");
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
        assert_eq!(learn(&c, base).unwrap().offset("research"), 0);
    }
}
