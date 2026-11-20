//! Tokens used in the app's chats ("สั่ง AI แก้ / ทำต่อ" and the Sessions page), for the usage report.
//!
//! The bot's own runs are counted in the `runs` table. Chats are not: Claude chats run as Claude Code sessions in the app
//! folder and Codex chats as `codex exec` threads in the idea folder, and each agent writes its own log. A Claude chat
//! starts with the session brief from the app's SessionStart hook ("# Idea Autopilot — remote session for idea #N"); a
//! Codex chat's first message is "Idea #N: … Read SESSION.md in this folder first". Either tells a chat apart from
//! anything else in those folders (bot runs, other Claude Code work) and names its idea. Usage is summed per chat,
//! model and hour.
//! Gemini, OpenCode and Mistral chats report no tokens, so they are not counted here.
//!
//! Logs are read once and kept in memory until the file changes (same size and modified time = same result).
use parking_lot::Mutex;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

const MARKER: &str = "remote session for idea #";
const CODEX_FIRST: &str = "Read SESSION.md in this folder first";
/// lines read to find the start of a chat
const HEAD_CLAUDE: usize = 15;
const HEAD_CODEX: usize = 60;

/// One chat's usage within one hour on one model.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChatHour {
    pub idea_id: i64,
    pub provider: &'static str,
    pub account: String,
    pub model: String,
    pub hour: i64,
    pub first: i64,
    pub last: i64,
    pub messages: i64,
    pub input: i64,
    pub output: i64,
    pub cache_read: i64,
    pub cache_write: i64,
}

type Key = (PathBuf, u64, std::time::SystemTime);
static CACHE: LazyLock<Mutex<HashMap<PathBuf, (Key, Vec<ChatHour>)>>> = LazyLock::new(Default::default);

fn ts(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s).ok().map(|d| d.timestamp())
}

