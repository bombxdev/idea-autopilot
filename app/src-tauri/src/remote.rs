//! Remote Control, one named session per idea: `claude --remote-control "Idea#4 : <title>"` runs
//! inside a pseudo-console the app owns (ConPTY), from the app's root folder, so it shows up by
//! name in the Claude mobile app / claude.ai/code *and* the app can type instructions into it
//! (`ask`). One session per idea: a new instruction goes to the open session; a closed one is
//! reopened with `--resume` so the conversation continues. Progress is read from the session's
//! transcript and turned into plain-Thai lines (narrate.rs) — no extra tokens.
//! A SessionStart hook hands the session its idea brief; while a session is open the bot leaves
//! that idea alone.
use parking_lot::Mutex;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

use crate::db;
use crate::state::AppState;
use crate::workspace;

/// How long a manual `ia hold` lasts without new activity.
pub const HOLD_SECS: i64 = 3 * 3600;
const FEED_MAX: usize = 40;
const TURN_DONE: &str = "✅ ทำรอบนี้เสร็จ — พิมพ์สั่งต่อได้";

#[derive(Debug, Clone, Serialize)]
pub struct FeedItem {
    pub ts: i64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct RemoteInfo {
    pub idea_id: i64,
    pub name: String,
    /// starting | working | idle | error
    pub status: String,
    pub started: i64,
    pub pid: u32,
    pub note: String,
    pub sid: String,
    /// what the AI is doing, newest last, in plain Thai
    pub feed: Vec<FeedItem>,
}

/// The live pseudo-console of each open session (not serializable, so kept apart from Runtime).
struct Pty {
    writer: Box<dyn Write + Send>,
    _master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    ready: Arc<AtomicBool>,
    queue: Vec<String>,
}

/// The last lines each session's console showed (for explaining a session that never got ready).
static CONSOLE: LazyLock<Mutex<HashMap<i64, String>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static PTYS: LazyLock<Mutex<HashMap<i64, Pty>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn session_name(id: i64, title: &str) -> String {
    format!("Idea#{id} : {}", title.chars().take(60).collect::<String>())
}

pub fn ia_path() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|p| p.join("ia.exe")))
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "ia".into())
}

/// The idea brief the session gets at start (printed by the project's SessionStart hook).
/// English on purpose: it goes through a PowerShell pipe.
fn session_md(id: i64, title: &str, dir: &std::path::Path) -> String {
    format!(
        r##"# Idea Autopilot — remote session for idea #{id}: {title}

The owner works with you on this idea live: messages come from the Idea Autopilot app (desktop or phone)
or from the Claude app through Remote Control. This is not the overnight bot. Reply in Thai.
- Work ONLY inside the idea folder: {dir}
  Read IDEA.md, REPORT.md, PROGRESS.md, DIRECTIVES.md, ANSWERS.md there first (only once per session).
- Safety rules from the workspace CLAUDE.md apply: stay in the idea folder, free tools only, never publish, deploy, sign up or pay.
- The owner follows your progress as a plain-language feed, so:
  - before each step write ONE short Thai sentence: what you are about to do and why (no code);
  - give every shell command a short Thai `description`;
  - keep it lean: no long recaps, no repeating file contents, finish with 1-3 lines of what changed.
- Ask the owner directly when a decision is needed. Keep REPORT.md / PROGRESS.md accurate.
- After each meaningful chunk of work and before finishing, report progress back (1-3 lines, Thai):
  "{ia}" note {id} "<summary>"
  It is saved on the idea, logged, and sent to the owner's Telegram / phone app.
"##,
        dir = dir.display(),
        ia = ia_path()
    )
}

