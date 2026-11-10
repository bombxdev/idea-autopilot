//! Which models can do the bot's work, tried for real.
//!
//! A bot step is not a chat answer: the agent reads a long task file, works with tools (read, write, run) and ends by
//! writing STATUS.json. Small models answer a chat fine but often drop out of that (idea #22: a free model read 10 lines
//! of its task, decided the file was empty and stopped). The model's own description does not tell — that one is listed
//! with tool support and a 256k context. So each model is given a short job of the same shape, through the same agent
//! and account the bot would use:
//!   a long task file whose real instruction is at the very end → read task.txt → write answer.txt → write STATUS.json.
//! Verdicts: "bot" (did it) · "chat" (could not: shown, but only for chatting) · "unknown" (could not be tried: no
//! credit, a limit, the network — tried again later).
//!
//! Results live in the app's own database (setting `model_checks`) and the ones known when a version is built ship
//! with it (`model_checks.json`), so a new install starts with the list already sorted. A model nobody has tried yet is
//! tried in the background while the bot is idle; a model that fails a real run is tried again.
use crate::accounts::Account;
use crate::state::AppState;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::sync::LazyLock;
use tauri::{AppHandle, Manager};

/// The results known when this version was built (see `export`).
const SHIPPED: &str = include_str!("../model_checks.json");
const RETRY_UNKNOWN_SECS: i64 = 3 * 24 * 3600;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Verdict {
    /// bot | chat | unknown
    pub v: String,
    pub ts: i64,
    #[serde(default)]
    pub secs: i64,
    #[serde(default)]
    pub note: String,
}

/// Only these agents get long model lists of mixed ability; Claude Code, Codex and Gemini CLI offer their own
/// coding models, which all do the work.
pub fn applies(kind: &str) -> bool {
    kind == "opencode" || kind == "mistral" || kind == crate::omniroute::VENDOR
}

/// What a verdict is filed under: the agent, except OmniRoute — its models run through Claude Code but are the
/// gateway's own list (any provider's model), so they are checked apart from Claude's.
pub fn check_kind(a: &Account) -> String {
    if crate::omniroute::is(a) { crate::omniroute::VENDOR.to_string() } else { a.kind.clone() }
}

pub fn key(kind: &str, model: &str) -> String {
    format!("{kind}|{model}")
}

fn shipped() -> BTreeMap<String, Verdict> {
    serde_json::from_str::<serde_json::Value>(SHIPPED)
        .ok()
        .and_then(|v| serde_json::from_value(v["checks"].clone()).ok())
        .unwrap_or_default()
}

