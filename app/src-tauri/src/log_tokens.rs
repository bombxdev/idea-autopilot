//! Token counts for bot runs that ended without them, read back from the agent's own log.
//!
//! The runner records tokens from the agent's output. Two cases leave a run at 0:
//! - Codex stopped before its final `turn.completed` line (timeout, cancel, error), although its rollout log
//!   (`sessions/**/rollout-*-<thread>.jsonl`) already holds the running total;
//! - Mistral Vibe prints no usage at all, but its session journal (`logs/session/unified/<id>/journal/*.jsonl`) has one
//!   usage block per model call.
//! The run is matched to its log by the thread id the runner saved, or else by the idea folder and the log that started
//! closest to the run (within a few minutes of its start, so back-to-back runs never share one log).
//! OpenCode's free OpenRouter models report 0 even in OpenCode's own database, so nothing can be recovered there.
//! Results are kept in memory per run, so the usage report reads each log once.
use parking_lot::Mutex;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// input (without cache), output, cache read, cache write
pub type Tokens = [i64; 4];

static FOUND: LazyLock<Mutex<HashMap<i64, Option<Tokens>>>> = LazyLock::new(Default::default);

/// What the report needs to find one run's log.
pub struct RunRef<'a> {
    pub id: i64,
    pub provider: &'a str,
    pub session_id: Option<&'a str>,
    pub idea_dir: PathBuf,
    pub started: i64,
    pub ended: i64,
}

fn ts(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s).ok().map(|d| d.timestamp())
}

/// how far after a run's start its agent may open the log
const START_SLACK: i64 = 300;

/// Distance of a log's start from the run's start, when it lies within [started - 60, started + START_SLACK].
fn near(t: i64, r: &RunRef) -> Option<i64> {
    (t >= r.started - 60 && t <= r.started + START_SLACK).then(|| (t - r.started).abs())
}

fn same_dir(a: &str, b: &Path) -> bool {
    let n = |s: &str| s.replace('\\', "/").trim_end_matches('/').to_lowercase();
    n(a) == n(&b.to_string_lossy())
}

/// The last running total of a Codex rollout. Codex counts cached input inside `input_tokens`.
pub fn codex_total(text: &str) -> Option<Tokens> {
    let line = text.lines().rev().find(|l| l.contains("\"total_token_usage\""))?;
    let v: Value = serde_json::from_str(line).ok()?;
    let t = &v["payload"]["info"]["total_token_usage"];
    let g = |k: &str| t[k].as_i64().unwrap_or(0);
    let cached = g("cached_input_tokens");
    Some([(g("input_tokens") - cached).max(0), g("output_tokens"), cached, g("cache_write_input_tokens")]).filter(|x| x.iter().any(|n| *n > 0))
}

/// The sum of a Vibe journal's usage blocks, each model call once (it is logged again as the next step's input).
pub fn vibe_total(text: &str) -> Option<Tokens> {
    let mut seen = HashSet::new();
    let mut sum = [0i64; 4];
    for line in text.lines().filter(|l| l.contains("\"usage\"")) {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        if v["type"] != "action_result" {
            continue;
        }
        let r = &v["payload"]["result"];
        let id = r["action_id"].as_str().unwrap_or_default().to_string();
        let u = &r["result"]["usage"];
        if !u.is_object() || !seen.insert(id) {
            continue;
        }
        let g = |k: &str| u[k].as_i64().unwrap_or(0);
        let cached = g("cached_input_tokens");
        sum[0] += (g("input_tokens") - cached).max(0);
        sum[1] += g("output_tokens");
        sum[2] += cached;
        sum[3] += g("cache_write_tokens");
    }
    Some(sum).filter(|x| x.iter().any(|n| *n > 0))
}

fn first_line(p: &Path) -> Option<String> {
    use std::io::BufRead;
    std::io::BufReader::new(std::fs::File::open(p).ok()?).lines().next()?.ok()
}