/// Project-level SessionStart hook: sessions started by Idea Autopilot (env var set) get their
/// brief as context; every other session in this project sees nothing.
fn ensure_hook(root: &std::path::Path) -> Result<(), String> {
    const CMD: &str = "[Console]::OutputEncoding = [Text.Encoding]::UTF8; if ($env:IDEA_AUTOPILOT_BRIEF) { Get-Content -Raw -Encoding UTF8 $env:IDEA_AUTOPILOT_BRIEF }";
    let path = root.join(".claude").join("settings.json");
    let mut v: Value = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| serde_json::json!({}));
    if let Some(list) = v.pointer_mut("/hooks/SessionStart").and_then(Value::as_array_mut) {
        list.retain(|e| !e.to_string().contains("IDEA_AUTOPILOT_IDEA_DIR")); // earlier design
    }
    if !v.to_string().contains("IDEA_AUTOPILOT_BRIEF") {
        let entry = serde_json::json!({ "matcher": "", "hooks": [{ "type": "command", "command": CMD, "shell": "powershell", "timeout": 10 }] });
        let hooks = v.as_object_mut().ok_or("settings.json is not an object")?.entry("hooks").or_insert_with(|| serde_json::json!({}));
        let list = hooks.as_object_mut().ok_or("hooks is not an object")?.entry("SessionStart").or_insert_with(|| serde_json::json!([]));
        list.as_array_mut().ok_or("SessionStart is not a list")?.push(entry);
    }
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&path, serde_json::to_string_pretty(&v).unwrap()).map_err(|e| e.to_string())
}

fn changed(app: &AppHandle) {
    let st = app.state::<AppState>();
    st.rt.lock().live_sig.clear(); // publish to the phone soon
    st.rt.lock().sync_dirty = true;
    let _ = app.emit("state-changed", ());
}

fn transcript(root: &std::path::Path, sid: &str) -> Option<PathBuf> {
    Some(dirs::home_dir()?.join(".claude").join("projects").join(crate::activity::encode_path(root)).join(format!("{sid}.jsonl")))
}

fn push_feed(app: &AppHandle, id: i64, text: String, status: Option<&str>) {
    let st = app.state::<AppState>();
    {
        let mut rt = st.rt.lock();
        let Some(s) = rt.sessions.get_mut(&id) else { return };
        if !text.is_empty() && s.feed.last().map(|f| f.text != text).unwrap_or(true) {
            s.feed.push(FeedItem { ts: db::now(), text });
            let n = s.feed.len();
            if n > FEED_MAX {
                s.feed.drain(..n - FEED_MAX);
            }
        }
        if let Some(x) = status {
            s.status = x.into();
        }
    }
    changed(app);
}

/// Strip ANSI escapes and all whitespace: the TUI positions its text with cursor moves, so
/// "remote-control is active" reaches us as "remote-controlisactive".
fn squash(s: &str) -> String {
    let mut o = String::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\x1b' {
            match it.peek() {
                Some('[') => {
                    it.next();
                    for d in it.by_ref() {
                        if d.is_ascii_alphabetic() || d == '~' {
                            break;
                        }
                    }
                }
                Some(']') => {
                    for d in it.by_ref() {
                        if d == '\x07' {
                            break;
                        }
                    }
                }
                _ => {
                    it.next();
                }
            }
        } else if !c.is_whitespace() {
            o.push(c);
        }
    }
    o
}

/// Type one instruction into the session: bracketed paste (keeps it one message even with
/// newlines), then Enter. While Claude is busy the message is queued by Claude Code itself.
fn type_in(p: &mut Pty, text: &str) {
    let clean = text.replace('\r', "").replace('\x1b', "");
    let _ = p.writer.write_all(format!("\x1b[200~{clean}\x1b[201~").as_bytes());
    let _ = p.writer.flush();
    std::thread::sleep(Duration::from_millis(400));
    let _ = p.writer.write_all(b"\r");
    let _ = p.writer.flush();
}

/// Is the bot supposed to leave this idea alone (open session or manual hold)?
pub fn held(st: &AppState, idea_id: i64) -> bool {
    let rt = st.rt.lock();
    rt.sessions.contains_key(&idea_id) || rt.holds.get(&idea_id).map(|t| *t > db::now()).unwrap_or(false)
}

pub fn hold(app: &AppHandle, id: i64, on: bool) -> Result<(), String> {
    let st = app.state::<AppState>();
    if on {
        st.rt.lock().holds.insert(id, db::now() + HOLD_SECS);
    } else {
        st.rt.lock().holds.remove(&id);
    }
    st.wake.notify_one();
    changed(app);
    Ok(())
}