fn local(c: &rusqlite::Connection) -> BTreeMap<String, Verdict> {
    crate::db::get_setting(c, "model_checks").and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

/// Shipped results, overridden by what this machine found itself.
pub fn all(c: &rusqlite::Connection) -> BTreeMap<String, Verdict> {
    let mut m = shipped();
    m.extend(local(c));
    m
}

pub fn record(c: &rusqlite::Connection, k: &str, v: Verdict) {
    let mut m = local(c);
    m.insert(k.to_string(), v);
    if let Ok(s) = serde_json::to_string(&m) {
        let _ = crate::db::set_setting(c, "model_checks", &s);
    }
}

/// Forget this machine's verdict so the model is tried again (a real run of it delivered nothing).
pub fn retry(c: &rusqlite::Connection, k: &str) {
    let mut m = local(c);
    m.insert(k.to_string(), Verdict { v: "retry".into(), ts: crate::db::now(), secs: 0, note: "ทำงานจริงไม่ส่งผล — รอตรวจใหม่".into() });
    if let Ok(s) = serde_json::to_string(&m) {
        let _ = crate::db::set_setting(c, "model_checks", &s);
    }
}

/// The owner asked to try this model again: it waits for the new try (hidden from the list meanwhile).
pub fn retry_pending(c: &rusqlite::Connection, k: &str) {
    let mut m = local(c);
    m.insert(k.to_string(), Verdict { v: "pending".into(), ts: crate::db::now(), secs: 0, note: "รอตรวจ".into() });
    if let Ok(s) = serde_json::to_string(&m) {
        let _ = crate::db::set_setting(c, "model_checks", &s);
    }
}

/// "bot" | "chat" | "unknown" | "pending" (not tried yet, or due again) | "retry" (failed a real run: avoided until tried again).
pub fn state(all: &BTreeMap<String, Verdict>, kind: &str, model: &str, now: i64) -> String {
    if !applies(kind) {
        return "bot".into();
    }
    match all.get(&key(kind, model)) {
        Some(v) if v.v == "unknown" && now - v.ts > RETRY_UNKNOWN_SECS => "pending".into(),
        Some(v) => v.v.clone(),
        None => "pending".into(),
    }
}

// ---------- the trial job ----------

fn secret_word(seed: u64) -> String {
    const W: [&str; 12] = ["mango", "rambutan", "durian", "lychee", "papaya", "longan", "guava", "pomelo", "tamarind", "jackfruit", "mangosteen", "salak"];
    format!("{}{}", W[(seed % 12) as usize], 10 + seed % 89)
}

/// The task file: house rules first (as long as a real one), the job itself at the very end, so a model that only
/// skims the top never finds it.
pub fn trial_prompt() -> String {
    let mut s = String::from("# Idea Autopilot — model check\n\nYou are a coding agent working inside this folder. Read ALL of this file before doing anything: the actual job is in the last section.\n\n## House rules\n");
    let rules = [
        "Work only inside the current folder; never touch files elsewhere.",
        "Prefer small, verifiable steps; check each result before moving on.",
        "Keep every file you write in UTF-8 without a byte order mark.",
        "Do not ask the user questions: nobody is watching this run. Make reasonable assumptions.",
        "Never install software, sign up for services, or spend money.",
        "When a step fails, read the error, fix the cause, and try again once.",
        "Write plain, short sentences in any notes you leave.",
        "Do not print secrets, keys, or tokens in any file or message.",
    ];
    for n in 0..9 {
        for (i, r) in rules.iter().enumerate() {
            s.push_str(&format!("{}. {r}\n", n * rules.len() + i + 1));
        }
    }
    s.push_str(
        "\n## The job (do exactly this)\n\
         1. Read the file `task.txt` in the current folder. It contains one word.\n\
         2. Create the file `answer.txt` containing only that word in UPPERCASE letters.\n\
         3. Create the file `STATUS.json` containing exactly: {\"summary\": \"done\", \"word\": \"<the word in UPPERCASE>\"}\n\
         4. Reply with the single word DONE.\n",
    );
    s
}

/// Did the agent do the job? Ok(()) or what was missing.
pub fn judge(dir: &std::path::Path, word: &str) -> Result<(), String> {
    let want = word.to_uppercase();
    let answer = std::fs::read_to_string(dir.join("answer.txt")).map_err(|_| "ไม่ได้เขียน answer.txt".to_string())?;
    if answer.trim().trim_start_matches('\u{feff}') != want {
        return Err(format!("answer.txt ผิด: “{}”", answer.trim().chars().take(40).collect::<String>()));
    }
    let raw = std::fs::read_to_string(dir.join("STATUS.json")).map_err(|_| "ไม่ได้เขียน STATUS.json".to_string())?;
    let v: serde_json::Value = serde_json::from_str(raw.trim().trim_start_matches('\u{feff}')).map_err(|_| "STATUS.json ไม่ใช่ JSON".to_string())?;
    if v["word"].as_str().map(|w| w.trim().to_uppercase()) != Some(want) {
        return Err("STATUS.json ไม่มีคำที่ถูกต้อง".into());
    }
    Ok(())
}

/// Could not be tried (as opposed to: tried and failed).
fn untried(text: &str) -> bool {
    let l = text.to_lowercase();
    ["credit", "afford", "insufficient", "quota", "rate limit", "rate_limit", "too many requests", "429", "401", "403", "unauthorized", "billing", "payment", "usage limit", "model unavailable", "not found", "no endpoints", "network", "timed out connecting", "connection", "api key", "api_key", "permission", "access denied", "forbidden", "not available for", "subscription"]
        .iter()
        .any(|w| l.contains(w))
}

/// Give one model the trial job through this account (blocking up to ~4 minutes).
pub async fn trial(app: &AppHandle, a: &Account, model: &str) -> Verdict {
    let st = app.state::<AppState>();
    let started = std::time::Instant::now();
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(7);
    let word = secret_word(seed);
    let dir = st.root.join("data").join("model-check").join(format!("{}", seed % 1_000_000));
    let _ = std::fs::create_dir_all(dir.join("logs"));
    let _ = std::fs::write(dir.join("task.txt"), format!("{word}\n"));
    let lines = std::sync::Arc::new(Mutex::new(Vec::<String>::new()));
    let sink = lines.clone();
    let out = crate::runner::run(
        crate::runner::RunSpec {
            cwd: &dir,
            prompt: trial_prompt(),
            provider: a.kind.clone(),
            model: model.to_string(),
            effort: "low".into(),
            tools: "Read,Write,Edit,Bash".into(),
            custom_cmd: String::new(),
            timeout_mins: 4,
            log_path: &dir.join("logs").join("check.jsonl"),
            lang: "en",
            env: crate::accounts::env_with_secret(a, &st.root),
        },
        std::sync::Arc::new(tokio::sync::Notify::new()),
        move |l| sink.lock().push(l),
        |_| {},
    )
    .await;
    let secs = started.elapsed().as_secs() as i64;
    let said = lines.lock().iter().rev().take(4).cloned().collect::<Vec<_>>().join(" / ");
    let verdict = match (&out, judge(&dir, &word)) {
        (_, Ok(())) => Verdict { v: "bot".into(), ts: crate::db::now(), secs, note: String::new() },
        (Ok(o), Err(why)) => {
            // only the agent's own error decides "could not be tried" — the model's words ("file not found") do not
            let text = format!("{} {}", o.status, o.error.clone().unwrap_or_default());
            let lines_err = lines.lock().iter().any(|l| l.contains("Error:") && untried(l));
            if o.status == "rate_limited" || (o.status != "ok" && untried(&text)) || lines_err {
                Verdict { v: "unknown".into(), ts: crate::db::now(), secs, note: short(&text) }
            } else if o.status == "timeout" {
                Verdict { v: "chat".into(), ts: crate::db::now(), secs, note: "งานสั้นๆ ไม่เสร็จใน 4 นาที".into() }
            } else {
                Verdict { v: "chat".into(), ts: crate::db::now(), secs, note: short(&format!("{why} · {said}")) }
            }
        }
        (Err(e), Err(_)) => Verdict { v: "unknown".into(), ts: crate::db::now(), secs, note: short(&format!("{e:#}")) },
    };
    let _ = std::fs::remove_dir_all(&dir);
    verdict
}

fn short(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(160).collect()
}

// ---------- the background queue ----------

static QUEUE: LazyLock<Mutex<VecDeque<(String, String)>>> = LazyLock::new(|| Mutex::new(VecDeque::new()));
static BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Ask for these models of this account to be tried first ("check now").
pub fn enqueue_front(account: &str, models: &[String]) {
    let mut q = QUEUE.lock();
    for m in models.iter().rev() {
        let item = (account.to_string(), m.clone());
        q.retain(|x| *x != item);
        q.push_front(item);
    }
}

pub fn busy() -> bool {
    BUSY.load(std::sync::atomic::Ordering::SeqCst)
}

// ---------- how far a "check now" got (shown under the account) ----------

#[derive(Default, Clone)]
struct Progress {
    want: Vec<String>,
    done: usize,
    bot: usize,
    chat: usize,
    unknown: usize,
    started: i64,
    finished: i64,
    current: Option<(String, i64)>,
    last: Option<(String, String, String)>,
}

static PROGRESS: LazyLock<Mutex<std::collections::HashMap<String, Progress>>> = LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

/// A new "check now" for these models of this account.
pub fn progress_start(account: &str, models: &[String]) {
    let p = Progress { want: models.to_vec(), started: crate::db::now(), ..Default::default() };
    PROGRESS.lock().insert(account.to_string(), p);
}

fn progress_trying(account: &str, model: &str) {
    if let Some(p) = PROGRESS.lock().get_mut(account).filter(|p| p.finished == 0 && p.want.iter().any(|m| m == model)) {
        p.current = Some((model.to_string(), crate::db::now()));
    }
}

fn progress_done(account: &str, model: &str, v: &Verdict) {
    let mut all = PROGRESS.lock();
    let Some(p) = all.get_mut(account).filter(|p| p.finished == 0 && p.want.iter().any(|m| m == model)) else { return };
    p.done += 1;
    match v.v.as_str() {
        "bot" => p.bot += 1,
        "chat" => p.chat += 1,
        _ => p.unknown += 1,
    }
    p.current = None;
    p.last = Some((model.to_string(), v.v.clone(), v.note.clone()));
    if p.done >= p.want.len() {
        p.finished = crate::db::now();
    }
}

/// Where this account's "check now" is: done/total, the model being tried and since when, the tally, the last
/// verdict — or why nothing is being tried right now. None = no check asked (or it ended over an hour ago).
pub fn progress_json(st: &AppState, account: &str) -> Option<serde_json::Value> {
    let p = PROGRESS.lock().get(account).cloned()?;
    let now = crate::db::now();
    if p.finished > 0 && now - p.finished > 3600 {
        return None;
    }
    // nothing being tried: say what it waits for
    let waiting = if p.finished > 0 || p.current.is_some() {
        None
    } else if let Some(why) = crate::omniroute::blocked(account) {
        Some(format!("บัญชีนี้ถูกพักไว้ — {why}"))
    } else if let Some((until, _)) = crate::limits::get(&st.db.lock(), account, now) {
        Some(format!("บัญชีนี้ติดลิมิต — จะตรวจต่อเมื่อโควต้ากลับมา ({})", crate::limits::when_text(until, now)))
    } else if busy() {
        Some("กำลังตรวจโมเดลของบัญชีอื่นอยู่ (ตรวจได้ทีละตัว)".to_string())
    } else if st.rt.lock().current.is_some() {
        Some("รอบอททำงานรอบนี้เสร็จก่อน — ตรวจโมเดลได้ตอนบอทว่างเท่านั้น".to_string())
    } else {
        Some("กำลังจะเริ่มตัวถัดไป…".to_string())
    };
    Some(serde_json::json!({
        "total": p.want.len(), "done": p.done, "bot": p.bot, "chat": p.chat, "unknown": p.unknown,
        "started": p.started, "finished": p.finished,
        "current": p.current.as_ref().map(|c| c.0.clone()), "current_since": p.current.as_ref().map(|c| c.1),
        "last": p.last.as_ref().map(|l| serde_json::json!({"model": l.0, "v": l.1, "note": l.2})),
        "waiting": waiting,
    }))
}

/// Called every scheduler tick: while the bot is idle, try the next model nobody has tried yet (one at a time).
pub fn tick(app: &AppHandle) {
    let st = app.state::<AppState>();
    if busy() || st.rt.lock().current.is_some() {
        return;
    }
    let accounts: Vec<Account> = crate::accounts::list(&st.db.lock()).into_iter().filter(|a| a.enabled && applies(&check_kind(a)) && crate::omniroute::blocked(&a.id).is_none()).collect();
    // the model list of each account (read in the background; new models show up here and get checked)
    for a in &accounts {
        crate::acct_models::refresh_bg(a, &st.root);
    }
    let next = {
        let c = st.db.lock();
        let all = all(&c);
        let now = crate::db::now();
        let pending = |a: &Account, m: &str| matches!(state(&all, &check_kind(a), m, now).as_str(), "pending" | "retry") && crate::limits::get(&c, &a.id, now).is_none();
        let mut q = QUEUE.lock();
        let mut found = None;
        let mut later = VecDeque::new();
        while let Some((id, m)) = q.pop_front() {
            if let Some(a) = accounts.iter().find(|a| a.id == id) {
                // the account waits (a limit): keep its models queued instead of losing them
                if crate::limits::get(&c, &a.id, now).is_some() {
                    later.push_back((id, m));
                    continue;
                }
                if pending(a, &m) {
                    found = Some((a.clone(), m));
                    break;
                }
                // already has a verdict: nothing to try, but a "check now" counts it as done
                let k = check_kind(a);
                if let Some(v) = all.get(&key(&k, &m)) {
                    progress_done(&a.id, &m, v);
                }
            } else if crate::omniroute::blocked(&id).is_some() {
                later.push_back((id, m));
            }
        }
        while let Some(x) = later.pop_back() {
            q.push_front(x);
        }
        drop(q);
        // then any listed model of an account that has never been tried, free ones first (they cost nothing)
        found.or_else(|| {
            accounts.iter().find_map(|a| {
                let list = crate::acct_models::cached(&a.id)?;
                let mut todo: Vec<&String> = list.iter().filter(|m| pending(a, m)).collect();
                todo.sort_by_key(|m| !crate::catalog::is_free_model(m));
                todo.first().map(|m| (a.clone(), (*m).clone()))
            })
        })
    };
    let Some((a, model)) = next else { return };
    // OmniRoute's catalog already says this one cannot call tools (read/write files, run commands): no trial needed
    if crate::omniroute::is(&a) && crate::omniroute::no_tools(&a.id, &model) {
        let v = Verdict { v: "chat".into(), ts: crate::db::now(), secs: 0, note: "OmniRoute บอกว่าโมเดลนี้เรียกเครื่องมือไม่ได้ (ไม่ต้องลองรัน)".into() };
        st.push_log(format!("🔎 {model}: ✗ ใช้แชตได้อย่างเดียว — OmniRoute บอกว่าเรียกเครื่องมือไม่ได้"));
        progress_done(&a.id, &model, &v);
        record(&st.db.lock(), &key(&check_kind(&a), &model), v);
        st.wake.notify_one();
        return;
    }
    BUSY.store(true, std::sync::atomic::Ordering::SeqCst);
    progress_trying(&a.id, &model);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let st = app.state::<AppState>();
        st.push_log(format!("🔎 ตรวจว่าโมเดล {model} ({}) ทำงานบอทได้ไหม…", a.id));
        let mut v = trial(&app, &a, &model).await;
        // one miss can be bad luck (the same model under another name passed): "chat only" takes two misses in a row
        if v.v == "chat" {
            st.push_log(format!("🔎 {model}: ไม่ผ่านครั้งแรก — ลองอีกครั้ง"));
            let again = trial(&app, &a, &model).await;
            v = if again.v == "chat" { Verdict { note: format!("ไม่ผ่าน 2 ครั้ง: {}", again.note), ..again } } else { again };
        }
        let label = match v.v.as_str() {
            "bot" => "✓ ทำงานบอทได้",
            "chat" => "✗ ใช้แชตได้อย่างเดียว",
            _ => "? ตรวจไม่ได้ตอนนี้",
        };
        st.push_log(format!("🔎 {model}: {label}{}", if v.note.is_empty() { String::new() } else { format!(" — {}", v.note) }));
        progress_done(&a.id, &model, &v);
        record(&st.db.lock(), &key(&check_kind(&a), &model), v);
        BUSY.store(false, std::sync::atomic::Ordering::SeqCst);
        st.wake.notify_one();
    });
}

