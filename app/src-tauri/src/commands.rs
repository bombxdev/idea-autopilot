//! Tauri commands called from the UI.
use chrono::{Duration, Local, NaiveDate};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager, State};

use crate::db;
use crate::scheduler::{self, Ready};
use crate::state::AppState;
use crate::{prompts, reports, sync, workspace};

type R<T> = Result<T, String>;

fn e<E: std::fmt::Display>(x: E) -> String {
    x.to_string()
}

#[tauri::command]
pub fn get_state(st: State<AppState>) -> Value {
    let mut v = scheduler::state_json(&st);
    v["live_log"] = json!(st.rt.lock().live_log.iter().rev().take(120).rev().cloned().collect::<Vec<_>>());
    v
}

#[tauri::command]
pub fn list_ideas(st: State<AppState>) -> R<Value> {
    let c = st.db.lock();
    let ideas = db::list_ideas(&c).map_err(e)?;
    let questions = db::questions(&c, None, true).map_err(e)?;
    let stages: Vec<Value> = prompts::STAGES.iter().map(|s| json!({"key": s.key, "label": s.label})).collect();
    Ok(json!({ "ideas": ideas, "questions": questions, "stages": stages }))
}

#[tauri::command]
pub fn add_idea(st: State<AppState>, text: String) -> R<i64> {
    if let Some(r) = sync::forward(&st, "add", json!({ "text": text })) {
        return r.map(|_| 0);
    }
    scheduler::add_idea(&st, &text).map_err(e)
}

/// Another copy of the program was started from a different folder: quit this one and start that.
#[tauri::command]
pub fn switch_copy(app: AppHandle) -> R<()> {
    crate::switch_copy(&app)
}

/// Fix the text of an idea (see scheduler::edit_idea).
#[tauri::command]
pub fn edit_idea(st: State<AppState>, id: i64, text: String) -> R<()> {
    if let Some(r) = sync::forward(&st, "edit", json!({ "id": id, "text": text })) {
        return r;
    }
    scheduler::edit_idea(&st, id, &text).map_err(e)
}

#[tauri::command]
pub fn get_idea(st: State<AppState>, id: i64) -> R<Value> {
    let c = st.db.lock();
    let idea = db::get_idea(&c, id).map_err(e)?;
    let dir = workspace::idea_dir(&st.ws, &idea.slug);
    let f = |n: &str| workspace::read_file(&dir, n);
    Ok(json!({
        "idea": idea,
        "files": {
            "report": f("REPORT.md"),
            "knowledge": f("KNOWLEDGE.md"),
            "plan": f("PLAN.md"),
            "progress": f("PROGRESS.md"),
            "idea": f("IDEA.md"),
            "issues": f("ISSUES.md"),
            "how_to_run": f("prototype/HOW_TO_RUN.md"),
        },
        "runs": db::runs(&c, Some(id), 0, 100).map_err(e)?,
        "questions": db::questions(&c, Some(id), false).map_err(e)?,
        "directives": db::directives(&c, id).map_err(e)?,
        "dir": dir,
    }))
}

#[tauri::command]
pub fn answer_question(st: State<AppState>, id: i64, answer: String) -> R<()> {
    if let Some(r) = sync::forward(&st, "answer", json!({ "id": id, "answer": answer })) {
        return r;
    }
    let (idea_id, guess, q) = {
        let c = st.db.lock();
        let all = db::questions(&c, None, false).map_err(e)?;
        let q = all.into_iter().find(|q| q.id == id).ok_or("ไม่พบคำถาม")?;
        let idea_id = db::answer_question(&c, id, &answer).map_err(e)?;
        (idea_id, q.guess.clone(), q.question)
    };
    // if the bot already finished on a different guess, ask it to revisit
    let idea = db::get_idea(&st.db.lock(), idea_id).map_err(e)?;
    if guess.as_deref() != Some(answer.as_str()) && (idea.status == "done" || idea.status == "ready") {
        scheduler::steer(&st, idea_id, &format!("เจ้าของตอบคำถาม \"{q}\" ว่า \"{answer}\" (ต่างจากที่เดาไว้) — ปรับงานให้ตรงกับคำตอบนี้"))
            .map_err(e)?;
    }
    st.wake.notify_one();
    Ok(())
}

