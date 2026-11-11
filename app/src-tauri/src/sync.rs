//! Multi-machine sync through a shared folder (cloud drive or a git repo).
//!
//! One machine is the *leader*: it runs the bot and publishes its state. Other machines are
//! *followers*: they show a read-only mirror and send their actions (new ideas, answers, …) as
//! command files the leader executes. Moving the main machine = "takeover": restore the
//! published state locally and claim leadership; the old leader steps down on its next tick.
//!
//! Layout of `<sync_dir>`:
//!   leader.json                 who leads + heartbeat
//!   state/*.json                ideas, questions, runs… (small text files, git-friendly)
//!   state/snapshots/YYYY-MM.csv quota history
//!   workspace/                  idea folders without heavy build dirs, plus knowledge/
//!   commands/*.json             actions from followers, deleted once executed
//!   inbox/                      drop .md/.txt ideas here from any device
//!   brain/                      portable "brain" export
use anyhow::{Context, Result};
use rusqlite::{types::ValueRef, Connection};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

use crate::db;
use crate::state::AppState;

const TABLES: &[&str] = &["ideas", "questions", "directives", "runs", "calendar_overrides", "digests", "settings"];
/// settings that belong to one machine and must not travel
const LOCAL_SETTINGS: &[&str] = &["autostart", "hotkey", "sync_dir", "brain_dir", "brain_personal", "brain_work", "extra_inbox", "digest_day", "seen_version"];

/// Settings that stay on this machine: machine paths, plus every secret (bot tokens, chat ids…).
fn is_local_setting(key: &str) -> bool {
    LOCAL_SETTINGS.contains(&key) || key.starts_with("remote_sid_") || key.contains("token") || (key.starts_with("telegram_") && key != "telegram_questions")
}
const SKIP_DIRS: &[&str] = &["node_modules", "target", ".venv", "venv", "__pycache__", ".git", ".next", ".cache"];
const PUBLISH_EVERY: i64 = 10 * 60;
const HEARTBEAT_EVERY: i64 = 2 * 60;
pub const STALE_LEADER: i64 = 30 * 60;
/// live status for the phone app is refreshed at most this often
const LIVE_EVERY: i64 = 2 * 60;

pub fn machine() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "this-pc".into())
}

pub fn sync_dir(st: &AppState) -> Option<PathBuf> {
    db::get_setting(&st.db.lock(), "sync_dir").filter(|s| !s.trim().is_empty()).map(PathBuf::from)
}

fn read_json(p: &Path) -> Option<Value> {
    std::fs::read_to_string(p).ok().and_then(|s| serde_json::from_str(&s).ok())
}

/// Write only when content changed (keeps git history and cloud sync quiet).
fn write_if_changed(p: &Path, content: &str) -> Result<bool> {
    if std::fs::read_to_string(p).ok().as_deref() == Some(content) {
        return Ok(false);
    }
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(p, content)?;
    Ok(true)
}

// ---------- role ----------

#[derive(Debug, Clone, serde::Serialize)]
pub struct Role {
    pub mode: String, // standalone | leader | follower
    pub machine: String,
    pub leader: Option<String>,
    pub heartbeat: Option<i64>,
    pub sync_dir: Option<String>,
    pub git: bool,
}

pub fn role(st: &AppState) -> Role {
    let me = machine();
    let Some(dir) = sync_dir(st) else {
        return Role { mode: "standalone".into(), machine: me, leader: None, heartbeat: None, sync_dir: None, git: false };
    };
    let l = read_json(&dir.join("leader.json"));
    let leader = l.as_ref().and_then(|v| v["machine"].as_str().map(String::from));
    let mode = match &leader {
        Some(m) if *m != me => "follower",
        _ => "leader",
    };
    Role {
        mode: mode.into(),
        machine: me,
        leader,
        heartbeat: l.and_then(|v| v["heartbeat"].as_i64()),
        git: dir.join(".git").exists(),
        sync_dir: Some(dir.to_string_lossy().into()),
    }
}

/// Read the role fresh from leader.json so a switch takes effect immediately.
pub fn is_follower(st: &AppState) -> bool {
    role(st).mode == "follower"
}