/// What the phone app may offer for each account (it cannot ask the agents itself): the models that passed the
/// check, and how many were left out — published as state/models.json by the sync.
pub fn options_for_phone(c: &rusqlite::Connection) -> serde_json::Value {
    let checks = all(c);
    let now = crate::db::now();
    let accounts: Vec<serde_json::Value> = crate::accounts::list(c)
        .into_iter()
        .filter(|a| a.enabled)
        .map(|a| {
            let ck = check_kind(&a);
            let (models, hidden): (Vec<String>, usize) = match a.kind.as_str() {
                _ if applies(&ck) => {
                    let list = crate::acct_models::cached(&a.id).unwrap_or_default();
                    let ok: Vec<String> = list.iter().filter(|m| state(&checks, &ck, m, now) == "bot").cloned().collect();
                    let hidden = list.len() - ok.len();
                    (ok, hidden)
                }
                "codex" => (crate::providers::codex_models_in(&a.home).into_iter().map(|m| m.slug).collect(), 0),
                _ => (crate::catalog::vendor(&a.vendor_id()).map(|v| v.models.iter().map(|s| s.to_string()).collect()).unwrap_or_default(), 0),
            };
            serde_json::json!({
                "id": a.id, "label": a.title(), "kind": a.kind, "program": crate::providers::label(&a.kind),
                "models": models, "hidden": hidden, "effort": crate::providers::def(&a.kind).is_some_and(|d| d.effort),
            })
        })
        .collect();
    serde_json::json!({ "accounts": accounts, "efforts": ["low", "medium", "high", "xhigh"] })
}

