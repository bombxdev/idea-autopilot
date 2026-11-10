//! Chatting inside the app with the other AIs (Gemini CLI, OpenCode, Mistral Vibe) — "สั่ง AI แก้ / ทำต่อ".
//!
//! Like codex_chat.rs, but these CLIs have no common way to continue a conversation, so every message is one headless run
//! in the idea's folder (the same command line the bot uses, see providers::build) and the conversation so far is written
//! into the prompt from `logs/agent-chat.md`. Their output lines become the live feed, a message sent while a run is
//! going waits for its turn, and the session looks like any other open session (the bot leaves the idea alone, the
//! Sessions page lists it, it can be closed).
use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, LazyLock};
use tauri::{AppHandle, Manager};

use crate::remote::ChatAi;
use crate::state::AppState;
use crate::{db, workspace};

const TURN_TIMEOUT_SECS: u64 = 30 * 60;
/// how much of the earlier conversation goes back into the prompt
const HISTORY_CHARS: usize = 7000;
const REPLY_KEEP: usize = 2500;

/// The AIs this chat can drive.
pub const KINDS: [&str; 3] = ["gemini", "opencode", "mistral"];

#[derive(Default)]
struct Chat {
    agent: String,
    queue: VecDeque<(String, ChatAi)>,
    running: bool,
    child: Option<Arc<Mutex<Child>>>,
    stopped: bool,
}

static CHATS: LazyLock<Mutex<HashMap<i64, Chat>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Is a chat of this kind open for this idea?
pub fn active(app: &AppHandle, id: i64) -> bool {
    CHATS.lock().contains_key(&id) && app.state::<AppState>().rt.lock().sessions.contains_key(&id)
}

pub fn agent_of(id: i64) -> Option<String> {
    CHATS.lock().get(&id).map(|c| c.agent.clone())
}

fn label(agent: &str) -> &'static str {
    match agent {
        "gemini" => "Gemini",
        "opencode" => "OpenCode",
        "mistral" => "Mistral Vibe",
        _ => "AI",
    }
}

fn short(s: &str, n: usize) -> String {
    let flat = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() > n {
        format!("{}…", flat.chars().take(n).collect::<String>())
    } else {
        flat
    }
}

fn feed(app: &AppHandle, id: i64, text: String, status: Option<&str>) {
    crate::remote::push_feed_pub(app, id, text, status);
}

fn history_path(dir: &std::path::Path) -> std::path::PathBuf {
    dir.join("logs").join("agent-chat.md")
}

/// The tail of the conversation so far (whole entries only, newest last).
fn history(dir: &std::path::Path) -> String {
    let raw = std::fs::read_to_string(history_path(dir)).unwrap_or_default();
    if raw.chars().count() <= HISTORY_CHARS {
        return raw;
    }
    let skip = raw.chars().count() - HISTORY_CHARS;
    let cut: String = raw.chars().skip(skip).collect();
    // start at the next entry boundary so a half entry is not shown
    match cut.find("\n## ") {
        Some(i) => cut[i + 1..].to_string(),
        None => cut,
    }
}

fn remember(dir: &std::path::Path, who: &str, text: &str) {
    let p = history_path(dir);
    let _ = std::fs::create_dir_all(p.parent().unwrap_or(dir));
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
        let _ = writeln!(f, "\n## {who}\n{}", text.trim());
    }
}

/// The prompt of one turn: where to read the rules, what was said before, what the owner says now.
fn build_prompt(id: i64, title: &str, past: &str, text: &str) -> String {
    let mut p = format!("Idea #{id}: {title}\nRead SESSION.md in this folder first (how this chat works), then AGENTS.md (rules and context of this idea).\n");
    if !past.trim().is_empty() {
        p.push_str("\nThe conversation so far (oldest first):\n");
        p.push_str(past.trim());
        p.push('\n');
    }
    p.push_str("\nNow do what the owner asks. Answer briefly in the owner's language, then say what you changed.\n\nOwner: ");
    p.push_str(text);
    p.push('\n');
    p
}