pub fn start(app: &AppHandle, id: i64) -> Result<(), String> {
    let st = app.state::<AppState>();
    if st.rt.lock().sessions.contains_key(&id) {
        return Ok(());
    }
    let idea = db::get_idea(&st.db.lock(), id).map_err(|e| e.to_string())?;
    if idea.status == "archived" || idea.status == "deleted" {
        return Err("ไอเดียนี้ถูกเก็บ (archive) แล้ว — กู้คืนก่อนถึงจะเปิดคุยต่อได้".into());
    }
    let dir = workspace::idea_dir(&st.ws, &idea.slug);
    if !dir.is_dir() {
        return Err("ไม่พบโฟลเดอร์ของไอเดีย".into());
    }
    let brief = dir.join("SESSION.md");
    std::fs::write(&brief, session_md(id, &idea.title, &dir)).map_err(|e| e.to_string())?;
    ensure_hook(&st.root)?;
    // the owner takes over: stop the bot if it is working on this idea right now
    if st.rt.lock().current.as_ref().map(|c| c.idea_id) == Some(id) {
        if let Some(c) = st.rt.lock().cancel.clone() {
            c.notify_one();
        }
    }
    // one conversation per idea: continue the previous one when its transcript still exists
    let key = format!("remote_sid_{id}");
    let old = db::get_setting(&st.db.lock(), &key).filter(|s| transcript(&st.root, s).is_some_and(|p| p.is_file()));
    let resume = old.is_some();
    let sid = old.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    db::set_setting(&st.db.lock(), &key, &sid).map_err(|e| e.to_string())?;
    let name = session_name(id, &idea.title);

    let pair = native_pty_system()
        .openpty(PtySize { rows: 40, cols: 140, pixel_width: 0, pixel_height: 0 })
        .map_err(|e| format!("เปิด console ไม่ได้: {e}"))?;
    let mut cmd = CommandBuilder::new(crate::providers::resolve_bin("claude").unwrap_or_else(|| "claude".into()));
    cmd.args(["--remote-control", &name, if resume { "--resume" } else { "--session-id" }, &sid]);
    cmd.cwd(&st.root);
    cmd.env("IDEA_AUTOPILOT_BRIEF", &brief);
    // markers inherited from a parent Claude session would turn off the transcript we read
    for (k, _) in std::env::vars() {
        if k.starts_with("CLAUDE_CODE_") || k == "CLAUDECODE" || k == "ANTHROPIC_BASE_URL" {
            cmd.env_remove(&k);
        }
    }
    let child = pair.slave.spawn_command(cmd).map_err(|e| format!("เรียก claude ไม่ได้: {e}"))?;
    drop(pair.slave);
    let pid = child.process_id().unwrap_or(0);
    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
    let ready = Arc::new(AtomicBool::new(false));
    let ready_flag = ready.clone();
    PTYS.lock().insert(id, Pty { writer, _master: pair.master, child, ready: ready.clone(), queue: Vec::new() });
    st.rt.lock().sessions.insert(
        id,
        RemoteInfo {
            idea_id: id,
            name: name.clone(),
            status: "starting".into(),
            started: db::now(),
            pid,
            sid: sid.clone(),
            feed: vec![FeedItem { ts: db::now(), text: if resume { "⏳ เปิด session เดิมต่อ…".into() } else { "⏳ กำลังเปิด session…".into() } }],
            ..Default::default()
        },
    );
    db::log_event(&st.db.lock(), "remote", Some(id), &format!("เปิด session {name}"));
    changed(app);

    // console reader: must keep draining the pseudo-console; also spots "ready" and login trouble
    let a = app.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        let mut tail = String::new();
        let mut warned = false;
        let mut trusted = false;
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if ready.load(Ordering::Relaxed) {
                        continue;
                    }
                    tail.push_str(&String::from_utf8_lossy(&buf[..n]));
                    if tail.len() > 64_000 {
                        tail.drain(..tail.len() - 32_000);
                    }
                    let s = squash(&tail);
                    // what the console shows, for the "not ready" explanation below
                    {
                        let clean = crate::runner::strip_ansi(&tail);
                        let lines: Vec<&str> = clean.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
                        let last: Vec<&str> = lines.iter().rev().take(8).rev().copied().collect();
                        CONSOLE.lock().insert(id, last.join("\n"));
                    }
                    // a first run in a new folder asks "do you trust this folder?" and waits for
                    // an answer nobody sees: the folder is the app's own, so accept (once)
                    if !trusted && (s.contains("trustthisfolder") || s.contains("Doyoutrust") || s.contains("trustthefiles")) {
                        trusted = true;
                        tail.clear();
                        if let Some(p) = PTYS.lock().get_mut(&id) {
                            let _ = p.writer.write_all(b"\r");
                            let _ = p.writer.flush();
                        }
                        push_feed(&a, id, "🔐 Claude Code ถามว่าเชื่อถือโฟลเดอร์นี้ไหม — ตอบ “เชื่อถือ” ให้แล้ว (เป็นโฟลเดอร์ของแอปเอง)".into(), None);
                        continue;
                    }
                    if s.contains("remote-controlisactive") {
                        ready.store(true, Ordering::Relaxed);
                        let queued = PTYS.lock().get_mut(&id).map(|p| std::mem::take(&mut p.queue)).unwrap_or_default();
                        push_feed(&a, id, "🛰 พร้อมแล้ว — เปิดดูในแอป Claude ได้".into(), Some(if queued.is_empty() { "idle" } else { "working" }));
                        for q in queued {
                            if let Some(p) = PTYS.lock().get_mut(&id) {
                                type_in(p, &q);
                            }
                        }
                    } else if !warned && s.contains("Notloggedin") {
                        warned = true;
                        push_feed(&a, id, "⚠️ Claude Code ยังไม่ได้ login — เปิด Windows Terminal แล้วพิมพ์ claude auth login".into(), Some("error"));
                    }
                }
            }
        }
        ended(&a, id);
    });

    // still not ready after a minute: say why instead of leaving "opening…" on screen forever
    let a = app.clone();
    let watch_ready = ready_flag.clone();
    let watch_sid = sid.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(60));
        let st = a.state::<AppState>();
        let still = st.rt.lock().sessions.get(&id).is_some_and(|s| s.sid == watch_sid && s.status == "starting");
        if still && !watch_ready.load(Ordering::Relaxed) {
            let screen = CONSOLE.lock().get(&id).cloned().unwrap_or_default();
            let _ = std::fs::write(st.root.join("data").join(format!("remote-{id}.log")), &screen);
            push_feed(
                &a,
                id,
                format!(
                    "⚠️ session ยังไม่พร้อมหลัง 1 นาที — หน้าจอ Claude Code ตอนนี้:\n{screen}\n\nสาเหตุที่พบบ่อย: (1) ยังไม่ login ด้วยบัญชี claude.ai แบบแพ็กเกจ Pro/Max (API key ใช้ Remote Control ไม่ได้) → เปิด terminal พิมพ์ claude auth login (2) Claude Code เก่าเกินไป → อัปเดต (3) องค์กรปิดฟีเจอร์ Remote Control ไว้"
                ),
                Some("error"),
            );
        }
    });

    // transcript tailer: turns what happens in the session into plain-Thai feed lines
    let a = app.clone();
    let path = transcript(&st.root, &sid).ok_or("หาโฟลเดอร์ ~/.claude ไม่เจอ")?;
    let mut pos = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    std::thread::spawn(move || {
        let mut partial = String::new();
        while a.state::<AppState>().rt.lock().sessions.get(&id).is_some_and(|s| s.sid == sid) {
            std::thread::sleep(Duration::from_millis(1500));
            let Ok(mut f) = std::fs::File::open(&path) else { continue };
            let len = f.metadata().map(|m| m.len()).unwrap_or(0);
            if len <= pos {
                continue;
            }
            use std::io::Seek;
            let _ = f.seek(std::io::SeekFrom::Start(pos));
            let mut chunk = Vec::new();
            let _ = f.take(len - pos).read_to_end(&mut chunk);
            pos = len;
            partial.push_str(&String::from_utf8_lossy(&chunk));
            let complete = partial.rfind('\n').map(|i| i + 1).unwrap_or(0);
            let lines: String = partial.drain(..complete).collect();
            for l in lines.lines() {
                let Ok(v) = serde_json::from_str::<Value>(l) else { continue };
                match v.get("type").and_then(Value::as_str) {
                    Some("user") => {
                        if let Some(t) = crate::narrate::user(&v) {
                            push_feed(&a, id, t, Some("working"));
                        } else if crate::narrate::had_error(&v) {
                            push_feed(&a, id, "⚠️ เจอปัญหาระหว่างทำ — กำลังหาทางแก้".into(), None);
                        }
                    }
                    Some("assistant") => {
                        let parts = v.pointer("/message/content").and_then(Value::as_array).cloned().unwrap_or_default();
                        // each content block is its own line (all carrying the message's stop_reason),
                        // so the end of a turn is taken from the system lines below, not from here
                        for t in crate::narrate::assistant(&parts) {
                            push_feed(&a, id, t, Some("working"));
                        }
                    }
                    Some("system") if matches!(v.get("subtype").and_then(Value::as_str), Some("stop_hook_summary" | "turn_duration")) => {
                        push_feed(&a, id, TURN_DONE.into(), Some("idle"));
                    }
                    _ => {}
                }
            }
        }
    });
    Ok(())
}

