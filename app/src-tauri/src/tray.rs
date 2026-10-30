//! System tray: status icon + quick actions.
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::db;
use crate::state::AppState;

const ICON_RUN: &[u8] = include_bytes!("../icons/tray-run.png");
const ICON_IDLE: &[u8] = include_bytes!("../icons/tray-idle.png");
const ICON_ASK: &[u8] = include_bytes!("../icons/tray-ask.png");
const ICON_PAUSE: &[u8] = include_bytes!("../icons/tray-pause.png");

pub struct TrayItems {
    pub pause: CheckMenuItem<Wry>,
    pub status: MenuItem<Wry>,
}

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let status = MenuItem::with_id(app, "status", "กำลังเริ่ม…", false, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "เปิด Idea Autopilot", true, None::<&str>)?;
    let hotkey = db::get_setting(&app.state::<AppState>().db.lock(), "hotkey").unwrap_or_default();
    let capture = MenuItem::with_id(app, "capture", format!("จดไอเดีย…   {}", hotkey.to_uppercase()), true, None::<&str>)?;
    let pause = CheckMenuItem::with_id(app, "pause", "พักบอท (Pause)", true, false, None::<&str>)?;
    let turbo = MenuItem::with_id(app, "turbo", "Turbo 2 ชั่วโมง", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "ออกจากโปรแกรม", true, None::<&str>)?;
    let sep = || PredefinedMenuItem::separator(app);
    let menu = Menu::with_items(app, &[&status, &sep()?, &open, &capture, &sep()?, &pause, &turbo, &sep()?, &quit])?;

    TrayIconBuilder::with_id("main")
        .icon(Image::from_bytes(ICON_IDLE)?)
        .tooltip("Idea Autopilot")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, ev| match ev.id().as_ref() {
            "open" => crate::show_main(app, None),
            "capture" => crate::show_capture(app),
            "pause" => {
                let st = app.state::<AppState>();
                let p = !st.rt.lock().paused;
                st.rt.lock().paused = p;
                let _ = db::set_setting(&st.db.lock(), "paused", if p { "1" } else { "0" });
                st.wake.notify_one();
                refresh(app);
            }
            "turbo" => {
                let st = app.state::<AppState>();
                st.rt.lock().turbo_until = db::now() + 2 * 3600;
                st.wake.notify_one();
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, ev| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev {
                crate::show_main(tray.app_handle(), None);
            }
        })
        .build(app)?;
    app.manage(TrayItems { pause, status });
    Ok(())
}

pub fn refresh(app: &AppHandle) {
    let st = app.state::<AppState>();
    let open_q = db::questions(&st.db.lock(), None, true).map(|q| q.len()).unwrap_or(0);
    let (icon, text) = {
        let rt = st.rt.lock();
        let week = rt.snap.as_ref().map(|s| format!("{:.0}%", s.week_pct)).unwrap_or("?".into());
        if rt.paused {
            (ICON_PAUSE, format!("พักบอทอยู่ · สัปดาห์นี้ใช้ {week}"))
        } else if let Some(c) = &rt.current {
            (ICON_RUN, format!("กำลังทำ: {} ({}) · {week}", c.title, c.stage))
        } else if open_q > 0 {
            (ICON_ASK, format!("{open_q} คำถามรอคุณ · {}", rt.decision.reason))
        } else {
            (ICON_IDLE, format!("{} · {week}", rt.decision.reason))
        }
    };
    if let Some(tray) = app.tray_by_id("main") {
        if let Ok(img) = Image::from_bytes(icon) {
            let _ = tray.set_icon(Some(img));
        }
        let tip: String = format!("Idea Autopilot\n{text}").chars().take(120).collect();
        let _ = tray.set_tooltip(Some(tip));
    }
    if let Some(items) = app.try_state::<TrayItems>() {
        let paused = st.rt.lock().paused;   // read flag first, then call the tray API with NO lock held
        let _ = items.status.set_text(text.chars().take(60).collect::<String>());
        let _ = items.pause.set_checked(paused);
    }
}