/// Send a message to this AI for the idea (opens the chat the first time).
pub fn ask(app: &AppHandle, id: i64, agent: &str, text: &str, ai: &ChatAi) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("พิมพ์สิ่งที่อยากให้ AI ทำก่อน".into());
    }
    if !KINDS.contains(&agent) {
        return Err(format!("ยังไม่รองรับแชตกับ {agent} ในแอป"));
    }
    if !crate::providers::installed(agent) {
        return Err(format!("ไม่พบ {} ในเครื่องนี้ — กด “ติดตั้ง CLI” ที่ตั้งค่า → บัญชี AI", label(agent)));
    }
    let st = app.state::<AppState>();
    let idea = db::get_idea(&st.db.lock(), id).map_err(|e| e.to_string())?;
    if idea.status == "archived" || idea.status == "deleted" {
        return Err("ไอเดียนี้ถูกเก็บ (archive) แล้ว — กู้คืนก่อนถึงจะคุยต่อได้".into());
    }
    let dir = workspace::idea_dir(&st.ws, &idea.slug);
    if !dir.is_dir() {
        return Err("ไม่พบโฟลเดอร์ของไอเดีย".into());
    }
    let open = st.rt.lock().sessions.contains_key(&id);
    if open {
        match CHATS.lock().get(&id) {
            None => return Err("session นี้เปิดกับ AI ตัวอื่นอยู่ — กด “ปิด session” ก่อน แล้วค่อยส่งใหม่".into()),
            Some(c) if c.agent != agent => {
                return Err(format!("session นี้เปิดกับ {} อยู่ — กด “ปิด session” ก่อน แล้วค่อยส่งให้ {}", label(&c.agent), label(agent)));
            }
            _ => {}
        }
    } else {
        let lang = crate::lang::name(&crate::scheduler::idea_lang(&st, &idea));
        let _ = std::fs::write(dir.join("SESSION.md"), crate::remote::session_brief(id, &idea.title, &dir, lang));
        // the owner takes over: stop the bot if it is working on this idea right now
        if st.rt.lock().current.as_ref().map(|c| c.idea_id) == Some(id) {
            if let Some(c) = st.rt.lock().cancel.clone() {
                c.notify_one();
            }
        }
        st.rt.lock().sessions.insert(
            id,
            crate::remote::RemoteInfo {
                idea_id: id,
                name: format!("Idea#{id} · {}", label(agent)),
                status: "idle".into(),
                started: db::now(),
                sid: agent.to_string(),
                ai: crate::remote::ai_label(&st.db.lock(), &ai.account, label(agent), agent, None, &ai.model),
                ..Default::default()
            },
        );
        db::log_event(&st.db.lock(), "remote", Some(id), &format!("เปิดแชตกับ {}", label(agent)));
    }
    feed(app, id, format!("🙋 คุณ: {}", short(text, 400)), None);
    let start_worker = {
        let mut m = CHATS.lock();
        let c = m.entry(id).or_default();
        c.agent = agent.to_string();
        c.stopped = false;
        c.queue.push_back((text.to_string(), ai.clone()));
        let go = !c.running;
        c.running = true;
        go
    };
    if start_worker {
        let app2 = app.clone();
        std::thread::spawn(move || worker(&app2, id));
    } else {
        feed(app, id, format!("📨 รับคำสั่งแล้ว จะส่งให้ {} ต่อจากรอบที่กำลังทำ", label(agent)), None);
    }
    Ok(())
}

fn worker(app: &AppHandle, id: i64) {
    loop {
        let next = {
            let mut m = CHATS.lock();
            let Some(c) = m.get_mut(&id) else { return };
            match c.queue.pop_front() {
                Some(x) if !c.stopped => Some((x, c.agent.clone())),
                _ => {
                    c.running = false;
                    None
                }
            }
        };
        let Some(((text, ai), agent)) = next else { break };
        if let Err(e) = turn(app, id, &agent, &text, &ai) {
            feed(app, id, format!("⚠️ {e}"), Some("idle"));
        }
    }
    feed(app, id, String::new(), Some("idle"));
}