/// Write every verdict found so far into the file that ships with the next build (run from the source folder).
pub fn export(st: &AppState) -> Result<String, String> {
    let path = st.root.join("app").join("src-tauri").join("model_checks.json");
    if !path.is_file() {
        return Err("ไม่ได้รันจากโฟลเดอร์ซอร์ส (ไม่พบ app/src-tauri/model_checks.json)".into());
    }
    let m: BTreeMap<String, Verdict> = all(&st.db.lock()).into_iter().filter(|(_, v)| v.v == "bot" || v.v == "chat").collect();
    let body = serde_json::json!({ "version": 1, "about": "Results of the bot-work check per agent|model (model_check.rs). bot = can do the bot's work, chat = chat only.", "checks": m });
    std::fs::write(&path, serde_json::to_string_pretty(&body).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok(format!("{} โมเดล → {}", m.len(), path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_now_counts_what_it_was_asked() {
        let v = |x: &str| Verdict { v: x.into(), ts: 0, secs: 1, note: "n".into() };
        progress_start("prog-t", &["a".to_string(), "b".to_string()]);
        progress_trying("prog-t", "a");
        assert_eq!(PROGRESS.lock()["prog-t"].current.as_ref().map(|c| c.0.as_str()), Some("a"));
        progress_done("prog-t", "a", &v("bot"));
        progress_done("prog-t", "zzz", &v("chat")); // not asked for: not counted
        let p = PROGRESS.lock()["prog-t"].clone();
        assert_eq!((p.done, p.bot, p.chat, p.finished), (1, 1, 0, 0));
        assert!(p.current.is_none());
        progress_done("prog-t", "b", &v("unknown"));
        let p = PROGRESS.lock()["prog-t"].clone();
        assert_eq!((p.done, p.unknown), (2, 1));
        assert!(p.finished > 0);
    }

    #[test]
    fn the_job_is_at_the_end_of_a_long_file() {
        let p = trial_prompt();
        assert!(p.len() > 4000, "the task file should be long like a real one");
        assert!(p.rfind("## The job").unwrap() > p.len() - 600);
    }

    #[test]
    fn judging_the_trial() {
        let d = std::env::temp_dir().join(format!("mc-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&d);
        assert!(judge(&d, "durian42").is_err());
        std::fs::write(d.join("answer.txt"), "DURIAN42\n").unwrap();
        assert_eq!(judge(&d, "durian42").unwrap_err(), "ไม่ได้เขียน STATUS.json");
        std::fs::write(d.join("STATUS.json"), "{\"summary\":\"done\",\"word\":\"DURIAN42\"}").unwrap();
        assert!(judge(&d, "durian42").is_ok());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn states_and_shipped_file() {
        let mut m = BTreeMap::new();
        m.insert(key("opencode", "a"), Verdict { v: "chat".into(), ts: 0, secs: 1, note: String::new() });
        m.insert(key("opencode", "b"), Verdict { v: "unknown".into(), ts: 0, secs: 1, note: String::new() });
        assert_eq!(state(&m, "opencode", "a", 10), "chat");
        assert_eq!(state(&m, "opencode", "b", 10), "unknown");
        assert_eq!(state(&m, "opencode", "b", RETRY_UNKNOWN_SECS + 10), "pending"); // tried again later
        assert_eq!(state(&m, "opencode", "new", 10), "pending");
        assert_eq!(state(&m, "claude", "anything", 10), "bot");
        // the file that ships with the app parses
        let _ = shipped();
        assert!(serde_json::from_str::<serde_json::Value>(SHIPPED).is_ok());
        assert!(untried("This request requires more credits") && untried("Error: Invalid API key. Please check") && !untried("I could not find the file"));
    }
}
