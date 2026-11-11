//! Local control API for the `ia` CLI (and anything else on this machine, e.g. Claude Code).
//! Listens on 127.0.0.1 only; every request must carry the token from `data/api.json`.
use serde_json::{json, Value};
use std::io::Read;
use tauri::{AppHandle, Emitter, Manager};

use crate::commands;
use crate::state::AppState;

pub const PORT: u16 = 47821;

fn token(st: &AppState) -> String {
    let path = st.root.join("data").join("api.json");
    if let Ok(v) = std::fs::read_to_string(&path).map(|s| serde_json::from_str::<Value>(&s).unwrap_or_default()) {
        if let Some(t) = v.get("token").and_then(Value::as_str) {
            return t.to_string();
        }
    }
    use std::hash::{BuildHasher, Hasher};
    let mut t = String::new();
    for i in 0..4 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos());
        h.write_u32(std::process::id() + i);
        t.push_str(&format!("{:016x}", h.finish()));
    }
    let _ = std::fs::write(&path, json!({ "port": PORT, "token": t }).to_string());
    t
}

pub fn start(app: AppHandle) {
    let tok = token(&app.state::<AppState>());
    std::thread::spawn(move || {
        let server = match tiny_http::Server::http(("127.0.0.1", PORT)) {
            Ok(s) => s,
            Err(e) => {
                app.state::<AppState>().push_log(format!("[api] เปิดพอร์ต {PORT} ไม่ได้: {e}"));
                return;
            }
        };
        for mut req in server.incoming_requests() {
            let authed = req
                .headers()
                .iter()
                .any(|h| h.field.equiv("X-Token") && h.value.as_str() == tok);
            let (code, body) = if !authed {
                (401, json!({"error": "bad token"}))
            } else {
                let mut raw = String::new();
                let _ = req.as_reader().take(1 << 20).read_to_string(&mut raw);
                let v: Value = serde_json::from_str(&raw).unwrap_or_default();
                let cmd = v.get("cmd").and_then(Value::as_str).unwrap_or("");
                match dispatch(&app, cmd, v.get("args").cloned().unwrap_or(Value::Null)) {
                    Ok(r) => (200, json!({"ok": r})),
                    Err(e) => (400, json!({"error": e})),
                }
            };
            let resp = tiny_http::Response::from_string(body.to_string())
                .with_status_code(code)
                .with_header("Content-Type: application/json; charset=utf-8".parse::<tiny_http::Header>().unwrap());
            let _ = req.respond(resp);
        }
    });
}

fn id(a: &Value) -> Result<i64, String> {
    a.get("id").and_then(Value::as_i64).ok_or_else(|| "ต้องระบุ id".to_string())
}

fn text(a: &Value, k: &str) -> Option<String> {
    a.get(k).and_then(Value::as_str).map(String::from)
}

/// Files the phone uploaded into the sync repo (`uploads/phone-<time>-<name>`) for this command: staged as attachments
/// (the same way a file dropped on the desktop is), then removed from the repo. None when there are none.
fn uploads(app: &AppHandle, a: &Value) -> Result<Option<Vec<String>>, String> {
    let paths: Vec<String> = a.get("files").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
    if paths.is_empty() {
        return Ok(None);
    }
    let st = app.state::<AppState>();
    let dir = crate::sync::sync_dir(&st).ok_or("ยังไม่ได้ตั้งโฟลเดอร์ sync")?;
    let mut tokens = vec![];
    for p in paths {
        if !p.starts_with("uploads/") || p.contains("..") || p.contains('\\') || p.matches('/').count() != 1 {
            return Err(format!("ไฟล์แนบไม่ถูกต้อง: {p}"));
        }
        let f = dir.join(&p);
        let bytes = std::fs::read(&f).map_err(|e| format!("ไม่พบไฟล์แนบ {p} ({e})"))?;
        let file = p.trim_start_matches("uploads/");
        // phone-<time>-<name> → <name>
        let name = file.splitn(3, '-').nth(2).filter(|n| !n.is_empty()).unwrap_or(file);
        let s = crate::attach::stage_bytes(&st.root, name, &bytes).map_err(|e| format!("{e:#}"))?;
        tokens.push(s.token);
        let _ = std::fs::remove_file(&f);
    }
    Ok(Some(tokens))
}