/// Send an instruction to the idea's session, opening (or reopening) it first when needed.
pub fn ask(app: &AppHandle, id: i64, text: &str) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("พิมพ์สิ่งที่อยากให้ AI ทำก่อน".into());
    }
    let open = app.state::<AppState>().rt.lock().sessions.contains_key(&id);
    if !open {
        start(app, id)?;
    }
    let sent_now = {
        let mut ptys = PTYS.lock();
        let p = ptys.get_mut(&id).ok_or("session ปิดไปแล้ว — ลองส่งใหม่อีกครั้ง")?;
        if p.ready.load(Ordering::Relaxed) {
            true
        } else {
            p.queue.push(text.to_string());
            false
        }
    };
    if sent_now {
        let t = text.to_string();
        std::thread::spawn(move || {
            if let Some(p) = PTYS.lock().get_mut(&id) {
                type_in(p, &t);
            }
        });
    } else {
        push_feed(app, id, format!("📨 รับคำสั่งแล้ว จะส่งให้ AI ทันทีที่ session พร้อม: {}", text.chars().take(120).collect::<String>()), None);
    }
    let st = app.state::<AppState>();
    db::log_event(&st.db.lock(), "remote", Some(id), &format!("สั่งงาน: {}", text.chars().take(200).collect::<String>()));
    Ok(())
}

