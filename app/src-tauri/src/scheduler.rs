//! The autopilot loop: refresh quota, ask the governor, pick the next idea stage, run it,
//! and fold the result back into the idea.
use chrono::{Local, Timelike};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::Notify;

use crate::calendar::Calendar;
use crate::db::{self, Idea};
use crate::governor::{self, Inputs, DEFAULT_RESERVE_PER_WORK_HOUR};
use crate::state::{AppState, CurrentRun};
use crate::{activity, prompts, reports, runner, usage, workspace};

const FETCH_EVERY: i64 = 5 * 60;
const MAX_REPEATS: i64 = 6;

pub fn calendar(st: &AppState) -> Calendar {
    let c = st.db.lock();
    let sched = db::get_setting(&c, "schedule")
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    Calendar::new(sched, &db::overrides(&c).unwrap_or_default())
}

/// An idea needs the owner's attention (ready to use, finished, stuck): desktop toast (not at night),
/// phone push through the Telegram bot (the phone's own do-not-disturb decides), in-app event.
pub fn notify_idea(app: &AppHandle, idea_id: i64, title: &str, body: &str) {
    let quiet = {
        let h = Local::now().hour();
        !(7..23).contains(&h)
    };
    if !quiet {
        let _ = app.notification().builder().title(title).body(body).show();
    }
    crate::chat::push(app, format!("{title}\n{body}"));
    let _ = app.emit("idea-notify", serde_json::json!({ "id": idea_id, "title": title, "body": body }));
}