/// A follower turns mutations into command files for the leader.
pub fn forward(st: &AppState, cmd: &str, args: Value) -> Option<Result<(), String>> {
    if !is_follower(st) {
        return None;
    }
    let dir = sync_dir(st)?.join("commands");
    let name = format!("{}-{}-{}.json", machine(), chrono::Utc::now().timestamp_millis(), cmd);
    let body = json!({ "cmd": cmd, "args": args, "from": machine(), "ts": db::now() });
    Some(
        std::fs::create_dir_all(&dir)
            .and_then(|_| std::fs::write(dir.join(name), body.to_string()))
            .map_err(|e| e.to_string()),
    )
}

// ---------- state dump / restore ----------

fn row_json(row: &rusqlite::Row, cols: &[String]) -> rusqlite::Result<Value> {
    let mut m = Map::new();
    for (i, c) in cols.iter().enumerate() {
        let v = match row.get_ref(i)? {
            ValueRef::Null => Value::Null,
            ValueRef::Integer(n) => json!(n),
            ValueRef::Real(f) => json!(f),
            ValueRef::Text(t) => json!(String::from_utf8_lossy(t)),
            ValueRef::Blob(_) => Value::Null,
        };
        m.insert(c.clone(), v);
    }
    Ok(Value::Object(m))
}