fn number_after(text: &str, at: usize) -> Option<i64> {
    let digits: String = text[at..].chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// The idea of a Claude chat: the brief the SessionStart hook added (a later mention of the text does not count).
fn claude_idea(line: &str) -> Option<i64> {
    if !line.contains("SessionStart") {
        return None;
    }
    number_after(line, line.find(MARKER)? + MARKER.len())
}

/// The idea of a Codex chat: its first message names it ("Idea #N: …") and points at SESSION.md.
fn codex_idea(line: &str) -> Option<i64> {
    let end = line.find(CODEX_FIRST)?;
    let at = line[..end].rfind("Idea #")? + "Idea #".len();
    number_after(line, at)
}

fn bucket(rows: &mut BTreeMap<(String, i64), ChatHour>, base: &ChatHour, model: &str, t: i64, u: [i64; 4]) {
    let hour = t - t.rem_euclid(3600);
    let e = rows.entry((model.to_string(), hour)).or_insert_with(|| ChatHour { model: model.to_string(), hour, first: t, last: t, ..base.clone() });
    e.first = e.first.min(t);
    e.last = e.last.max(t);
    e.messages += 1;
    e.input += u[0];
    e.output += u[1];
    e.cache_read += u[2];
    e.cache_write += u[3];
}

/// A Claude Code transcript (.jsonl). One assistant message is written as several lines that repeat its usage, so each
/// message id is counted once (its last line wins).
pub fn read_claude(text: &str, account: &str) -> Vec<ChatHour> {
    let Some(idea_id) = text.lines().take(HEAD_CLAUDE).find_map(claude_idea) else { return vec![] };
    let mut msgs: HashMap<String, (i64, String, [i64; 4])> = HashMap::new();
    let mut order = Vec::new();
    for line in text.lines() {
        if !line.contains("\"usage\"") {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        let m = &v["message"];
        let (Some(id), Some(t)) = (m["id"].as_str(), v["timestamp"].as_str().and_then(ts)) else { continue };
        let u = &m["usage"];
        let g = |k: &str| u[k].as_i64().unwrap_or(0);
        let model = m["model"].as_str().unwrap_or("claude").to_string();
        if model == "<synthetic>" {
            continue;
        }
        if !msgs.contains_key(id) {
            order.push(id.to_string());
        }
        msgs.insert(id.to_string(), (t, model, [g("input_tokens"), g("output_tokens"), g("cache_read_input_tokens"), g("cache_creation_input_tokens")]));
    }
    let base = ChatHour { idea_id, provider: "claude", account: account.into(), ..Default::default() };
    let mut rows = BTreeMap::new();
    for id in order {
        let (t, model, u) = &msgs[&id];
        bucket(&mut rows, &base, model, *t, *u);
    }
    rows.into_values().collect()
}

/// A Codex rollout (.jsonl). `token_count` events carry the running total of the thread, so each call's usage is the
/// growth of that total (a repeated event adds nothing). Codex counts cached input inside `input_tokens`.
pub fn read_codex(text: &str, account: &str) -> Vec<ChatHour> {
    let Some(idea_id) = text.lines().take(HEAD_CODEX).find_map(codex_idea) else { return vec![] };
    let base = ChatHour { idea_id, provider: "codex", account: account.into(), ..Default::default() };
    let mut rows = BTreeMap::new();
    let mut model = String::from("codex");
    let mut prev = [0i64; 3]; // input incl. cache, cached, output
    for line in text.lines() {
        if !(line.contains("\"turn_context\"") || line.contains("\"token_count\"")) {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        let p = &v["payload"];
        if v["type"] == "turn_context" {
            if let Some(m) = p["model"].as_str() {
                model = m.to_string();
            }
            continue;
        }
        let tot = &p["info"]["total_token_usage"];
        if !tot.is_object() {
            continue;
        }
        let g = |k: &str| tot[k].as_i64().unwrap_or(0);
        let now = [g("input_tokens"), g("cached_input_tokens"), g("output_tokens")];
        let d = [now[0] - prev[0], now[1] - prev[1], now[2] - prev[2]];
        if d.iter().all(|x| *x <= 0) {
            continue;
        }
        prev = now;
        let Some(t) = v["timestamp"].as_str().and_then(ts) else { continue };
        let cached = d[1].max(0);
        bucket(&mut rows, &base, &model, t, [(d[0] - cached).max(0), d[2].max(0), cached, 0]);
    }
    rows.into_values().collect()
}

fn read_file(p: &Path, codex: bool, account: &str) -> Vec<ChatHour> {
    let Ok(meta) = std::fs::metadata(p) else { return vec![] };
    let key: Key = (p.to_path_buf(), meta.len(), meta.modified().unwrap_or(std::time::UNIX_EPOCH));
    if let Some((k, rows)) = CACHE.lock().get(p) {
        if *k == key {
            return rows.clone();
        }
    }
    // a chat log names its idea in the first lines: skip anything that does not, without reading the whole file
    let rows = (|| {
        let f = std::fs::File::open(p).ok()?;
        let find = if codex { codex_idea } else { claude_idea };
        BufReader::new(f).lines().take(if codex { HEAD_CODEX } else { HEAD_CLAUDE }).map_while(Result::ok).find_map(|l| find(&l))?;
        let text = std::fs::read_to_string(p).ok()?;
        Some(if codex { read_codex(&text, account) } else { read_claude(&text, account) })
    })()
    .unwrap_or_default();
    CACHE.lock().insert(p.to_path_buf(), (key, rows.clone()));
    rows
}

fn jsonl_under(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() && depth > 0 {
            jsonl_under(&p, out, depth - 1);
        } else if p.extension().is_some_and(|x| x == "jsonl") {
            out.push(p);
        }
    }
}

/// Every chat hour that touches [from, to). `root` is the app folder (Claude chats run there).
pub fn collect(root: &Path, from: i64, to: i64) -> Vec<ChatHour> {
    let project = crate::activity::encode_path(root);
    // (account, ~/.claude-like home, ~/.codex-like home): the machine's own login, then each account's own folder
    let mut homes: Vec<(String, PathBuf, PathBuf)> = Vec::new();
    if let Some(h) = dirs::home_dir() {
        homes.push((String::new(), h.join(".claude"), h.join(".codex")));
    }
    if let Ok(rd) = std::fs::read_dir(root.join("data").join("accounts")) {
        for e in rd.flatten().filter(|e| e.path().is_dir()) {
            let id = e.file_name().to_string_lossy().to_string();
            homes.push((id, e.path(), e.path()));
        }
    }
    let from_secs = std::time::UNIX_EPOCH + std::time::Duration::from_secs(from.max(0) as u64);
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for (account, claude, codex) in homes {
        let mut files = Vec::new();
        jsonl_under(&claude.join("projects").join(&project), &mut files, 0);
        let n_claude = files.len();
        jsonl_under(&codex.join("sessions"), &mut files, 3);
        for (i, f) in files.into_iter().enumerate() {
            // a log last written before the period cannot hold anything in it
            if std::fs::metadata(&f).and_then(|m| m.modified()).is_ok_and(|m| m < from_secs) || !seen.insert(f.clone()) {
                continue;
            }
            out.extend(read_file(&f, i >= n_claude, &account).into_iter().filter(|r| r.hour + 3600 > from && r.hour < to));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_chat_counts_each_message_once_and_names_its_idea() {
        let t = r##"{"type":"attachment","attachment":{"hookEvent":"SessionStart","content":"# Idea Autopilot — remote session for idea #14: x"},"timestamp":"2026-11-04T03:00:00Z"}
{"type":"assistant","timestamp":"2026-11-04T03:10:00Z","message":{"id":"m1","model":"claude-opus-5-5","usage":{"input_tokens":2,"output_tokens":10,"cache_read_input_tokens":100,"cache_creation_input_tokens":5}}}
{"type":"assistant","timestamp":"2026-11-04T03:10:01Z","message":{"id":"m1","model":"claude-opus-5-5","usage":{"input_tokens":2,"output_tokens":40,"cache_read_input_tokens":100,"cache_creation_input_tokens":5}}}
{"type":"assistant","timestamp":"2026-11-04T04:05:00Z","message":{"id":"m2","model":"claude-opus-5-5","usage":{"input_tokens":1,"output_tokens":7,"cache_read_input_tokens":200,"cache_creation_input_tokens":0}}}"##;
        let r = read_claude(t, "");
        assert_eq!(r.len(), 2, "two hours");
        assert_eq!((r[0].idea_id, r[0].messages, r[0].output, r[0].cache_read), (14, 1, 40, 100));
        assert_eq!(r[1].cache_read, 200);
        assert!(read_claude(&t.replace("remote session for idea", "something else"), "").is_empty(), "not a chat");
        assert!(read_claude(&t.replace("SessionStart", "UserPrompt"), "").is_empty(), "a quoted brief is not a chat");
    }

    #[test]
    fn codex_chat_uses_the_growth_of_the_running_total() {
        let t = r##"{"type":"session_meta","payload":{"cwd":"x"}}
{"type":"event_msg","payload":{"type":"user_message","message":"Idea #20: y\nRead SESSION.md in this folder first (how this chat works)"}}
{"type":"turn_context","payload":{"model":"gpt-6-astra"}}
{"timestamp":"2026-11-04T15:14:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":1000,"cached_input_tokens":800,"output_tokens":50}}}}
{"timestamp":"2026-11-04T15:14:01Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":1000,"cached_input_tokens":800,"output_tokens":50}}}}
{"timestamp":"2026-11-04T15:20:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":1500,"cached_input_tokens":1200,"output_tokens":80}}}}"##;
        let r = read_codex(t, "Codex_Second");
        assert_eq!(r.len(), 1);
        let h = &r[0];
        assert_eq!((h.idea_id, h.model.as_str(), h.messages), (20, "gpt-6-astra", 2), "a repeated total adds nothing");
        assert_eq!((h.input, h.cache_read, h.output), (300, 1200, 80));
    }
}