pub fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
    crate::chat::push(app, format!("{title}
{body}"));
}

pub async fn run_loop(app: AppHandle) {
    loop {
        if let Err(e) = tick(&app).await {
            let st = app.state::<AppState>();
            st.push_log(format!("[scheduler] {e:#}"));
        }
        let _ = app.emit("state-changed", ());
        let st = app.state::<AppState>();
        let _ = tokio::time::timeout(std::time::Duration::from_secs(30), st.wake.notified()).await;
    }
}

async fn tick(app: &AppHandle) -> anyhow::Result<()> {
    let st = app.state::<AppState>();
    let now = Local::now();
    let ts = now.timestamp();

    if let Err(e) = crate::sync::tick(app) {
        st.push_log(format!("[sync] {e:#}"));
    }
    import_inbox(app);
    crate::remote::tick(app);

    // which AI does the work, and how its quota is measured (providers.rs)
    let (provider, meter) = {
        let c = st.db.lock();
        let p = crate::providers::chosen(&c);
        let m = crate::providers::meter(&c, &p);
        (p, m)
    };
    let budget_mode = meter == crate::providers::Meter::Budget;
    // Claude and the user share one plan quota, so their own use matters; other agents don't
    let user_active = provider == "claude" && activity::user_active(&st.ws);
    let need_fetch = {
        let rt = st.rt.lock();
        budget_mode || ts - rt.last_fetch >= FETCH_EVERY || rt.snap.is_none()
    };
    if need_fetch {
        let (snap, note) = if budget_mode {
            // our own meter is cheap: recompute every tick
            let c = st.db.lock();
            let cfg = usage::BudgetCfg::load(&c);
            let spans = db::run_spans_since(&c, usage::week_start(now, cfg.reset_day).timestamp().min(ts - 5 * 3600)).unwrap_or_default();
            (Some(usage::budget_snapshot(&spans, now, cfg)), None)
        } else {
            usage::fetch().await
        };
        let mut rt = st.rt.lock();
        rt.last_fetch = ts;
        rt.snap_note = note;
        if let Some(s) = snap {
            rt.snap = Some(s);
            if !budget_mode || ts - rt.last_snap_log >= FETCH_EVERY {
                rt.snap_pending = true;
                rt.last_snap_log = ts;
            }
        }
    }

    let cal = calendar(&st);
    // learn the user's per-work-hour usage once an hour
    if ts - st.rt.lock().last_learn > 3600 {
        let snaps = db::snapshots_since(&st.db.lock(), ts - 28 * 86400).unwrap_or_default();
        let learned = governor::learn_reserve(&db::deltas(&snaps), &cal);
        let fallback = db::get_setting(&st.db.lock(), "reserve_per_hour")
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_RESERVE_PER_WORK_HOUR);
        let policy = crate::tuner::learn(&st.db.lock(), |s| crate::router::base(s).0).unwrap_or_default();
        let mut rt = st.rt.lock();
        rt.policy = policy;
        rt.last_learn = ts;
        rt.reserve_learned = learned.is_some() && !budget_mode;
        // the self-metered budget only counts the bot, so there is no user usage to reserve for
        rt.reserve_per_hour = if budget_mode { 0.0 } else { learned.unwrap_or(fallback) };
    }

    let day_start = cal.bot_day_start(now).timestamp();
    let bot_used_today: f64 = {
        let snaps = db::snapshots_since(&st.db.lock(), day_start).unwrap_or_default();
        db::deltas(&snaps).iter().map(|d| d.bot).sum()
    };

    let pending = next_idea(&st).is_some();
    let decision = {
        // read what the governor needs under a SHORT rt lock, then release it so
        // the recalc + the nested db write don't hold rt (the main-thread UI commands
        // from the page's load()/refresh() would otherwise block on it -> "Not Responding")
        let (snap, paused, turbo, reserve_per_hour, has_current, snap_pending) = {
            let mut rt = st.rt.lock();
            rt.user_active = user_active;
            if rt.turbo_until != 0 && rt.turbo_until < ts {
                rt.turbo_until = 0;
            }
            (rt.snap.clone(), rt.paused, rt.turbo_until > ts, rt.reserve_per_hour, rt.current.is_some(), std::mem::take(&mut rt.snap_pending))
        };
        // recalc with NO lock held
        let d = governor::decide(&Inputs {
            now,
            snap: snap.as_ref(),
            user_active,
            paused,
            turbo,
            reserve_per_hour,
            bot_used_today,
            cal: &cal,
        });
        // persist the snapshot with only the db lock (never nested inside rt)
        if snap_pending {
            if let Some(s) = &snap {
                let code = if has_current { "running" } else { d.code.as_str() };
                let _ = db::insert_snapshot_code(&st.db.lock(), s, has_current, user_active, Some(code));
            }
        }
        // store results under a brief rt lock
        {
            let mut rt = st.rt.lock();
            rt.decision = d.clone();
            rt.keep_awake = rt.current.is_some() || (d.allow && pending) || rt.turbo_until > ts;
        }
        d
    };
    crate::tray::refresh(app);

    // stop a running stage only in clear-cut cases
    let (running, forced, cancel) = {
        let rt = st.rt.lock();
        (rt.current.is_some(), rt.current.as_ref().map(|c| c.forced).unwrap_or(false), rt.cancel.clone())
    };
    if running {
        if let Some(why) = governor::should_kill(&decision) {
            if !forced || decision.paused {
                st.push_log(format!("■ หยุดงานที่รันอยู่: {why}"));
                if let Some(c) = cancel {
                    c.notify_one();
                }
            }
        }
    } else if crate::sync::is_follower(&st) {
        // followers never run the bot; the leader machine does
    } else {
        let forced_id = st.rt.lock().force.take();
        let cooldown = st.rt.lock().cooldown_until > ts;
        let pick = match forced_id {
            Some(id) => db::get_idea(&st.db.lock(), id).ok().map(|i| (i, true)),
            None if decision.allow && !cooldown => next_idea(&st).map(|i| (i, false)),
            None => None,
        };
        if let Some((idea, forced)) = pick {
            if let Some(stage) = idea.current_stage() {
                launch(app.clone(), idea, stage, forced)?;
            }
        }
    }

    // portable brain: refresh once a day
    let last_brain: i64 = db::get_setting(&st.db.lock(), "brain_exported").and_then(|v| v.parse().ok()).unwrap_or(0);
    if ts - last_brain > 20 * 3600 {
        if let Err(e) = crate::brain::export(&st) {
            st.push_log(format!("[brain] {e:#}"));
        }
    }

    // morning digest at 07:30
    if now.hour() == 7 && now.minute() >= 30 || now.hour() > 7 {
        let today = now.format("%Y-%m-%d").to_string();
        let last = db::get_setting(&st.db.lock(), "digest_day");
        if last.as_deref() != Some(today.as_str()) {
            db::set_setting(&st.db.lock(), "digest_day", &today)?;
            let d = reports::make_digest(&st)?;
            notify(app, "☀️ สรุปเช้านี้พร้อมแล้ว", d.get("headline").and_then(|v| v.as_str()).unwrap_or(""));
        }
    }
    Ok(())
}