#[tauri::command]
pub fn idea_action(st: State<AppState>, id: i64, action: String, text: Option<String>) -> R<()> {
    if let Some(r) = sync::forward(&st, "action", json!({ "id": id, "action": action, "text": text })) {
        return r;
    }
    let text = text.unwrap_or_default();
    let extra = if text.trim().is_empty() { String::new() } else { format!(" — {}", text.trim()) };
    match action.as_str() {
        "go" => scheduler::steer(&st, id, &format!("ไปต่อได้เลย ทำขั้นถัดไปที่เหมาะสม (ถ้าตัดสินแล้วว่าน่าทำ ให้วางแผนและสร้าง){extra}")),
        "deeper" => scheduler::steer(&st, id, &format!("ขุดลึกขึ้นในประเด็นที่ยังไม่ชัดหรือสำคัญที่สุด หาข้อมูลจริงเพิ่ม{extra}")),
        "alt" => scheduler::steer(&st, id, &format!("ลองอีกแนวทางหนึ่งที่ต่างจากเดิม แล้วเทียบกัน (ใช้ขั้น compare){extra}")),
        "say" => {
            if text.trim().is_empty() {
                return Err("พิมพ์คำสั่งก่อน".into());
            }
            scheduler::steer(&st, id, text.trim())
        }
        _ => {
            let c = st.db.lock();
            let mut idea = db::get_idea(&c, id).map_err(e)?;
            match action.as_str() {
                "stop" => idea.status = "done".into(),
                "archive" => idea.status = "archived".into(),
                "delete" => idea.status = "deleted".into(),
                "restore" | "retry" => {
                    idea.fail_count = 0;
                    idea.status = if idea.recipe.is_empty() {
                        "inbox".into()
                    } else if (idea.stage_idx as usize) < idea.recipe.len() {
                        "active".into()
                    } else if idea.run_cmd.is_some() {
                        "ready".into()
                    } else {
                        "done".into()
                    };
                }
                "urgent" => idea.priority = if idea.priority > 0 { 0 } else { 1 },
                "run_now" => {
                    drop(c);
                    st.rt.lock().force = Some(id);
                    st.wake.notify_one();
                    return Ok(());
                }
                other => return Err(format!("unknown action {other}")),
            }
            db::save_idea(&c, &idea).map_err(e)?;
            db::log_event(&c, &action, Some(id), &format!("{action}: {}", idea.title));
            Ok(())
        }
    }
    .map_err(e)?;
    st.wake.notify_one();
    Ok(())
}

#[tauri::command]
pub fn reorder(st: State<AppState>, ids: Vec<i64>) -> R<()> {
    if let Some(r) = sync::forward(&st, "reorder", json!({ "ids": ids })) {
        return r;
    }
    let c = st.db.lock();
    for (n, id) in ids.iter().enumerate() {
        c.execute("UPDATE ideas SET sort=?2 WHERE id=?1", rusqlite::params![id, n as f64]).map_err(e)?;
    }
    Ok(())
}

#[tauri::command]
pub fn set_paused(app: AppHandle, st: State<AppState>, paused: bool) -> R<()> {
    if let Some(r) = sync::forward(&st, if paused { "pause" } else { "resume" }, Value::Null) {
        return r;
    }
    st.rt.lock().paused = paused;
    db::set_setting(&st.db.lock(), "paused", if paused { "1" } else { "0" }).map_err(e)?;
    st.wake.notify_one();
    crate::tray::refresh(&app);
    Ok(())
}

#[tauri::command]
pub fn set_turbo(st: State<AppState>, hours: f64) -> R<()> {
    if let Some(r) = sync::forward(&st, "turbo", json!({ "hours": hours })) {
        return r;
    }
    st.rt.lock().turbo_until = if hours <= 0.0 { 0 } else { db::now() + (hours * 3600.0) as i64 };
    st.wake.notify_one();
    Ok(())
}

#[tauri::command]
pub fn stop_current(st: State<AppState>) -> R<()> {
    if let Some(r) = sync::forward(&st, "stop", Value::Null) {
        return r;
    }
    if let Some(c) = st.rt.lock().cancel.clone() {
        c.notify_one();
    }
    Ok(())
}

#[tauri::command]
pub fn refresh_usage(st: State<AppState>) -> R<()> {
    st.rt.lock().last_fetch = 0;
    st.wake.notify_one();
    Ok(())
}

#[tauri::command]
pub fn get_reports(st: State<AppState>, days: i64) -> R<Value> {
    reports::reports(&st, days).map_err(e)
}

