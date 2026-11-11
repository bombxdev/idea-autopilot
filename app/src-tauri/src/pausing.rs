//! Pause everything before the PC is switched off, and carry on later (ISSUE-067).
//!
//! "Pause all" remembers which chats were open (and whether their AI was in the middle of something), closes them,
//! stops the bot's run and pauses the bot: the PC can be switched off at once. "Carry on" reopens the same
//! conversations (Claude Code `--resume`), tells an AI that was working to pick up where it stopped, and un-pauses.
//! What was open is also written down every scheduler tick, so a PC switched off without pausing (power cut, a
//! hurried shutdown, an update restart) offers the same "carry on" when the app starts again.
use crate::db;
use crate::remote::ChatAi;
use crate::state::AppState;
use parking_lot::Mutex;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::LazyLock;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Saved {
    pub id: i64,
    pub title: String,
    /// its AI was in the middle of something (it is told to carry on), not just an open chat
    pub working: bool,
    #[serde(default)]
    pub ai: ChatAi,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Shelf {
    pub ts: i64,
    /// the owner pressed "pause all" (else: the app found it left over from a PC switched off while working)
    pub by_owner: bool,
    pub sessions: Vec<Saved>,
    /// the bot's run that was stopped: (idea id, title, stage)
    #[serde(default)]
    pub run: Option<(i64, String, String)>,
    /// the bot was already paused before (carrying on leaves it paused)
    #[serde(default)]
    pub was_paused: bool,
}

/// what to carry on with (shown with a "carry on" button until used or dismissed)
const KEY: &str = "shelf";
/// what is open right now, kept every tick: a PC switched off suddenly leaves it behind
const LIVE: &str = "shelf_live";
/// the app is quitting: closing its sessions must not be written down as "nothing open"
pub static EXITING: AtomicBool = AtomicBool::new(false);
static LAST: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new(String::new()));

/// The message to an AI whose conversation is reopened after the PC was off.
pub const CARRY_ON: &str = "The PC was switched off while you were working (the owner paused everything to shut it down). This is the same conversation, reopened. First check what you had already finished (the files, PROGRESS.md, your last step), then carry on with the task you were doing — redo the last step if it did not finish. Do not start over.";

fn now_open(app: &AppHandle) -> Shelf {
    let st = app.state::<AppState>();
    let (list, run) = {
        let rt = st.rt.lock();
        let list: Vec<(i64, bool)> = rt
            .sessions
            .values()
            // only Claude Code conversations can be reopened; the other AIs answer one message at a time
            .filter(|s| crate::remote::is_claude_sid(&s.sid))
            .map(|s| (s.idea_id, s.status == "working" || s.status == "starting"))
            .collect();
        (list, rt.current.as_ref().map(|c| (c.idea_id, c.title.clone(), c.stage.clone())))
    };
    let mut sessions: Vec<Saved> = list
        .into_iter()
        .map(|(id, working)| Saved {
            id,
            title: db::get_idea(&st.db.lock(), id).map(|i| i.title).unwrap_or_default(),
            working,
            ai: crate::remote::open_ai(id),
        })
        .collect();
    sessions.sort_by_key(|s| s.id);
    let was_paused = st.rt.lock().paused;
    Shelf { ts: db::now(), by_owner: false, sessions, run, was_paused }
}

pub fn get(c: &Connection) -> Option<Shelf> {
    db::get_setting(c, KEY).and_then(|v| serde_json::from_str(&v).ok())
}

fn put(c: &Connection, key: &str, s: Option<&Shelf>) {
    let _ = db::set_setting(c, key, &s.and_then(|s| serde_json::to_string(s).ok()).unwrap_or_default());
}

/// Called every scheduler tick: write down what is open (only when it changed).
pub fn keep(app: &AppHandle) {
    if EXITING.load(Ordering::Relaxed) {
        return;
    }
    let mut s = now_open(app);
    let sig = serde_json::to_string(&(&s.sessions, &s.run)).unwrap_or_default();
    if *LAST.lock() == sig {
        return;
    }
    *LAST.lock() = sig;
    let st = app.state::<AppState>();
    s.run = None; // a stopped run carries on by itself (the bot picks the idea up again): only chats need a hand
    put(&st.db.lock(), LIVE, (!s.sessions.is_empty()).then_some(&s));
}

/// At start-up: chats left open by a PC that went off become a "carry on" offer.
pub fn on_start(c: &Connection) {
    let left: Option<Shelf> = db::get_setting(c, LIVE).and_then(|v| serde_json::from_str(&v).ok());
    put(c, LIVE, None);
    if get(c).is_some() {
        return; // the owner's own pause wins
    }
    if let Some(s) = left.filter(|s| !s.sessions.is_empty()) {
        put(c, KEY, Some(&s));
    }
}

fn set_paused(app: &AppHandle, paused: bool) {
    let st = app.state::<AppState>();
    st.rt.lock().paused = paused;
    let _ = db::set_setting(&st.db.lock(), "paused", if paused { "1" } else { "0" });
    st.wake.notify_one();
    crate::tray::refresh(app);
}