fn next_idea(st: &AppState) -> Option<Idea> {
    let ideas = db::list_ideas(&st.db.lock()).ok()?;
    let now = db::now();
    let grace = db::get_setting(&st.db.lock(), "grace_secs").and_then(|v| v.trim().parse::<i64>().ok()).filter(|v| *v >= 0).unwrap_or(90);
    ideas
        .into_iter()
        // ideas the owner is driving through Remote Control are left alone
        .find(|i| {
            (i.status == "inbox" || i.status == "active")
                && i.current_stage().is_some()
                && !crate::remote::held(st, i.id)
                // a just-typed (or just-edited) idea waits a little: time to notice a slip and fix the text
                && !(i.status == "inbox" && now - i.updated < grace)
        })
}

fn import_inbox(app: &AppHandle) {
    let st = app.state::<AppState>();
    let mut dirs = vec![st.ws.join("inbox")];
    if let Some(s) = crate::sync::sync_dir(&st).filter(|_| !crate::sync::is_follower(&st)) {
        dirs.push(s.join("inbox"));
    }
    if let Some(extra) = db::get_setting(&st.db.lock(), "extra_inbox") {
        dirs.extend(extra.split(';').map(str::trim).filter(|x| !x.is_empty()).map(std::path::PathBuf::from));
    }
    for dir in dirs {
        import_dir(&st, &dir);
    }
}

fn import_dir(st: &AppState, dir: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("");
        if !p.is_file() || !(ext == "md" || ext == "txt") {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(&p) {
            let ok = match crate::sync::forward(st, "add", serde_json::json!({ "text": text })) {
                Some(r) => r.is_ok(),
                None => add_idea(st, &text).is_ok(),
            };
            if !text.trim().is_empty() && ok {
                let done = dir.join("imported");
                let _ = std::fs::create_dir_all(&done);
                let _ = std::fs::rename(&p, done.join(e.file_name()));
            }
        }
    }
}

/// Parse a one-line idea ("!" = urgent, "?" = research only) and create its folder.
pub fn add_idea(st: &AppState, text: &str) -> anyhow::Result<i64> {
    let mut t = text.trim();
    let mut priority = 0;
    let mut research_only = false;
    loop {
        if let Some(r) = t.strip_prefix('!') {
            priority = 1;
            t = r.trim_start();
        } else if let Some(r) = t.strip_prefix('?') {
            research_only = true;
            t = r.trim_start();
        } else {
            break;
        }
    }
    anyhow::ensure!(!t.is_empty(), "ไอเดียว่าง");
    let title: String = t.lines().next().unwrap_or(t).chars().take(80).collect();
    let c = st.db.lock();
    let next_id: i64 = c.query_row("SELECT COALESCE(MAX(id),0)+1 FROM ideas", [], |r| r.get(0))?;
    let slug = workspace::make_slug(next_id, &title);
    workspace::create_idea_folder(&st.ws, &slug, &title, t)?;
    let id = db::insert_idea(&c, &slug, &title, t, priority, research_only)?;
    db::log_event(&c, "added", Some(id), &format!("จดไอเดีย: {title}"));
    drop(c);
    st.rt.lock().sync_dirty = true;
    st.wake.notify_one();
    Ok(id)
}

/// A tier forced in settings (per stage first, then global); "auto" or unset = router decides.
/// Claude family names from older settings (haiku/sonnet/opus) still work.
fn forced_model(st: &AppState, stage: &str) -> Option<String> {
    let c = st.db.lock();
    db::get_setting(&c, &format!("model_{stage}"))
        .filter(|v| !v.is_empty() && v != "auto")
        .or_else(|| db::get_setting(&c, "model_default").filter(|v| !v.is_empty() && v != "auto"))
}

/// An effort forced in settings (per stage first, then global); unset or "auto" = the router decides.
fn forced_effort(st: &AppState, stage: &str) -> Option<String> {
    let c = st.db.lock();
    let ok = |v: &String| ["low", "medium", "high", "xhigh"].contains(&v.as_str());
    db::get_setting(&c, &format!("effort_{stage}")).filter(ok).or_else(|| db::get_setting(&c, "effort_default").filter(ok))
}