#[tauri::command]
pub fn export_csv(app: AppHandle, st: State<AppState>) -> R<String> {
    let dir = reports::export_csv(&st).map_err(e)?;
    open_path(&app, &dir);
    Ok(dir.to_string_lossy().into())
}

#[tauri::command]
pub fn get_digest(st: State<AppState>) -> R<Value> {
    let d = db::latest_digest(&st.db.lock()).map_err(e)?;
    Ok(match d {
        Some((ts, body)) => json!({"ts": ts, "body": body}),
        None => Value::Null,
    })
}

#[tauri::command]
pub fn make_digest(st: State<AppState>) -> R<Value> {
    let body = reports::make_digest(&st).map_err(e)?;
    Ok(json!({"ts": db::now(), "body": body}))
}

#[tauri::command]
pub fn get_events(st: State<AppState>, hours: i64) -> R<Value> {
    Ok(json!(db::events_since(&st.db.lock(), db::now() - hours * 3600, 200).map_err(e)?))
}

#[tauri::command]
pub fn get_calendar(st: State<AppState>, days: i64) -> R<Value> {
    let cal = scheduler::calendar(&st);
    let rows = db::overrides(&st.db.lock()).map_err(e)?;
    let today = Local::now().date_naive();
    let list: Vec<Value> = (0..days)
        .map(|n| {
            let d = today + Duration::days(n);
            let note = rows.iter().find(|r| r.0 == d.format("%Y-%m-%d").to_string()).map(|r| r.2.clone());
            json!({
                "date": d.format("%Y-%m-%d").to_string(),
                "kind": cal.day_kind(d),
                "default": cal.default_kind(d),
                "hours": cal.day_slots(d),
                "override": note.is_some(),
                "note": note,
            })
        })
        .collect();
    Ok(json!(list))
}

/// Every per-date override, past and future (the calendar view only covers the coming weeks).
#[tauri::command]
pub fn list_days(st: State<AppState>) -> R<Value> {
    let rows = db::overrides(&st.db.lock()).map_err(e)?;
    Ok(json!(rows.into_iter().map(|(date, kind, note)| json!({ "date": date, "kind": kind, "note": note })).collect::<Vec<_>>()))
}

#[tauri::command]
pub fn set_day(st: State<AppState>, date: String, kind: Option<String>, note: Option<String>) -> R<()> {
    NaiveDate::parse_from_str(&date, "%Y-%m-%d").map_err(e)?;
    if let Some(r) = sync::forward(&st, "set_day", json!({ "date": date, "kind": kind, "note": note })) {
        return r;
    }
    db::set_override(&st.db.lock(), &date, kind.as_deref(), note.as_deref().unwrap_or("")).map_err(e)?;
    st.wake.notify_one();
    Ok(())
}

const SETTING_KEYS: &[&str] = &[
    "model_default", "model_build", "model_verify", "reserve_per_hour", "autostart", "extra_inbox", "schedule",
    "sync_dir", "brain_dir", "telegram_token", "telegram_chat", "telegram_questions",
    // which AI works, how its quota is measured, report language, first-run flag
    "provider", "tier_models", "custom_cmd", "meter", "budget_hours_week", "budget_hours_5h", "budget_reset_day",
    "language", "onboarded", "effort_default", "effort_build", "effort_verify", "send_key", "grace_secs", "telegram_progress", "seen_version",
];

#[tauri::command]
pub fn get_settings(st: State<AppState>) -> Value {
    let c = st.db.lock();
    let mut m = serde_json::Map::new();
    for k in SETTING_KEYS {
        m.insert((*k).into(), json!(db::get_setting(&c, k)));
    }
    m.insert("root".into(), json!(st.root));
    let sched: crate::calendar::Schedule = db::get_setting(&c, "schedule")
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    m.insert("schedule".into(), json!(sched));
    m.insert("hotkey".into(), json!(db::get_setting(&c, "hotkey")));
    for k in ["telegram_owner", "telegram_status"] {
        m.insert(k.into(), json!(db::get_setting(&c, k)));
    }
    Value::Object(m)
}