fn dump_table(c: &Connection, table: &str, extra: &str) -> Result<Vec<Value>> {
    let mut st = c.prepare(&format!("SELECT * FROM {table} {extra}"))?;
    let cols: Vec<String> = st.column_names().iter().map(|s| s.to_string()).collect();
    let rows = st.query_map([], |r| row_json(r, &cols))?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn publish_state(c: &Connection, dir: &Path) -> Result<()> {
    let state = dir.join("state");
    for t in TABLES {
        let mut rows = dump_table(c, t, "")?;
        if *t == "settings" {
            rows.retain(|r| !is_local_setting(r["key"].as_str().unwrap_or("")));
        }
        write_if_changed(&state.join(format!("{t}.json")), &serde_json::to_string_pretty(&rows)?)?;
    }
    // the AIs and checked models the phone app can choose from
    write_if_changed(&state.join("models.json"), &serde_json::to_string_pretty(&crate::model_check::options_for_phone(c))?)?;
    let events = dump_table(c, "events", "ORDER BY id DESC LIMIT 500")?;
    write_if_changed(&state.join("events.json"), &serde_json::to_string_pretty(&events)?)?;
    // quota history, one CSV per month (only the current month changes)
    let snaps = dump_table(c, "usage_snapshots", "ORDER BY ts")?;
    let mut months: std::collections::BTreeMap<String, String> = Default::default();
    for s in snaps {
        let ts = s["ts"].as_i64().unwrap_or(0);
        let m = chrono::DateTime::from_timestamp(ts, 0).map(|d| d.format("%Y-%m").to_string()).unwrap_or_default();
        let line = format!(
            "{},{},{},{},{},{},{},{}\n",
            ts,
            s["five_pct"],
            s["five_reset"].as_i64().map(|x| x.to_string()).unwrap_or_default(),
            s["week_pct"],
            s["week_reset"].as_i64().map(|x| x.to_string()).unwrap_or_default(),
            s["source"].as_str().unwrap_or(""),
            s["bot_running"],
            s["user_active"]
        );
        months.entry(m).or_insert_with(|| "ts,five_pct,five_reset,week_pct,week_reset,source,bot_running,user_active\n".into()).push_str(&line);
    }
    for (m, csv) in months {
        write_if_changed(&state.join("snapshots").join(format!("{m}.csv")), &csv)?;
    }
    Ok(())
}

/// Rebuild a database from published state files.
pub fn restore_state(c: &Connection, dir: &Path, keep_local_settings: bool) -> Result<()> {
    let state = dir.join("state");
    // replacing whole tables: children and parents are rewritten together, so suspend FK checks
    c.execute_batch("PRAGMA foreign_keys = OFF; BEGIN")?;
    let res = (|| -> Result<()> {
        for t in TABLES.iter().chain(["events"].iter()) {
            let rows = read_json(&state.join(format!("{t}.json"))).and_then(|v| v.as_array().cloned()).unwrap_or_default();
            if *t == "settings" && keep_local_settings {
                let keys: Vec<String> = rows.iter().filter_map(|r| r["key"].as_str().map(String::from)).collect();
                for k in keys {
                    c.execute("DELETE FROM settings WHERE key=?1", [k])?;
                }
            } else {
                c.execute(&format!("DELETE FROM {t}"), [])?;
            }
            for r in rows {
                let Some(o) = r.as_object() else { continue };
                let cols: Vec<&String> = o.keys().collect();
                let sql = format!(
                    "INSERT OR REPLACE INTO {t}({}) VALUES({})",
                    cols.iter().map(|c| c.as_str()).collect::<Vec<_>>().join(","),
                    (1..=cols.len()).map(|i| format!("?{i}")).collect::<Vec<_>>().join(",")
                );
                let vals: Vec<rusqlite::types::Value> = cols
                    .iter()
                    .map(|k| match &o[k.as_str()] {
                        Value::Null => rusqlite::types::Value::Null,
                        Value::Number(n) if n.is_i64() => rusqlite::types::Value::Integer(n.as_i64().unwrap()),
                        Value::Number(n) => rusqlite::types::Value::Real(n.as_f64().unwrap_or(0.0)),
                        Value::String(s) => rusqlite::types::Value::Text(s.clone()),
                        other => rusqlite::types::Value::Text(other.to_string()),
                    })
                    .collect();
                c.execute(&sql, rusqlite::params_from_iter(vals))?;
            }
        }
        c.execute("DELETE FROM usage_snapshots", [])?;
        if let Ok(entries) = std::fs::read_dir(state.join("snapshots")) {
            let mut files: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
            files.sort();
            for f in files {
                let text = std::fs::read_to_string(&f).unwrap_or_default();
                for line in text.lines().skip(1) {
                    let p: Vec<&str> = line.split(',').collect();
                    if p.len() < 8 {
                        continue;
                    }
                    let opt = |s: &str| s.parse::<i64>().ok();
                    c.execute(
                        "INSERT INTO usage_snapshots(ts,five_pct,five_reset,week_pct,week_reset,source,bot_running,user_active) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                        rusqlite::params![
                            p[0].parse::<i64>().unwrap_or(0),
                            p[1].parse::<f64>().unwrap_or(0.0),
                            opt(p[2]),
                            p[3].parse::<f64>().unwrap_or(0.0),
                            opt(p[4]),
                            p[5],
                            p[6].parse::<i64>().unwrap_or(0),
                            p[7].parse::<i64>().unwrap_or(0)
                        ],
                    )?;
                }
            }
        }
        Ok(())
    })();
    let out = match res {
        Ok(()) => c.execute_batch("COMMIT").map_err(Into::into),
        Err(e) => {
            let _ = c.execute_batch("ROLLBACK");
            Err(e)
        }
    };
    let _ = c.execute_batch("PRAGMA foreign_keys = ON");
    out
}

// ---------- folder mirroring ----------

/// Copy new/changed files from `src` to `dst`, skipping heavy build folders.
pub fn mirror(src: &Path, dst: &Path) -> Result<usize> {
    let mut n = 0;
    let Ok(entries) = std::fs::read_dir(src) else { return Ok(0) };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let (sp, dp) = (e.path(), dst.join(&name));
        let Ok(meta) = e.metadata() else { continue };
        if meta.is_dir() {
            if SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            n += mirror(&sp, &dp)?;
        } else {
            let same = std::fs::metadata(&dp)
                .map(|d| d.len() == meta.len() && d.modified().ok() >= meta.modified().ok())
                .unwrap_or(false);
            if !same && meta.len() < 50 * 1024 * 1024 {
                std::fs::create_dir_all(dst)?;
                std::fs::copy(&sp, &dp)?;
                n += 1;
            }
        }
    }
    Ok(n)
}

// ---------- git ----------

fn git(dir: &Path, args: &[&str]) -> Result<String> {
    git_run(Some(dir), args)
}