/// Stop everything and remember it: the PC can be switched off right after.
pub fn pause_all(app: &AppHandle) -> Result<Shelf, String> {
    let st = app.state::<AppState>();
    let mut s = now_open(app);
    s.by_owner = true;
    // pressed twice (or after a restart that left an offer): keep what was there, add what is open now
    if let Some(old) = get(&st.db.lock()) {
        for o in old.sessions {
            if !s.sessions.iter().any(|x| x.id == o.id) {
                s.sessions.push(o);
            }
        }
        s.run = s.run.or(old.run);
        if old.by_owner {
            s.was_paused = old.was_paused; // the bot is paused now only because of that first press
        }
    }
    set_paused(app, true);
    if let Some(c) = st.rt.lock().cancel.clone() {
        c.notify_one();
    }
    put(&st.db.lock(), KEY, Some(&s));
    for x in &s.sessions {
        let _ = crate::remote::stop(app, x.id);
    }
    put(&st.db.lock(), LIVE, None);
    *LAST.lock() = String::new();
    let what = s.sessions.iter().map(|x| format!("#{}", x.id)).chain(s.run.iter().map(|r| format!("บอท #{}", r.0))).collect::<Vec<_>>().join(", ");
    db::log_event(&st.db.lock(), "pause_all", None, &format!("พักทั้งหมดก่อนปิดเครื่อง{}", if what.is_empty() { String::new() } else { format!(" ({what})") }));
    st.push_log(format!("⏸ พักทั้งหมดแล้ว — ปิดเครื่องได้ · กด ▶ ทำต่อ เมื่อกลับมา{}", if what.is_empty() { String::new() } else { format!(" ({what})") }));
    let _ = tauri::Emitter::emit(app, "state-changed", ());
    Ok(s)
}

/// Reopen what was paused and carry on. Returns how many chats were reopened.
pub fn resume_all(app: &AppHandle) -> Result<usize, String> {
    let st = app.state::<AppState>();
    let Some(s) = get(&st.db.lock()) else { return Err("ไม่มีงานที่พักไว้".into()) };
    put(&st.db.lock(), KEY, None);
    if s.by_owner {
        set_paused(app, s.was_paused);
    }
    let mut n = 0;
    let mut errs = Vec::new();
    for x in &s.sessions {
        let r = if x.working { crate::remote::tell(app, x.id, CARRY_ON, &x.ai) } else { crate::remote::start_with(app, x.id, &x.ai) };
        match r {
            Ok(()) => n += 1,
            Err(e) => errs.push(format!("#{}: {e}", x.id)),
        }
    }
    db::log_event(&st.db.lock(), "resume_all", None, &format!("ทำต่อจากที่พักไว้ — เปิดแชต {n} อัน"));
    st.push_log(format!("▶ ทำต่อจากที่พักไว้ — เปิดแชต {n} อัน{}", if s.by_owner && !s.was_paused { " และให้บอททำงานต่อ" } else { "" }));
    let _ = tauri::Emitter::emit(app, "state-changed", ());
    if errs.is_empty() { Ok(n) } else { Err(format!("เปิดต่อไม่ได้บางอัน: {}", errs.join(" · "))) }
}

/// The owner does not want to carry on: forget it (an owner's pause leaves the bot as it was before).
pub fn dismiss(app: &AppHandle) {
    let st = app.state::<AppState>();
    if let Some(s) = get(&st.db.lock()) {
        if s.by_owner {
            set_paused(app, s.was_paused);
        }
    }
    put(&st.db.lock(), KEY, None);
    let _ = tauri::Emitter::emit(app, "state-changed", ());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chats_left_open_by_a_pc_switched_off_are_offered_to_carry_on() {
        let path = std::env::temp_dir().join(format!("ia-shelf-{}-{}.db", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let c = db::open(&path).unwrap();
        // nothing was open: nothing to offer
        on_start(&c);
        assert!(get(&c).is_none());
        // a chat was open when the PC went off
        let open = Shelf { ts: 1, sessions: vec![Saved { id: 24, title: "x".into(), working: true, ai: ChatAi::default() }], ..Default::default() };
        put(&c, LIVE, Some(&open));
        on_start(&c);
        let s = get(&c).unwrap();
        assert_eq!((s.sessions.len(), s.sessions[0].id, s.by_owner), (1, 24, false));
        // the left-over list is used once
        assert!(db::get_setting(&c, LIVE).unwrap_or_default().is_empty());
        // the owner's own pause is not replaced by what an update restart left behind
        let mine = Shelf { by_owner: true, sessions: vec![Saved { id: 7, ..Default::default() }], ..Default::default() };
        put(&c, KEY, Some(&mine));
        put(&c, LIVE, Some(&open));
        on_start(&c);
        assert_eq!(get(&c).unwrap().sessions[0].id, 7);
        let _ = std::fs::remove_file(&path);
    }
}