#[tauri::command]
pub fn set_setting(st: State<AppState>, key: String, value: String) -> R<()> {
    if !SETTING_KEYS.contains(&key.as_str()) {
        return Err("unknown setting".into());
    }
    if key == "schedule" {
        serde_json::from_str::<crate::calendar::Schedule>(&value).map_err(|e| format!("ตารางงานไม่ถูกต้อง: {e}"))?;
    }
    if key == "autostart" {
        crate::set_autostart(value == "1")?;
    }
    if key == "language" {
        workspace::set_language(&value);
    }
    if key == "tier_models" && !value.trim().is_empty() {
        serde_json::from_str::<Value>(&value).map_err(|e| format!("ชื่อโมเดลไม่ถูกต้อง: {e}"))?;
    }
    if matches!(key.as_str(), "provider" | "custom_cmd" | "meter" | "budget_hours_week" | "budget_hours_5h" | "budget_reset_day") {
        // the quota source may have changed: read it again right away instead of waiting 5 minutes
        let mut rt = st.rt.lock();
        rt.snap = None;
        rt.last_fetch = 0;
    }
    if key == "sync_dir" && !value.trim().is_empty() {
        sync::ensure_layout(std::path::Path::new(value.trim())).map_err(e)?;
    }
    if key == "telegram_token" || key == "telegram_chat" {
        // new bot or unbind → next chat to message the bot becomes the owner
        let c = st.db.lock();
        for k in ["telegram_chat", "telegram_owner", "telegram_status"] {
            db::set_setting(&c, k, "").map_err(e)?;
        }
    }
    db::set_setting(&st.db.lock(), &key, &value).map_err(e)?;
    st.rt.lock().last_learn = 0;
    st.wake.notify_one();
    Ok(())
}

#[tauri::command]
pub fn library(st: State<AppState>, q: Option<String>) -> R<Value> {
    let c = st.db.lock();
    let q = q.unwrap_or_default().to_lowercase();
    let mut out = Vec::new();
    for i in db::list_ideas(&c).map_err(e)? {
        let dir = workspace::idea_dir(&st.ws, &i.slug);
        let report = workspace::read_file(&dir, "REPORT.md");
        let know = workspace::read_file(&dir, "KNOWLEDGE.md");
        if report.is_none() && know.is_none() {
            continue;
        }
        let text = format!("{} {} {}", i.title, report.clone().unwrap_or_default(), know.clone().unwrap_or_default());
        let snippet = if q.is_empty() {
            None
        } else {
            let lower = text.to_lowercase();
            match lower.find(&q) {
                Some(pos) => {
                    let start = lower[..pos].char_indices().rev().nth(60).map(|(p, _)| p).unwrap_or(0);
                    let start = if text.is_char_boundary(start) { start } else { 0 };
                    Some(text[start..].chars().take(180).collect::<String>())
                }
                None => continue,
            }
        };
        out.push(json!({
            "id": i.id, "title": i.title, "summary": i.summary, "intents": i.intents,
            "knowledge": i.knowledge, "verdict": i.verdict, "status": i.status, "updated": i.updated,
            "snippet": snippet,
        }));
    }
    Ok(json!(out))
}

fn open_path(app: &AppHandle, p: &std::path::Path) {
    use tauri_plugin_opener::OpenerExt;
    let _ = app.opener().open_path(p.to_string_lossy(), None::<&str>);
}

#[tauri::command]
pub fn open_folder(app: AppHandle, st: State<AppState>, id: i64) -> R<()> {
    let idea = db::get_idea(&st.db.lock(), id).map_err(e)?;
    open_path(&app, &workspace::idea_dir(&st.ws, &idea.slug));
    Ok(())
}

