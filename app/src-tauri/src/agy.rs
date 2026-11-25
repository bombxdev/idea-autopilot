//! Antigravity CLI (`agy`, Google's successor of Gemini CLI — ISSUE-151).
//!
//! - Headless: `agy -p <prompt> --output-format stream-json`. Events are NDJSON keyed by `event`: `init` (model,
//!   cwd), `step_update` (one per change of a step: `step_type` user_input / agent_response / tool …, `state`
//!   ACTIVE / DONE / ERROR, `text_delta` pieces of the answer, `tool_info`), and `result` (status, response, usage).
//! - Quota: `agy -p /usage --output-format json` answers at once without using the model; its `command.data.groups`
//!   hold the plan's buckets (weekly, and a 5-hour one on paid plans) as `remaining_fraction` + `reset_time`.
//! - Accounts: the login is a Google sign-in kept in the OS keyring, shared by every process. An account of its own
//!   gets its own home folder (`USERPROFILE`/`HOME`, where agy keeps `~/.gemini`) and file token storage
//!   (`AGY_CLI_FORCE_FILE_TOKEN_STORAGE`), so its sign-in is a file in that folder and two accounts stay apart.
use serde_json::Value;
use std::collections::BTreeMap;

/// Reads one run's stream-json events into lines for the live log, plus what the run reports at the end.
#[derive(Default)]
pub struct Reader {
    text: BTreeMap<i64, String>,
    pub conversation: Option<String>,
    pub model: Option<String>,
    /// the `result` event: (status, response, input, output, cache read, turns)
    pub result: Option<(String, String, i64, i64, i64, i64)>,
}

fn short(s: &str, n: usize) -> String {
    let t: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() > n {
        format!("{}…", t.chars().take(n).collect::<String>())
    } else {
        t
    }
}

/// The parameter of a tool call that says what it is about (a path, a command, a query, a URL).
fn about(params: &Value) -> String {
    let o = match params.as_object() {
        Some(o) => o,
        None => return String::new(),
    };
    for k in ["CommandLine", "Command", "command", "Path", "AbsolutePath", "TargetFile", "path", "file", "Query", "query", "Url", "url", "SearchPath"] {
        if let Some(v) = o.get(k).and_then(Value::as_str) {
            return short(v, 120);
        }
    }
    o.values().find_map(Value::as_str).map(|v| short(v, 80)).unwrap_or_default()
}

impl Reader {
    /// One stdout line. Some(text) = a line for the live log; Some("") = a heartbeat; None = not an event.
    pub fn feed(&mut self, line: &str) -> Option<String> {
        let v: Value = serde_json::from_str(line.trim()).ok()?;
        match v.get("event").and_then(Value::as_str)? {
            "init" => {
                self.conversation = v.pointer("/conversation_id").and_then(Value::as_str).filter(|s| !s.is_empty()).map(String::from);
                self.model = v.pointer("/init/model").and_then(Value::as_str).map(String::from);
                Some(String::new())
            }
            "step_update" => {
                let s = v.get("step_update")?;
                let idx = s.get("step_index").and_then(Value::as_i64).unwrap_or(0);
                let state = s.get("state").and_then(Value::as_str).unwrap_or("");
                let kind = s.get("step_type").and_then(Value::as_str).unwrap_or("");
                if let Some(d) = s.get("text_delta").and_then(Value::as_str) {
                    self.text.entry(idx).or_default().push_str(d);
                }
                match (kind, state) {
                    ("agent_response", "DONE") => {
                        let t = self.text.remove(&idx).unwrap_or_default();
                        Some(if t.trim().is_empty() { String::new() } else { format!("💬 {}", short(&t, 240)) })
                    }
                    ("tool", "ACTIVE") => {
                        let name = s.get("tool_name").and_then(Value::as_str).unwrap_or("tool");
                        let what = s.pointer("/tool_info/parameters").map(about).unwrap_or_default();
                        Some(if what.is_empty() { format!("🔧 {name}") } else { format!("🔧 {name}: {what}") })
                    }
                    ("tool", "ERROR") => {
                        let name = s.get("tool_name").and_then(Value::as_str).unwrap_or("tool");
                        let msg = s.pointer("/tool_info/error/message").and_then(Value::as_str).unwrap_or("");
                        Some(format!("⚠️ {name}: {}", short(msg, 200)))
                    }
                    _ => Some(String::new()),
                }
            }
            "result" => {
                let r = v.get("result")?;
                let g = |k: &str| r.pointer(&format!("/usage/{k}")).and_then(Value::as_i64).unwrap_or(0);
                let status = r.get("status").and_then(Value::as_str).unwrap_or("").to_string();
                let mut resp = r.get("response").and_then(Value::as_str).unwrap_or("").to_string();
                if let Some(e) = r.get("error") {
                    let e = e.as_str().map(String::from).unwrap_or_else(|| e.to_string());
                    resp = format!("{resp}\n{e}");
                }
                if self.conversation.is_none() {
                    self.conversation = r.get("conversation_id").and_then(Value::as_str).filter(|s| !s.is_empty()).map(String::from);
                }
                self.result = Some((status.clone(), resp, g("input_tokens"), g("output_tokens") + g("thinking_tokens"), g("cache_read_tokens"), r.get("num_turns").and_then(Value::as_i64).unwrap_or(0)));
                Some(if status == "SUCCESS" { "✓ จบขั้น".into() } else { format!("⚠️ Antigravity จบด้วยสถานะ {status}") })
            }
            _ => Some(String::new()),
        }
    }
}

