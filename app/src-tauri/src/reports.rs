//! Morning digest, usage history reports and CSV export.
use anyhow::Result;
use chrono::{Datelike, Local, TimeZone, Timelike};
use serde_json::{json, Value};
use std::collections::BTreeMap;

use crate::db;
use crate::state::AppState;

fn local(ts: i64) -> chrono::DateTime<Local> {
    Local.timestamp_opt(ts, 0).single().unwrap_or_else(Local::now)
}

pub fn make_digest(st: &AppState) -> Result<Value> {
    let c = st.db.lock();
    let since = db::latest_digest(&c)?.map(|(t, _)| t).unwrap_or(db::now() - 86400);
    let runs = db::runs(&c, None, since, 500)?;
    let ideas = db::list_ideas(&c)?;
    let touched: Vec<&db::Idea> = ideas.iter().filter(|i| i.updated >= since && i.status != "inbox").collect();
    let ready: Vec<Value> = touched
        .iter()
        .filter(|i| i.status == "ready")
        .map(|i| json!({"id": i.id, "title": i.title, "summary": i.summary}))
        .collect();
    let finished: Vec<Value> = touched
        .iter()
        .filter(|i| i.status == "done")
        .map(|i| json!({"id": i.id, "title": i.title, "summary": i.summary, "verdict": i.verdict}))
        .collect();
    let progressed: Vec<Value> = touched
        .iter()
        .filter(|i| i.status == "active")
        .map(|i| json!({"id": i.id, "title": i.title, "summary": i.summary, "stage": i.current_stage()}))
        .collect();
    let open_q = db::questions(&c, None, true)?.len();
    let needs: Vec<Value> = ideas
        .iter()
        .filter(|i| !i.needs.is_empty() && i.status != "archived")
        .map(|i| json!({"id": i.id, "title": i.title, "needs": i.needs}))
        .collect();
    let snaps = db::snapshots_since(&c, since)?;
    let deltas = db::deltas(&snaps);
    let bot: f64 = deltas.iter().map(|d| d.bot).sum();
    let user: f64 = deltas.iter().map(|d| d.user).sum();
    let week = snaps.last().map(|s| s.week_pct);
    let tokens: i64 = runs.iter().map(|r| r.input_tokens + r.output_tokens + r.cache_write).sum();

    let mut parts = Vec::new();
    if !ready.is_empty() {
        parts.push(format!("{} พร้อมลองเล่น", ready.len()));
    }
    if !finished.is_empty() {
        parts.push(format!("{} เสร็จ", finished.len()));
    }
    if !progressed.is_empty() {
        parts.push(format!("{} คืบหน้า", progressed.len()));
    }
    if open_q > 0 {
        parts.push(format!("{open_q} คำถามรอคุณ"));
    }
    let headline = if parts.is_empty() {
        "เมื่อคืนไม่มีงานใหม่ — ลองจดไอเดียทิ้งไว้สักอัน".to_string()
    } else {
        format!("{} · บอทใช้ไป {:.0}% ของสัปดาห์", parts.join(" · "), bot)
    };
    let body = json!({
        "since": since,
        "headline": headline,
        "runs": runs.len(),
        "tokens": tokens,
        "ready": ready,
        "finished": finished,
        "progressed": progressed,
        "open_questions": open_q,
        "needs": needs,
        "bot_pct": bot,
        "user_pct": user,
        "week_pct": week,
    });
    db::insert_digest(&c, &body)?;
    Ok(body)
}