fn launch(app: AppHandle, idea: Idea, stage: String, forced: bool) -> anyhow::Result<()> {
    let st = app.state::<AppState>();
    let dir = workspace::idea_dir(&st.ws, &idea.slug);
    workspace::create_idea_folder(&st.ws, &idea.slug, &idea.title, &idea.body)?;
    if let Err(e) = workspace::ensure_repo(&dir, &idea, &crate::remote::ia_path()) {
        st.push_log(format!("[git] {e:#}"));
    }
    let others = {
        let c = st.db.lock();
        let all = db::list_ideas(&c)?;
        let others: Vec<Idea> = all.into_iter().filter(|i| i.id != idea.id).collect();
        workspace::write_inputs(&dir, &db::directives(&c, idea.id)?, &db::questions(&c, Some(idea.id), false)?)?;
        workspace::ideas_summary_for_context(&others)
    };
    let _ = std::fs::remove_file(dir.join("STATUS.json"));
    let prompt = prompts::build_prompt(&idea, &stage, &others);
    let decision = st.rt.lock().decision.clone();
    let learned = crate::router::Learned {
        offset: st.rt.lock().policy.offset(&stage),
        probe: crate::tuner::should_probe(&st.db.lock(), &stage),
    };
    let mut choice = crate::router::choose(&stage, &idea, &decision, forced_model(&st, &stage).as_deref(), learned);
    if let Some(e) = forced_effort(&st, &stage) {
        choice.effort = e;
        choice.reason.push_str(" · ตั้ง effort ไว้เอง");
    }
    let minutes = prompts::stage(&stage).map(|s| s.minutes).unwrap_or(30);
    let (provider, model_name, custom_cmd) = {
        let c = st.db.lock();
        let p = crate::providers::chosen(&c);
        // a tier name goes through the agent's tier map; anything else is an exact model the user typed
        let m = match crate::router::tier_of(&choice.model) {
            Some(t) => crate::providers::tier_model(&c, &p, t),
            None => choice.model.clone(),
        };
        (p, m, db::get_setting(&c, "custom_cmd").unwrap_or_default())
    };
    let (week, five) = {
        let rt = st.rt.lock();
        (rt.snap.as_ref().map(|s| s.week_pct), rt.snap.as_ref().map(|s| s.five_pct))
    };
    let run_id = {
        let c = st.db.lock();
        let id = db::start_run(&c, idea.id, &stage, &choice, week, five)?;
        let label = prompts::stage(&stage).map(|s| s.label).unwrap_or("");
        db::log_event(&c, "run", Some(idea.id), &format!("เริ่ม{label}: {}", idea.title));
        id
    };
    // recipes for this run: one variant per family, copied into the idea folder (the agent may only work there)
    {
        let c = st.db.lock();
        let offers = crate::skills::select(&c);
        crate::skills::record_offered(&c, run_id, &offers);
        crate::skills::materialize(&st.root, &dir, &offers);
        if !offers.is_empty() {
            let names: Vec<&str> = offers.iter().map(|o| o.name.as_str()).collect();
            drop(c);
            st.push_log(format!("🧰 สกิลที่เสนอให้รอบนี้: {}", names.join(", ")));
        }
    }
    let cancel = Arc::new(Notify::new());
    {
        let mut rt = st.rt.lock();
        rt.current = Some(CurrentRun {
            run_id,
            idea_id: idea.id,
            stage: stage.clone(),
            title: idea.title.clone(),
            started: db::now(),
            forced,
            deadline: db::now() + ((minutes + 5) * 60) as i64,
            stage_no: idea.stage_idx + 1,
            stage_total: idea.recipe.len() as i64,
            last_activity: db::now(),
            last_line: String::new(),
            provider: crate::providers::label(&provider),
            model: if model_name.is_empty() { choice.model.clone() } else { model_name.clone() },
        });
        rt.cancel = Some(cancel.clone());
        rt.live_log.clear();
    }
    st.push_log(format!(
        "▶ {} — {} ({} · {}{} · {} — {})",
        idea.title,
        stage,
        crate::providers::label(&provider),
        choice.model,
        if model_name.is_empty() { String::new() } else { format!(" = {model_name}") },
        choice.effort,
        choice.reason
    ));

    tauri::async_runtime::spawn(async move {
        let st = app.state::<AppState>();
        let log_path = dir.join("logs").join(format!("run-{run_id:05}-{stage}.jsonl"));
        let app2 = app.clone();
        let app3 = app.clone();
        let res = runner::run(
            runner::RunSpec {
                cwd: &dir,
                prompt,
                provider,
                model: model_name,
                effort: choice.effort,
                tools: choice.tools,
                custom_cmd,
                timeout_mins: minutes + 5,
                log_path: &log_path,
            },
            cancel,
            move |line| {
                let st = app2.state::<AppState>();
                if let Some(c) = st.rt.lock().current.as_mut() {
                    c.last_activity = db::now();
                    if let Some(l) = line.lines().rev().find(|l| !l.trim().is_empty()) {
                        c.last_line = l.trim().chars().take(160).collect();
                    }
                }
                if !line.is_empty() {
                    st.push_log(line);
                    let _ = app2.emit("live-log", ());
                }
            },
            move |q| {
                // live quota while the bot works; the loop re-checks limits right away
                let st = app3.state::<AppState>();
                let changed = st.rt.lock().snap.as_ref().map(|s| (s.five_pct - q.five_pct).abs() >= 1.0).unwrap_or(true);
                st.rt.lock().snap = Some(q);
                if changed {
                    st.wake.notify_one();
                }
            },
        )
        .await;
        let outcome = res.unwrap_or_else(|e| runner::Outcome {
            status: "error".into(),
            error: Some(format!("{e:#}")),
            ..Default::default()
        });
        if let Err(e) = finish(&app, idea.id, run_id, &stage, &outcome) {
            st.push_log(format!("[finish] {e:#}"));
        }
        {
            let mut rt = st.rt.lock();
            rt.current = None;
            rt.cancel = None;
            rt.last_fetch = 0; // refresh quota right after a run
            rt.sync_dirty = true;
            rt.last_learn = 0; // re-learn the model policy with this run's result
        }
        st.wake.notify_one();
        let _ = app.emit("state-changed", ());
    });
    Ok(())
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct StatusFile {
    summary: Option<String>,
    title: Option<String>,
    /// short English name for the folder (clarify stage), e.g. "phi peaks card game"
    slug_en: Option<String>,
    intents: Option<Vec<String>>,
    recipe: Option<Vec<String>>,
    recipe_remaining: Option<Vec<String>>,
    verdict: Option<String>,
    questions: Vec<StatusQuestion>,
    repeat: bool,
    ready: Option<Ready>,
    knowledge: bool,
    needs_from_user: Vec<String>,
    /// skills the agent really opened and followed this run
    skills_used: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct StatusQuestion {
    q: String,
    options: Vec<String>,
    guess: Option<String>,
}

#[derive(Debug, Default, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Ready {
    pub how_to_run: String,
    pub cwd: String,
    /// a file to open with its default app (e.g. index.html), relative to the idea folder
    pub open: String,
}

fn clean_recipe(v: Vec<String>, research_only: bool) -> Vec<String> {
    v.into_iter()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| prompts::is_known(s))
        .filter(|s| !research_only || matches!(s.as_str(), "research" | "verdict" | "learn" | "experiment"))
        .collect()
}