/// The quota from `agy -p /usage --output-format json`: used % of the 5-hour and weekly windows of the model group
/// the account works with (Claude/GPT models have their own group). Windows whose reset has passed count as empty.
pub fn quota(v: &Value, model: &str, now: i64) -> Option<crate::usage::Snapshot> {
    let groups = v.pointer("/command/data/groups")?.as_array()?;
    let third = {
        let m = model.to_lowercase();
        m.starts_with("claude") || m.starts_with("gpt")
    };
    let pick = groups
        .iter()
        .find(|g| {
            let n = g.get("name").and_then(Value::as_str).unwrap_or("").to_lowercase();
            if third { n.contains("claude") || n.contains("gpt") } else { n.contains("gemini") }
        })
        .or_else(|| groups.first())?;
    let (mut five, mut week): (Option<(f64, Option<i64>)>, Option<(f64, Option<i64>)>) = (None, None);
    for b in pick.get("buckets")?.as_array()? {
        let left = b.get("remaining_fraction").and_then(Value::as_f64)?;
        let reset = b.get("reset_time").and_then(Value::as_str).and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok()).map(|t| t.timestamp());
        let (used, reset) = match reset {
            Some(r) if r <= now => (0.0, None),
            r => (((1.0 - left) * 100.0).clamp(0.0, 100.0), r),
        };
        let win = b.get("window").and_then(Value::as_str).unwrap_or("").to_lowercase();
        let slot = if win.contains("week") { &mut week } else { &mut five };
        // two buckets of one window: the fuller one decides
        if slot.is_none_or(|(u, _)| used > u) {
            *slot = Some((used, reset));
        }
    }
    let week = week.or(five)?;
    let five = five.unwrap_or((0.0, None));
    Some(crate::usage::Snapshot { ts: now, five_pct: five.0, five_reset: five.1, week_pct: week.0, week_reset: week.1, source: "antigravity".into() })
}

