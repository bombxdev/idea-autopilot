//! Hand an idea to an AI agent. Claude runs inside the app (remote.rs, with a live plain-language
//! feed). Any other agent (Codex, Gemini, OpenCode…) is started in the idea's own git repo with
//! the same rules (AGENTS.md): through Orca when it is installed, otherwise in a plain terminal
//! window. Only agents found on this machine are offered, so nobody needs all of them.
use serde::Serialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::OnceLock;
use tauri::{AppHandle, Manager};

use crate::db;
use crate::state::AppState;
use crate::workspace;

#[derive(Debug, Clone, Serialize)]
pub struct AgentInfo {
    pub id: String,
    pub label: String,
    /// app = inside Idea Autopilot (live feed) · orca = started in Orca · terminal = a console window
    pub via: String,
    pub hint: String,
}

/// id, label, what it is good for — Claude first: it is the main agent.
const KNOWN: &[(&str, &str, &str)] = &[
    ("claude", "Claude", "หลัก · เห็นความคืบหน้าในแอป · ใช้โควต้า Claude"),
    ("codex", "Codex (OpenAI)", "งานโค้ด/ขัดเกลาหน้าตา · ใช้โควต้า OpenAI"),
    ("gemini", "Gemini (Google)", "อ่านเอกสารยาว/ค้นคว้า · ใช้โควต้า Google"),
    ("mistral", "Mistral Vibe", "โมเดลของ Mistral (Devstral) · ใช้โควต้า Mistral"),
    ("opencode", "OpenCode", "เลือกโมเดลได้หลายค่าย"),
];

fn hidden(c: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
}

fn on_path(cli: &str) -> bool {
    let mut c = std::process::Command::new(if cfg!(windows) { "where" } else { "which" });
    c.arg(cli);
    hidden(&mut c);
    c.output().map(|o| o.status.success()).unwrap_or(false)
}

pub fn orca_exe() -> Option<PathBuf> {
    let p = dirs::data_local_dir()?.join("Programs").join("orca").join("resources").join("bin").join("orca.exe");
    if p.is_file() {
        return Some(p);
    }
    on_path("orca").then(|| PathBuf::from("orca"))
}

fn orca_cached() -> bool {
    static HAS: OnceLock<bool> = OnceLock::new();
    *HAS.get_or_init(|| orca_exe().is_some())
}

/// Agents the owner can hand work to: installed on this machine and switched on. Once accounts have been set up
/// (settings → AI accounts), an AI whose accounts are all off or removed is not offered; before that, every
/// installed AI is (providers.rs re-checks PATH every 30 s).
pub fn list(c: &rusqlite::Connection) -> Vec<AgentInfo> {
    let orca = orca_cached();
    let configured = db::get_setting(c, "ai_accounts").is_some();
    let on: Vec<String> = crate::accounts::list(c).into_iter().filter(|a| a.enabled).map(|a| a.kind).collect();
    KNOWN
        .iter()
        .filter(|(id, ..)| crate::providers::installed(id) && (!configured || on.iter().any(|k| k == id)))
        .map(|(id, label, hint)| AgentInfo {
            id: id.to_string(),
            label: label.to_string(),
            via: if *id == "claude" { "app" } else if orca && *id != "mistral" { "orca" } else { "terminal" }.into(),
            hint: hint.to_string(),
        })
        .collect()
}

fn orca(args: &[&str]) -> Result<Value, String> {
    let exe = orca_exe().ok_or("ไม่พบ Orca — ติดตั้งจาก github.com/stablyai/orca ก่อน")?;
    let mut c = std::process::Command::new(exe);
    c.args(args).arg("--json");
    hidden(&mut c);
    let o = c.output().map_err(|e| format!("เรียก Orca ไม่ได้: {e}"))?;
    let out = String::from_utf8_lossy(&o.stdout);
    let v: Value = serde_json::from_str(out.trim()).unwrap_or(Value::Null);
    if v.get("ok").and_then(Value::as_bool) == Some(true) {
        return Ok(v["result"].clone());
    }
    let err = v.pointer("/error/message").and_then(Value::as_str).map(String::from).unwrap_or_else(|| {
        let e = String::from_utf8_lossy(&o.stderr);
        if e.trim().is_empty() { out.chars().take(300).collect() } else { e.chars().take(300).collect() }
    });
    Err(err)
}