fn finish(app: &AppHandle, idea_id: i64, run_id: i64, stage: &str, out: &runner::Outcome) -> anyhow::Result<()> {
    let st = app.state::<AppState>();
    let mut idea = db::get_idea(&st.db.lock(), idea_id)?;
    let dir = workspace::idea_dir(&st.ws, &idea.slug);
    let status_path = dir.join("STATUS.json");
    let status: Option<StatusFile> = std::fs::read_to_string(&status_path)
        .ok()
        .and_then(|s| serde_json::from_str(s.trim_start_matches('\u{feff}')).ok());
    if status_path.exists() {
        let _ = std::fs::rename(&status_path, dir.join("logs").join(format!("status-{run_id:05}.json")));
    }

    let (week_after, five_after) = {
        let rt = st.rt.lock();
        (rt.snap.as_ref().map(|s| s.week_pct), rt.snap.as_ref().map(|s| s.five_pct))
    };
    db::finish_run(
        &st.db.lock(),
        &db::Run {
            id: run_id,
            status: out.status.clone(),
            input_tokens: out.input_tokens,
            output_tokens: out.output_tokens,
            cache_read: out.cache_read,
            cache_write: out.cache_write,
            cost_usd: out.cost_usd,
            num_turns: out.num_turns,
            session_id: out.session_id.clone(),
            model_id: out.model_id.clone(),
            week_after,
            five_after,
            error: out.error.clone(),
            ..Default::default()
        },
    )?;
    {
        // which offered skills were really used (the agent's own report + the marker left when a skill file is read)
        let reported = status.as_ref().map(|s| s.skills_used.clone()).unwrap_or_default();
        let log = dir.join("logs").join(format!("run-{run_id:05}-{stage}.jsonl"));
        crate::skills::finish_run(&st.db.lock(), run_id, &log, &reported);
    }

    // every problem is written down in the idea's ISSUES.md with its likely cause and what to do
    if matches!(out.status.as_str(), "error" | "timeout" | "rate_limited") {
        workspace::log_issue(&dir, run_id, stage, &out.status, out.error.as_deref().unwrap_or(""), false);
    }
    // if the user archived/stopped the idea meanwhile, only record the run
    if !(idea.status == "inbox" || idea.status == "active") {
        return Ok(());
    }

    match out.status.as_str() {
        "cancelled" => return Ok(()),
        "rate_limited" => {
            st.rt.lock().cooldown_until = db::now() + 20 * 60;
            db::log_event(&st.db.lock(), "limit", Some(idea_id), "ชนลิมิต — พัก 20 นาทีแล้วลองใหม่");
            return Ok(());
        }
        "error" => {
            idea.fail_count += 1;
            if idea.fail_count >= 3 {
                idea.status = "failed".into();
                idea.summary = format!("ทำขั้น {stage} ไม่สำเร็จ 3 ครั้ง: {}", out.error.clone().unwrap_or_default().chars().take(120).collect::<String>());
                notify_idea(app, idea.id, &format!("⚠️ ไอเดียติดปัญหา · #{}", idea.id), &format!("{} — ดูสาเหตุและวิธีแก้ในแท็บ “ปัญหา”", idea.title));
            }
            db::save_idea(&st.db.lock(), &idea)?;
            db::log_event(&st.db.lock(), "error", Some(idea_id), &format!("ขั้น {stage} ผิดพลาด"));
            return Ok(());
        }
        _ => {}
    }

    if idea.fail_count > 0 {
        workspace::log_issue(&dir, run_id, stage, "recovered", &format!("{}", idea.fail_count), true);
    }
    idea.fail_count = 0;
    let timed_out = out.status == "timeout";
    let s = status.unwrap_or_default();
    if let Some(sum) = s.summary.as_ref().filter(|x| !x.trim().is_empty()) {
        let _ = st.db.lock().execute("UPDATE runs SET summary=?2 WHERE id=?1", rusqlite::params![run_id, sum.trim()]);
    }
    if let Some(sum) = s.summary.as_ref().filter(|x| !x.trim().is_empty()) {
        idea.summary = sum.trim().to_string();
    }
    // keep at most 3 open questions per idea so the user isn't buried
    let existing = db::questions(&st.db.lock(), Some(idea.id), false)?;
    let mut open = existing.iter().filter(|q| q.answer.is_none()).count();
    for q in s.questions.iter().filter(|q| !q.q.trim().is_empty()) {
        if open >= 3 || existing.iter().any(|e| e.question == q.q.trim()) {
            continue;
        }
        db::insert_question(&st.db.lock(), idea.id, q.q.trim(), &q.options, q.guess.as_deref())?;
        open += 1;
    }
    if let Some(r) = s.ready.as_ref().filter(|r| !r.how_to_run.trim().is_empty() || !r.open.trim().is_empty()) {
        idea.run_cmd = Some(serde_json::to_string(r)?);
    }
    if s.knowledge {
        idea.knowledge = true;
        if let Ok(k) = std::fs::read_to_string(dir.join("KNOWLEDGE.md")) {
            let _ = std::fs::write(st.ws.join("knowledge").join(format!("{}.md", idea.slug)), k);
        }
    }
    for n in s.needs_from_user {
        if !idea.needs.contains(&n) {
            idea.needs.push(n);
        }
    }

    let mut english: Option<String> = None;
    match stage {
        "clarify" => {
            if let Some(t) = s.title.filter(|t| !t.trim().is_empty()) {
                idea.title = t.trim().chars().take(60).collect();
            }
            english = s.slug_en.filter(|t| !t.trim().is_empty());
            if let Some(i) = s.intents {
                idea.intents = i;
            }
            let mut recipe = clean_recipe(s.recipe.unwrap_or_default(), idea.research_only);
            if recipe.is_empty() {
                recipe = vec!["research".into(), "verdict".into()];
            }
            idea.recipe = recipe;
            idea.stage_idx = 0;
            idea.status = "active".into();
        }
        _ if idea.recipe.get(idea.stage_idx as usize).map(String::as_str) != Some(stage) => {
            // the user steered this idea while the stage was running; keep their new plan
            if let Some(v) = s.verdict {
                idea.verdict = Some(v.to_uppercase());
            }
        }
        _ => {
            let at = (idea.stage_idx as usize).min(idea.recipe.len());
            if let Some(rest) = s.recipe_remaining {
                let mut r: Vec<String> = idea.recipe[..(at + 1).min(idea.recipe.len())].to_vec();
                r.extend(clean_recipe(rest, idea.research_only));
                idea.recipe = r;
            }
            if let Some(v) = s.verdict {
                idea.verdict = Some(v.to_uppercase());
            }
            let again = (s.repeat || (timed_out && stage == "build")) && idea.repeat_count < MAX_REPEATS;
            if again {
                idea.repeat_count += 1;
            } else {
                idea.stage_idx += 1;
                idea.repeat_count = 0;
            }
            if stage == "verdict" && idea.verdict.as_deref() == Some("NO-GO") {
                idea.stage_idx = idea.recipe.len() as i64;
            }
        }
    }

    let finished = idea.status == "active" && idea.stage_idx as usize >= idea.recipe.len();
    if finished {
        idea.status = if idea.run_cmd.is_some() { "ready".into() } else { "done".into() };
    }
    db::save_idea(&st.db.lock(), &idea)?;
    let label = prompts::stage(stage).map(|s| s.label).unwrap_or(stage);
    // history for every idea: one commit per stage, so any agent/tool can diff or roll back
    let _ = workspace::ensure_repo(&dir, &idea, &crate::remote::ia_path());
    workspace::commit(&dir, &format!("bot: {label} — {}", idea.title));
    if let Some(en) = english {
        if let Err(e) = rename_to_english(&st, &mut idea, &en) {
            st.push_log(format!("[rename] {e:#}"));
        }
    }
    db::log_event(&st.db.lock(), "stage", Some(idea.id), &format!("{label}เสร็จ: {}", idea.title));
    // progress to the phone after every stage (the final "ready / finished" message is sent separately)
    if !finished && db::get_setting(&st.db.lock(), "telegram_progress").as_deref() != Some("0") {
        crate::chat::push(app, format!("⏳ #{} {} — {label}เสร็จ ({}/{})", idea.id, idea.title, idea.stage_idx, idea.recipe.len()));
    }
    if finished {
        let (t, b) = if idea.status == "ready" {
            ("▶ พร้อมใช้งานแล้ว", format!("ไอเดีย #{}: {} — เปิดแอปแล้วกด ▶ ลองเล่น", idea.id, idea.title))
        } else {
            ("✓ ไอเดียเสร็จแล้ว", format!("ไอเดีย #{}: {} — {}", idea.id, idea.title, idea.summary))
        };
        db::log_event(&st.db.lock(), "finished", Some(idea.id), &format!("{t}: {}", idea.title));
        notify_idea(app, idea.id, &format!("{t} · #{}", idea.id), &b);
    }
    Ok(())
}

