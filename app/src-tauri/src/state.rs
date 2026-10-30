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
    /// when the stage is stopped for taking too long (unix seconds)
    pub deadline: i64,
    /// position of this stage in the idea's recipe, e.g. 2 of 5
    pub stage_no: i64,
    pub stage_total: i64,
    /// last time the agent produced anything (a line, a tool call, a thought)
    pub last_activity: i64,
    pub last_line: String,
    pub provider: String,
    pub model: String,
}

#[derive(Default)]
pub struct Runtime {
    pub paused: bool,
    pub turbo_until: i64,
    pub snap: Option<Snapshot>,
    pub snap_note: Option<String>,
    pub last_fetch: i64,
    pub user_active: bool,
    pub decision: Decision,
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