/// Run `agy -p /usage` for one account (its environment) and read its quota. Needs no model call, so it costs nothing.
pub async fn fetch_quota(env: &[(String, String)], model: &str) -> (Option<crate::usage::Snapshot>, Option<String>) {
    let Some(bin) = crate::providers::resolve_bin("agy") else { return (None, Some("ไม่พบ agy ในเครื่อง".into())) };
    let mut c = tokio::process::Command::new(bin);
    c.args(["-p", "/usage", "--output-format", "json"])
        .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .env("AGY_CLI_DISABLE_AUTO_UPDATE", "true")
        .current_dir(std::env::temp_dir())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    c.creation_flags(0x0800_0000);
    let out = match tokio::time::timeout(std::time::Duration::from_secs(45), c.output()).await {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => return (None, Some(format!("เรียก agy ไม่ได้: {e}"))),
        Err(_) => return (None, Some("agy ตอบช้าเกิน 45 วินาที".into())),
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let err = String::from_utf8_lossy(&out.stderr);
    if format!("{text} {err}").to_lowercase().contains("sign in") {
        return (None, Some("บัญชีนี้ยังไม่ได้ล็อกอิน Antigravity — กด “ล็อกอิน” ของบัญชีนี้".into()));
    }
    match serde_json::from_str::<Value>(text.trim()).ok().and_then(|v| quota(&v, model, crate::db::now())) {
        Some(s) => (Some(s), None),
        None => (None, Some(format!("อ่านโควต้าของ Antigravity ไม่ได้: {}", short(&format!("{text} {err}"), 160)))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_reads_as_lines_and_a_result() {
        let mut r = Reader::default();
        let lines = [
            r#"{"event":"init","conversation_id":"c1","init":{"model":"gemini-3.8-flash-low","cwd":"x"}}"#,
            r#"{"event":"step_update","step_update":{"step_index":1,"state":"ACTIVE","step_type":"agent_response","text_delta":"Hello "}}"#,
            r#"{"event":"step_update","step_update":{"step_index":1,"state":"DONE","step_type":"agent_response","text_delta":"world"}}"#,
            r#"{"event":"step_update","step_update":{"step_index":2,"state":"ACTIVE","step_type":"tool","tool_name":"read_file","tool_info":{"name":"read_file","parameters":{"Path":"C:\\a.txt"}}}}"#,
            r#"{"event":"step_update","step_update":{"step_index":2,"state":"ERROR","step_type":"tool","tool_name":"read_file","tool_info":{"error":{"type":"TOOL_ERROR","message":"file does not exist"}}}}"#,
            r#"{"event":"result","result":{"conversation_id":"c1","status":"SUCCESS","response":"ok","num_turns":1,"usage":{"input_tokens":100,"output_tokens":5,"thinking_tokens":3,"cache_read_tokens":40}}}"#,
        ];
        let out: Vec<String> = lines.iter().filter_map(|l| r.feed(l)).collect();
        assert_eq!(out[1], "");
        assert_eq!(out[2], "💬 Hello world");
        assert_eq!(out[3], "🔧 read_file: C:\\a.txt");
        assert!(out[4].starts_with("⚠️ read_file: file does not exist"));
        assert_eq!(r.conversation.as_deref(), Some("c1"));
        assert_eq!(r.model.as_deref(), Some("gemini-3.8-flash-low"));
        assert_eq!(r.result, Some(("SUCCESS".into(), "ok".into(), 100, 8, 40, 1)));
        assert!(r.feed("not json").is_none());
    }

    #[test]
    fn quota_follows_the_models_group() {
        let v: Value = serde_json::from_str(r#"{"command":{"name":"usage","data":{"groups":[
            {"name":"Gemini Models","buckets":[{"id":"gemini-weekly","window":"weekly","remaining_fraction":0.75,"reset_time":"2099-12-02T13:08:13Z"},
                                               {"id":"gemini-5h","window":"five_hour","remaining_fraction":0.5,"reset_time":"2099-11-25T18:00:00Z"}]},
            {"name":"Claude and GPT models","buckets":[{"id":"3p-weekly","window":"weekly","remaining_fraction":0.1,"reset_time":"2099-12-02T13:13:05Z"}]}]}}}"#).unwrap();
        let g = quota(&v, "gemini-4-hydrogen-high", 0).unwrap();
        assert_eq!((g.week_pct.round(), g.five_pct.round()), (25.0, 50.0));
        assert!(g.week_reset.is_some());
        let c = quota(&v, "claude-opus-4-6-thinking", 0).unwrap();
        assert_eq!((c.week_pct.round(), c.five_pct), (90.0, 0.0));
        // a window that already reset is empty
        let later = quota(&v, "", 4_200_000_000).unwrap();
        assert_eq!(later.week_pct, 0.0);
        assert!(quota(&serde_json::json!({}), "", 0).is_none());
    }
}

#[cfg(test)]
mod live {
    /// The machine's own agy sign-in: `cargo test --lib agy::live -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn reads_the_real_quota() {
        let (s, note) = super::fetch_quota(&[], "").await;
        println!("{s:?} {note:?}");
        let s = s.expect("quota");
        assert!(s.week_pct >= 0.0 && s.week_reset.is_some());
        let models = crate::acct_models::parse_agy_models(&String::from_utf8_lossy(&std::process::Command::new(crate::providers::resolve_bin("agy").unwrap()).arg("models").output().unwrap().stdout));
        println!("{models:?}");
        assert!(models.iter().any(|m| m.starts_with("gemini")));
    }
}