/// Give a folder that is only `idea-0016` its English name (`idea-0016-shared-group-calendar`) so
/// it can be recognised at a glance. Skipped while a live session works inside the folder.
pub fn rename_to_english(st: &AppState, idea: &mut Idea, english: &str) -> anyhow::Result<()> {
    if workspace::has_english_name(&idea.slug) || st.rt.lock().sessions.contains_key(&idea.id) {
        return Ok(());
    }
    let Some(new) = workspace::slug_with_english(idea.id, english) else { return Ok(()) };
    let (from, to) = (workspace::idea_dir(&st.ws, &idea.slug), workspace::idea_dir(&st.ws, &new));
    if to.exists() {
        anyhow::bail!("{} มีอยู่แล้ว", to.display());
    }
    std::fs::rename(&from, &to)?;
    let k = st.ws.join("knowledge");
    let _ = std::fs::rename(k.join(format!("{}.md", idea.slug)), k.join(format!("{new}.md")));
    if let Some(sd) = crate::sync::sync_dir(st) {
        let w = sd.join("workspace").join("ideas");
        let _ = std::fs::rename(w.join(&idea.slug), w.join(&new));
    }
    db::set_slug(&st.db.lock(), idea.id, &new)?;
    db::log_event(&st.db.lock(), "rename", Some(idea.id), &format!("ตั้งชื่อโฟลเดอร์ {} → {new}", idea.slug));
    idea.slug = new;
    Ok(())
}

