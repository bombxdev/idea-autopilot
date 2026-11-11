//! Chatting with Codex inside the app ("สั่ง AI แก้ / ทำต่อ" with a Codex account).
//!
//! Claude Code keeps a live console that the app types into (remote.rs). Codex works differently: `codex exec --json`
//! runs one turn and prints its events, and `codex exec resume <thread>` carries the same conversation on. So every
//! message from the owner is one turn run in the idea's folder; the events become the plain-language feed, the thread id
//! is remembered, and a message sent while a turn is running waits for its turn. The session looks like any other open
//! session to the rest of the app (the bot leaves the idea alone, the Sessions page lists it, it can be closed).
//!
//! Works only with what `codex exec` offers: no live phone view, and effort/model are fixed per turn.
use parking_lot::Mutex;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, LazyLock};
use tauri::{AppHandle, Manager};

use crate::remote::ChatAi;
use crate::state::AppState;
use crate::{db, workspace};

const TURN_TIMEOUT_SECS: u64 = 30 * 60;

#[derive(Default)]
struct Chat {
    queue: VecDeque<(String, ChatAi)>,
    running: bool,
    child: Option<Arc<Mutex<Child>>>,
    stopped: bool,
    since: i64,
    /// the AI the owner's last message chose (an answer to the AI's question goes the same way)
    last_ai: ChatAi,
    cut: bool,
}

pub fn last_ai(id: i64) -> ChatAi {
    CHATS.lock().get(&id).map(|c| c.last_ai.clone()).unwrap_or_default()
}

/// A reply is running: (since when, messages waiting behind it).
pub fn busy(id: i64) -> Option<(i64, usize)> {
    CHATS.lock().get(&id).filter(|c| c.child.is_some() && c.since > 0).map(|c| (c.since, c.queue.len()))
}

/// The owner cuts in: stop the running reply, keep the waiting messages (ISSUE-068).
pub fn cut_in(id: i64) -> bool {
    let child = {
        let mut m = CHATS.lock();
        let Some(c) = m.get_mut(&id) else { return false };
        let Some(ch) = c.child.clone() else { return false };
        c.cut = true;
        ch
    };
    kill(&child);
    true
}

fn take_cut(id: i64) -> bool {
    CHATS.lock().get_mut(&id).map(|c| std::mem::take(&mut c.cut)).unwrap_or(false)
}

static CHATS: LazyLock<Mutex<HashMap<i64, Chat>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Is a Codex chat open for this idea?
pub fn active(app: &AppHandle, id: i64) -> bool {
    CHATS.lock().contains_key(&id) && app.state::<AppState>().rt.lock().sessions.contains_key(&id)
}

fn tid_key(id: i64) -> String {
    format!("codex_tid_{id}")
}

/// "<account>|<thread id>": a thread belongs to the login (CODEX_HOME) it was made with.
fn saved_thread(app: &AppHandle, id: i64, account: &str) -> Option<String> {
    let v = db::get_setting(&app.state::<AppState>().db.lock(), &tid_key(id))?;
    let (a, t) = v.split_once('|')?;
    (a == account && !t.is_empty()).then(|| t.to_string())
}

fn save_thread(app: &AppHandle, id: i64, account: &str, tid: &str) {
    let _ = db::set_setting(&app.state::<AppState>().db.lock(), &tid_key(id), &format!("{account}|{tid}"));
}

fn short(s: &str, n: usize) -> String {
    let flat = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() > n {
        format!("{}…", flat.chars().take(n).collect::<String>())
    } else {
        flat
    }
}

/// `"C:\...\powershell.exe" -Command 'dir .'` → `dir .`
fn command_text(raw: &str) -> String {
    let c = raw.trim();
    let inner = match c.find("-Command ") {
        Some(i) => &c[i + 9..],
        None => match c.find("-lc ") {
            Some(i) => &c[i + 4..],
            None => c,
        },
    };
    short(inner.trim().trim_matches(['\'', '"']), 110)
}