fn turn(app: &AppHandle, id: i64, agent: &str, text: &str, ai: &ChatAi) -> Result<(), String> {
    let st = app.state::<AppState>();
    let idea = db::get_idea(&st.db.lock(), id).map_err(|e| e.to_string())?;
    let dir = workspace::idea_dir(&st.ws, &idea.slug);
    let want = ai.resolved(&st.db.lock(), id);
    // an account of another kind (the idea's own setting, say) is not this agent's login: use the agent's default
    let account = if want.account.is_empty() { None } else { crate::accounts::find(&st.db.lock(), &want.account).filter(|a| a.kind == agent) };
    let account_id = account.as_ref().map(|a| a.id.clone()).unwrap_or_default();
    let model = match &account {
        Some(a) => {
            let checks = crate::model_check::all(&st.db.lock());
            crate::acct_models::fit(a, &st.root, &want.model, &checks).0
        }
        None => String::new(),
    };

    let prompt = build_prompt(id, &idea.title, &history(&dir), text);
    // agents that take a file name read the prompt from logs/prompt.md; the others get it on stdin
    let _ = std::fs::create_dir_all(dir.join("logs"));
    let prompt_file = dir.join("logs").join("prompt.md");
    std::fs::write(&prompt_file, &prompt).map_err(|e| e.to_string())?;
    let inv = crate::providers::build(agent, &model, "", "", "", &prompt_file)?;
    remember(&dir, "Owner", text);

    let mut cmd = Command::new(&inv.program);
    cmd.args(&inv.args).current_dir(&dir).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(a) = &account {
        for (k, v) in crate::accounts::env_with_secret(a, &st.root) {
            cmd.env(k, v);
        }
    }
    cmd.env("IDEA_AUTOPILOT", "1");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    feed(app, id, format!("⏳ ส่งให้ {}…", label(agent)), Some("working"));
    let mut child = cmd.spawn().map_err(|e| format!("เรียก {} ไม่ได้: {e}", label(agent)))?;
    if let Some(mut si) = child.stdin.take() {
        if inv.stdin_prompt {
            let _ = si.write_all(prompt.as_bytes());
        }
    }
    let stdout = child.stdout.take().ok_or("อ่านผลไม่ได้")?;
    let stderr = child.stderr.take();
    let shared = Arc::new(Mutex::new(child));
    if let Some(c) = CHATS.lock().get_mut(&id) {
        c.child = Some(shared.clone());
    }
    let watch = shared.clone();
    let started = std::time::Instant::now();
    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let done2 = done.clone();
    std::thread::spawn(move || {
        while !done2.load(std::sync::atomic::Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_secs(5));
            if started.elapsed().as_secs() > TURN_TIMEOUT_SECS {
                let _ = watch.lock().kill();
                break;
            }
        }
    });
    let err_tail: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let et = err_tail.clone();
    let err_thread = std::thread::spawn(move || {
        if let Some(e) = stderr {
            for l in BufReader::new(e).lines().map_while(Result::ok) {
                let mut g = et.lock();
                g.push(crate::runner::strip_ansi(&l));
                let n = g.len();
                if n > 12 {
                    g.drain(..n - 12);
                }
            }
        }
    });

    let mut reply = String::new();
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        let clean = crate::runner::strip_ansi(&line);
        let t = clean.trim();
        if t.is_empty() {
            continue;
        }
        if reply.chars().count() < REPLY_KEEP * 2 {
            reply.push_str(t);
            reply.push('\n');
        }
        feed(app, id, format!("💬 {}", short(t, 700)), None);
    }
    let status = shared.lock().wait().ok();
    done.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = err_thread.join();
    if let Some(c) = CHATS.lock().get_mut(&id) {
        c.child = None;
    }
    let ok = status.map(|s| s.success()).unwrap_or(false);
    // keep the end of the reply: that is where the agent says what it did
    let n = reply.chars().count();
    let kept: String = if n > REPLY_KEEP { reply.chars().skip(n - REPLY_KEEP).collect() } else { reply.clone() };
    if !kept.trim().is_empty() {
        remember(&dir, label(agent), &kept);
    }
    if !ok {
        let tail = err_tail.lock().join("\n");
        let why = format!("{reply}\n{tail}");
        if !account_id.is_empty() && crate::runner::looks_like_limit(&why) {
            let snap = st.rt.lock().acct_snaps.get(&account_id).cloned();
            let now = db::now();
            let acct = crate::accounts::find(&st.db.lock(), &account_id);
            let until = crate::vendor_limits::limit_until(acct.as_ref(), &why, snap.as_ref(), now);
            let reason = crate::limits::why(&why);
            crate::limits::set(&st.db.lock(), &account_id, until, &reason);
            return Err(format!("บัญชีนี้ชนลิมิต: {reason} — รอถึง {}", crate::limits::when_text(until, now)));
        }
        let code = status.and_then(|s| s.code()).map(|c| c.to_string()).unwrap_or_else(|| "?".into());
        return Err(format!("{} จบก่อนเวลา (exit {code}): {}", label(agent), short(if tail.trim().is_empty() { &reply } else { &tail }, 400)));
    }
    feed(app, id, "✅ จบรอบนี้".into(), None);
    Ok(())
}

/// Close the chat: stop a running turn and forget the queue (the conversation stays in logs/agent-chat.md and is
/// continued when the owner writes again).
pub fn stop(id: i64) {
    let child = {
        let mut m = CHATS.lock();
        let Some(c) = m.get_mut(&id) else { return };
        c.queue.clear();
        c.stopped = true;
        c.child.take()
    };
    if let Some(ch) = child {
        let _ = ch.lock().kill();
    }
    CHATS.lock().remove(&id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prompt_carries_the_conversation_and_the_new_message() {
        let p = build_prompt(7, "Demo", "## Owner\nhello\n\n## Gemini\nhi", "make it blue");
        assert!(p.contains("SESSION.md") && p.contains("## Gemini") && p.trim_end().ends_with("Owner: make it blue"));
        assert!(!build_prompt(7, "Demo", "", "x").contains("conversation so far"));
    }

    #[test]
    fn old_history_is_cut_at_an_entry_boundary() {
        let dir = std::env::temp_dir().join(format!("ia-agentchat-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for i in 0..40 {
            remember(&dir, "Owner", &format!("{i} {}", "x".repeat(400)));
        }
        let h = history(&dir);
        assert!(h.chars().count() <= HISTORY_CHARS);
        assert!(h.starts_with("## "), "starts at an entry: {}", &h[..20.min(h.len())]);
        assert!(h.contains("39 xxx"));
    }
}