/// History for the quota page: daily user/bot split, hourly heatmap, weekly leftovers.
pub fn reports(st: &AppState, days: i64) -> Result<Value> {
    let c = st.db.lock();
    let since = db::now() - days * 86400;
    let snaps = db::snapshots_since(&c, since)?;
    let deltas = db::deltas(&snaps);

    let mut daily: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    let mut heat = vec![vec![0.0f64; 24]; 7];
    for d in &deltas {
        let t = local(d.ts);
        let e = daily.entry(t.format("%Y-%m-%d").to_string()).or_default();
        e.0 += d.user;
        e.1 += d.bot;
        heat[t.weekday().num_days_from_monday() as usize][t.hour() as usize] += d.user + d.bot;
    }
    let weeks_seen = (days as f64 / 7.0).max(1.0);
    for row in heat.iter_mut() {
        for v in row.iter_mut() {
            *v /= weeks_seen;
        }
    }

    // weekly leftover: last snapshot before each reset
    let mut leftovers: BTreeMap<i64, f64> = BTreeMap::new();
    for s in &snaps {
        if let Some(r) = s.week_reset {
            if s.ts <= r {
                leftovers.insert(r, 100.0 - s.week_pct);
            }
        }
    }
    let now = db::now();
    let weeks: Vec<Value> = leftovers
        .into_iter()
        .map(|(r, left)| json!({"reset": r, "left": left, "closed": r < now}))
        .collect();

    // downsample the timeline to ~400 points
    let step = (snaps.len() / 400).max(1);
    let timeline: Vec<Value> = snaps
        .iter()
        .step_by(step)
        .map(|s| json!([s.ts, s.week_pct, s.five_pct, s.bot_running]))
        .collect();

    let runs = db::runs(&c, None, since, 5000)?;
    let mut by_stage: BTreeMap<String, (i64, i64, f64)> = BTreeMap::new();
    for r in &runs {
        let e = by_stage.entry(r.stage.clone()).or_default();
        e.0 += 1;
        e.1 += r.input_tokens + r.output_tokens + r.cache_write;
        e.2 += r.week_after.zip(r.week_before).map(|(a, b)| (a - b).max(0.0)).unwrap_or(0.0);
    }

    let mut by_model: BTreeMap<String, (i64, i64, f64, i64)> = BTreeMap::new();
    for r in &runs {
        let key = crate::modelname::with_effort(&r.model_label, r.effort.as_deref());
        let e = by_model.entry(key).or_default();
        e.0 += 1;
        e.1 += r.input_tokens + r.output_tokens + r.cache_write;
        e.2 += r.week_after.zip(r.week_before).map(|(a, b)| (a - b).max(0.0)).unwrap_or(0.0);
        e.3 += (r.status == "ok") as i64;
    }

    Ok(json!({
        "models": by_model.into_iter().map(|(m, (n, t, p, ok))| json!({"model": m, "runs": n, "tokens": t, "pct": p, "ok": ok})).collect::<Vec<_>>(),
        "daily": daily.into_iter().map(|(d, (u, b))| json!({"date": d, "user": u, "bot": b})).collect::<Vec<_>>(),
        "heat": heat,
        "weeks": weeks,
        "timeline": timeline,
        "stages": by_stage.into_iter().map(|(s, (n, t, p))| json!({"stage": s, "runs": n, "tokens": t, "pct": p})).collect::<Vec<_>>(),
        "total_runs": runs.len(),
    }))
}

