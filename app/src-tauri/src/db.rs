//! SQLite storage: usage snapshots, ideas, runs, questions, calendar, digests.
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::usage::Snapshot;

pub fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA foreign_keys = ON;
        CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS usage_snapshots (
            id INTEGER PRIMARY KEY,
            ts INTEGER NOT NULL,
            five_pct REAL NOT NULL,
            five_reset INTEGER,
            week_pct REAL NOT NULL,
            week_reset INTEGER,
            source TEXT NOT NULL,
            bot_running INTEGER NOT NULL DEFAULT 0,
            user_active INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS ix_snap_ts ON usage_snapshots(ts);
        CREATE TABLE IF NOT EXISTS ideas (
            id INTEGER PRIMARY KEY,
            slug TEXT NOT NULL UNIQUE,
            title TEXT NOT NULL,
            body TEXT NOT NULL,
            intents TEXT NOT NULL DEFAULT '[]',
            recipe TEXT NOT NULL DEFAULT '[]',
            stage_idx INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'inbox',
            priority INTEGER NOT NULL DEFAULT 0,
            research_only INTEGER NOT NULL DEFAULT 0,
            summary TEXT NOT NULL DEFAULT '',
            verdict TEXT,
            run_cmd TEXT,
            knowledge INTEGER NOT NULL DEFAULT 0,
            needs TEXT NOT NULL DEFAULT '[]',
            repeat_count INTEGER NOT NULL DEFAULT 0,
            fail_count INTEGER NOT NULL DEFAULT 0,
            sort REAL NOT NULL DEFAULT 0,
            created INTEGER NOT NULL,
            updated INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS questions (
            id INTEGER PRIMARY KEY,
            idea_id INTEGER NOT NULL REFERENCES ideas(id),
            question TEXT NOT NULL,
            options TEXT NOT NULL DEFAULT '[]',
            guess TEXT,
            answer TEXT,
            created INTEGER NOT NULL,
            answered INTEGER
        );
        CREATE TABLE IF NOT EXISTS directives (
            id INTEGER PRIMARY KEY,
            idea_id INTEGER NOT NULL REFERENCES ideas(id),
            text TEXT NOT NULL,
            created INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS runs (
            id INTEGER PRIMARY KEY,
            idea_id INTEGER NOT NULL REFERENCES ideas(id),
            stage TEXT NOT NULL,
            model TEXT NOT NULL,
            started INTEGER NOT NULL,
            ended INTEGER,
            status TEXT NOT NULL DEFAULT 'running',
            input_tokens INTEGER NOT NULL DEFAULT 0,
            output_tokens INTEGER NOT NULL DEFAULT 0,
            cache_read INTEGER NOT NULL DEFAULT 0,
            cache_write INTEGER NOT NULL DEFAULT 0,
            cost_usd REAL NOT NULL DEFAULT 0,
            num_turns INTEGER NOT NULL DEFAULT 0,
            session_id TEXT,
            week_before REAL,
            week_after REAL,
            five_before REAL,
            five_after REAL,
            error TEXT
        );
        CREATE TABLE IF NOT EXISTS calendar_overrides (
            date TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            note TEXT NOT NULL DEFAULT ''
        );
        CREATE TABLE IF NOT EXISTS digests (
            id INTEGER PRIMARY KEY,
            ts INTEGER NOT NULL,
            body TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS events (
            id INTEGER PRIMARY KEY,
            ts INTEGER NOT NULL,
            kind TEXT NOT NULL,
            idea_id INTEGER,
            text TEXT NOT NULL
        );
        -- skill library (files live in data/skills/<slug>/SKILL.md); see skills.rs
        CREATE TABLE IF NOT EXISTS skills (
            slug TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            family TEXT NOT NULL DEFAULT '',
            enabled INTEGER NOT NULL DEFAULT 1,
            pinned INTEGER NOT NULL DEFAULT 0,
            trust TEXT NOT NULL DEFAULT 'local',
            source TEXT NOT NULL DEFAULT '',
            sha256 TEXT NOT NULL DEFAULT '',
            commit_sha TEXT NOT NULL DEFAULT '',
            license TEXT NOT NULL DEFAULT '',
            installed INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS skill_runs (
            run_id INTEGER NOT NULL,
            slug TEXT NOT NULL,
            used INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (run_id, slug)
        );
        "#,
    )?;
    // migrations for columns added after the first release
    // the owner's requests as a checklist: "" waiting · done (the AI says so) · ok (the owner checked) · no (cannot) — ISSUE-063
    let _ = conn.execute("ALTER TABLE directives ADD COLUMN state TEXT NOT NULL DEFAULT ''", []);
    let _ = conn.execute("ALTER TABLE directives ADD COLUMN note TEXT NOT NULL DEFAULT ''", []);
    let _ = conn.execute("ALTER TABLE directives ADD COLUMN changed INTEGER NOT NULL DEFAULT 0", []);
    let _ = conn.execute("ALTER TABLE runs ADD COLUMN effort TEXT", []);
    let _ = conn.execute("ALTER TABLE runs ADD COLUMN reason TEXT", []);
    let _ = conn.execute("ALTER TABLE runs ADD COLUMN summary TEXT", []);
    let _ = conn.execute("ALTER TABLE runs ADD COLUMN model_id TEXT", []);
    // which agent ran it and the exact model name that was requested ("" = the agent's own default)
    let _ = conn.execute("ALTER TABLE runs ADD COLUMN provider TEXT NOT NULL DEFAULT ''", []);
    let _ = conn.execute("ALTER TABLE runs ADD COLUMN model_used TEXT NOT NULL DEFAULT ''", []);
    // the account (claude01, ...) that ran it, and the quota history of every account for the charts
    let _ = conn.execute("ALTER TABLE runs ADD COLUMN account TEXT NOT NULL DEFAULT ''", []);
    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS account_usage (ts INTEGER NOT NULL, account TEXT NOT NULL, five_pct REAL NOT NULL, week_pct REAL NOT NULL, source TEXT NOT NULL DEFAULT '')",
        [],
    );
    let _ = conn.execute("CREATE INDEX IF NOT EXISTS ix_acct_usage ON account_usage(account, ts)", []);
    // 1 = ran in full-power mode
    let _ = conn.execute("ALTER TABLE runs ADD COLUMN power INTEGER NOT NULL DEFAULT 0", []);
    let _ = conn.execute("ALTER TABLE usage_snapshots ADD COLUMN code TEXT", []);
    // SkillSpector deep check of each skill: status pass|caution|fail, score, engine version, when, which file version, top findings (json)
    for (col, ty) in [("ss_status", "TEXT NOT NULL DEFAULT ''"), ("ss_score", "INTEGER NOT NULL DEFAULT 0"), ("ss_version", "TEXT NOT NULL DEFAULT ''"),
                      ("ss_at", "INTEGER NOT NULL DEFAULT 0"), ("ss_sha", "TEXT NOT NULL DEFAULT ''"), ("ss_issues", "TEXT NOT NULL DEFAULT '[]'")] {
        let _ = conn.execute(&format!("ALTER TABLE skills ADD COLUMN {col} {ty}"), []);
    }
    // which set the idea belongs to: "personal" or "work" (company material, exported and stored apart)
    let _ = conn.execute("ALTER TABLE ideas ADD COLUMN scope TEXT NOT NULL DEFAULT 'personal'", []);
    // board: pinned = the owner's focus (top of its column); fixing = the owner says it still needs work
    // (kept out of the "ready" column until the owner marks it working)
    let _ = conn.execute("ALTER TABLE ideas ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0", []);
    let _ = conn.execute("ALTER TABLE ideas ADD COLUMN fixing INTEGER NOT NULL DEFAULT 0", []);
    Ok(conn)
}

pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

// ---------- settings ----------

pub fn get_setting(c: &Connection, key: &str) -> Option<String> {
    c.query_row("SELECT value FROM settings WHERE key=?1", [key], |r| r.get(0))
        .optional()
        .ok()
        .flatten()
}

pub fn set_setting(c: &Connection, key: &str, value: &str) -> Result<()> {
    c.execute(
        "INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![key, value],
    )?;
    Ok(())
}

// ---------- snapshots ----------

/// `code` = why the bot was running / waiting at that moment (governor decision code).
pub fn insert_snapshot_code(c: &Connection, s: &Snapshot, bot_running: bool, user_active: bool, code: Option<&str>) -> Result<()> {
    c.execute(
        "INSERT INTO usage_snapshots(ts,five_pct,five_reset,week_pct,week_reset,source,bot_running,user_active,code)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![s.ts, s.five_pct, s.five_reset, s.week_pct, s.week_reset, s.source, bot_running, user_active, code],
    )?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct SnapRow {
    pub ts: i64,
    pub five_pct: f64,
    pub week_pct: f64,
    pub week_reset: Option<i64>,
    pub bot_running: bool,
    pub user_active: bool,
}

pub fn snapshots_since(c: &Connection, since: i64) -> Result<Vec<SnapRow>> {
    let mut st = c.prepare(
        "SELECT ts,five_pct,week_pct,week_reset,bot_running,user_active FROM usage_snapshots WHERE ts>=?1 ORDER BY ts",
    )?;
    let rows = st
        .query_map([since], |r| {
            Ok(SnapRow {
                ts: r.get(0)?,
                five_pct: r.get(1)?,
                week_pct: r.get(2)?,
                week_reset: r.get(3)?,
                bot_running: r.get::<_, i64>(4)? != 0,
                user_active: r.get::<_, i64>(5)? != 0,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// One usage increment between two consecutive snapshots, split into user / bot share.
#[derive(Debug, Clone, Serialize)]
pub struct Delta {
    pub ts: i64,
    pub user: f64,
    pub bot: f64,
}

/// Turn consecutive snapshots into attributed weekly-% increments.
/// Gaps over 20 minutes or a weekly reset in between are skipped.
pub fn deltas(snaps: &[SnapRow]) -> Vec<Delta> {
    let mut out = Vec::new();
    for w in snaps.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        if b.ts - a.ts > 20 * 60 || b.ts <= a.ts {
            continue;
        }
        if a.week_reset != b.week_reset && b.week_pct < a.week_pct {
            continue; // weekly reset happened
        }
        let d = (b.week_pct - a.week_pct).max(0.0);
        if d == 0.0 {
            continue;
        }
        let bot_share = if a.bot_running || b.bot_running {
            if a.user_active || b.user_active { 0.5 } else { 1.0 }
        } else {
            0.0
        };
        out.push(Delta { ts: b.ts, user: d * (1.0 - bot_share), bot: d * bot_share });
    }
    out
}

// ---------- ideas ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Idea {
    pub id: i64,
    pub slug: String,
    pub title: String,
    pub body: String,
    pub intents: Vec<String>,
    pub recipe: Vec<String>,
    pub stage_idx: i64,
    pub status: String,
    pub priority: i64,
    pub research_only: bool,
    pub summary: String,
    pub verdict: Option<String>,
    pub run_cmd: Option<String>,
    pub knowledge: bool,
    pub needs: Vec<String>,
    pub repeat_count: i64,
    pub fail_count: i64,
    pub sort: f64,
    pub created: i64,
    pub updated: i64,
    /// "personal" or "work"
    #[serde(default = "personal")]
    pub scope: String,
    /// the owner pinned it to the top of its board column
    #[serde(default)]
    pub pinned: bool,
    /// the owner is still fixing it (asked for changes after it was ready/done); cleared by "works now"
    #[serde(default)]
    pub fixing: bool,
}

fn personal() -> String {
    "personal".into()
}

/// "work" or "personal" (anything else counts as personal).
pub fn norm_scope(s: &str) -> &'static str {
    if s.trim().eq_ignore_ascii_case("work") { "work" } else { "personal" }
}

impl Idea {
    pub fn current_stage(&self) -> Option<String> {
        if self.status == "inbox" {
            return Some("clarify".into());
        }
        self.recipe.get(self.stage_idx as usize).cloned()
    }
}

const IDEA_COLS: &str = "id,slug,title,body,intents,recipe,stage_idx,status,priority,research_only,summary,verdict,run_cmd,knowledge,needs,repeat_count,fail_count,sort,created,updated,scope,pinned,fixing";

fn json_vec(s: String) -> Vec<String> {
    serde_json::from_str(&s).unwrap_or_default()
}

fn idea_from_row(r: &Row) -> rusqlite::Result<Idea> {
    Ok(Idea {
        id: r.get(0)?,
        slug: r.get(1)?,
        title: r.get(2)?,
        body: r.get(3)?,
        intents: json_vec(r.get(4)?),
        recipe: json_vec(r.get(5)?),
        stage_idx: r.get(6)?,
        status: r.get(7)?,
        priority: r.get(8)?,
        research_only: r.get::<_, i64>(9)? != 0,
        summary: r.get(10)?,
        verdict: r.get(11)?,
        run_cmd: r.get(12)?,
        knowledge: r.get::<_, i64>(13)? != 0,
        needs: json_vec(r.get(14)?),
        repeat_count: r.get(15)?,
        fail_count: r.get(16)?,
        sort: r.get(17)?,
        created: r.get(18)?,
        updated: r.get(19)?,
        scope: r.get(20)?,
        pinned: r.get::<_, i64>(21)? != 0,
        fixing: r.get::<_, i64>(22)? != 0,
    })
}

pub fn list_ideas(c: &Connection) -> Result<Vec<Idea>> {
    let mut st = c.prepare(&format!(
        "SELECT {IDEA_COLS} FROM ideas WHERE status!='deleted' ORDER BY priority DESC, sort ASC, id ASC"
    ))?;
    let v = st.query_map([], idea_from_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(v)
}

pub fn get_idea(c: &Connection, id: i64) -> Result<Idea> {
    Ok(c.query_row(&format!("SELECT {IDEA_COLS} FROM ideas WHERE id=?1"), [id], idea_from_row)?)
}

pub fn insert_idea(c: &Connection, slug: &str, title: &str, body: &str, priority: i64, research_only: bool) -> Result<i64> {
    let t = now();
    let sort: f64 = c
        .query_row("SELECT COALESCE(MAX(sort),0)+1 FROM ideas", [], |r| r.get(0))
        .unwrap_or(1.0);
    c.execute(
        "INSERT INTO ideas(slug,title,body,priority,research_only,sort,created,updated) VALUES(?1,?2,?3,?4,?5,?6,?7,?7)",
        params![slug, title, body, priority, research_only, sort, t],
    )?;
    Ok(c.last_insert_rowid())
}

pub fn set_scope(c: &Connection, id: i64, scope: &str) -> Result<()> {
    c.execute("UPDATE ideas SET scope=?2, updated=?3 WHERE id=?1", params![id, norm_scope(scope), now()])?;
    Ok(())
}

/// Board flags, written on their own so a bot run saving the idea never undoes them.
pub fn set_pinned(c: &Connection, id: i64, on: bool) -> Result<()> {
    c.execute("UPDATE ideas SET pinned=?2 WHERE id=?1", params![id, on])?;
    Ok(())
}

pub fn set_fixing(c: &Connection, id: i64, on: bool) -> Result<()> {
    c.execute("UPDATE ideas SET fixing=?2 WHERE id=?1", params![id, on])?;
    Ok(())
}

/// The owner asked for changes to something already ready or done: it is not ready any more.
pub fn mark_fixing(c: &Connection, id: i64) {
    let _ = c.execute("UPDATE ideas SET fixing=1 WHERE id=?1 AND status IN ('ready','done')", [id]);
}

pub fn set_title(c: &Connection, id: i64, title: &str) -> Result<()> {
    c.execute("UPDATE ideas SET title=?2, updated=?3 WHERE id=?1", params![id, title, now()])?;
    Ok(())
}

pub fn set_slug(c: &Connection, id: i64, slug: &str) -> Result<()> {
    c.execute("UPDATE ideas SET slug=?2, updated=?3 WHERE id=?1", params![id, slug, now()])?;
    Ok(())
}

pub fn save_idea(c: &Connection, i: &Idea) -> Result<()> {
    c.execute(
        "UPDATE ideas SET title=?2,body=?3,intents=?4,recipe=?5,stage_idx=?6,status=?7,priority=?8,research_only=?9,
         summary=?10,verdict=?11,run_cmd=?12,knowledge=?13,needs=?14,repeat_count=?15,fail_count=?16,sort=?17,updated=?18,scope=?19
         WHERE id=?1",
        params![
            i.id,
            i.title,
            i.body,
            serde_json::to_string(&i.intents)?,
            serde_json::to_string(&i.recipe)?,
            i.stage_idx,
            i.status,
            i.priority,
            i.research_only,
            i.summary,
            i.verdict,
            i.run_cmd,
            i.knowledge,
            serde_json::to_string(&i.needs)?,
            i.repeat_count,
            i.fail_count,
            i.sort,
            now(),
            norm_scope(&i.scope)
        ],
    )?;
    Ok(())
}

// ---------- questions / directives ----------

#[derive(Debug, Clone, Serialize)]
pub struct Question {
    pub id: i64,
    pub idea_id: i64,
    pub question: String,
    pub options: Vec<String>,
    pub guess: Option<String>,
    pub answer: Option<String>,
    pub created: i64,
}

pub fn insert_question(c: &Connection, idea_id: i64, q: &str, options: &[String], guess: Option<&str>) -> Result<()> {
    c.execute(
        "INSERT INTO questions(idea_id,question,options,guess,created) VALUES(?1,?2,?3,?4,?5)",
        params![idea_id, q, serde_json::to_string(options)?, guess, now()],
    )?;
    Ok(())
}

pub fn questions(c: &Connection, idea_id: Option<i64>, only_open: bool) -> Result<Vec<Question>> {
    let sql = format!(
        "SELECT id,idea_id,question,options,guess,answer,created FROM questions WHERE (?1 IS NULL OR idea_id=?1) {} ORDER BY id",
        if only_open { "AND answer IS NULL" } else { "" }
    );
    let mut st = c.prepare(&sql)?;
    let v = st
        .query_map([idea_id], |r| {
            Ok(Question {
                id: r.get(0)?,
                idea_id: r.get(1)?,
                question: r.get(2)?,
                options: json_vec(r.get(3)?),
                guess: r.get(4)?,
                answer: r.get(5)?,
                created: r.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(v)
}

pub fn answer_question(c: &Connection, id: i64, answer: &str) -> Result<i64> {
    c.execute("UPDATE questions SET answer=?2, answered=?3 WHERE id=?1", params![id, answer, now()])?;
    Ok(c.query_row("SELECT idea_id FROM questions WHERE id=?1", [id], |r| r.get(0))?)
}

pub fn add_directive(c: &Connection, idea_id: i64, text: &str) -> Result<i64> {
    c.execute("INSERT INTO directives(idea_id,text,created) VALUES(?1,?2,?3)", params![idea_id, text, now()])?;
    Ok(c.last_insert_rowid())
}

/// One instruction of the owner, with where it stands.
#[derive(Debug, Clone, Serialize, Default)]
pub struct Request {
    pub id: i64,
    pub ts: i64,
    pub text: String,
    /// "" waiting · "done" the AI says it is done · "ok" the owner checked it works · "no" the AI cannot do it
    pub state: String,
    pub note: String,
    pub changed: i64,
    /// a request to be ticked off (asked in a chat or reported as not done), not a steer like "go on"
    pub checklist: bool,
}

/// Is this directive a request to tick off?
pub fn is_request(text: &str) -> bool {
    text.starts_with("(สั่งในแชต)") || text.starts_with("(เจ้าของแจ้ง")
}

pub fn directive_rows(c: &Connection, idea_id: i64) -> Result<Vec<Request>> {
    let mut st = c.prepare("SELECT id,created,text,state,note,changed FROM directives WHERE idea_id=?1 ORDER BY id")?;
    let v = st
        .query_map([idea_id], |r| {
            let text: String = r.get(2)?;
            Ok(Request { id: r.get(0)?, ts: r.get(1)?, checklist: is_request(&text), text, state: r.get(3)?, note: r.get(4)?, changed: r.get(5)? })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(v)
}

/// Change where a request stands; returns its idea.
pub fn set_request(c: &Connection, id: i64, state: &str, note: &str) -> Result<i64> {
    let n = c.execute("UPDATE directives SET state=?2, note=?3, changed=?4 WHERE id=?1", params![id, state, note, now()])?;
    if n == 0 {
        anyhow::bail!("ไม่พบคำสั่งเลข {id}");
    }
    Ok(c.query_row("SELECT idea_id FROM directives WHERE id=?1", [id], |r| r.get(0))?)
}

pub fn directives(c: &Connection, idea_id: i64) -> Result<Vec<(i64, String)>> {
    let mut st = c.prepare("SELECT created,text FROM directives WHERE idea_id=?1 ORDER BY id")?;
    let v = st
        .query_map([idea_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(v)
}

// ---------- runs ----------

#[derive(Debug, Clone, Serialize, Default)]
pub struct Run {
    pub id: i64,
    pub idea_id: i64,
    pub stage: String,
    pub model: String,
    pub effort: Option<String>,
    pub reason: Option<String>,
    pub summary: Option<String>,
    pub model_id: Option<String>,
    pub provider: String,
    pub model_used: String,
    pub account: String,
    pub power: bool,
    /// readable name of the model that really ran, e.g. "Opus 5.5" (see modelname.rs)
    pub model_label: String,
    pub started: i64,
    pub ended: Option<i64>,
    pub status: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read: i64,
    pub cache_write: i64,
    pub cost_usd: f64,
    pub num_turns: i64,
    pub session_id: Option<String>,
    pub week_before: Option<f64>,
    pub week_after: Option<f64>,
    pub five_before: Option<f64>,
    pub five_after: Option<f64>,
    pub error: Option<String>,
}

pub fn start_run(c: &Connection, idea_id: i64, stage: &str, choice: &crate::router::Choice, week: Option<f64>, five: Option<f64>, provider: &str, model_used: &str, power: bool, account: &str) -> Result<i64> {
    c.execute(
        "INSERT INTO runs(idea_id,stage,model,effort,reason,started,week_before,five_before,provider,model_used,power,account) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        params![idea_id, stage, choice.model, choice.effort, choice.reason, now(), week, five, provider, model_used, power as i64, account],
    )?;
    Ok(c.last_insert_rowid())
}

pub fn finish_run(c: &Connection, r: &Run) -> Result<()> {
    c.execute(
        "UPDATE runs SET ended=?2,status=?3,input_tokens=?4,output_tokens=?5,cache_read=?6,cache_write=?7,cost_usd=?8,
         num_turns=?9,session_id=?10,week_after=?11,five_after=?12,error=?13,model_id=COALESCE(?14,model_id) WHERE id=?1",
        params![
            r.id,
            now(),
            r.status,
            r.input_tokens,
            r.output_tokens,
            r.cache_read,
            r.cache_write,
            r.cost_usd,
            r.num_turns,
            r.session_id,
            r.week_after,
            r.five_after,
            r.error,
            r.model_id
        ],
    )?;
    Ok(())
}

pub fn runs(c: &Connection, idea_id: Option<i64>, since: i64, limit: i64) -> Result<Vec<Run>> {
    let mut st = c.prepare(
        "SELECT id,idea_id,stage,model,started,ended,status,input_tokens,output_tokens,cache_read,cache_write,cost_usd,
         num_turns,session_id,week_before,week_after,five_before,five_after,error,effort,reason,summary,model_id,provider,model_used,power,account
         FROM runs WHERE (?1 IS NULL OR idea_id=?1) AND started>=?2 ORDER BY id DESC LIMIT ?3",
    )?;
    let v = st
        .query_map(params![idea_id, since, limit], |r| {
            let (model, model_id, provider, model_used): (String, Option<String>, String, String) = (r.get(3)?, r.get(22)?, r.get(23)?, r.get(24)?);
            let asked = if model_used.is_empty() { model.clone() } else { model_used.clone() };
            Ok(Run {
                id: r.get(0)?,
                idea_id: r.get(1)?,
                stage: r.get(2)?,
                model_label: crate::modelname::label(&provider, model_id.as_deref(), &asked),
                model,
                provider,
                model_used,
                power: r.get::<_, i64>(25)? != 0,
                account: r.get(26)?,
                started: r.get(4)?,
                ended: r.get(5)?,
                status: r.get(6)?,
                input_tokens: r.get(7)?,
                output_tokens: r.get(8)?,
                cache_read: r.get(9)?,
                cache_write: r.get(10)?,
                cost_usd: r.get(11)?,
                num_turns: r.get(12)?,
                session_id: r.get(13)?,
                week_before: r.get(14)?,
                week_after: r.get(15)?,
                five_before: r.get(16)?,
                five_after: r.get(17)?,
                error: r.get(18)?,
                effort: r.get(19)?,
                reason: r.get(20)?,
                summary: r.get(21)?,
                model_id,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(v)
}

/// Bot run spans of one account (runs recorded before accounts existed count for the account named after its AI).
pub fn run_spans_since_account(c: &Connection, since: i64, account: &str, legacy: bool) -> Result<Vec<(i64, Option<i64>)>> {
    let mut st = c.prepare("SELECT started, ended FROM runs WHERE COALESCE(ended, ?2) >= ?1 AND (account=?3 OR (account='' AND ?4=1))")?;
    let rows = st.query_map(params![since, now(), account, legacy as i64], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<i64>>(1)?)))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn insert_account_usage(c: &Connection, account: &str, s: &crate::usage::Snapshot) {
    let _ = c.execute(
        "INSERT INTO account_usage(ts,account,five_pct,week_pct,source) VALUES(?1,?2,?3,?4,?5)",
        params![s.ts, account, s.five_pct, s.week_pct, s.source],
    );
}

/// Quota history per account for the charts: account -> [(ts, five%, week%)], oldest first.
pub fn account_series(c: &Connection, since: i64) -> std::collections::BTreeMap<String, Vec<(i64, f64, f64)>> {
    let mut out: std::collections::BTreeMap<String, Vec<(i64, f64, f64)>> = Default::default();
    if let Ok(mut st) = c.prepare("SELECT account, ts, five_pct, week_pct FROM account_usage WHERE ts>=?1 ORDER BY ts") {
        if let Ok(rows) = st.query_map([since], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, f64>(2)?, r.get::<_, f64>(3)?))) {
            for (a, ts, f, w) in rows.flatten() {
                out.entry(a).or_default().push((ts, f, w));
            }
        }
    }
    out
}

/// Mark runs left "running" by a previous crash as interrupted.
/// (started, ended) of every bot run that touched the time since `since` — for the self-metered budget.
pub fn run_spans_since(c: &Connection, since: i64) -> Result<Vec<(i64, Option<i64>)>> {
    let mut st = c.prepare("SELECT started, ended FROM runs WHERE COALESCE(ended, ?2) >= ?1")?;
    let rows = st.query_map(params![since, now()], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<i64>>(1)?)))?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

/// Nothing saved yet: a brand-new install (shows the setup screen, asks before auto-start).
pub fn is_fresh(c: &Connection) -> bool {
    let n = |t: &str| c.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get::<_, i64>(0)).unwrap_or(0);
    n("ideas") == 0 && n("runs") == 0
}

pub fn recover_runs(c: &Connection) -> Result<()> {
    c.execute(
        "UPDATE runs SET status='interrupted', ended=?1 WHERE status='running'",
        [now()],
    )?;
    Ok(())
}

// ---------- calendar ----------

pub fn overrides(c: &Connection) -> Result<Vec<(String, String, String)>> {
    let mut st = c.prepare("SELECT date,kind,note FROM calendar_overrides ORDER BY date")?;
    let v = st
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(v)
}

pub fn set_override(c: &Connection, date: &str, kind: Option<&str>, note: &str) -> Result<()> {
    match kind {
        Some(k) => c.execute(
            "INSERT INTO calendar_overrides(date,kind,note) VALUES(?1,?2,?3)
             ON CONFLICT(date) DO UPDATE SET kind=excluded.kind, note=excluded.note",
            params![date, k, note],
        )?,
        None => c.execute("DELETE FROM calendar_overrides WHERE date=?1", [date])?,
    };
    Ok(())
}

// ---------- events / digests ----------

#[derive(Debug, Clone, Serialize)]
pub struct Event {
    pub ts: i64,
    pub kind: String,
    pub idea_id: Option<i64>,
    pub text: String,
}

pub fn log_event(c: &Connection, kind: &str, idea_id: Option<i64>, text: &str) {
    let _ = c.execute(
        "INSERT INTO events(ts,kind,idea_id,text) VALUES(?1,?2,?3,?4)",
        params![now(), kind, idea_id, text],
    );
}

pub fn events_since(c: &Connection, since: i64, limit: i64) -> Result<Vec<Event>> {
    let mut st = c.prepare("SELECT ts,kind,idea_id,text FROM events WHERE ts>=?1 ORDER BY id DESC LIMIT ?2")?;
    let v = st
        .query_map(params![since, limit], |r| {
            Ok(Event { ts: r.get(0)?, kind: r.get(1)?, idea_id: r.get(2)?, text: r.get(3)? })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(v)
}

pub fn insert_digest(c: &Connection, body: &serde_json::Value) -> Result<()> {
    c.execute("INSERT INTO digests(ts,body) VALUES(?1,?2)", params![now(), body.to_string()])?;
    Ok(())
}

pub fn latest_digest(c: &Connection) -> Result<Option<(i64, serde_json::Value)>> {
    let r: Option<(i64, String)> = c
        .query_row("SELECT ts,body FROM digests ORDER BY id DESC LIMIT 1", [], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;
    Ok(r.map(|(t, b)| (t, serde_json::from_str(&b).unwrap_or_default())))
}

#[cfg(test)]
mod board_tests {
    use super::*;

    #[test]
    fn the_owners_requests_are_listed_in_directives_md() {
        let dir = std::env::temp_dir().join(format!("ia_req_{}_{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let _ = std::fs::create_dir_all(&dir);
        let c = open(&dir.join("t.db")).unwrap();
        let id = insert_idea(&c, "s", "t", "", 0, false).unwrap();
        add_directive(&c, id, "ไปต่อได้เลย").unwrap();
        let r = add_directive(&c, id, "(สั่งในแชต) login ผ่าน claude เองบนมือถือ").unwrap();
        let rows = directive_rows(&c, id).unwrap();
        assert_eq!(rows.iter().filter(|x| x.checklist).count(), 1);
        assert_eq!(set_request(&c, r, "done", "ทดสอบบนมือถือแล้ว").unwrap(), id);
        assert!(set_request(&c, 99999, "done", "").is_err());
        crate::workspace::write_directives(&dir, &directive_rows(&c, id).unwrap()).unwrap();
        let md = std::fs::read_to_string(dir.join("DIRECTIVES.md")).unwrap();
        // every order is listed for later runs, without a checklist for the owner to tick (ISSUE-070)
        assert!(md.contains("] (สั่งในแชต) login ผ่าน claude เองบนมือถือ") && md.contains("] ไปต่อได้เลย"));
        assert!(!md.contains("**#") && !md.contains("req <"));
    }

    #[test]
    fn asking_for_changes_takes_a_ready_idea_out_of_ready_and_bot_saves_keep_flags() {
        let dir = std::env::temp_dir().join(format!("ia_board_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let c = open(&dir.join("t.db")).unwrap();
        let id = insert_idea(&c, "s", "t", "", 0, false).unwrap();
        // an idea still being worked on is not "fixing"
        mark_fixing(&c, id);
        assert!(!get_idea(&c, id).unwrap().fixing);
        let mut i = get_idea(&c, id).unwrap();
        i.status = "ready".into();
        save_idea(&c, &i).unwrap();
        mark_fixing(&c, id);
        set_pinned(&c, id, true).unwrap();
        // a bot run saving the idea must not clear the owner's flags
        let i = get_idea(&c, id).unwrap();
        save_idea(&c, &i).unwrap();
        let i = get_idea(&c, id).unwrap();
        assert!(i.fixing && i.pinned);
        set_fixing(&c, id, false).unwrap();
        assert!(!get_idea(&c, id).unwrap().fixing);
        drop(c);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