/// Change the text of an idea after it was added (a slip of the Enter key, a better wording).
/// Before the bot has gone beyond interpretation the idea starts over from a clean slate, so the
/// title, the English folder name and the plan all follow the new text; later, the bot is told.
pub fn edit_idea(st: &AppState, id: i64, text: &str) -> anyhow::Result<()> {
    let mut t = text.trim();
    let mut priority = None;
    let mut research_only = None;
    loop {
        if let Some(r) = t.strip_prefix('!') {
            priority = Some(1);
            t = r.trim_start();
        } else if let Some(r) = t.strip_prefix('?') {
            research_only = Some(true);
            t = r.trim_start();
        } else {
            break;
        }
    }
    anyhow::ensure!(!t.is_empty(), "ไอเดียว่าง");
    let title: String = t.lines().next().unwrap_or(t).chars().take(80).collect();
    let mut idea = db::get_idea(&st.db.lock(), id)?;
    let old_slug = idea.slug.clone();
    let early = idea.status == "inbox" || idea.stage_idx <= 1;
    idea.title = title.clone();
    idea.body = t.to_string();
    if let Some(p) = priority {
        idea.priority = p;
    }
    if let Some(r) = research_only {
        idea.research_only = r;
    }
    let busy = st.rt.lock().sessions.contains_key(&id) || st.rt.lock().current.as_ref().map(|c| c.idea_id) == Some(id);
    if early && !busy {
        idea.recipe.clear();
        idea.intents.clear();
        idea.stage_idx = 0;
        idea.summary.clear();
        idea.status = "inbox".into();
        idea.repeat_count = 0;
        idea.fail_count = 0;
        // folder name follows the new text again (the AI gives it an English name when it re-reads it)
        let new_slug = workspace::make_slug(id, &title);
        let (from, to) = (workspace::idea_dir(&st.ws, &old_slug), workspace::idea_dir(&st.ws, &new_slug));
        if new_slug != old_slug && from.is_dir() && !to.exists() {
            std::fs::rename(&from, &to)?;
            let k = st.ws.join("knowledge");
            let _ = std::fs::rename(k.join(format!("{old_slug}.md")), k.join(format!("{new_slug}.md")));
            db::set_slug(&st.db.lock(), id, &new_slug)?;
            idea.slug = new_slug;
        }
    }
    db::save_idea(&st.db.lock(), &idea)?;
    workspace::rewrite_idea_md(&workspace::idea_dir(&st.ws, &idea.slug), &idea.title, &idea.body);
    db::log_event(&st.db.lock(), "edited", Some(id), &format!("แก้ข้อความไอเดีย: {title}"));
    if !early || busy {
        steer(st, id, &format!("เจ้าของแก้ข้อความไอเดียใหม่ (อ่าน IDEA.md ส่วน “ไอเดียจากเจ้าของ”): {title}"))?;
    }
    st.rt.lock().sync_dirty = true;
    st.wake.notify_one();
    Ok(())
}