pub fn dispatch(app: &AppHandle, cmd: &str, a: Value) -> Result<Value, String> {
    let st = || app.state::<AppState>();
    let unit = |r: Result<(), String>| r.map(|_| Value::Null);
    match cmd {
        "status" => {
            let mut v = commands::get_state(st());
            if let Some(l) = v.get_mut("live_log").and_then(Value::as_array_mut) {
                let n = l.len().saturating_sub(15);
                l.drain(..n);
            }
            v["open_questions"] = json!(crate::db::questions(&st().db.lock(), None, true).map(|q| q.len()).unwrap_or(0));
            Ok(v)
        }
        "ideas" => commands::list_ideas(st()),
        "idea" => commands::get_idea(st(), id(&a)?),
        // from the phone: `files` = photos it uploaded to the sync repo, `ai` = { account, model, effort } of the new idea
        "add" => commands::add_idea(st(), text(&a, "text").ok_or("ต้องมีข้อความไอเดีย")?, uploads(app, &a)?, a.get("ai").and_then(|v| serde_json::from_value(v.clone()).ok()), text(&a, "scope")).map(|i| json!(i)),
        "scope" => commands::idea_scope_set(st(), id(&a)?, text(&a, "scope").unwrap_or_default()).map(|_| json!(null)),
        "edit" => unit(commands::edit_idea(st(), id(&a)?, text(&a, "text").ok_or("ต้องมีข้อความไอเดีย")?)),
        "answer" => {
            let qid = id(&a)?;
            let mut ans = text(&a, "answer").ok_or("ต้องมีคำตอบ")?;
            // "2" picks the 2nd option
            if let Ok(n) = ans.trim().parse::<usize>() {
                let qs = crate::db::questions(&st().db.lock(), None, false).map_err(|e| e.to_string())?;
                if let Some(o) = qs.iter().find(|q| q.id == qid).and_then(|q| q.options.get(n.wrapping_sub(1))) {
                    ans = o.clone();
                }
            }
            unit(commands::answer_question(app.clone(), st(), qid, ans))
        }
        "action" => unit(commands::idea_action(st(), id(&a)?, text(&a, "action").ok_or("ต้องมี action")?, text(&a, "text"), None)),
        "pause" => unit(commands::set_paused(app.clone(), st(), true)),
        "resume" => unit(commands::set_paused(app.clone(), st(), false)),
        "pause_all" => commands::pause_all(app.clone(), st()),
        "resume_all" => commands::resume_all(app.clone(), st()).map(|n| json!(n)),
        "shelf_dismiss" => unit(commands::shelf_dismiss(app.clone(), st())),
        "turbo" => unit(commands::set_turbo(st(), a.get("hours").and_then(Value::as_f64).unwrap_or(2.0))),
        "stop" => unit(commands::stop_current(st())),
        "refresh" => unit(commands::refresh_usage(st())),
        "digest" => commands::make_digest(st()),
        "reports" => commands::get_reports(st(), a.get("days").and_then(Value::as_i64).unwrap_or(7)),
        "calendar" => commands::get_calendar(st(), a.get("days").and_then(Value::as_i64).unwrap_or(14)),
        "set_day" => unit(commands::set_day(st(), text(&a, "date").ok_or("ต้องมี date")?, text(&a, "kind"), text(&a, "note"))),
        "run" => unit(commands::run_prototype(app.clone(), st(), id(&a)?)),
        "events" => commands::get_events(st(), a.get("hours").and_then(Value::as_i64).unwrap_or(24)),
        "reorder" => {
            let ids: Vec<i64> = serde_json::from_value(a.get("ids").cloned().unwrap_or_default()).map_err(|e| e.to_string())?;
            unit(commands::reorder(st(), ids))
        }
        "sync" => Ok(json!(crate::sync::role(&st()))),
        "models" => Ok(commands::get_policy(st())),
        "set" => unit(commands::set_setting(st(), text(&a, "key").ok_or("ต้องมี key")?, text(&a, "value").unwrap_or_default())),
        "activity" => commands::get_activity(st(), a.get("days").and_then(Value::as_i64).unwrap_or(1)),
        "runlog" => commands::get_run_log(st(), id(&a)?).map(|v| json!(v)),
        "takeover" => crate::sync::takeover(app).map(|_| Value::Null).map_err(|e| format!("{e:#}")),
        "brain" => crate::brain::export(&st()).map(|p| json!(p)).map_err(|e| format!("{e:#}")),
        "remote" => {
            let on = a.get("on").and_then(Value::as_bool).unwrap_or(true);
            unit(commands::idea_remote(app.clone(), st(), id(&a)?, on))
        }
        "handoff" => crate::commands::idea_handoff(app.clone(), st(), id(&a)?, text(&a, "agent").unwrap_or_default(), text(&a, "text").unwrap_or_default(), None, None).map(|m| json!(m)),
        "agents" => Ok(crate::agents::list_json(&st().db.lock())),
        // `ai` (optional): { account, model, effort } chosen for this message, as in the desktop chat
        "ask" => unit(commands::idea_ask(app.clone(), st(), id(&a)?, text(&a, "text").unwrap_or_default(), uploads(app, &a)?, a.get("ai").and_then(|v| serde_json::from_value(v.clone()).ok()))),
        "idea_ai" => unit(commands::idea_ai_set(st(), id(&a)?, text(&a, "account"), text(&a, "model"), text(&a, "effort"))),
        "hold" => {
            let on = a.get("on").and_then(Value::as_bool).unwrap_or(true);
            unit(commands::idea_hold(app.clone(), st(), id(&a)?, on))
        }
        "note" => unit(commands::idea_note(app.clone(), st(), id(&a)?, text(&a, "text").unwrap_or_default())),
        "reqs" => crate::remote::requests(&st(), id(&a)?),
        "cut_in" => unit(crate::remote::cut_in(app, id(&a)?)),
        // the AI asks the owner something mid-work: a tap-to-answer question on the PC and the phone (ISSUE-068)
        "question" => {
            let idea = id(&a)?;
            let q = text(&a, "text").filter(|t| !t.trim().is_empty()).ok_or("ต้องมีคำถาม")?;
            let opts: Vec<String> = text(&a, "options").unwrap_or_default().split('|').map(|o| o.trim().to_string()).filter(|o| !o.is_empty()).collect();
            {
                let s = st();
                let c = s.db.lock();
                crate::db::insert_question(&c, idea, q.trim(), &opts, None).map_err(|e| e.to_string())?;
                crate::db::log_event(&c, "question", Some(idea), &format!("AI ถาม: {}", q.trim()));
            }
            st().rt.lock().sync_dirty = true;
            crate::scheduler::notify_idea(app, idea, "AI ถามคุณ", q.trim());
            let _ = app.emit("state-changed", ());
            Ok(Value::Null)
        }
        "req" => unit(crate::remote::set_request(app, a.get("rid").and_then(Value::as_i64).ok_or("ต้องระบุเลขคำสั่ง")?, &text(&a, "state").unwrap_or_default(), &text(&a, "note").unwrap_or_default(), false)),
        "ui" => {
            crate::show_main(app, None);
            let _ = app.emit_to("main", "navigate", json!({ "page": text(&a, "page"), "idea": a.get("id") }));
            Ok(Value::Null)
        }
        // accounts: their state, the models each can run, and a short real run (the same as the buttons in Settings)
        "accounts" => Ok(commands::accounts_list(st())),
        "account_models" => tauri::async_runtime::block_on(commands::account_models(app.clone(), text(&a, "account").ok_or("ต้องระบุ account")?)),
        "model_check" => commands::model_check_now(st(), text(&a, "account").ok_or("ต้องระบุ account")?, a.get("models").and_then(|v| serde_json::from_value(v.clone()).ok())).map(|n| json!(n)),
        "model_checks" => Ok(json!(crate::model_check::all(&st().db.lock()))),
        "model_checks_export" => crate::model_check::export(&st()).map(|p| json!(p)),
        "account_test" => tauri::async_runtime::block_on(commands::account_test(app.clone(), text(&a, "account").ok_or("ต้องระบุ account")?)),
        other => Err(format!("ไม่รู้จักคำสั่ง {other}")),
    }
}