/// What a Codex event means for the owner, as one feed line (None = nothing worth showing).
pub fn feed_line(ev: &Value) -> Option<String> {
    let kind = ev.get("type")?.as_str()?;
    let item = ev.get("item");
    let itype = item.and_then(|i| i.get("type")).and_then(Value::as_str).unwrap_or("");
    match (kind, itype) {
        ("item.completed", "agent_message") => {
            let t = item?.get("text")?.as_str()?.trim();
            (!t.is_empty()).then(|| format!("💬 {}", crate::narrate::whole(t)))
        }
        ("item.started", "command_execution") => Some(format!("⚙️ รันคำสั่ง: {}", command_text(item?.get("command")?.as_str()?))),
        ("item.completed", "command_execution") => {
            let code = item?.get("exit_code").and_then(Value::as_i64)?;
            (code != 0).then(|| format!("⚠️ คำสั่งไม่สำเร็จ (exit {code})"))
        }
        ("item.completed", "file_change") => {
            let names: Vec<String> = item?
                .get("changes")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|c| c.get("path").and_then(Value::as_str)).map(|p| p.rsplit(['/', '\\']).next().unwrap_or(p).to_string()).collect())
                .unwrap_or_default();
            Some(if names.is_empty() { "📝 แก้ไฟล์".to_string() } else { format!("📝 แก้ไฟล์: {}", short(&names.join(", "), 120)) })
        }
        ("item.started", "web_search") => {
            let q = item?.get("query").and_then(Value::as_str).unwrap_or("");
            Some(format!("🔎 ค้นเว็บ: {}", short(q, 100)))
        }
        ("item.started", "mcp_tool_call") => Some(format!("🧩 ใช้เครื่องมือ: {}", item?.get("tool").and_then(Value::as_str).unwrap_or("?"))),
        ("item.completed", "error") => Some(format!("⚠️ {}", short(item?.get("message")?.as_str()?, 300))),
        ("turn.failed", _) => {
            let m = ev.pointer("/error/message").and_then(Value::as_str).unwrap_or("รอบนี้ไม่สำเร็จ");
            Some(format!("⚠️ {}", short(m, 400)))
        }
        ("error", _) => Some(format!("⚠️ {}", short(ev.get("message")?.as_str()?, 400))),
        ("turn.started", _) => Some("▶ Codex เริ่มคิวงานรอบนี้".to_string()),
        ("item.completed", "reasoning") => {
            let t = item?.get("text")?.as_str()?.trim();
            (!t.is_empty()).then(|| format!("🧠 {}", short(t, 200)))
        }
        ("turn.completed", _) => Some("✅ จบรอบนี้".to_string()),
        _ => None,
    }
}

fn feed(app: &AppHandle, id: i64, text: String, status: Option<&str>) {
    crate::remote::push_feed_pub(app, id, text, status);
}

/// Send a message to Codex for this idea (opens the chat the first time).
pub fn ask(app: &AppHandle, id: i64, text: &str, ai: &ChatAi) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("พิมพ์สิ่งที่อยากให้ AI ทำก่อน".into());
    }
    if !crate::providers::installed("codex") {
        return Err("ไม่พบ Codex ในเครื่องนี้ — กด “ติดตั้ง CLI” ที่ตั้งค่า → บัญชี AI".into());
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
    if open && !CHATS.lock().contains_key(&id) {
        return Err("session นี้เปิดกับ Claude อยู่ — กด “ปิด session” ก่อน แล้วค่อยส่งให้ Codex".into());
    }
    if !open {
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
                name: format!("Idea#{id} · Codex"),
                status: "idle".into(),
                started: db::now(),
                sid: "codex".into(),
                ai: crate::remote::ai_label(&st.db.lock(), &ai.account, "Codex", "codex", None, &ai.model),
                ..Default::default()
            },
        );
        db::log_event(&st.db.lock(), "remote", Some(id), "เปิดแชตกับ Codex");
    }
    feed(app, id, format!("🙋 คุณ: {}", short(text, 400)), None);
    let start_worker = {
        let mut m = CHATS.lock();
        let c = m.entry(id).or_default();
        c.stopped = false;
        c.queue.push_back((text.to_string(), ai.clone()));
        c.last_ai = ai.clone();
        let go = !c.running;
        c.running = true;
        go
    };
    if start_worker {
        let app2 = app.clone();
        std::thread::spawn(move || worker(&app2, id));
    } else {
        feed(app, id, "📨 รับคำสั่งแล้ว จะส่งให้ Codex ต่อจากรอบที่กำลังทำ".into(), None);
    }
    Ok(())
}

fn worker(app: &AppHandle, id: i64) {
    loop {
        let next = {
            let mut m = CHATS.lock();
            let Some(c) = m.get_mut(&id) else { return };
            match c.queue.pop_front() {
                Some(x) if !c.stopped => Some(x),
                _ => {
                    c.running = false;
                    None
                }
            }
        };
        let Some((text, ai)) = next else { break };
        if let Err(e) = turn(app, id, &text, &ai) {
            feed(app, id, format!("⚠️ {e}"), Some("idle"));
        }
    }
    feed(app, id, String::new(), Some("idle"));
}

