mod accounts;
mod acct_models;
mod model_check;
mod activity;
mod agents;
mod api;
mod attach;
mod calendar;
mod cli_install;
mod agent_chat;
mod codex_chat;
mod limits;
mod vendor_limits;
mod omniroute;
mod catalog;
mod chat;
mod commands;
mod db;
mod governor;
mod idea_ai;
mod lang;
mod modelname;
mod economics;
mod power;
mod naming;
mod narrate;
mod prompts;
mod providers;
mod remote;
mod reports;
mod router;
mod runner;
mod scheduler;
mod tuner;
mod skill;
mod skillspector;
mod skills;
mod state;
mod sync;
mod brain;
mod tray;
mod update;
mod ui_translate;
mod usage;
mod workspace;

use parking_lot::Mutex;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use state::{AppState, Runtime};

/// Where data lives — nothing machine-specific is compiled in:
/// 1. `IDEA_AUTOPILOT_HOME`
/// 2. portable layout: a `data/` folder next to the exe or one level up (e.g. `<root>/bin/app.exe`)
/// 3. `%USERPROFILE%\IdeaAutopilot`
pub fn root_dir() -> PathBuf {
    if let Ok(p) = std::env::var("IDEA_AUTOPILOT_HOME") {
        return PathBuf::from(p);
    }
    root_for_exe(&std::env::current_exe().unwrap_or_default())
}

/// The data folder a copy of the program at `exe` would use (portable layout, else the home folder).
pub fn root_for_exe(exe: &std::path::Path) -> PathBuf {
    if let Some(dir) = exe.parent().map(PathBuf::from) {
        for cand in [dir.clone(), dir.join("..")] {
            if cand.join("data").is_dir() {
                return cand.canonicalize().map(|p| PathBuf::from(p.to_string_lossy().trim_start_matches(r"\\?\"))).unwrap_or(cand);
            }
        }
    }
    dirs::home_dir().unwrap_or_default().join("IdeaAutopilot")
}

fn same_file(a: &std::path::Path, b: &std::path::Path) -> bool {
    let n = |p: &std::path::Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf()).to_string_lossy().to_lowercase();
    n(a) == n(b)
}

/// The other copy of the program that tried to start while this one runs (set by the guard below).
static OTHER_COPY: parking_lot::Mutex<Option<PathBuf>> = parking_lot::const_mutex(None);

/// Quit this copy and start the one the owner just launched from somewhere else. Only the path
/// that was actually seen can be started.
pub fn switch_copy(app: &AppHandle) -> Result<(), String> {
    let exe = OTHER_COPY.lock().clone().ok_or("ไม่พบตัวใหม่ที่จะสลับไป")?;
    if !exe.is_file() {
        return Err(format!("ไม่พบไฟล์ {}", exe.display()));
    }
    let mut c = std::process::Command::new(&exe);
    c.arg("--wait-old");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0000_0008); // DETACHED_PROCESS: survives this copy quitting
    }
    c.spawn().map_err(|e| format!("เปิดตัวใหม่ไม่ได้: {e}"))?;
    app.exit(0);
    Ok(())
}

/// Windows only shows a toast for an app it knows by name; a portable app has no installer to
/// register that, so do it once here (harmless to repeat).
fn register_toast_identity() {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut c = std::process::Command::new("reg");
        c.args([
            "add",
            r"HKCU\Software\Classes\AppUserModelId\app.ideaautopilot.desktop",
            "/v",
            "DisplayName",
            "/t",
            "REG_SZ",
            "/d",
            "Idea Autopilot",
            "/f",
        ]);
        c.creation_flags(0x0800_0000);
        let _ = c.output();
    }
}

pub fn show_main(app: &AppHandle, idea: Option<i64>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
    if let Some(id) = idea {
        let _ = app.emit_to("main", "open-idea", id);
    }
}

const CAPTURE_W: f64 = 680.0;
const CAPTURE_H: f64 = 168.0;