/// Insert a "replan" step at the current position so the bot re-reads the new directive.
pub fn steer(st: &AppState, id: i64, directive: &str) -> anyhow::Result<()> {
    let c = st.db.lock();
    let mut idea = db::get_idea(&c, id)?;
    db::add_directive(&c, id, directive)?;
    if idea.status != "inbox" {
        let at = (idea.stage_idx as usize).min(idea.recipe.len());
        idea.recipe.insert(at, "replan".into());
        idea.stage_idx = at as i64;
        idea.repeat_count = 0;
        idea.fail_count = 0;
        idea.status = "active".into();
    }
    db::save_idea(&c, &idea)?;
    db::log_event(&c, "steer", Some(id), &format!("สั่งงาน: {directive}"));
    drop(c);
    st.wake.notify_one();
    Ok(())
}

pub fn state_json(st: &AppState) -> serde_json::Value {
    // looked up before taking the runtime lock: it may start a short `where` process
    let providers = crate::providers::summary(&st.db.lock());
    let onboarded = db::get_setting(&st.db.lock(), "onboarded").as_deref() == Some("1");
    let seen_version = db::get_setting(&st.db.lock(), "seen_version");
    let rt = st.rt.lock();
    json!({
        "decision": rt.decision,
        "snap": rt.snap,
        "snap_note": rt.snap_note,
        "current": rt.current,
        "paused": rt.paused,
        "turbo_until": rt.turbo_until,
        "user_active": rt.user_active,
        "reserve_per_hour": rt.reserve_per_hour,
        "reserve_learned": rt.reserve_learned,
        "cooldown_until": rt.cooldown_until,
        "role": rt.role,
        "sessions": rt.sessions.values().collect::<Vec<_>>(),
        "agents": crate::agents::list(),
        "version": env!("CARGO_PKG_VERSION"),
        "seen_version": seen_version,
        "providers": providers,
        "onboarded": onboarded,
        "holds": rt.holds.iter().filter(|(_, t)| **t > crate::db::now()).map(|(i, _)| *i).collect::<Vec<_>>(),
        "machine": crate::sync::machine(),
    })
}
