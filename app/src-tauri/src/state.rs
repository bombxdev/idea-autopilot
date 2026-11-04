//! Shared app state.
use parking_lot::Mutex;
use rusqlite::Connection;
use serde::Serialize;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Notify;

use crate::governor::Decision;
use crate::usage::Snapshot;

#[derive(Debug, Clone, Serialize)]
pub struct CurrentRun {
    pub run_id: i64,
    pub idea_id: i64,
    pub stage: String,
    pub title: String,
    pub started: i64,
    pub forced: bool,
    /// running in full-power mode (power.rs)
    pub power: bool,
    /// when the stage is stopped for taking too long (unix seconds)
    pub deadline: i64,
    /// position of this stage in the idea's recipe, e.g. 2 of 5
    pub stage_no: i64,
    pub stage_total: i64,
    /// last time the agent produced anything (a line, a tool call, a thought)
    pub last_activity: i64,
    pub last_line: String,
    /// language of the live feed for this run (lang.rs code), so the panel speaks the idea's language
    pub lang: String,
    pub provider: String,
    pub model: String,
}

#[derive(Default)]
pub struct Runtime {
    pub paused: bool,
    pub turbo_until: i64,
    pub snap: Option<Snapshot>,
    /// the account the bot works with right now (None = the single AI chosen in settings, as before)
    pub account: Option<crate::accounts::Account>,
    /// latest quota of every usable account, when it was read, and any problem reading it
    pub acct_snaps: std::collections::HashMap<String, Snapshot>,
    pub acct_fetch: std::collections::HashMap<String, i64>,
    pub acct_notes: std::collections::HashMap<String, String>,
    pub acct_allowed: std::collections::HashMap<String, (bool, String)>,
    /// the full governor decision of every usable account (the quota page draws a plan per account)
    pub acct_decisions: std::collections::HashMap<String, Decision>,
    pub snap_note: Option<String>,
    pub last_fetch: i64,
    pub user_active: bool,
    pub decision: Decision,
    /// what the governor would say with the thrift rules lifted — decides whether a full-power idea may run now
    pub power_decision: Decision,
    pub current: Option<CurrentRun>,
    pub cancel: Option<Arc<Notify>>,
    pub live_log: VecDeque<String>,
    pub force: Option<i64>,
    pub cooldown_until: i64,
    pub reserve_per_hour: f64,
    pub reserve_learned: bool,
    pub last_learn: i64,
    pub keep_awake: bool,
    /// standalone | leader | follower (see sync.rs)
    pub role: String,
    pub sync_pulled: i64,
    pub sync_published: i64,
    pub sync_heartbeat: i64,
    pub sync_live: i64,
    pub live_sig: String,
    pub sync_dirty: bool,
    pub snap_pending: bool,
    /// when a quota snapshot was last written to the database (the self-meter is read every tick)
    pub last_snap_log: i64,
    pub policy: crate::tuner::Policy,
    /// idea id -> its named Remote Control session (see remote.rs)
    pub sessions: std::collections::BTreeMap<i64, crate::remote::RemoteInfo>,
    /// idea id -> until when a remote session holds it (the bot skips held ideas)
    pub holds: std::collections::BTreeMap<i64, i64>,
}

pub struct AppState {
    pub root: PathBuf,
    pub ws: PathBuf,
    pub db: Mutex<Connection>,
    pub rt: Mutex<Runtime>,
    pub wake: Notify,
}

impl AppState {
    pub fn push_log(&self, line: String) {
        let mut rt = self.rt.lock();
        for l in line.lines() {
            rt.live_log.push_back(l.to_string());
        }
        while rt.live_log.len() > 400 {
            rt.live_log.pop_front();
        }
    }
}

#[cfg(test)]
mod lock_rules {
    /// `a.lock().x() && a.lock().y()` keeps the first guard alive while the second asks for the same
    /// (non-reentrant) lock: the app freezes for good. Take the lock once and ask both questions of it.
    #[test]
    fn no_expression_takes_the_same_lock_twice() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut bad = Vec::new();
        for f in std::fs::read_dir(dir).unwrap().flatten() {
            let p = f.path();
            if p.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            for (i, l) in std::fs::read_to_string(&p).unwrap().lines().enumerate() {
                let code = l.split("//").next().unwrap_or("");
                for key in ["rt.lock()", "db.lock()"] {
                    if code.matches(key).count() >= 2 && (code.contains("&&") || code.contains("||")) {
                        bad.push(format!("{}:{}: {}", p.file_name().unwrap().to_string_lossy(), i + 1, l.trim()));
                    }
                }
            }
        }
        assert!(bad.is_empty(), "same lock twice in one expression:\n{}", bad.join("\n"));
    }
}