/// Top-left (physical px) that centers the capture box on the monitor under the mouse pointer.
fn capture_origin(app: &AppHandle) -> Option<tauri::PhysicalPosition<i32>> {
    let cur = app.cursor_position().ok()?;
    let mon = app.monitor_from_point(cur.x, cur.y).ok().flatten()?;
    let (p, s, k) = (mon.position(), mon.size(), mon.scale_factor());
    let x = p.x as f64 + (s.width as f64 - CAPTURE_W * k) / 2.0;
    let y = p.y as f64 + (s.height as f64 - CAPTURE_H * k) / 2.0;
    Some(tauri::PhysicalPosition::new(x as i32, y as i32))
}

pub fn show_capture(app: &AppHandle) {
    let origin = capture_origin(app);
    if let Some(w) = app.get_webview_window("capture") {
        match origin {
            Some(o) => {
                let _ = w.set_position(o);
            }
            None => {
                let _ = w.center();
            }
        }
        let _ = w.show();
        let _ = w.set_focus();
        let _ = app.emit_to("capture", "capture-shown", ());
        return;
    }
    let b = WebviewWindowBuilder::new(app, "capture", WebviewUrl::App("capture.html".into()))
        .title("จดไอเดีย")
        .inner_size(CAPTURE_W, CAPTURE_H)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(true);
    if let Ok(w) = b.build() {
        match origin {
            Some(o) => {
                let _ = w.set_position(o);
            }
            None => {
                let _ = w.center();
            }
        }
    }
}