fn codex(homes: &[PathBuf], r: &RunRef) -> Option<Tokens> {
    let mut files = Vec::new();
    for h in homes {
        collect(&h.join("sessions"), &mut files, 3);
    }
    let by_id = r.session_id.filter(|s| !s.is_empty()).and_then(|sid| files.iter().find(|f| f.to_string_lossy().contains(sid)));
    let file = by_id.cloned().or_else(|| {
        // no thread id: the rollout that started in this run's idea folder during the run
        files
            .iter()
            .filter_map(|f| {
                let v: Value = serde_json::from_str(&first_line(f)?).ok()?;
                let p = &v["payload"];
                if !p["cwd"].as_str().is_some_and(|c| same_dir(c, &r.idea_dir)) {
                    return None;
                }
                Some((near(p["timestamp"].as_str().and_then(ts)?, r)?, f))
            })
            .min_by_key(|(d, _)| *d)
            .map(|(_, f)| f.clone())
    })?;
    codex_total(&std::fs::read_to_string(file).ok()?)
}

fn vibe(homes: &[PathBuf], r: &RunRef) -> Option<Tokens> {
    let mut best: Option<(i64, PathBuf)> = None;
    for h in homes {
        let Ok(rd) = std::fs::read_dir(h.join("logs").join("session").join("unified")) else { continue };
        for e in rd.flatten() {
            let Some(meta) = std::fs::read_to_string(e.path().join("meta.json")).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()) else { continue };
            let dir = meta["environment"]["working_directory"].as_str().unwrap_or_default();
            let Some(d) = meta["start_time"].as_str().and_then(ts).and_then(|t| near(t, r)) else { continue };
            if same_dir(dir, &r.idea_dir) && best.as_ref().is_none_or(|(b, _)| d < *b) {
                best = Some((d, e.path()));
            }
        }
    }
    let (_, session) = best?;
    let mut journals = Vec::new();
    collect(&session.join("journal"), &mut journals, 0);
    let text: String = journals.iter().filter_map(|j| std::fs::read_to_string(j).ok()).collect::<Vec<_>>().join("\n");
    vibe_total(&text)
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() && depth > 0 {
            collect(&p, out, depth - 1);
        } else if p.extension().is_some_and(|x| x == "jsonl") {
            out.push(p);
        }
    }
}

/// Tokens of a run that recorded none, from its agent's log (None = no log, or the log has none either).
pub fn recover(root: &Path, r: &RunRef) -> Option<Tokens> {
    if let Some(hit) = FOUND.lock().get(&r.id) {
        return *hit;
    }
    let mut homes: Vec<PathBuf> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(root.join("data").join("accounts")) {
        homes.extend(rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()));
    }
    let found = match r.provider {
        "codex" => {
            if let Some(h) = dirs::home_dir() {
                homes.push(h.join(".codex"));
            }
            codex(&homes, r)
        }
        "mistral" => {
            if let Some(h) = dirs::home_dir() {
                homes.push(h.join(".vibe"));
            }
            vibe(&homes, r)
        }
        _ => None,
    };
    // a run still going may log more later: remember only finished answers
    if r.ended > 0 {
        FOUND.lock().insert(r.id, found);
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_takes_the_last_running_total() {
        let t = r#"{"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"cached_input_tokens":60,"output_tokens":5}}}}
{"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":900,"cached_input_tokens":700,"output_tokens":40}}}}"#;
        assert_eq!(codex_total(t), Some([200, 40, 700, 0]));
        assert_eq!(codex_total("{}"), None);
    }

    #[test]
    fn vibe_counts_each_call_once() {
        let call = |id: &str, kind: &str| {
            format!(r#"{{"type":"{kind}","payload":{{"result":{{"action_id":"{id}","result":{{"usage":{{"input_tokens":1000,"cached_input_tokens":400,"output_tokens":20,"cache_write_tokens":0}}}}}}}}}}"#)
        };
        let t = [call("a", "action_result"), call("a", "core_input"), call("a", "action_result"), call("b", "action_result")].join("\n");
        assert_eq!(vibe_total(&t), Some([1200, 40, 800, 0]));
    }
}