/// Everything the activity page needs: runs, events, per-day bot timeline, why the bot waited.
pub fn activity(st: &AppState, days: i64) -> Result<Value> {
    let c = st.db.lock();
    let since = db::now() - days * 86400;
    let ideas: BTreeMap<i64, (String, Vec<String>, i64, String)> = db::list_ideas(&c)?
        .into_iter()
        .map(|i| (i.id, (i.title, i.recipe, i.stage_idx, i.status)))
        .collect();
    let runs = db::runs(&c, None, since, 5000)?;
    let now = db::now();
    let run_rows: Vec<Value> = runs
        .iter()
        .map(|r| {
            let end = r.ended.unwrap_or(now);
            json!({
                "id": r.id, "idea_id": r.idea_id,
                "title": ideas.get(&r.idea_id).map(|x| x.0.clone()).unwrap_or_default(),
                "stage": r.stage, "model": r.model, "model_label": r.model_label, "effort": r.effort, "reason": r.reason, "power": r.power,
                "started": r.started, "ended": r.ended, "secs": end - r.started,
                "status": r.status, "summary": r.summary, "error": r.error,
                "tokens": r.input_tokens + r.output_tokens + r.cache_write, "cache_read": r.cache_read,
                "turns": r.num_turns,
                "pct": r.week_after.zip(r.week_before).map(|(a, b)| (a - b).max(0.0)),
            })
        })
        .collect();

    // why the bot was running or waiting, in minutes (snapshots are ~5 min apart)
    let mut waits: BTreeMap<String, i64> = BTreeMap::new();
    let mut st_q = c.prepare("SELECT ts, code FROM usage_snapshots WHERE ts>=?1 AND code IS NOT NULL ORDER BY ts")?;
    let codes: Vec<(i64, String)> = st_q.query_map([since], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
    for w in codes.windows(2) {
        let mins = ((w[1].0 - w[0].0) / 60).min(15);
        *waits.entry(w[0].1.clone()).or_default() += mins;
    }

    // stats
    let finished: Vec<&db::Run> = runs.iter().filter(|r| r.status != "running").collect();
    let ok = finished.iter().filter(|r| r.status == "ok").count();
    let avg_secs = if finished.is_empty() { 0 } else { finished.iter().map(|r| r.ended.unwrap_or(r.started) - r.started).sum::<i64>() / finished.len() as i64 };
    // average duration per stage, for ETA
    let mut per_stage: BTreeMap<String, (i64, i64)> = BTreeMap::new();
    for r in db::runs(&c, None, 0, 5000)?.iter().filter(|r| r.status == "ok") {
        let e = per_stage.entry(r.stage.clone()).or_default();
        e.0 += r.ended.unwrap_or(r.started) - r.started;
        e.1 += 1;
    }
    let stage_avg: BTreeMap<String, i64> = per_stage.into_iter().map(|(k, (s, n))| (k, s / n.max(1))).collect();
    let eta: Vec<Value> = ideas
        .iter()
        .filter(|(_, v)| v.3 == "active" || v.3 == "inbox")
        .map(|(id, (title, recipe, idx, status))| {
            let rest: Vec<&String> = if status == "inbox" { recipe.iter().collect() } else { recipe.iter().skip(*idx as usize).collect() };
            let secs: i64 = rest.iter().map(|s| stage_avg.get(s.as_str()).copied().unwrap_or(20 * 60)).sum::<i64>()
                + if status == "inbox" { stage_avg.get("clarify").copied().unwrap_or(5 * 60) } else { 0 };
            json!({"id": id, "title": title, "stages_left": rest.len() + (status == "inbox") as usize, "secs": secs})
        })
        .collect();

    Ok(json!({
        "runs": run_rows,
        "events": db::events_since(&c, since, 1000)?,
        "waits": waits,
        "stage_avg": stage_avg,
        "eta": eta,
        "stats": {
            "runs": runs.len(), "ok": ok, "failed": finished.len() - ok,
            "avg_secs": avg_secs,
            "tokens": runs.iter().map(|r| r.input_tokens + r.output_tokens + r.cache_write).sum::<i64>(),
            "pct": runs.iter().filter_map(|r| r.week_after.zip(r.week_before).map(|(a, b)| (a - b).max(0.0))).sum::<f64>(),
            "busy_secs": runs.iter().map(|r| r.ended.unwrap_or(now) - r.started).sum::<i64>(),
        },
    }))
}

/// Readable lines from a run's stream log.
pub fn run_log(st: &AppState, run_id: i64) -> Result<Vec<String>> {
    let c = st.db.lock();
    let (idea_id, stage): (i64, String) = c.query_row("SELECT idea_id, stage FROM runs WHERE id=?1", [run_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let idea = db::get_idea(&c, idea_id)?;
    drop(c);
    let lang = crate::scheduler::idea_lang(st, &idea);
    let path = crate::workspace::idea_dir(&st.ws, &idea.slug).join("logs").join(format!("run-{run_id:05}-{stage}.jsonl"));
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    Ok(text
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter_map(|v| crate::runner::describe(&v, &lang))
        .collect())
}

/// Every run that started in [from, to): who ran it, with which model, and what it cost in tokens and time.
/// The usage report aggregates these in the interface so its filters stay instant.
/// `model` is the exact model name when the agent reported one, else the name that was requested.
pub fn usage_runs(st: &AppState, from: i64, to: i64) -> Result<Value> {
    // chat logs are files: read them before taking the database lock
    let chats = crate::chat_usage::collect(&st.root, from, to);
    let c = st.db.lock();
    let mut q = c.prepare(
        "SELECT r.id, r.idea_id, COALESCE(i.title, ''), r.stage, r.status, r.started, r.ended,
                r.input_tokens, r.output_tokens, r.cache_read, r.cache_write, r.cost_usd, r.num_turns,
                COALESCE(NULLIF(r.model_id, ''), NULLIF(r.model_used, ''), r.model), r.model,
                COALESCE(NULLIF(r.provider, ''), 'claude'), r.account, COALESCE(r.effort, ''), COALESCE(i.scope, 'personal'),
                r.session_id, COALESCE(i.slug, '')
         FROM runs r LEFT JOIN ideas i ON i.id = r.idea_id
         WHERE r.started >= ?1 AND r.started < ?2 ORDER BY r.started",
    )?;
    let now = db::now();
    let rows = q
        .query_map([from, to], |r| {
            let (started, ended): (i64, Option<i64>) = (r.get(5)?, r.get(6)?);
            Ok(json!({
                "id": r.get::<_, i64>(0)?, "idea_id": r.get::<_, i64>(1)?, "idea": r.get::<_, String>(2)?,
                "stage": r.get::<_, String>(3)?, "status": r.get::<_, String>(4)?, "started": started,
                "secs": (ended.unwrap_or(now) - started).max(0),
                "input": r.get::<_, i64>(7)?, "output": r.get::<_, i64>(8)?, "cache_read": r.get::<_, i64>(9)?,
                "cache_write": r.get::<_, i64>(10)?, "cost": r.get::<_, f64>(11)?, "turns": r.get::<_, i64>(12)?,
                "model": r.get::<_, String>(13)?, "tier": r.get::<_, String>(14)?, "provider": r.get::<_, String>(15)?,
                "account": r.get::<_, String>(16)?, "effort": r.get::<_, String>(17)?, "scope": r.get::<_, String>(18)?,
                "source": "bot", "msgs": 0, "from_log": false,
                "_sid": r.get::<_, Option<String>>(19)?, "_slug": r.get::<_, String>(20)?, "_ended": ended.unwrap_or(0),
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    // chats: one row per chat, model and hour, shaped like a run (stage "chat", no run count of its own)
    let ideas: std::collections::HashMap<i64, (String, String)> =
        db::list_ideas(&c)?.into_iter().map(|i| (i.id, (i.title, i.scope))).collect();
    let first: Option<i64> = c.query_row("SELECT MIN(started) FROM runs", [], |r| r.get(0)).ok().flatten();
    drop(q);
    drop(c);
    let mut rows = rows;
    // a Codex or Mistral run that recorded no tokens: take them from the agent's own log (marked, so the page can say so)
    for row in rows.iter_mut() {
        let zero = ["input", "output", "cache_read", "cache_write"].iter().all(|k| row[*k].as_i64() == Some(0));
        let provider = row["provider"].as_str().unwrap_or_default().to_string();
        if zero && (provider == "codex" || provider == "mistral") {
            let r = crate::log_tokens::RunRef {
                id: row["id"].as_i64().unwrap_or(0),
                provider: &provider,
                session_id: row["_sid"].as_str(),
                idea_dir: crate::workspace::idea_dir(&st.ws, row["_slug"].as_str().unwrap_or_default()),
                started: row["started"].as_i64().unwrap_or(0),
                ended: row["_ended"].as_i64().unwrap_or(0),
            };
            if let Some(t) = crate::log_tokens::recover(&st.root, &r) {
                for (k, v) in ["input", "output", "cache_read", "cache_write"].iter().zip(t) {
                    row[*k] = json!(v);
                }
                row["from_log"] = json!(true);
            }
        }
        if let Some(o) = row.as_object_mut() {
            o.retain(|k, _| !k.starts_with('_'));
        }
    }
    for (n, h) in chats.into_iter().enumerate() {
        let (title, scope) = ideas.get(&h.idea_id).cloned().unwrap_or_default();
        rows.push(json!({
            "id": -(n as i64) - 1, "idea_id": h.idea_id, "idea": title, "stage": "chat", "status": "ok", "started": h.first,
            "secs": (h.last - h.first).max(60), "input": h.input, "output": h.output, "cache_read": h.cache_read,
            "cache_write": h.cache_write, "cost": 0.0, "turns": h.messages, "model": h.model, "tier": "", "provider": h.provider,
            "account": h.account, "effort": "", "scope": if scope.is_empty() { "personal".into() } else { scope },
            "source": "chat", "msgs": h.messages, "from_log": true,
        }));
    }
    Ok(json!({ "runs": rows, "first": first }))
}

/// A report the interface rendered (PNG, PDF or CSV, base64), saved under data/exports/. Returns the file path.
pub fn save_export(st: &AppState, name: &str, b64: &str) -> Result<std::path::PathBuf> {
    use base64::Engine;
    let ext = std::path::Path::new(name).extension().and_then(|e| e.to_str()).unwrap_or("");
    anyhow::ensure!(matches!(ext, "png" | "pdf" | "csv"), "บันทึกได้เฉพาะ .png .pdf หรือ .csv");
    let safe: String = name.chars().map(|c| if c.is_alphanumeric() || "-_.".contains(c) { c } else { '-' }).collect();
    let data = base64::engine::general_purpose::STANDARD.decode(b64.trim())?;
    anyhow::ensure!(!data.is_empty() && data.len() < 40 * 1024 * 1024, "ไฟล์ว่างหรือใหญ่เกิน 40 MB");
    let dir = st.root.join("data").join("exports");
    std::fs::create_dir_all(&dir)?;
    let p = dir.join(safe);
    std::fs::write(&p, data)?;
    Ok(p)
}

pub fn export_csv(st: &AppState) -> Result<std::path::PathBuf> {
    let dir = st.root.join("data").join("exports");
    std::fs::create_dir_all(&dir)?;
    let stamp = Local::now().format("%Y%m%d-%H%M").to_string();
    let c = st.db.lock();

    let mut s = String::from("time,week_pct,five_pct,week_reset,bot_running,user_active\n");
    for r in db::snapshots_since(&c, 0)? {
        s.push_str(&format!(
            "{},{},{},{},{},{}\n",
            local(r.ts).format("%Y-%m-%d %H:%M:%S"),
            r.week_pct,
            r.five_pct,
            r.week_reset.map(|t| local(t).format("%Y-%m-%d %H:%M").to_string()).unwrap_or_default(),
            r.bot_running as i32,
            r.user_active as i32
        ));
    }
    let p1 = dir.join(format!("usage-{stamp}.csv"));
    std::fs::write(&p1, format!("\u{feff}{s}"))?;

    let ideas: BTreeMap<i64, String> = db::list_ideas(&c)?.into_iter().map(|i| (i.id, i.title)).collect();
    let mut r = String::from("started,ended,idea,stage,model,status,input,output,cache_read,cache_write,turns,week_before,week_after\n");
    for x in db::runs(&c, None, 0, 100000)? {
        r.push_str(&format!(
            "{},{},\"{}\",{},{},{},{},{},{},{},{},{},{}\n",
            local(x.started).format("%Y-%m-%d %H:%M:%S"),
            x.ended.map(|t| local(t).format("%Y-%m-%d %H:%M:%S").to_string()).unwrap_or_default(),
            ideas.get(&x.idea_id).cloned().unwrap_or_default().replace('"', "'"),
            x.stage,
            x.model,
            x.status,
            x.input_tokens,
            x.output_tokens,
            x.cache_read,
            x.cache_write,
            x.num_turns,
            x.week_before.map(|v| v.to_string()).unwrap_or_default(),
            x.week_after.map(|v| v.to_string()).unwrap_or_default()
        ));
    }
    std::fs::write(dir.join(format!("runs-{stamp}.csv")), format!("\u{feff}{r}"))?;
    Ok(dir)
}