/// Start with Windows via HKCU\…\Run (hidden to tray with --minimized).
pub fn set_autostart(on: bool) -> Result<(), String> {
    const KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut c = std::process::Command::new("reg");
    if on {
        let val = format!("\"{}\" --minimized", exe.display());
        c.args(["add", KEY, "/v", "Idea Autopilot", "/t", "REG_SZ", "/d", &val, "/f"]);
    } else {
        c.args(["delete", KEY, "/v", "Idea Autopilot", "/f"]);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    let out = c.output().map_err(|e| e.to_string())?;
    if out.status.success() || !on {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

/// Keeps Windows from sleeping while the bot has work (ES_CONTINUOUS is per-thread,
/// so one dedicated thread owns the flag).
fn keep_awake_thread(app: AppHandle) {
    std::thread::spawn(move || {
        #[cfg(windows)]
        use windows_sys::Win32::System::Power::{SetThreadExecutionState, ES_CONTINUOUS, ES_SYSTEM_REQUIRED};
        let mut last = false;
        loop {
            let want = app.state::<AppState>().rt.lock().keep_awake;
            if want != last {
                #[cfg(windows)]
                unsafe {
                    SetThreadExecutionState(if want { ES_CONTINUOUS | ES_SYSTEM_REQUIRED } else { ES_CONTINUOUS });
                }
                last = want;
            }
            std::thread::sleep(std::time::Duration::from_secs(20));
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // started from inside a Claude session (a terminal of the Claude app, a deploy script it ran): its markers and
    // gateway address would be inherited by every Claude Code the bot starts — changing where it connects and turning
    // off Remote Control. Dropped once, before any thread or child exists. An account's own ones are set per run.
    for (k, _) in std::env::vars() {
        if remote::inherited_claude_var(&k) {
            std::env::remove_var(&k);
        }
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            show_main(app, None);
            // started from a different place than the copy that is already running (moved or copied
            // the folder): this running copy keeps its old paths, so let the owner switch to the new one
            if let (Some(new), Ok(mine)) = (args.first().map(PathBuf::from), std::env::current_exe()) {
                if new.is_file() && !same_file(&new, &mine) {
                    *OTHER_COPY.lock() = Some(new.clone());
                    let _ = app.emit_to(
                        "main",
                        "other-copy",
                        serde_json::json!({ "exe": new, "root": root_for_exe(&new), "mine": mine, "my_root": root_dir() }),
                    );
                }
            }
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let root = root_dir();
            std::fs::create_dir_all(root.join("data"))?;
            let conn = db::open(&root.join("data").join("quota.db"))?;
            db::recover_runs(&conn)?;
            // A brand-new install starts empty and shows the setup screen. An install that
            // already has ideas keeps the schedule and behaviour it had before those settings existed.
            if !db::is_fresh(&conn) {
                if db::get_setting(&conn, "schedule").is_none() {
                    let _ = db::set_setting(&conn, "schedule", &serde_json::to_string(&calendar::Schedule::legacy()).unwrap_or_default());
                }
                if db::get_setting(&conn, "onboarded").is_none() {
                    let _ = db::set_setting(&conn, "onboarded", "1");
                }
            }
            // the program folder moved since last time: say so, and note anything the old location left behind
            let here = root.display().to_string();
            if let Some(prev) = db::get_setting(&conn, "last_root").filter(|p| !p.eq_ignore_ascii_case(&here)) {
                let _ = db::log_event(&conn, "moved", None, &format!("โฟลเดอร์โปรแกรมย้ายจาก {prev} มาที่ {here} — ปรับ path ที่ผูกไว้ (เปิดพร้อม Windows, skill, ไฟล์ของแต่ละไอเดีย) ให้ตามที่ใหม่แล้ว"));
            }
            let _ = db::set_setting(&conn, "last_root", &here);
            for k in ["sync_dir", "brain_dir", "extra_inbox"] {
                if let Some(v) = db::get_setting(&conn, k).filter(|v| !v.trim().is_empty()) {
                    let first = v.split(';').next().unwrap_or("").trim().to_string();
                    if !first.is_empty() && !std::path::Path::new(&first).exists() {
                        let _ = db::log_event(&conn, "path_missing", None, &format!("โฟลเดอร์ที่ตั้งไว้ใน “{k}” หาไม่เจอแล้ว: {first} — ตั้งค่าใหม่ในหน้าตั้งค่า"));
                    }
                }
            }
            register_toast_identity();
            workspace::set_language(&db::get_setting(&conn, "language").unwrap_or_default());
            providers::set_codex_approval(&db::get_setting(&conn, "codex_approval").unwrap_or_default());
            lang::set_ui(&db::get_setting(&conn, "language").unwrap_or_default());
            let ws = workspace::ensure(&root)?;
            let paused = db::get_setting(&conn, "paused").as_deref() == Some("1");
            // Start with Windows only once the owner agreed (setup screen, or an existing install);
            // re-register every launch so the path follows the current exe
            let onboarded = db::get_setting(&conn, "onboarded").as_deref() == Some("1");
            let autostart = db::get_setting(&conn, "autostart").map(|v| v == "1").unwrap_or(onboarded);
            app.manage(AppState {
                root,
                ws,
                db: Mutex::new(conn),
                rt: Mutex::new(Runtime {
                    paused,
                    reserve_per_hour: governor::DEFAULT_RESERVE_PER_WORK_HOUR,
                    ..Default::default()
                }),
                wake: tokio::sync::Notify::new(),
            });
            let handle = app.handle().clone();

            if autostart {
                if let Err(e) = set_autostart(true) {
                    handle.state::<AppState>().push_log(format!("[autostart] {e}"));
                }
                let _ = db::set_setting(&handle.state::<AppState>().db.lock(), "autostart", "1");
            }

            {
                // first free hotkey wins; a taken hotkey must never stop the app
                use tauri_plugin_global_shortcut::{Builder, GlobalShortcutExt, ShortcutState};
                handle.plugin(Builder::new().build())?;
                let mut bound = String::new();
                for key in ["ctrl+alt+space", "ctrl+shift+space", "ctrl+alt+i", "ctrl+shift+i"] {
                    let ok = handle
                        .global_shortcut()
                        .on_shortcut(key, |app, _sc, ev| {
                            if ev.state == ShortcutState::Pressed {
                                show_capture(app);
                            }
                        })
                        .is_ok();
                    if ok {
                        bound = key.to_string();
                        break;
                    }
                }
                let _ = db::set_setting(&handle.state::<AppState>().db.lock(), "hotkey", &bound);
            }

            skillspector::start(handle.clone());
            update::start(handle.clone());
            tray::build(&handle)?;
            {
                // learn the model policy before the UI asks for it
                let st = handle.state::<AppState>();
                let p = tuner::learn(&st.db.lock(), |s| router::base(s).0).unwrap_or_default();
                st.rt.lock().policy = p;
            }
            keep_awake_thread(handle.clone());
            api::start(handle.clone());
            skill::install(&handle.state::<AppState>());
            // every idea folder is its own git repo with AGENTS.md (see workspace::ensure_repo)
            let h = handle.clone();
            std::thread::spawn(move || {
                let st = h.state::<state::AppState>();
                let ideas = db::list_ideas(&st.db.lock()).unwrap_or_default();
                for i in ideas.iter().filter(|i| i.status != "deleted") {
                    if let Err(e) = workspace::ensure_repo(&workspace::idea_dir(&st.ws, &i.slug), i, &remote::ia_path()) {
                        st.push_log(format!("[git] #{} {e:#}", i.id));
                    }
                }
            });
            tauri::async_runtime::spawn(scheduler::run_loop(handle.clone()));
            tauri::async_runtime::spawn(chat::run_loop(handle.clone()));

            if !std::env::args().any(|a| a == "--minimized") {
                show_main(&handle, None);
            }
            Ok(())
        })
        .on_window_event(|w, ev| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = ev {
                // closing hides to tray; quit from the tray menu
                api.prevent_close();
                let _ = w.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::list_ideas,
            commands::add_idea,
            commands::get_idea,
            commands::answer_question,
            commands::idea_action,
            commands::reorder,
            commands::set_paused,
            commands::set_turbo,
            commands::stop_current,
            commands::refresh_usage,
            commands::get_reports,
            commands::export_csv,
            commands::get_digest,
            commands::make_digest,
            commands::get_events,
            commands::get_calendar,
            commands::set_day,
            commands::get_settings,
            commands::set_setting,
            commands::library,
            commands::open_folder,
            commands::run_prototype,
            commands::hide_capture,
            commands::export_brain,
            commands::get_activity,
            commands::get_policy,
            commands::list_days,
            commands::idea_remote,
            commands::session_revive,
            commands::request_set,
            commands::idea_ask,
            commands::codex_thread,
            commands::idea_handoff,
            commands::list_agents,
            commands::edit_idea,
            commands::switch_copy,
            commands::get_providers,
            commands::get_about,
            commands::test_provider,
            commands::idea_note,
            commands::idea_hold,
            commands::get_run_log,
            commands::sync_status,
            commands::takeover,
            commands::show_main,
            skills::skills_list,
            skills::skills_read,
            skills::skills_set,
            skills::skills_delete,
            skills::skills_add_local,
            skills::skills_check_text,
            skills::skills_shop_search,
            skills::skills_shop_browse,
            skills::skills_shop_resolve,
            skills::skills_shop_preview,
            skills::skills_shop_install,
            commands::model_economics,
            update::update_status,
            update::update_apply,
            ui_translate::ui_dict,
            ui_translate::ui_translate_start,
            commands::session_close,
            commands::session_close_all,
            commands::sessions_overview,
            commands::accounts_list,
            commands::account_add,
            commands::account_update,
            commands::account_omni_dup_ok,
            commands::account_remove,
            commands::idea_ai_set,
            commands::idea_scope_set,
            cli_install::cli_install_start,
            cli_install::cli_install_status,
            cli_install::cli_install_command,
            commands::account_set_key,
            commands::account_clear_key,
            commands::account_models,
            commands::model_check_now,
            commands::account_test,
            commands::account_login,
            commands::accounts_series,
            commands::attach_stage,
            commands::attach_stage_path,
            commands::attach_unstage,
            commands::attach_info,
            commands::attach_list,
            commands::attach_preview,
            commands::attach_delete,
            commands::power_set_idea,
            skills::skills_deep_check,
            skills::skills_slash_list,
            skillspector::skillspector_status,
            skillspector::skillspector_install,
            skillspector::skillspector_rescan,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Idea Autopilot")
        .run(|app, ev| {
            if let tauri::RunEvent::Exit = ev {
                // don't leave remote sessions running without the app that manages them
                remote::stop_all(app);
            }
        });
}