/// Send work on idea `id` to `agent`. Claude → the idea's live session in the app; others → Orca.
pub fn handoff(app: &AppHandle, id: i64, agent: &str, text: &str, ai: &crate::remote::ChatAi) -> Result<String, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("พิมพ์สิ่งที่อยากให้ AI ทำก่อน".into());
    }
    let agent = if agent.is_empty() { "claude" } else { agent };
    if agent == "claude" {
        if !crate::providers::installed("claude") {
            return Err("ไม่พบ Claude Code ในเครื่องนี้ — เลือก AI ตัวอื่นที่ติดตั้งไว้".into());
        }
        crate::remote::ask_with(app, id, text, ai)?;
        return Ok("ส่งให้ Claude แล้ว".into());
    }
    let st = app.state::<AppState>();
    let known = list(&st.db.lock());
    let info = known.into_iter().find(|a| a.id == agent).ok_or(format!("ไม่พบ {agent} ในเครื่องนี้ หรือบัญชีของ AI นี้ปิดอยู่"))?;
    let idea = db::get_idea(&st.db.lock(), id).map_err(|e| e.to_string())?;
    let dir = workspace::idea_dir(&st.ws, &idea.slug);
    if !dir.is_dir() {
        return Err("ไม่พบโฟลเดอร์ของไอเดีย".into());
    }
    let ia = crate::remote::ia_path();
    workspace::ensure_repo(&dir, &idea, &ia).map_err(|e| format!("{e:#}"))?;
    // a snapshot first, so whatever the other agent does can be compared or rolled back
    workspace::commit(&dir, &format!("ก่อนส่งให้ {}: {}", info.label, text.chars().take(60).collect::<String>()));
    // Orca does not know Mistral Vibe: it always opens in a terminal window
    if !orca_cached() || agent == "mistral" {
        return terminal_handoff(app, id, &info, &dir, text, &idea.title);
    }
    let path = dir.to_string_lossy().replace('\\', "/");
    // registering twice is harmless; Orca answers with the existing project
    let _ = orca(&["repo", "add", "--path", &path]);
    let prompt = format!(
        "Idea #{id}: {title}\nอ่าน AGENTS.md ก่อน (กฎและบริบทของไอเดียนี้) แล้วทำงานนี้จากเจ้าของ:\n\n{text}\n\n\
         เสร็จแล้ว: อัปเดต PROGRESS.md และสรุปบนสุดของ REPORT.md, commit ใน repo นี้ (ห้าม push) \
         แล้วรัน \"{ia}\" note {id} \"<สรุป 1-3 บรรทัดภาษาไทย>\"",
        title = idea.title
    );
    let r = orca(&["agent", "launch", "--agent", agent, "--worktree", &format!("path:{path}"), "--prompt", &prompt, "--focus"])?;
    // the bot leaves the idea alone while the other agent works on the same files
    crate::remote::hold(app, id, true)?;
    let kind = r.pointer("/launch/outcome/kind").or(r.pointer("/outcome/kind")).and_then(Value::as_str).unwrap_or("");
    db::log_event(&st.db.lock(), "handoff", Some(id), &format!("ส่งให้ {} ผ่าน Orca: {}", info.label, text.chars().take(200).collect::<String>()));
    Ok(format!("เปิด {} ใน Orca แล้ว{} — บอทจะเว้นไอเดียนี้ 3 ชม.", info.label, if kind == "chat" { " (แชต)" } else { "" }))
}

/// No Orca: write the task into HANDOFF.md and open the agent in its own console window,
/// pointed at that file (the short English line avoids any quoting trouble in cmd.exe).
fn terminal_handoff(app: &AppHandle, id: i64, info: &AgentInfo, dir: &std::path::Path, text: &str, title: &str) -> Result<String, String> {
    let ia = crate::remote::ia_path();
    let brief = format!(
        "# Handoff — Idea #{id}: {title}\n\nRead AGENTS.md first (rules and context for this idea), then do this task from the owner:\n\n{text}\n\n\
         When done: update PROGRESS.md and the summary at the top of REPORT.md, commit in this repo (never push), \
         then run `\"{ia}\" note {id} \"<1-3 line summary>\"`.\n"
    );
    std::fs::write(dir.join("HANDOFF.md"), brief).map_err(|e| e.to_string())?;
    let msg = "Read HANDOFF.md in this folder and do what it says.";
    let launch = match info.id.as_str() {
        "gemini" => format!("gemini -i \"{msg}\""),
        "opencode" => format!("opencode --prompt \"{msg}\""),
        "mistral" => format!("vibe \"{msg}\""),
        other => format!("{other} \"{msg}\""),
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut c = std::process::Command::new("cmd");
        c.arg("/C");
        c.raw_arg(format!("start \"Idea #{id} - {}\" /D \"{}\" cmd /K {launch}", info.id, dir.display()));
        c.spawn().map_err(|e| format!("เปิดหน้าต่างไม่ได้: {e}"))?;
    }
    #[cfg(not(windows))]
    {
        let _ = &launch;
        return Err("เปิดหน้าต่าง terminal อัตโนมัติรองรับเฉพาะ Windows — เปิด agent เองในโฟลเดอร์ไอเดียแล้วสั่งให้อ่าน HANDOFF.md".into());
    }
    crate::remote::hold(app, id, true)?;
    let st = app.state::<AppState>();
    db::log_event(&st.db.lock(), "handoff", Some(id), &format!("ส่งให้ {} (หน้าต่าง terminal): {}", info.label, text.chars().take(200).collect::<String>()));
    Ok(format!("เปิด {} ในหน้าต่างใหม่แล้ว — บอทจะเว้นไอเดียนี้ 3 ชม.", info.label))
}

pub fn list_json(c: &rusqlite::Connection) -> Value {
    json!(list(c))
}