/// Run git (inside `dir` when given) without a console window.
pub fn git_run(dir: Option<&Path>, args: &[&str]) -> Result<String> {
    let mut c = std::process::Command::new("git");
    if let Some(d) = dir {
        c.arg("-C").arg(d);
    }
    c.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    let out = c.output().context("git ไม่ได้ติดตั้ง?")?;
    if !out.status.success() {
        anyhow::bail!("git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into())
}

pub fn git_pull(dir: &Path) -> Result<()> {
    // a brand-new empty remote has nothing to pull yet
    if git(dir, &["rev-parse", "--abbrev-ref", "@{u}"]).is_err() {
        return Ok(());
    }
    git(dir, &["pull", "--rebase", "--autostash", "-q"]).map(|_| ())
}

fn git_push(dir: &Path) -> Result<()> {
    git_commit_push(dir, &format!("sync from {}", machine()))
}

/// Commit everything that changed and push it (nothing to commit = nothing to do).
pub fn git_commit_push(dir: &Path, message: &str) -> Result<()> {
    git(dir, &["add", "-A"])?;
    if git(dir, &["status", "--porcelain"])?.trim().is_empty() {
        return Ok(());
    }
    git(dir, &["commit", "-q", "-m", message])?;
    git(dir, &["push", "-q", "-u", "origin", "HEAD"]).map(|_| ())
}

// ---------- periodic work ----------

fn write_leader(dir: &Path) -> Result<()> {
    let v = json!({ "machine": machine(), "heartbeat": db::now(), "version": env!("CARGO_PKG_VERSION") });
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("leader.json"), serde_json::to_string_pretty(&v)?)?;
    Ok(())
}

/// Called from the scheduler loop.
pub fn tick(app: &AppHandle) -> Result<()> {
    let st = app.state::<AppState>();
    let Some(dir) = sync_dir(&st) else {
        st.rt.lock().role = "standalone".into();
        return Ok(());
    };
    let now = db::now();
    let git_mode = dir.join(".git").exists();
    let (last_pull, last_pub, last_beat) = {
        let rt = st.rt.lock();
        (rt.sync_pulled, rt.sync_published, rt.sync_heartbeat)
    };
    // an open remote session means the owner is steering from the phone: pick commands up faster
    let live_session = !st.rt.lock().sessions.is_empty();
    // (a phone command waits for this pull: with a chat open, every 20 s — ISSUE-064)
    if git_mode && now - last_pull >= if live_session { 20 } else { 2 * 60 } {
        if let Err(e) = git_pull(&dir) {
            st.push_log(format!("[sync] {e:#}"));
        }
        st.rt.lock().sync_pulled = now;
    }

    let r = role(&st);
    let was = std::mem::replace(&mut st.rt.lock().role, r.mode.clone());
    if r.mode == "leader" && r.leader.is_none() {
        write_leader(&dir)?;
    }
    if was == "leader" && r.mode == "follower" {
        st.push_log(format!("[sync] เครื่อง {} เป็นเครื่องหลักแล้ว — เครื่องนี้เปลี่ยนเป็นเครื่องรอง", r.leader.clone().unwrap_or_default()));
        if let Some(c) = st.rt.lock().cancel.clone() {
            c.notify_one();
        }
    }

    if r.mode == "leader" {
        if now - last_beat >= HEARTBEAT_EVERY {
            write_leader(&dir)?;
            st.rt.lock().sync_heartbeat = now;
        }
        // run followers' commands
        if let Ok(entries) = std::fs::read_dir(dir.join("commands")) {
            let mut files: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
            files.sort();
            for f in files {
                if let Some(v) = read_json(&f) {
                    let cmd = v["cmd"].as_str().unwrap_or("");
                    let res = crate::api::dispatch(app, cmd, v["args"].clone());
                    // show the phone what became of it at once, not at the next live interval
                    st.rt.lock().sync_live = 0;
                    st.push_log(format!("[sync] คำสั่งจาก {}: {cmd} {}", v["from"].as_str().unwrap_or("?"), if res.is_ok() { "✓" } else { "✕" }));
                }
                let _ = std::fs::remove_file(&f);
            }
        }
        let dirty = std::mem::take(&mut st.rt.lock().sync_dirty);
        if dirty || now - last_pub >= PUBLISH_EVERY {
            publish_state(&st.db.lock(), &dir)?;
            mirror(&st.ws.join("ideas"), &dir.join("workspace").join("ideas"))?;
            mirror(&st.ws.join("knowledge"), &dir.join("workspace").join("knowledge"))?;
            st.rt.lock().sync_published = now;
            if git_mode {
                if let Err(e) = git_push(&dir) {
                    st.push_log(format!("[sync] {e:#}"));
                }
            }
        }
        // live status for the phone app: written/pushed only when something actually changed
        // wait until the governor has produced a real decision (first tick runs sync before it)
        let decided = !st.rt.lock().decision.code.is_empty();
        if decided && now - st.rt.lock().sync_live >= if live_session { 10 } else { LIVE_EVERY } {
            st.rt.lock().sync_live = now;
            let mut v = crate::scheduler::state_json(&st);
            let log: Vec<String> = st.rt.lock().live_log.iter().rev().take(25).rev().cloned().collect();
            v["live_log"] = json!(log);
            v["open_questions"] = json!(db::questions(&st.db.lock(), None, true).map(|q| q.len()).unwrap_or(0));
            // the skills the phone may call with /name (ISSUE-073)
            v["slash"] = json!(crate::skills::slash_list(&st.db.lock()));
            let sessions: String = st.rt.lock().sessions.values().map(|s| format!("{}:{}:{}:{};", s.idea_id, s.status, s.feed.last().map(|f| f.ts).unwrap_or(0), s.ai)).collect();
            let sig = format!(
                "{}|{}|{}|{}|{}|{}",
                v["decision"]["code"], v["current"]["run_id"], v["paused"], log.last().cloned().unwrap_or_default(), v["open_questions"], sessions
            );
            if sig != st.rt.lock().live_sig {
                st.rt.lock().live_sig = sig;
                v["ts"] = json!(now);
                v["machine"] = json!(machine());
                write_if_changed(&dir.join("state").join("live.json"), &serde_json::to_string_pretty(&v)?)?;
                if git_mode {
                    if let Err(e) = git_push(&dir) {
                        st.push_log(format!("[sync] {e:#}"));
                    }
                }
            }
        }
    } else {
        // follower: refresh the read-only mirror when the leader published something new
        let stamp = std::fs::metadata(dir.join("state").join("ideas.json"))
            .and_then(|m| m.modified())
            .ok()
            .map(|t| chrono::DateTime::<chrono::Utc>::from(t).timestamp())
            .unwrap_or(0);
        if stamp > last_pub {
            restore_state(&st.db.lock(), &dir, true)?;
            mirror(&dir.join("workspace").join("ideas"), &st.ws.join("ideas"))?;
            mirror(&dir.join("workspace").join("knowledge"), &st.ws.join("knowledge"))?;
            st.rt.lock().sync_published = stamp;
        }
        if git_mode && std::fs::read_dir(dir.join("commands")).map(|d| d.count() > 0).unwrap_or(false) {
            if let Err(e) = git_push(&dir) {
                st.push_log(format!("[sync] {e:#}"));
            }
        }
    }
    Ok(())
}

/// Make this machine the leader: restore the latest published state and claim the role.
pub fn takeover(app: &AppHandle) -> Result<()> {
    let st = app.state::<AppState>();
    let dir = sync_dir(&st).context("ยังไม่ได้ตั้งโฟลเดอร์ sync")?;
    if dir.join(".git").exists() {
        let _ = git_pull(&dir);
    }
    if dir.join("state").join("ideas.json").exists() {
        restore_state(&st.db.lock(), &dir, true)?;
        mirror(&dir.join("workspace").join("ideas"), &st.ws.join("ideas"))?;
        mirror(&dir.join("workspace").join("knowledge"), &st.ws.join("knowledge"))?;
    }
    write_leader(&dir)?;
    {
        let mut rt = st.rt.lock();
        rt.role = "leader".into();
        rt.sync_dirty = true;
    }
    db::log_event(&st.db.lock(), "sync", None, &format!("ย้ายเครื่องหลักมาที่ {}", machine()));
    if dir.join(".git").exists() {
        let _ = git_push(&dir);
    }
    st.wake.notify_one();
    Ok(())
}

pub fn ensure_layout(dir: &Path) -> Result<()> {
    for d in ["state", "workspace", "commands", "inbox", "brain"] {
        std::fs::create_dir_all(dir.join(d))?;
    }
    // company material (brain/work) never leaves this machine through the sync folder
    let gi = dir.join(".gitignore");
    let cur = std::fs::read_to_string(&gi).unwrap_or_default();
    if !cur.lines().any(|l| l.trim() == "brain/work/") {
        let sep = if cur.is_empty() || cur.ends_with('\n') { "" } else { "\n" };
        std::fs::write(&gi, format!("{cur}{sep}brain/work/\n"))?;
    }
    write_if_changed(
        &dir.join("README.md"),
        "# Idea Autopilot sync folder\n\nโฟลเดอร์นี้ใช้ sync ระหว่างเครื่อง (cloud drive หรือ git repo ส่วนตัว)\n\n\
         - `leader.json` เครื่องหลักที่รันบอท\n- `state/` ข้อมูลไอเดีย/คำถาม/สถิติ (ไฟล์ข้อความ)\n\
         - `workspace/` รายงานและงานของแต่ละไอเดีย\n- `commands/` คำสั่งจากเครื่องรอง\n\
         - `inbox/` วางไฟล์ .md/.txt เพื่อจดไอเดียจากอุปกรณ์ไหนก็ได้\n- `brain/` สมองพกพา (ส่วนตัว / งาน)\n",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_roundtrip() {
        let tmp = std::env::temp_dir().join(format!("ia-sync-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let a = db::open(&tmp.join("a.db")).unwrap();
        let id = db::insert_idea(&a, "idea-0001", "ทดสอบ", "body", 1, false).unwrap();
        db::set_scope(&a, id, "work").unwrap();
        db::insert_question(&a, id, "ข้อไหน?", &["ก".into(), "ข".into()], Some("ก")).unwrap();
        db::set_setting(&a, "schedule", "{}").unwrap();
        db::set_setting(&a, "sync_dir", "X:/local-only").unwrap();
        db::set_setting(&a, "telegram_token", "SECRET-1").unwrap();
        db::set_setting(&a, "telegram_chat", "SECRET-2").unwrap();
        db::insert_snapshot_code(&a, &crate::usage::Snapshot { ts: 1_790_000_000, five_pct: 3.0, five_reset: None, week_pct: 40.0, week_reset: Some(1_790_500_000), source: "t".into() }, true, false, None).unwrap();
        publish_state(&a, &tmp).unwrap();

        let b = db::open(&tmp.join("b.db")).unwrap();
        db::set_setting(&b, "sync_dir", "Y:/mine").unwrap();
        // the target already has its own ideas with child rows (FK) — restore must replace them
        let old = db::insert_idea(&b, "idea-0009", "เก่า", "", 0, false).unwrap();
        db::insert_question(&b, old, "q", &[], None).unwrap();
        db::add_directive(&b, old, "d").unwrap();
        restore_state(&b, &tmp, true).unwrap();
        let ideas = db::list_ideas(&b).unwrap();
        assert_eq!(ideas.len(), 1);
        assert_eq!(ideas[0].title, "ทดสอบ");
        assert_eq!(ideas[0].scope, "work", "the set an idea belongs to travels with it");
        assert_eq!(db::questions(&b, None, true).unwrap()[0].options, vec!["ก", "ข"]);
        assert_eq!(db::snapshots_since(&b, 0).unwrap()[0].week_pct, 40.0);
        assert_eq!(db::get_setting(&b, "schedule").as_deref(), Some("{}"));
        assert_eq!(db::get_setting(&b, "sync_dir").as_deref(), Some("Y:/mine"), "local settings must not travel");
        let published = std::fs::read_to_string(tmp.join("state").join("settings.json")).unwrap();
        assert!(!published.contains("SECRET"), "tokens must never be published");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
