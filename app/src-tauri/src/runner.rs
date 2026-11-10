//! Runs one stage of one idea through a headless AI agent (see providers.rs), streaming its log.
//! Claude emits structured stream-json (live quota, tokens, narrated steps); every other agent is
//! read as plain text lines, and the run counts as OK when the process exits with code 0.
use anyhow::{Context, Result};
use serde_json::Value;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::Notify;

#[derive(Debug, Default, Clone)]
pub struct Outcome {
    pub status: String, // ok | error | timeout | cancelled | rate_limited
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read: i64,
    pub cache_write: i64,
    pub cost_usd: f64,
    pub num_turns: i64,
    pub session_id: Option<String>,
    /// concrete model id the alias resolved to, e.g. claude-sonnet-5-5
    pub model_id: Option<String>,
    pub error: Option<String>,
}

pub struct RunSpec<'a> {
    pub cwd: &'a Path,
    pub prompt: String,
    /// providers::chosen — claude | codex | gemini | opencode | custom
    pub provider: String,
    /// concrete model name for this run ("" = the agent's own default)
    pub model: String,
    pub effort: String,
    pub tools: String,
    pub custom_cmd: String,
    pub timeout_mins: u64,
    pub log_path: &'a Path,
    /// language of the live feed (lang.rs code)
    pub lang: &'a str,
    /// extra environment (an account's own login folder)
    pub env: Vec<(String, String)>,
}

/// One readable line for the live log, if this event is worth showing.
pub fn describe(v: &Value, lang: &str) -> Option<String> {
    match v.get("type")?.as_str()? {
        "assistant" => {
            let out = crate::narrate::assistant(v.pointer("/message/content")?.as_array()?, lang);
            if out.is_empty() { None } else { Some(out.join("\n")) }
        }
        "result" => Some(crate::narrate::tr(lang, "stage_done", &v.get("num_turns").and_then(Value::as_i64).unwrap_or(0).to_string())),
        _ => None,
    }
}

pub(crate) fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\u{1b}' {
            if it.peek() == Some(&'[') {
                it.next();
                for n in it.by_ref() {
                    if n.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else if c != '\r' {
            out.push(c);
        }
    }
    out
}

fn short(s: &str, n: usize) -> String {
    let t: String = s.chars().take(n).collect();
    if s.chars().count() > n { format!("{t}…") } else { t }
}

#[cfg(windows)]
pub fn kill_tree(pid: u32) {
    let mut c = std::process::Command::new("taskkill");
    c.args(["/T", "/F", "/PID", &pid.to_string()]);
    use std::os::windows::process::CommandExt;
    c.creation_flags(0x0800_0000);
    let _ = c.status();
}

#[cfg(not(windows))]
pub fn kill_tree(pid: u32) {
    let _ = std::process::Command::new("kill").args(["-9", &pid.to_string()]).status();
}

/// Live quota from `rate_limit_event` lines (utilization comes as a 0–1 fraction).
fn quota_from_event(v: &Value) -> Option<crate::usage::Snapshot> {
    let w = v.pointer("/rate_limit_info/unifiedWindows")?;
    let five = w.get("five_hour")?;
    let week = w.get("seven_day")?;
    Some(crate::usage::Snapshot {
        ts: crate::db::now(),
        five_pct: five.get("utilization")?.as_f64()? * 100.0,
        five_reset: five.get("resetsAt").and_then(Value::as_i64),
        week_pct: week.get("utilization")?.as_f64()? * 100.0,
        week_reset: week.get("resetsAt").and_then(Value::as_i64),
        source: "stream".into(),
    })
}

/// Does this text from an AI say its usage limit or quota is used up?
pub fn looks_like_limit(text: &str) -> bool {
    let l = text.to_lowercase();
    LIMIT_WORDS.iter().any(|w| l.contains(w))
}

const LIMIT_WORDS: [&str; 11] = [
    "usage limit", "rate limit", "limit reached", "hit your limit", "quota", "resource_exhausted", "429", "too many requests", "insufficient_quota", "out of credits", "refill",
];