/// How a turn reaches its conversation.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode<'a> {
    /// a brand-new thread
    New,
    /// carry the saved thread on
    Resume(&'a str),
    /// copy the saved thread (all its history) into a new one: used when the old one is busy in another process
    Fork(&'a str),
}

/// The arguments of one turn (without the program): the first message starts a thread, later ones resume it.
fn turn_args(mode: Mode, model: &str, effort: Option<&str>) -> Vec<String> {
    let mut a: Vec<String> = vec!["exec".into()];
    match mode {
        Mode::Resume(t) => a.extend(["resume".into(), t.into()]),
        Mode::Fork(t) => a.extend(["fork".into(), t.into()]),
        Mode::New => {}
    }
    let new = mode == Mode::New;
    a.extend(["--skip-git-repo-check".into(), "--json".into()]);
    if new {
        a.extend(["--color".into(), "never".into()]);
    }
    // sandbox, network and the approval mode of the owner's setting (`resume`/`fork` do not take -s, so it goes in as config)
    a.extend(crate::providers::codex_approval_args(new));
    if !model.is_empty() {
        a.extend(["-m".into(), model.into()]);
    }
    if let Some(e) = effort {
        a.extend(["-c".into(), format!("model_reasoning_effort={e}")]);
    }
    a.push("-".into());
    a
}

fn turn(app: &AppHandle, id: i64, text: &str, ai: &ChatAi) -> Result<(), String> {
    let st = app.state::<AppState>();
    let idea = db::get_idea(&st.db.lock(), id).map_err(|e| e.to_string())?;
    let dir = workspace::idea_dir(&st.ws, &idea.slug);
    let want = ai.resolved(&st.db.lock(), id);
    let account = if want.account.is_empty() { None } else { crate::accounts::find(&st.db.lock(), &want.account) };
    let account_id = account.as_ref().map(|a| a.id.clone()).unwrap_or_default();
    let thread = saved_thread(app, id, &account_id);
    let effort = crate::providers::effort_for("codex", &want.model, &want.effort).filter(|_| !want.effort.is_empty());
    let bin = crate::providers::resolve_bin("codex").ok_or("ไม่พบ Codex ในเครื่องนี้")?;

    // the first message carries the session brief; later ones are just the message (the thread remembers)
    let fresh_prompt = format!(
        "Idea #{id}: {title}\nRead SESSION.md in this folder first (how this chat works), then AGENTS.md (rules and context of this idea). Then do what the owner asks:\n\n{text}",
        title = idea.title
    );
    let run = |mode: Mode| -> Result<(), Failure> {
        let prompt = if mode == Mode::New { fresh_prompt.as_str() } else { text };
        let mut cmd = Command::new(&bin);
        cmd.args(turn_args(mode, &want.model, effort.as_deref())).current_dir(&dir).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        if let Some(a) = &account {
            for (k, v) in crate::accounts::env_with_secret(a, &st.root) {
                cmd.env(k, v);
            }
        }
        run_once(app, id, cmd, prompt, &account_id)
    };

    let mut mode = match thread.as_deref() {
        Some(t) => Mode::Resume(t),
        None => Mode::New,
    };
    feed(app, id, if thread.is_some() { "⏳ ส่งให้ Codex ต่อจากบทสนทนาเดิม…".into() } else { "⏳ เปิดบทสนทนาใหม่กับ Codex…".into() }, Some("working"));
    // at most: resume → fork (thread busy in another process) → new (thread gone, or fork failed)
    let mut result = run(mode);
    for _ in 0..2 {
        let Err(f) = &result else { break };
        if stopped(id) || crate::runner::looks_like_limit(&f.why) {
            break;
        }
        mode = match (mode, thread.as_deref()) {
            (Mode::Resume(_), Some(t)) if busy_elsewhere(&f.why) => {
                feed(app, id, "🔀 บทสนทนาเดิมถูกเปิดค้างอยู่ที่อื่น (เช่นหน้าต่าง Codex / terminal) — แตกเป็นบทสนทนาใหม่ที่มีประวัติเดิมครบ แล้วทำต่อให้เลย".into(), Some("working"));
                Mode::Fork(t)
            }
            (Mode::Resume(_), _) if thread_gone(&f.why) => {
                feed(app, id, "🆕 หาบทสนทนาเดิมของ Codex ไม่เจอแล้ว (อาจถูกล้างหรือเปลี่ยนบัญชี) — เปิดบทสนทนาใหม่ ให้อ่านไฟล์ในโฟลเดอร์ไอเดียเพื่อรู้บริบท".into(), Some("working"));
                Mode::New
            }
            (Mode::Fork(_), _) => {
                feed(app, id, "🆕 แตกบทสนทนาไม่สำเร็จ — เปิดบทสนทนาใหม่ ให้อ่านไฟล์ในโฟลเดอร์ไอเดียเพื่อรู้บริบท".into(), Some("working"));
                Mode::New
            }
            _ => break,
        };
        result = run(mode);
    }

    let Err(f) = result else { return Ok(()) };
    if stopped(id) {
        return Ok(());
    }
    if take_cut(id) {
        feed(app, id, "✋ หยุดรอบนี้ตามที่คุณสั่ง — ส่งข้อความของคุณต่อเลย".into(), None);
        return Ok(());
    }
    // the account hit its usage limit: remember until when, like the bot's own runs do
    if !account_id.is_empty() && crate::runner::looks_like_limit(&f.why) {
        let snap = st.rt.lock().acct_snaps.get(&account_id).cloned();
        let now = db::now();
        let acct = crate::accounts::find(&st.db.lock(), &account_id);
        let until = crate::vendor_limits::limit_until(acct.as_ref(), &f.why, snap.as_ref(), now);
        let reason = crate::limits::why(&f.why);
        crate::limits::set(&st.db.lock(), &account_id, until, &reason);
        return Err(format!("บัญชีนี้ชนลิมิต: {reason} — รอถึง {} (บอทและแชตจะใช้บัญชีนี้ได้อีกเมื่อโควต้ามา)", crate::limits::when_text(until, now)));
    }
    let code = f.code.map(|c| c.to_string()).unwrap_or_else(|| "?".into());
    Err(format!("Codex จบก่อนเวลา (exit {code}): {}", short(&f.why, 400)))
}

/// The saved thread is held by another Codex process (e.g. the owner opened it with `codex resume` in a terminal).
fn busy_elsewhere(why: &str) -> bool {
    let w = why.to_lowercase();
    w.contains("active writer") || w.contains("thread-store conflict")
}

/// The saved thread no longer exists (history cleaned up, another machine, another login).
fn thread_gone(why: &str) -> bool {
    let w = why.to_lowercase();
    ["no rollout found", "thread not found", "session not found", "no session found", "no such thread", "unknown thread", "failed to resume"]
        .iter()
        .any(|k| w.contains(k))
}

/// Why a run did not complete, and its exit code.
struct Failure {
    why: String,
    code: Option<i32>,
}

fn stopped(id: i64) -> bool {
    CHATS.lock().get(&id).is_none_or(|c| c.stopped)
}

/// Run one Codex process to the end, feeding its events.
fn run_once(app: &AppHandle, id: i64, mut cmd: Command, prompt: &str, account_id: &str) -> Result<(), Failure> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let fail = |why: String| Failure { why, code: None };
    let mut child = cmd.spawn().map_err(|e| fail(format!("เรียก Codex ไม่ได้: {e}")))?;
    if let Some(mut si) = child.stdin.take() {
        let _ = si.write_all(prompt.as_bytes());
    }
    let stdout = child.stdout.take().ok_or_else(|| fail("อ่านผลจาก Codex ไม่ได้".into()))?;
    let stderr = child.stderr.take();
    let shared = Arc::new(Mutex::new(child));
    if let Some(c) = CHATS.lock().get_mut(&id) {
        c.child = Some(shared.clone());
        c.since = db::now();
    }
    // a turn that runs far too long is stopped (the whole tree: a leftover codex.exe would keep the thread locked)
    let watch = shared.clone();
    let started = std::time::Instant::now();
    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let done2 = done.clone();
    std::thread::spawn(move || {
        while !done2.load(std::sync::atomic::Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_secs(5));
            if started.elapsed().as_secs() > TURN_TIMEOUT_SECS {
                kill(&watch);
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
                g.push(l);
                let n = g.len();
                if n > 12 {
                    g.drain(..n - 12);
                }
            }
        }
    });

    let mut completed = false;
    let mut failure_text = String::new();
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        let Ok(ev) = serde_json::from_str::<Value>(&line) else { continue };
        if ev.get("type").and_then(Value::as_str) == Some("thread.started") {
            if let Some(t) = ev.get("thread_id").and_then(Value::as_str) {
                save_thread(app, id, account_id, t);
            }
        }
        if let Some(t) = feed_line(&ev) {
            if t.starts_with("⚠️") && !t.starts_with("⚠️ คำสั่ง") {
                failure_text.push_str(&t);
                failure_text.push('\n');
            }
            feed(app, id, t, None);
        }
        if ev.get("type").and_then(Value::as_str) == Some("turn.completed") {
            completed = true;
        }
    }
    let status = shared.lock().wait().ok();
    done.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = err_thread.join();
    if let Some(c) = CHATS.lock().get_mut(&id) {
        c.child = None;
        c.since = 0;
    }
    if completed {
        return Ok(());
    }
    // keep both: the JSON error and stderr (the thread conflict only shows on stderr)
    let tail = err_tail.lock().join("\n");
    let why = if failure_text.is_empty() { tail } else { format!("{failure_text}{tail}") };
    Err(Failure { why: why.trim().to_string(), code: status.and_then(|s| s.code()) })
}