fn ended(app: &AppHandle, id: i64) {
    PTYS.lock().remove(&id);
    let st = app.state::<AppState>();
    let Some(info) = st.rt.lock().sessions.remove(&id) else { return };
    if let Ok(i) = db::get_idea(&st.db.lock(), id) {
        let _ = std::fs::remove_file(workspace::idea_dir(&st.ws, &i.slug).join("SESSION.md"));
    }
    db::log_event(&st.db.lock(), "remote", Some(id), &format!("ปิด session {}", info.name));
    st.wake.notify_one();
    changed(app);
}

pub fn stop(app: &AppHandle, id: i64) -> Result<(), String> {
    if let Some(mut p) = PTYS.lock().remove(&id) {
        let _ = p.child.kill();
    }
    ended(app, id);
    Ok(())
}

pub fn stop_all(app: &AppHandle) {
    let ids: Vec<i64> = app.state::<AppState>().rt.lock().sessions.keys().copied().collect();
    for id in ids {
        let _ = stop(app, id);
    }
}

/// Called from the scheduler: close sessions of archived/deleted ideas.
pub fn tick(app: &AppHandle) {
    let st = app.state::<AppState>();
    let ids: Vec<i64> = st.rt.lock().sessions.keys().copied().collect();
    for id in ids {
        let gone = db::get_idea(&st.db.lock(), id).map(|i| i.status == "archived" || i.status == "deleted").unwrap_or(true);
        if gone {
            let _ = stop(app, id);
        }
    }
}

/// Progress note from a remote session (or anyone): shown on the idea, logged, sent to Telegram.
pub fn note(app: &AppHandle, id: i64, text: &str) -> Result<Value, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("ต้องมีข้อความสรุป".into());
    }
    let st = app.state::<AppState>();
    let title = {
        let c = st.db.lock();
        let mut idea = db::get_idea(&c, id).map_err(|e| e.to_string())?;
        idea.summary = text.chars().take(400).collect();
        db::save_idea(&c, &idea).map_err(|e| e.to_string())?;
        db::log_event(&c, "note", Some(id), text);
        idea.title
    };
    push_feed(app, id, format!("📝 สรุป: {}", text.chars().take(300).collect::<String>()), None);
    crate::chat::push(app, format!("📝 #{id} {title}\n{text}"));
    changed(app);
    Ok(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spots_ready_through_cursor_moves() {
        let screen = "\x1b[2;3H/remote-control\x1b[1Cis\x1b[1Cactive · Continue";
        assert!(squash(screen).contains("remote-controlisactive"));
    }
}