pub async fn run(
    spec: RunSpec<'_>,
    cancel: Arc<Notify>,
    on_line: impl Fn(String),
    on_quota: impl Fn(crate::usage::Snapshot),
) -> Result<Outcome> {
    // the prompt is kept as a file too: agents without stdin support are told to read it
    let prompt_file = spec.cwd.join("logs").join("prompt.md");
    let _ = std::fs::create_dir_all(spec.cwd.join("logs"));
    let _ = std::fs::write(&prompt_file, &spec.prompt);
    let inv = crate::providers::build(&spec.provider, &spec.model, &spec.effort, &spec.tools, &spec.custom_cmd, &prompt_file)
        .map_err(anyhow::Error::msg)?;

    let mut cmd = if let Some(line) = &inv.shell_line {
        #[cfg(windows)]
        {
            let mut c = Command::new("cmd");
            c.as_std_mut().arg("/C");
            std::os::windows::process::CommandExt::raw_arg(c.as_std_mut(), line);
            c
        }
        #[cfg(not(windows))]
        {
            let mut c = Command::new("sh");
            c.args(["-c", line]);
            c
        }
    } else {
        let mut c = Command::new(&inv.program);
        c.args(&inv.args);
        c
    };
    cmd.envs(spec.env.iter().map(|(k, v)| (k.as_str(), v.as_str())));
    cmd.current_dir(spec.cwd)
        .env("IDEA_AUTOPILOT", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW

    let mut child = cmd.spawn().with_context(|| format!("เรียก {} ไม่ได้ (ติดตั้งแล้วอยู่ใน PATH ไหม?)", spec.provider))?;
    let pid = child.id().unwrap_or(0);
    {
        let mut stdin = child.stdin.take().context("stdin")?;
        if inv.stdin_prompt {
            stdin.write_all(spec.prompt.as_bytes()).await?;
        }
        stdin.shutdown().await?;
    }
    let stdout = child.stdout.take().context("stdout")?;
    let stderr = child.stderr.take().context("stderr")?;
    let mut log = tokio::fs::File::create(spec.log_path).await?;
    let mut lines = BufReader::new(stdout).lines();
    let mut out = Outcome { status: "error".into(), ..Default::default() };
    let mut result_text = String::new();
    let stream = inv.claude_stream;
    let codex_json = spec.provider == "codex";
    // other agents narrate progress on stderr (Codex) or stdout (Gemini, OpenCode): show both
    let tail = Arc::new(parking_lot::Mutex::new(String::new()));
    let (etx, mut erx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let err_task = tokio::spawn(async move {
        let mut r = BufReader::new(stderr).lines();
        while let Ok(Some(l)) = r.next_line().await {
            let _ = etx.send(l);
        }
    });
    let mut shown = 0usize;
    let mut feed = |text: &str, tail: &parking_lot::Mutex<String>| {
        let clean = strip_ansi(text);
        let t = clean.trim();
        if t.is_empty() {
            return;
        }
        {
            let mut g = tail.lock();
            g.push_str(t);
            g.push('\n');
            if g.len() > 6000 {
                let cut = g.len() - 4000;
                let cut = (cut..g.len()).find(|i| g.is_char_boundary(*i)).unwrap_or(0);
                g.drain(..cut);
            }
        }
        if !stream && shown < 4000 {
            shown += 1;
            on_line(format!("💬 {}", short(t, 240)));
        }
    };

    let deadline = tokio::time::sleep(std::time::Duration::from_secs(spec.timeout_mins * 60));
    tokio::pin!(deadline);
    let mut ended_by: Option<&str> = None;
    let mut stdout_open = true;
    let mut stderr_open = true;
    loop {
        tokio::select! {
            line = lines.next_line(), if stdout_open => {
                match line {
                    Ok(Some(l)) => {
                        let _ = log.write_all(l.as_bytes()).await;
                        let _ = log.write_all(b"\n").await;
                        if codex_json {
                            // `codex exec --json`: events become plain-language lines (same words as the Codex chat)
                            if let Ok(v) = serde_json::from_str::<Value>(&l) {
                                // the conversation id lets the owner carry on in Codex itself or in the app's chat
                                if v.get("type").and_then(Value::as_str) == Some("thread.started") {
                                    out.session_id = v.get("thread_id").and_then(Value::as_str).map(String::from);
                                }
                                if v.get("type").and_then(Value::as_str) == Some("turn.completed") {
                                    let g = |k: &str| v.pointer(&format!("/usage/{k}")).and_then(Value::as_i64).unwrap_or(0);
                                    out.input_tokens += g("input_tokens");
                                    out.cache_read += g("cached_input_tokens");
                                    out.output_tokens += g("output_tokens");
                                    out.num_turns += 1;
                                }
                                match crate::codex_chat::feed_line(&v) {
                                    Some(t) => {
                                        if t.starts_with("⚠️") {
                                            let mut g = tail.lock();
                                            g.push_str(&t);
                                            g.push('\n');
                                        }
                                        on_line(t);
                                    }
                                    None => on_line(String::new()), // heartbeat
                                }
                            }
                            continue;
                        }
                        if !stream {
                            feed(&l, &tail);
                            continue;
                        }
                        if let Ok(v) = serde_json::from_str::<Value>(&l) {
                            // an empty line is a heartbeat: the agent is alive even when it says nothing
                            match describe(&v, spec.lang) { Some(d) => on_line(d), None => on_line(String::new()) }
                            if let Some(q) = quota_from_event(&v) { on_quota(q); }
                            if v.get("type").and_then(Value::as_str) == Some("system") && v.get("subtype").and_then(Value::as_str) == Some("init") {
                                out.model_id = v.get("model").and_then(Value::as_str).map(String::from);
                            }
                            if v.get("type").and_then(Value::as_str) == Some("result") {
                                let u = v.get("usage").cloned().unwrap_or(Value::Null);
                                let g = |k: &str| u.get(k).and_then(Value::as_i64).unwrap_or(0);
                                out.input_tokens = g("input_tokens");
                                out.output_tokens = g("output_tokens");
                                out.cache_read = g("cache_read_input_tokens");
                                out.cache_write = g("cache_creation_input_tokens");
                                out.cost_usd = v.get("total_cost_usd").and_then(Value::as_f64).unwrap_or(0.0);
                                out.num_turns = v.get("num_turns").and_then(Value::as_i64).unwrap_or(0);
                                out.session_id = v.get("session_id").and_then(Value::as_str).map(String::from);
                                let is_err = v.get("is_error").and_then(Value::as_bool).unwrap_or(false);
                                result_text = v.get("result").and_then(Value::as_str).unwrap_or("").to_string();
                                out.status = if is_err { "error".into() } else { "ok".into() };
                            }
                        }
                    }
                    _ => stdout_open = false,
                }
            }
            l = erx.recv(), if stderr_open => {
                match l {
                    Some(l) => {
                        let _ = log.write_all(format!("[stderr] {l}\n").as_bytes()).await;
                        if codex_json {
                            // with --json the story is on stdout; stderr is hook/skill noise: kept for error reports only
                            let mut g = tail.lock();
                            g.push_str(strip_ansi(&l).trim());
                            g.push('\n');
                            if g.len() > 6000 {
                                let cut = g.len() - 4000;
                                let cut = (cut..g.len()).find(|i| g.is_char_boundary(*i)).unwrap_or(0);
                                g.drain(..cut);
                            }
                        } else {
                            feed(&l, &tail);
                        }
                    }
                    None => stderr_open = false,
                }
            }
            _ = &mut deadline => { ended_by = Some("timeout"); break; }
            _ = cancel.notified() => { ended_by = Some("cancelled"); break; }
        }
        // the agent is done once both of its output streams have closed
        if !stdout_open && !stderr_open {
            break;
        }
    }
    if let Some(why) = ended_by {
        kill_tree(pid);
        let _ = child.kill().await;
        out.status = why.into();
        on_line(if why == "timeout" { "⏱ หมดเวลาของขั้นนี้ — ทำต่อรอบหน้า".into() } else { "■ หยุดแล้ว".into() });
    }
    let code = child.wait().await.ok().and_then(|s| s.code());
    let _ = err_task.await;
    while let Ok(l) = erx.try_recv() {
        feed(&l, &tail);
    }
    let tail_text = tail.lock().clone();
    if !stream && ended_by.is_none() {
        out.status = if code == Some(0) { "ok".into() } else { "error".into() };
        out.model_id = Some(if spec.model.is_empty() { format!("{}-default", spec.provider) } else { spec.model.clone() });
        if out.status == "ok" {
            on_line("✓ จบขั้น".into());
        }
    }
    let lower = format!("{result_text} {tail_text}").to_lowercase();
    if out.status == "error" {
        // Claude reports limits in plain words; for other agents also match their error codes
        let words = if stream { &LIMIT_WORDS[..4] } else { &LIMIT_WORDS[..] };
        if words.iter().any(|w| lower.contains(w)) {
            out.status = "rate_limited".into();
        }
        let msg = if !result_text.is_empty() { result_text.clone() } else { tail_text.clone() };
        out.error = Some(format!("exit {:?}: {}", code, msg.chars().rev().take(600).collect::<Vec<_>>().into_iter().rev().collect::<String>()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("ia-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    async fn run_custom(name: &str, cmd: &str) -> (Outcome, Vec<String>) {
        let dir = tmp(name);
        let lines = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let sink = lines.clone();
        let out = run(
            RunSpec {
                cwd: &dir,
                prompt: "hi".into(),
                provider: "custom".into(),
                model: String::new(),
                effort: "low".into(),
                tools: String::new(),
                custom_cmd: cmd.into(),
                timeout_mins: 1,
                log_path: &dir.join("run.log"),
                lang: "en",
                env: vec![],
            },
            Arc::new(Notify::new()),
            move |l| sink.lock().push(l),
            |_| {},
        )
        .await
        .unwrap();
        let got = lines.lock().clone();
        let _ = std::fs::remove_dir_all(&dir);
        (out, got)
    }

    #[tokio::test]
    async fn any_command_line_agent_can_do_a_run() {
        let (out, lines) = run_custom("ok", "echo hello from my agent").await;
        assert_eq!(out.status, "ok", "{:?}", out.error);
        assert!(lines.iter().any(|l| l.contains("hello from my agent")), "{lines:?}");
    }

    #[tokio::test]
    async fn failing_agent_is_an_error_and_limit_text_is_recognised() {
        let (out, _) = run_custom("fail", "exit 3").await;
        assert_eq!(out.status, "error");
        let (out, _) = run_custom("limit", "echo 429 Too Many Requests 1>&2 & exit 1").await;
        assert_eq!(out.status, "rate_limited", "{:?}", out.error);
    }

    /// Needs a logged-in Claude Code and spends a few tokens: `cargo test claude_real -- --ignored`
    #[tokio::test]
    #[ignore]
    async fn claude_real_still_streams_and_reports() {
        let dir = tmp("claude");
        let lines = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let sink = lines.clone();
        let out = run(
            RunSpec {
                cwd: &dir,
                prompt: "Reply with the single word OK. Do not use any tools.".into(),
                provider: "claude".into(),
                model: "haiku".into(),
                effort: "low".into(),
                tools: "Read".into(),
                custom_cmd: String::new(),
                timeout_mins: 2,
                log_path: &dir.join("run.log"),
                lang: "en",
                env: vec![],
            },
            Arc::new(Notify::new()),
            move |l| sink.lock().push(l),
            |_| {},
        )
        .await
        .unwrap();
        println!("{out:?}
{:?}", lines.lock());
        assert_eq!(out.status, "ok", "{:?}", out.error);
        assert!(out.output_tokens > 0 && out.model_id.is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn missing_agent_explains_itself() {
        let dir = tmp("none");
        let err = run(
            RunSpec {
                cwd: &dir,
                prompt: "hi".into(),
                provider: "none".into(),
                model: String::new(),
                effort: "low".into(),
                tools: String::new(),
                custom_cmd: String::new(),
                timeout_mins: 1,
                log_path: &dir.join("run.log"),
                lang: "en",
                env: vec![],
            },
            Arc::new(Notify::new()),
            |_| {},
            |_| {},
        )
        .await
        .unwrap_err();
        assert!(format!("{err:#}").contains("ยังไม่มี AI"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