/// Drop the `\\?\` prefix canonicalize() adds on Windows — cmd.exe can't use it as a working dir.
fn plain(p: std::path::PathBuf) -> std::path::PathBuf {
    std::path::PathBuf::from(p.to_string_lossy().trim_start_matches(r"\\?\").to_string())
}

/// Does this look like a runnable command rather than a sentence of instructions?
fn looks_like_command(s: &str) -> bool {
    let first = s.split_whitespace().next().unwrap_or("").to_lowercase();
    !s.trim().is_empty()
        && s.is_ascii()
        && (["npm", "npx", "pnpm", "yarn", "node", "python", "py", "cargo", "dotnet", "java", "start", "go", "deno", "bun"].contains(&first.as_str())
            || first.ends_with(".exe")
            || first.ends_with(".bat")
            || first.ends_with(".cmd")
            || first.starts_with(".\\")
            || first.starts_with("./"))
}

fn open_in_console(dir: &std::path::Path, command: &str) -> R<()> {
    let mut c = std::process::Command::new("cmd");
    c.current_dir(dir).args(["/c", "start", "Idea Autopilot", "cmd", "/k", command]);
    c.spawn().map(|_| ()).map_err(e)
}

/// Start what the bot built (the user clicked ▶). Prefers what can't go wrong:
/// an explicit file to open, a real command, an index.html, a package.json, then the docs/folder.
#[tauri::command]
pub fn run_prototype(app: AppHandle, st: State<AppState>, id: i64) -> R<()> {
    use tauri_plugin_opener::OpenerExt;
    let idea = db::get_idea(&st.db.lock(), id).map_err(e)?;
    let ready: Ready = idea.run_cmd.as_deref().and_then(|s| serde_json::from_str(s).ok()).unwrap_or_default();
    let base = plain(workspace::idea_dir(&st.ws, &idea.slug).canonicalize().map_err(|_| "ไม่พบโฟลเดอร์ของไอเดีย".to_string())?);
    let inside = |rel: &str| -> Option<std::path::PathBuf> {
        let p = plain(base.join(rel.trim().trim_matches(['/', '\\'])).canonicalize().ok()?);
        p.starts_with(&base).then_some(p)
    };
    let open_file = |p: &std::path::Path| -> R<()> {
        app.opener().open_path(p.to_string_lossy(), None::<&str>).map_err(e)
    };

    if let Some(p) = Some(ready.open.as_str()).filter(|s| !s.trim().is_empty()).and_then(inside).filter(|p| p.is_file()) {
        return open_file(&p);
    }
    let cwd = Some(ready.cwd.as_str()).filter(|s| !s.trim().is_empty()).and_then(inside).filter(|p| p.is_dir());
    if let Some(dir) = &cwd {
        if looks_like_command(&ready.how_to_run) {
            return open_in_console(dir, ready.how_to_run.trim());
        }
    }
    // the bot didn't leave something runnable: look for the usual entry points
    let dirs: Vec<std::path::PathBuf> = [cwd.clone(), inside("prototype"), Some(base.clone())].into_iter().flatten().collect();
    for d in &dirs {
        for f in ["index.html", "dist/index.html", "build/index.html", "public/index.html"] {
            if d.join(f).is_file() {
                return open_file(&d.join(f));
            }
        }
    }
    for d in &dirs {
        if let Ok(pkg) = std::fs::read_to_string(d.join("package.json")) {
            let v: Value = serde_json::from_str(&pkg).unwrap_or_default();
            let script = ["dev", "start", "preview"].into_iter().find(|s| v["scripts"][*s].is_string()).unwrap_or("start");
            return open_in_console(d, &format!("npm install && npm run {script}"));
        }
    }
    for d in &dirs {
        if d.join("HOW_TO_RUN.md").is_file() {
            return open_file(&d.join("HOW_TO_RUN.md"));
        }
    }
    open_file(&dirs[0])
}

/// Open/close the named Remote Control session of one idea (runs on the main machine).
#[tauri::command]
pub fn idea_remote(app: AppHandle, st: State<AppState>, id: i64, on: bool) -> R<()> {
    if let Some(r) = sync::forward(&st, "remote", json!({ "id": id, "on": on })) {
        return r;
    }
    if on { crate::remote::start(&app, id) } else { crate::remote::stop(&app, id) }
}

/// Send an instruction to the idea's Remote Control session (opens or reopens it when needed).
#[tauri::command]
pub fn idea_ask(app: AppHandle, st: State<AppState>, id: i64, text: String) -> R<()> {
    if let Some(r) = sync::forward(&st, "ask", json!({ "id": id, "text": text })) {
        return r;
    }
    crate::remote::ask(&app, id, &text)
}

/// Send work on an idea to an AI agent: Claude (default, in the app) or another one through Orca.
#[tauri::command]
pub fn idea_handoff(app: AppHandle, st: State<AppState>, id: i64, agent: String, text: String) -> R<String> {
    if let Some(r) = sync::forward(&st, "handoff", json!({ "id": id, "agent": agent, "text": text })) {
        return r.map(|_| "ส่งคำสั่งไปเครื่องหลักแล้ว".into());
    }
    crate::agents::handoff(&app, id, &agent, &text)
}

/// Version, the full change history and the last version this machine's owner has seen.
#[tauri::command]
pub fn get_about(st: State<AppState>) -> Value {
    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "changelog": include_str!("../../../CHANGELOG.md"),
        "seen": db::get_setting(&st.db.lock(), "seen_version"),
    })
}

