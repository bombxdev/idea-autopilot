//! What each model really costs per *successful* stage, from the bot's own run history — no tokens.
//!
//! A pricier model that finishes in one go can cost less than a cheap one that needs three tries,
//! and a newer release of the same tier can cost less than the old one (or more). So models are
//! compared by `cost per good run` = everything spent on them ÷ runs that came out good (bad runs
//! are paid for too), not by tier or list price:
//! - a model replaces the usual one for a stage only with ≥5 runs of its own, quality within 5 points
//!   of the usual one, and at least 10% cheaper per good run;
//! - models the owner lists in `extra_models` (e.g. last generation's) get an occasional try until
//!   they have that evidence, so the comparison can exist at all;
//! - full-power runs are left out of the numbers (they use the strongest model and highest effort).
//! Cost = what the agent reported (USD) when it did; otherwise estimated from tokens (marked `~`).
use rusqlite::Connection;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

const WINDOW: usize = 20;
pub const MIN_EVIDENCE: usize = 5;
const USUAL_MIN: usize = 3;
const MIN_SAVING: f64 = 0.10;
const QUALITY_SLACK: f64 = 0.05;
const PROBE_EVERY: i64 = 3;

#[derive(Debug, Clone, Default)]
pub struct Row {
    stage: String,
    provider: String,
    arm: String,
    asked: String,
    good: bool,
    /// None = the run reported neither money nor tokens: its cost is unknown, not zero
    cost: Option<f64>,
    estimated: bool,
    turns: i64,
    secs: i64,
    // to read the tokens back from the agent's log when the run recorded none
    id: i64,
    session_id: Option<String>,
    slug: String,
    started: i64,
    ended: i64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Arm {
    pub arm: String,
    pub label: String,
    pub runs: usize,
    pub good: usize,
    /// spend of the runs whose cost is known
    pub cost: f64,
    /// runs whose cost is known
    pub priced: usize,
    pub turns: i64,
    pub secs: i64,
    pub estimated: bool,
}

impl Arm {
    pub fn rate(&self) -> f64 {
        if self.runs == 0 { 0.0 } else { self.good as f64 / self.runs as f64 }
    }
    /// Can this model's cost be compared at all? Not while its runs say nothing about cost.
    pub fn known(&self) -> bool {
        self.priced * 2 >= self.runs.max(1)
    }
    /// average spend per run ÷ success rate (bad runs are paid for too); infinite while nothing has come out good
    pub fn per_good(&self) -> f64 {
        if self.good == 0 || self.priced == 0 { f64::INFINITY } else { self.cost / self.priced as f64 / self.rate() }
    }
}

/// What a run cost: the agent's own USD figure, else a token estimate (output weighs 5x input, cache reads 0.1x).
/// None when the run reported neither — free OpenRouter models, Mistral Vibe, a Codex run stopped part-way —
/// so a run with no figures never looks free.
pub fn run_cost(usd: f64, input: i64, output: i64, cache_read: i64, cache_write: i64) -> Option<(f64, bool)> {
    if usd > 0.0 {
        return Some((usd, false));
    }
    let units = input as f64 + 5.0 * output as f64 + 0.1 * cache_read as f64 + 1.25 * cache_write as f64;
    (units > 0.0).then(|| (units / 1e6 * 3.0, true))
}

/// The runs the comparison is made from (read under the db lock; `fill` afterwards, without it).
pub fn rows(c: &Connection) -> Vec<Row> {
    load(c).unwrap_or_default()
}

/// Runs that recorded no cost: read their tokens back from the agent's own log where it has one (Codex, Mistral Vibe).
pub fn fill(rows: &mut [Row], root: &Path, ws: &Path) {
    for r in rows.iter_mut().filter(|r| r.cost.is_none() && matches!(r.provider.as_str(), "codex" | "mistral")) {
        let rr = crate::log_tokens::RunRef {
            id: r.id,
            provider: &r.provider,
            session_id: r.session_id.as_deref(),
            idea_dir: crate::workspace::idea_dir(ws, &r.slug),
            started: r.started,
            ended: r.ended,
        };
        if let Some([i, o, cr, cw]) = crate::log_tokens::recover(root, &rr) {
            r.cost = run_cost(0.0, i, o, cr, cw).map(|(c, _)| c);
            r.estimated = true;
        }
    }
}

fn load(c: &Connection) -> rusqlite::Result<Vec<Row>> {
    // same quality rule as the tuner: finished OK, wrote its status, and the owner did not send it back within a day
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
        "SELECT r.stage, COALESCE(r.provider,''), r.model, COALESCE(r.model_used,''), r.model_id, r.status, r.summary IS NOT NULL, r.idea_id,
                COALESCE(r.ended, r.started), r.started,
                COALESCE(r.input_tokens,0), COALESCE(r.output_tokens,0), COALESCE(r.cache_read,0), COALESCE(r.cache_write,0), COALESCE(r.cost_usd,0),
                COALESCE(r.num_turns,0), r.id, r.session_id, COALESCE(i.slug,'')
         FROM runs r LEFT JOIN ideas i ON i.id = r.idea_id
         WHERE r.status NOT IN ('running','cancelled') AND r.power=0 ORDER BY r.id DESC LIMIT 2000",
    )?;
    let rows = st
        .query_map([], |r| {
            let (stage, provider, model, used, model_id): (String, String, String, String, Option<String>) = (r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?);
            // runs from before the agent was stored were all Claude's
            let provider = crate::tuner::agent_of(&provider).to_string();
            let (status, has_summary, idea, ended, started): (String, bool, i64, i64, i64) = (r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?);
            let (inp, out, cr, cw, usd, turns): (i64, i64, i64, i64, f64, i64) = (r.get(10)?, r.get(11)?, r.get(12)?, r.get(13)?, r.get(14)?, r.get(15)?);
            let (id, session_id, slug): (i64, Option<String>, String) = (r.get(16)?, r.get(17)?, r.get(18)?);
            let arm = match model_id.filter(|m| !m.is_empty() && !m.ends_with("-default")) {
                Some(id) => id,
                None if !used.is_empty() => used.clone(),
                None => model.clone(),
            };
            let steered = neg.iter().any(|(i, t)| *i == idea && *t >= ended && *t - ended < 86400);
            let priced = run_cost(usd, inp, out, cr, cw);
            Ok(Row {
                stage,
                provider,
                arm,
                asked: used,
                good: status == "ok" && has_summary && !steered,
                cost: priced.map(|p| p.0),
                estimated: priced.is_some_and(|p| p.1),
                turns,
                secs: (ended - started).max(0),
                id,
                session_id,
                slug,
                started,
                ended,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn arms_of<'a>(rows: impl Iterator<Item = &'a Row>) -> Vec<Arm> {
    let mut by: BTreeMap<String, Vec<&Row>> = BTreeMap::new();
    for r in rows {
        by.entry(r.arm.clone()).or_default().push(r);
    }
    let mut out: Vec<Arm> = by
        .into_iter()
        .map(|(arm, rs)| {
            // rows are newest first: the rolling window keeps the latest runs
            let rs: Vec<&&Row> = rs.iter().take(WINDOW).collect();
            let mut a = Arm { label: crate::modelname::label("", Some(&arm), ""), arm, ..Default::default() };
            for r in rs {
                a.runs += 1;
                a.good += r.good as usize;
                if let Some(c) = r.cost {
                    a.cost += c;
                    a.priced += 1;
                }
                a.turns += r.turns;
                a.secs += r.secs;
                a.estimated |= r.estimated;
            }
            a
        })
        .collect();
    out.sort_by(|a, b| b.runs.cmp(&a.runs));
    out
}

#[derive(Debug, Clone)]
pub struct Pick {
    pub model: String,
    pub note: String,
}

fn money(x: f64, est: bool) -> String {
    format!("{}${:.2}", if est { "~" } else { "" }, x)
}

/// The cheapest model per good run for this stage on this agent, if one clearly beats the usual model.
/// Models whose runs report no cost are never compared: unknown is not free.
pub fn pick_from(rows: &[Row], provider: &str, stage: &str, usual: &str) -> Option<Pick> {
    let provider = crate::tuner::agent_of(provider);
    let arms = arms_of(rows.iter().filter(|r| r.stage == stage && r.provider == provider));
    let base = arms.iter().find(|a| a.arm == usual)?;
    if base.runs < USUAL_MIN || !base.known() {
        return None;
    }
    let (bq, bc) = (base.rate(), base.per_good());
    let best = arms
        .iter()
        .filter(|a| a.arm != usual && a.runs >= MIN_EVIDENCE && a.known())
        .filter(|a| a.rate() >= bq - QUALITY_SLACK && (bc.is_infinite() || a.per_good() <= bc * (1.0 - MIN_SAVING)))
        .min_by(|a, b| a.per_good().partial_cmp(&b.per_good()).unwrap_or(std::cmp::Ordering::Equal))?;
    if best.per_good().is_infinite() {
        return None;
    }
    let est = best.estimated || base.estimated;
    Some(Pick {
        model: best.arm.clone(),
        note: format!(
            "คุ้มสุดสำหรับขั้นนี้: {} {}/ครั้งที่สำเร็จ (สำเร็จ {:.0}%) เทียบ {} {}/ครั้ง (สำเร็จ {:.0}%)",
            best.label,
            money(best.per_good(), est),
            best.rate() * 100.0,
            base.label,
            if bc.is_infinite() { "ยังไม่สำเร็จเลย".to_string() } else { money(bc, est) },
            bq * 100.0
        ),
    })
}

/// A model from `extra_models` that still lacks evidence for this stage on this agent, tried now and then so it can be
/// compared at all. `n` = runs of this stage on this agent so far (`tuner::stage_runs`); the caller skips this on a run
/// that already probes a cheaper tier, so one run never changes two things at once.
pub fn probe_extra(rows: &[Row], n: i64, provider: &str, stage: &str, extras: &[String]) -> Option<String> {
    if extras.is_empty() || n % PROBE_EVERY != PROBE_EVERY - 1 {
        return None;
    }
    let provider = crate::tuner::agent_of(provider);
    extras
        .iter()
        .map(|m| (rows.iter().filter(|r| r.stage == stage && r.provider == provider && (r.arm == *m || r.asked == *m)).count(), m))
        .filter(|(k, _)| *k < MIN_EVIDENCE)
        .min_by_key(|(k, _)| *k)
        .map(|(_, m)| m.clone())
}

pub fn extras(c: &Connection) -> Vec<String> {
    crate::db::get_setting(c, "extra_models")
        .unwrap_or_default()
        .split([',', '\n', ';'])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s.len() < 80 && s.chars().all(|ch| ch.is_ascii_alphanumeric() || "-._:/[]".contains(ch)))
        .collect()
}

/// For the Quota page: every model that has run, overall and per stage.
/// `rows` from `rows()` + `fill()`, `extras` from `extras()`.
pub fn report(rows: &[Row], extras: Vec<String>) -> Value {
    let row = |a: &Arm| {
        json!({
            "arm": a.arm, "label": a.label, "runs": a.runs, "ok_pct": (a.rate() * 100.0).round(),
            // null = the runs reported no cost at all (not free)
            "avg_cost": if a.priced > 0 { json!(a.cost / a.priced as f64) } else { Value::Null },
            "per_good": if a.known() && a.per_good().is_finite() { json!(a.per_good()) } else { Value::Null },
            "avg_turns": if a.runs > 0 { a.turns as f64 / a.runs as f64 } else { 0.0 },
            "avg_min": if a.runs > 0 { a.secs as f64 / a.runs as f64 / 60.0 } else { 0.0 },
            "estimated": a.estimated,
        })
    };
    let overall: Vec<Value> = arms_of(rows.iter()).iter().map(row).collect();
    let mut stages: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let names: std::collections::BTreeSet<&str> = rows.iter().map(|r| r.stage.as_str()).collect();
    for s in names {
        stages.insert(s.to_string(), arms_of(rows.iter().filter(|r| r.stage == s)).iter().map(row).collect());
    }
    json!({ "overall": overall, "stages": stages, "min_evidence": MIN_EVIDENCE, "extra_models": extras })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(stage: &str, arm: &str, good: bool, cost: f64) -> Row {
        Row { stage: stage.into(), provider: "claude".into(), arm: arm.into(), good, cost: Some(cost), turns: 10, secs: 600, ..Default::default() }
    }

    #[test]
    fn a_model_that_reports_no_cost_never_looks_free() {
        let mut rows = many("build", "claude-sonnet-5", 6, 6, 1.0);
        rows.extend((0..6).map(|_| Row { cost: None, ..run("build", "free-looking", true, 0.0) }));
        assert!(pick_from(&rows, "claude", "build", "claude-sonnet-5").is_none());
        // nor can a model with unknown cost be the yardstick
        assert!(pick_from(&rows, "claude", "build", "free-looking").is_none());
    }

    #[test]
    fn agents_are_compared_apart() {
        let mut rows = many("build", "claude-sonnet-5", 6, 6, 1.0);
        rows.extend(many("build", "gpt-cheap", 6, 6, 0.1).into_iter().map(|r| Row { provider: "codex".into(), ..r }));
        assert!(pick_from(&rows, "claude", "build", "claude-sonnet-5").is_none());
        // runs from before the agent was stored count as Claude's (load() maps "" to "claude")
        assert_eq!(crate::tuner::agent_of(""), "claude");
    }

    #[test]
    fn extra_models_are_probed_per_agent() {
        let extras = vec!["claude-opus-4-8".to_string()];
        assert_eq!(probe_extra(&[], 2, "claude", "build", &extras).as_deref(), Some("claude-opus-4-8"));
        assert!(probe_extra(&[], 3, "claude", "build", &extras).is_none());
        let mut rows = many("build", "claude-opus-4-8", 5, 5, 1.0);
        assert!(probe_extra(&rows, 2, "claude", "build", &extras).is_none(), "enough evidence already");
        rows.iter_mut().for_each(|r| r.provider = "codex".into());
        assert!(probe_extra(&rows, 2, "claude", "build", &extras).is_some(), "evidence on another agent does not count");
    }

    fn many(stage: &str, arm: &str, n: usize, good: usize, cost: f64) -> Vec<Row> {
        (0..n).map(|i| run(stage, arm, i < good, cost)).collect()
    }

    #[test]
    fn cheaper_per_run_is_not_cheaper_per_success() {
        // the usual model: $1 a run, always right. The "cheap" one: $0.6 a run, right half the time = $1.2 per success
        let mut rows = many("build", "claude-sonnet-5", 6, 6, 1.0);
        rows.extend(many("build", "claude-haiku-5", 6, 3, 0.6));
        assert!(pick_from(&rows, "claude", "build", "claude-sonnet-5").is_none());
    }

    #[test]
    fn a_pricier_model_that_gets_it_right_first_time_wins() {
        // usual: $1 a run but only 4 in 6 good -> $1.5 per success; the strong one: $1.2 a run, 6 in 6 -> $1.2
        let mut rows = many("build", "claude-sonnet-5", 6, 4, 1.0);
        rows.extend(many("build", "claude-opus-5-5", 6, 6, 1.2));
        let p = pick_from(&rows, "claude", "build", "claude-sonnet-5").unwrap();
        assert_eq!(p.model, "claude-opus-5-5");
        assert!(p.note.contains("Opus 5.5"), "{}", p.note);
    }

    #[test]
    fn last_generation_that_costs_more_loses_to_the_new_one() {
        let mut rows = many("research", "claude-opus-5-5", 6, 6, 0.8);
        rows.extend(many("research", "claude-opus-4-8", 6, 6, 1.1));
        // already on the cheaper one: nothing to switch to
        assert!(pick_from(&rows, "claude", "research", "claude-opus-5-5").is_none());
        // on the dearer one: switch
        assert_eq!(pick_from(&rows, "claude", "research", "claude-opus-4-8").unwrap().model, "claude-opus-5-5");
    }

    #[test]
    fn needs_evidence_and_keeps_quality() {
        let mut rows = many("build", "a", 6, 6, 1.0);
        rows.extend(many("build", "b", 4, 4, 0.1)); // too few runs
        assert!(pick_from(&rows, "claude", "build", "a").is_none());
        let mut rows = many("build", "a", 6, 6, 1.0);
        rows.extend(many("build", "c", 6, 4, 0.1)); // cheap but worse by more than 5 points
        assert!(pick_from(&rows, "claude", "build", "a").is_none());
        // too little saving
        let mut rows = many("build", "a", 6, 6, 1.0);
        rows.extend(many("build", "d", 6, 6, 0.95));
        assert!(pick_from(&rows, "claude", "build", "a").is_none());
    }

    #[test]
    fn token_estimate_when_no_usd() {
        assert_eq!(run_cost(0.5, 1, 1, 1, 1), Some((0.5, false)));
        assert_eq!(run_cost(0.0, 0, 0, 0, 0), None);
        let (c, est) = run_cost(0.0, 1_000_000, 100_000, 0, 0).unwrap();
        assert!(est && c > 0.0);
    }
}