/// Stop a Codex process and everything it started (codex.cmd → node → codex.exe on Windows).
fn kill(child: &Arc<Mutex<Child>>) {
    let mut c = child.lock();
    crate::runner::kill_tree(c.id());
    let _ = c.kill();
}

/// Close the chat: stop a running turn, forget the queue (the conversation itself stays in Codex's own history and is
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
        kill(&ch);
    }
    CHATS.lock().remove(&id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn events_become_plain_feed_lines() {
        assert_eq!(feed_line(&json!({"type":"item.completed","item":{"type":"agent_message","text":"OK"}})).as_deref(), Some("💬 OK"));
        let cmd = json!({"type":"item.started","item":{"type":"command_execution","command":"\"C:\\\\Windows\\\\System32\\\\WindowsPowerShell\\\\v1.0\\\\powershell.exe\" -Command 'dir .'"}});
        assert_eq!(feed_line(&cmd).as_deref(), Some("⚙️ รันคำสั่ง: dir ."));
        assert!(feed_line(&json!({"type":"item.completed","item":{"type":"command_execution","exit_code":0}})).is_none());
        assert!(feed_line(&json!({"type":"item.completed","item":{"type":"command_execution","exit_code":2}})).unwrap().contains("exit 2"));
        let fc = json!({"type":"item.completed","item":{"type":"file_change","changes":[{"path":"D:/x/app.js","kind":"update"}]}});
        assert_eq!(feed_line(&fc).as_deref(), Some("📝 แก้ไฟล์: app.js"));
        assert_eq!(feed_line(&json!({"type":"turn.completed","usage":{}})).as_deref(), Some("✅ จบรอบนี้"));
        assert!(feed_line(&json!({"type":"turn.failed","error":{"message":"usage limit"}})).unwrap().contains("usage limit"));
        assert!(feed_line(&json!({"type":"turn.started"})).unwrap().starts_with("▶"));
        assert!(feed_line(&json!({"type":"item.completed","item":{"type":"reasoning","text":"  thinking about it "}})).unwrap().starts_with("🧠"));
    }

    #[test]
    fn first_turn_starts_a_thread_later_ones_resume_it() {
        let first = turn_args(Mode::New, "", None);
        assert_eq!(&first[..2], ["exec", "--skip-git-repo-check"]);
        assert!(first.contains(&"-s".to_string()) && !first.contains(&"resume".to_string()));
        let later = turn_args(Mode::Resume("abc"), "gpt-x", Some("high"));
        assert_eq!(&later[..3], ["exec", "resume", "abc"]);
        assert!(!later.contains(&"-s".to_string()), "resume does not accept -s");
        assert!(later.contains(&"sandbox_mode=workspace-write".to_string()));
        assert!(later.contains(&"model_reasoning_effort=high".to_string()) && later.contains(&"gpt-x".to_string()));
        assert_eq!(later.last().map(String::as_str), Some("-"));
        let fork = turn_args(Mode::Fork("abc"), "", None);
        assert_eq!(&fork[..3], ["exec", "fork", "abc"]);
        assert!(!fork.contains(&"-s".to_string()), "fork does not accept -s");
    }

    #[test]
    fn recognises_a_thread_held_elsewhere_or_gone() {
        let conflict = "ERROR codex_core::session: Failed to create session: thread-store conflict: thread 01a1 already has an active writer";
        assert!(busy_elsewhere(conflict) && !thread_gone(conflict));
        assert!(thread_gone("Error: no rollout found for thread id abc"));
        assert!(!busy_elsewhere("usage limit reached"));
    }
}