/// Installed AI agents, the one in use, and how quota is measured.
#[tauri::command]
pub fn get_providers(st: State<AppState>) -> Value {
    crate::providers::summary(&st.db.lock())
}

/// Really run a tiny prompt through `provider` (default: the one in use), so a wrong login,
/// missing model name or bad custom command shows up now instead of at 2 a.m.
#[tauri::command]
pub async fn test_provider(app: AppHandle, provider: Option<String>) -> R<Value> {
    let st = app.state::<AppState>();
    let (prov, model, custom) = {
        let c = st.db.lock();
        let p = provider.filter(|p| !p.is_empty() && p != "auto").unwrap_or_else(|| crate::providers::chosen(&c));
        let m = crate::providers::tier_model(&c, &p, 0);
        (p, m, db::get_setting(&c, "custom_cmd").unwrap_or_default())
    };
    let dir = st.root.join("data").join("provider-test");
    std::fs::create_dir_all(&dir).map_err(e)?;
    let lines = std::sync::Arc::new(parking_lot::Mutex::new(Vec::<String>::new()));
    let sink = lines.clone();
    let started = std::time::Instant::now();
    let out = crate::runner::run(
        crate::runner::RunSpec {
            cwd: &dir,
            prompt: "Reply with the single word OK. Do not use any tools or read any files.".into(),
            provider: prov.clone(),
            model,
            effort: "low".into(),
            tools: "Read".into(),
            custom_cmd: custom,
            timeout_mins: 2,
            log_path: &dir.join("last.log"),
        },
        std::sync::Arc::new(tokio::sync::Notify::new()),
        move |l| sink.lock().push(l),
        |_| {},
    )
    .await
    .map_err(|x| format!("{x:#}"))?;
    let seen: Vec<String> = lines.lock().iter().rev().take(6).rev().cloned().collect();
    Ok(json!({
        "ok": out.status == "ok",
        "status": out.status,
        "seconds": started.elapsed().as_secs(),
        "error": out.error,
        "lines": seen,
        "provider": prov,
    }))
}

#[tauri::command]
pub fn list_agents() -> Value {
    crate::agents::list_json()
}

#[tauri::command]
pub fn idea_hold(app: AppHandle, st: State<AppState>, id: i64, on: bool) -> R<()> {
    if let Some(r) = sync::forward(&st, "hold", json!({ "id": id, "on": on })) {
        return r;
    }
    crate::remote::hold(&app, id, on)
}

#[tauri::command]
pub fn idea_note(app: AppHandle, st: State<AppState>, id: i64, text: String) -> R<()> {
    if let Some(r) = sync::forward(&st, "note", json!({ "id": id, "text": text })) {
        return r;
    }
    crate::remote::note(&app, id, &text).map(|_| ())
}

#[tauri::command]
pub fn get_policy(st: State<AppState>) -> Value {
    json!(st.rt.lock().policy)
}

#[tauri::command]
pub fn get_activity(st: State<AppState>, days: i64) -> R<Value> {
    reports::activity(&st, days).map_err(e)
}

#[tauri::command]
pub fn get_run_log(st: State<AppState>, run_id: i64) -> R<Vec<String>> {
    reports::run_log(&st, run_id).map_err(e)
}

#[tauri::command]
pub fn export_brain(app: AppHandle, st: State<AppState>) -> R<String> {
    let p = crate::brain::export(&st).map_err(e)?;
    open_path(&app, std::path::Path::new(&p));
    Ok(p)
}

#[tauri::command]
pub fn sync_status(st: State<AppState>) -> Value {
    let mut v = json!(sync::role(&st));
    v["brain_dir"] = json!(crate::brain::brain_dir(&st));
    v["brain_exported"] = json!(db::get_setting(&st.db.lock(), "brain_exported").and_then(|x| x.parse::<i64>().ok()));
    v["stale_after"] = json!(sync::STALE_LEADER);
    v
}

#[tauri::command]
pub fn takeover(app: AppHandle) -> R<()> {
    sync::takeover(&app).map_err(|x| format!("{x:#}"))
}

#[tauri::command]
pub fn hide_capture(app: AppHandle) {
    if let Some(w) = app.get_webview_window("capture") {
        let _ = w.hide();
    }
}

#[tauri::command]
pub fn show_main(app: AppHandle, idea: Option<i64>) {
    crate::show_main(&app, idea);
}
