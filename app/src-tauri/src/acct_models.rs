//! The models one account can really run, read from that account's own agent with its own login / key.
//!
//! Two agents keep their list somewhere the provider's API does not tell:
//! * OpenCode 2.x answers through a background service started with *its* environment, so `opencode models` (and
//!   `opencode run`) without `--standalone` ignore the account's key and folders: an OpenRouter account then sees no
//!   OpenRouter model at all. Every call made for an account therefore runs with `--standalone`.
//! * Mistral Vibe takes `--model <alias>` where the aliases are Vibe's own (glm-5-3, mistral-medium-3.5, …), not the
//!   names of Mistral's API. Vibe has no "list" command, but an unknown alias makes it print the list and stop at once
//!   (no request is sent), which is what is read here.
//!
//! The list is kept per account for 30 minutes. A run never waits for it: `fit` uses what is known and refreshes in the
//! background, so a model the account cannot run is replaced by one it can (or by the agent's own default).
use crate::accounts::Account;
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::Path;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

static CACHE: LazyLock<Mutex<HashMap<String, (Instant, Vec<String>)>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static BUSY: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));
const FRESH: Duration = Duration::from_secs(1800);
/// why the list of an account is shorter than its key allows (shown under the list)
static NOTES: LazyLock<Mutex<HashMap<String, String>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn note(id: &str) -> Option<String> {
    NOTES.lock().get(id).cloned().or_else(|| crate::omniroute::note(id))
}

/// Models that hit a limit of their own (a free model rate-limited upstream) while the account itself is fine:
/// (account, model) → until (unix seconds). Other models of the account carry on meanwhile.
static MODEL_LOCKS: LazyLock<Mutex<HashMap<(String, String), i64>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn lock_model(account: &str, model: &str, until: i64) {
    MODEL_LOCKS.lock().insert((account.to_string(), model.to_string()), until);
}

pub fn model_locked(account: &str, model: &str, now: i64) -> Option<i64> {
    MODEL_LOCKS.lock().get(&(account.to_string(), model.to_string())).copied().filter(|u| *u > now)
}

/// Does the limit message speak of this one model (not of the whole account)?
pub fn model_level_limit(text: &str) -> bool {
    let l = text.to_lowercase();
    ["rate-limited upstream", "temporarily rate-limited", "model is overloaded", "model is currently overloaded", "model_not_available", "model is currently unavailable", "no endpoints found", "is not available for this model", "for this model"]
        .iter()
        .any(|w| l.contains(w))
}

/// Another model of this account that passed the bot-work check and is not locked — where a model-level limit can
/// move to. None = the whole account has to wait.
pub fn alternative(a: &Account, model: &str, checks: &std::collections::BTreeMap<String, crate::model_check::Verdict>, now: i64) -> Option<String> {
    let ck = crate::model_check::check_kind(a);
    let known = cached(&a.id)?;
    let good: Vec<&String> = known
        .iter()
        .filter(|k| k.as_str() != model && crate::model_check::state(checks, &ck, k, now) == "bot" && model_locked(&a.id, k, now).is_none())
        .collect();
    good.iter().find(|k| crate::catalog::is_free_model(k)).or(good.first()).map(|s| s.to_string())
}

/// Whether this OpenCode has the `--standalone` flag (2.x); asked once.
pub fn opencode_standalone() -> bool {
    static S: LazyLock<bool> = LazyLock::new(|| {
        let Some(bin) = crate::providers::resolve_bin("opencode") else { return false };
        let mut c = std::process::Command::new(bin);
        c.arg("--help");
        output(c, 30).is_some_and(|o| o.contains("--standalone"))
    });
    *S
}

/// OpenCode's data folder for this account (`XDG_DATA_HOME/opencode`, default ~/.local/share/opencode).
pub fn opencode_data(a: &Account) -> Option<std::path::PathBuf> {
    let base = if a.home.is_empty() { dirs::home_dir()?.join(".local").join("share") } else { Path::new(&a.home).join("data") };
    Some(base.join("opencode"))
}

/// The OpenCode Console sign-in OpenCode will use: (label, bearer token, org id).
/// OpenCode 2.x keeps sign-ins in the `credential` table of opencode.db (the active one counts); auth.json is the 1.x
/// file and may still hold an old, expired key, so it is read only when the database has nothing.
pub fn opencode_console_cred(a: &Account) -> Option<(String, String, Option<String>)> {
    let dir = opencode_data(a)?;
    let db = dir.join("opencode.db");
    if db.is_file() {
        if let Ok(c) = rusqlite::Connection::open_with_flags(&db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY) {
            let row: Option<(String, String)> = c
                .query_row(
                    "SELECT label, value FROM credential WHERE integration_id='opencode' AND active=1 ORDER BY time_updated DESC LIMIT 1",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .ok();
            if let Some((label, value)) = row {
                let v: serde_json::Value = serde_json::from_str(&value).ok()?;
                let bearer = v.get("access").or_else(|| v.get("key")).and_then(|x| x.as_str())?.to_string();
                let org = v.pointer("/metadata/orgID").and_then(|x| x.as_str()).map(String::from);
                return Some((label, bearer, org));
            }
            // the database answers but holds no active OpenCode sign-in
            let any: i64 = c.query_row("SELECT COUNT(*) FROM credential", [], |r| r.get(0)).unwrap_or(0);
            if any > 0 {
                return None;
            }
        }
    }
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("auth.json")).ok()?).ok()?;
    let key = v.pointer("/opencode/key").and_then(|x| x.as_str())?.to_string();
    Some(("API key".into(), key, None))
}

/// Any sign-in at all (OpenCode Console or a provider such as Mistral) — what "logged in" means for OpenCode.
pub fn opencode_signed_in(a: &Account) -> bool {
    let Some(dir) = opencode_data(a) else { return false };
    if let Ok(c) = rusqlite::Connection::open_with_flags(dir.join("opencode.db"), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY) {
        if let Ok(n) = c.query_row("SELECT COUNT(*) FROM credential WHERE active=1", [], |r| r.get::<_, i64>(0)) {
            if n > 0 {
                return true;
            }
        }
    }
    std::fs::read_to_string(dir.join("auth.json")).ok().and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()).and_then(|v| v.as_object().map(|o| !o.is_empty())).unwrap_or(false)
}

/// Why OpenCode's own models fail, asked of opencode.ai with the sign-in OpenCode uses: Some((reason, retry_at)).
/// `retry_at` = when the free models come back (from Retry-After), so the bot can wait for it.
pub async fn opencode_diagnose(a: &Account) -> Option<(String, Option<i64>)> {
    let Some((label, bearer, org)) = opencode_console_cred(a) else {
        return Some(("ยังไม่ได้ล็อกอิน OpenCode Console — กดปุ่ม “ล็อกอิน” ของบัญชีนี้ แล้วเลือก OpenCode Console".into(), None));
    };
    let c = reqwest::Client::builder().timeout(Duration::from_secs(20)).build().ok()?;
    let r = c.get("https://opencode.ai/zen/v1/models").bearer_auth(&bearer).send().await.ok()?;
    if r.status().as_u16() == 401 {
        let body = r.text().await.unwrap_or_default();
        let what = if body.contains("api_key_expired") { "หมดอายุแล้ว" } else { "ใช้ไม่ได้" };
        return Some((format!("การล็อกอิน OpenCode Console “{label}” {what} — กดปุ่ม “ล็อกอิน” ของบัญชีนี้เพื่อล็อกอินใหม่"), None));
    }
    // the sign-in works: ask for one tiny answer from a free model; a used-up free tier answers 429 with Retry-After
    let mut req = c
        .post("https://opencode.ai/zen/v1/chat/completions")
        .bearer_auth(&bearer)
        .json(&serde_json::json!({ "model": "big-pickle", "messages": [{ "role": "user", "content": "OK" }], "max_tokens": 1 }));
    if let Some(o) = &org {
        req = req.header("x-opencode-org-id", o);
    }
    let r = req.send().await.ok()?;
    if r.status().as_u16() != 429 {
        return None;
    }
    let after = r.headers().get("retry-after").and_then(|h| h.to_str().ok()).and_then(|s| s.trim().parse::<i64>().ok());
    let until = after.map(|s| chrono::Local::now().timestamp() + s);
    let when = until
        .and_then(|t| chrono::DateTime::from_timestamp(t, 0))
        .map(|t| t.with_timezone(&chrono::Local).format("%d/%m %H:%M").to_string())
        .unwrap_or_else(|| "รอบถัดไป".into());
    Some((
        format!(
            "ล็อกอิน “{}” ใช้ได้ปกติ แต่โควต้าโมเดลฟรีของ OpenCode หมดแล้ว (opencode.ai ตอบ 429) — กลับมาราว {} · โควต้าฟรีนี้นับรวมทั้งเครือข่าย (ไม่ล็อกอินก็ได้ผลเดียวกัน) ระหว่างนี้ใช้โมเดลของค่ายที่เชื่อมไว้ใน OpenCode เช่น mistral/… หรือสมัคร OpenCode Go",
            label, when
        ),
        until,
    ))
}

/// "openrouter/" for an account of a provider reached through OpenCode; None for OpenCode's own login (any model).
pub fn opencode_prefix(a: &Account) -> Option<String> {
    let v = a.vendor_id();
    (a.kind == "opencode" && !v.is_empty() && v != "opencode").then(|| format!("{v}/"))
}

/// Run a command (no window, no input) and give back stdout + stderr, or None when it could not start or ran too long.
fn output(mut c: std::process::Command, secs: u64) -> Option<String> {
    use std::process::Stdio;
    c.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    let mut child = c.spawn().ok()?;
    let mut so = child.stdout.take()?;
    let mut se = child.stderr.take()?;
    let t1 = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = so.read_to_string(&mut s);
        s
    });
    let t2 = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = se.read_to_string(&mut s);
        s
    });
    let end = Instant::now() + Duration::from_secs(secs);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() > end => {
                let _ = child.kill();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(150)),
            Err(_) => return None,
        }
    }
    Some(format!("{}\n{}", t1.join().unwrap_or_default(), t2.join().unwrap_or_default()))
}

/// The aliases in Vibe's "Unknown model: x. Available: a, b, c" message.
pub fn parse_vibe_list(text: &str) -> Vec<String> {
    let Some(i) = text.find("Available:") else { return vec![] };
    let rest = text[i + "Available:".len()..].lines().next().unwrap_or("");
    rest.split(',').map(|s| s.trim().trim_end_matches('.').to_string()).filter(|s| !s.is_empty() && !s.contains(' ')).collect()
}

/// Vibe's own models (from its effective config, Vibe 3.1): alias, API name, display name. Vibe drops its defaults as soon as
/// config.toml lists any model, so they are written back next to the extra ones.
const VIBE_DEFAULTS: [(&str, &str, &str); 4] = [
    ("mistral-medium-3.5", "mistral-vibe-cli-latest", "Mistral Medium 3.5"),
    ("mistral-large-4-0-3", "mistral-large-4-0", "Mistral Large 4"),
    ("glm-5-3", "zai-glm-5-3", "GLM-5.3 (Mistral Hosted)"),
    ("zai-glm-5-3-flash", "zai-glm-5-3-flash", "GLM-5.3 Flash (Mistral Hosted)"),
];
const BLOCK_START: &str = "# >>> idea-autopilot: models (written by the app, replaced on “read the model list again”)";
const BLOCK_END: &str = "# <<< idea-autopilot: models";

/// The models of a Mistral key that can drive a coding agent: chat + tool calls (OCR, embeddings, speech are left out).
pub async fn mistral_api_models(key: &str) -> Result<Vec<String>, String> {
    let c = reqwest::Client::builder().timeout(Duration::from_secs(20)).build().map_err(|x| x.to_string())?;
    let r = c.get("https://api.mistral.ai/v1/models").bearer_auth(key).send().await.map_err(|x| x.to_string())?;
    if !r.status().is_success() {
        return Err(format!("API ของ Mistral ตอบ HTTP {}", r.status()));
    }
    let v: serde_json::Value = r.json().await.map_err(|x| x.to_string())?;
    let mut ids: Vec<String> = v["data"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|m| m.pointer("/capabilities/completion_chat").and_then(|x| x.as_bool()) == Some(true))
                .filter(|m| m.pointer("/capabilities/function_calling").and_then(|x| x.as_bool()) == Some(true))
                .filter(|m| m.get("deprecation").is_none_or(|d| d.is_null()))
                .filter_map(|m| m["id"].as_str().map(String::from))
                .filter(|id| safe_name(id))
                .collect()
        })
        .unwrap_or_default();
    ids.sort();
    ids.dedup();
    Ok(ids)
}

fn safe_name(s: &str) -> bool {
    !s.is_empty() && s.len() < 80 && s.chars().all(|c| c.is_ascii_alphanumeric() || "-._".contains(c))
}

/// The model list for config.toml, in the `[[models]]` form Vibe itself writes back (it rewrites the file on its own,
/// and a `[models."x"]` table next to its `[[models]]` list makes it read keys in the wrong place).
pub fn vibe_block(api: &[String]) -> String {
    let mut out = format!("{BLOCK_START}\n");
    let mut seen: Vec<&str> = vec![];
    for (alias, name, label) in VIBE_DEFAULTS {
        out.push_str(&format!("[[models]]\nname = \"{name}\"\nprovider = \"mistral\"\nalias = \"{alias}\"\ndisplay_name = \"{label}\"\ntemperature = 1.0\nthinking = \"high\"\n\n"));
        seen.push(alias);
    }
    for id in api.iter().filter(|i| safe_name(i)) {
        if seen.contains(&id.as_str()) {
            continue;
        }
        out.push_str(&format!("[[models]]\nname = \"{id}\"\nprovider = \"mistral\"\nalias = \"{id}\"\n\n"));
    }
    out.push_str(BLOCK_END);
    out.push('\n');
    out
}

/// Is this TOML header a model entry (either form) or a sub-table of one?
fn model_header(h: &str) -> bool {
    let h = h.trim();
    h == "[[models]]" || h.starts_with("[models.") || h.starts_with("[[models.") || h == "[models]"
}

/// The account's config.toml with its model list replaced by `block` (this folder belongs to the app's account, so the
/// list is the app's to keep); every other setting stays, ahead of the list.
pub fn merge_block(existing: &str, block: &str) -> String {
    let mut keep: Vec<&str> = vec![];
    let mut in_models = false;
    for line in existing.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_models = model_header(t);
        }
        if in_models || t == BLOCK_START || t == BLOCK_END {
            continue;
        }
        keep.push(line);
    }
    let rest = keep.join("\n");
    let rest = rest.trim();
    let mut out = String::new();
    if !rest.is_empty() {
        out.push_str(rest);
        out.push_str("\n\n");
    }
    out.push_str(block);
    out
}

/// Make every coding model of the key usable in Vibe for this account (its own VIBE_HOME only; the machine's own
/// ~/.vibe is never written). Ok(n) = models added besides Vibe's own.
fn vibe_sync(a: &Account, root: &Path) -> Result<usize, String> {
    if a.home.is_empty() {
        return Err("บัญชีนี้ใช้ ~/.vibe ของเครื่อง — แอปไม่แก้ config ของคุณ จึงเห็นเฉพาะโมเดลของ Vibe เอง".into());
    }
    let env = crate::accounts::env_with_secret(a, root);
    let key = env.iter().find(|(k, _)| k == "MISTRAL_API_KEY").map(|(_, v)| v.clone()).ok_or("ยังไม่มี key")?;
    let api = tauri::async_runtime::block_on(mistral_api_models(&key))?;
    let path = Path::new(&a.home).join("config.toml");
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    let merged = merge_block(&old, &vibe_block(&api));
    if merged != old {
        std::fs::write(&path, merged).map_err(|e| e.to_string())?;
    }
    Ok(api.iter().filter(|i| !VIBE_DEFAULTS.iter().any(|d| d.0 == i.as_str())).count())
}

/// Ask the account's own agent (blocking, up to ~2 minutes). Err = why nothing could be read.
pub fn fetch(a: &Account, root: &Path) -> Result<Vec<String>, String> {
    let env = crate::accounts::env_with_secret(a, root);
    let scratch = root.join("data").join("provider-test");
    let _ = std::fs::create_dir_all(&scratch);
    let list = match a.kind.as_str() {
        _ if crate::omniroute::is(a) => crate::omniroute::models(a, crate::accounts::secret(root, &a.id).as_deref())?,
        "opencode" => {
            let bin = crate::providers::resolve_bin("opencode").ok_or("ไม่พบ opencode ในเครื่อง")?;
            let mut c = std::process::Command::new(bin);
            c.arg("models").current_dir(&scratch).envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())));
            if opencode_standalone() {
                c.arg("--standalone");
            }
            let out = output(c, 120).ok_or("opencode models ไม่ตอบภายใน 2 นาที")?;
            let prefix = opencode_prefix(a);
            let mut l: Vec<String> = out
                .lines()
                .map(str::trim)
                .filter(|l| l.contains('/') && !l.contains(' '))
                .filter(|l| prefix.as_ref().is_none_or(|p| l.starts_with(p.as_str())))
                .map(String::from)
                .collect();
            l.dedup();
            l
        }
        "mistral" => {
            // first make the key's other coding models known to Vibe, then read what Vibe now offers
            if let Err(why) = vibe_sync(a, root) {
                NOTES.lock().insert(a.id.clone(), why);
            } else {
                NOTES.lock().remove(&a.id);
            }
            let bin = crate::providers::resolve_bin("vibe").ok_or("ไม่พบ vibe ในเครื่อง")?;
            let mut c = std::process::Command::new(bin);
            c.args(["-p", "x", "--model", "__list_models__"]).current_dir(&scratch).envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())));
            let out = output(c, 90).ok_or("vibe ไม่ตอบภายใน 90 วินาที")?;
            let l = parse_vibe_list(&out);
            if l.is_empty() {
                let why = out.lines().map(str::trim).filter(|x| !x.is_empty()).last().unwrap_or("").chars().take(200).collect::<String>();
                return Err(format!("Vibe ไม่บอกรายชื่อโมเดล: {why}"));
            }
            l
        }
        _ => return Err("AI ตัวนี้ไม่มีรายชื่อโมเดลให้อ่านจากโปรแกรม".into()),
    };
    if list.is_empty() {
        return Err(match opencode_prefix(a) {
            Some(p) => format!("OpenCode ไม่พบโมเดล {p}… ด้วย key ของบัญชีนี้ — key ถูกไหม?"),
            None => "ไม่พบรายชื่อโมเดล".into(),
        });
    }
    CACHE.lock().insert(a.id.clone(), (Instant::now(), list.clone()));
    Ok(list)
}

pub fn cached(id: &str) -> Option<Vec<String>> {
    CACHE.lock().get(id).map(|(_, l)| l.clone())
}

pub fn refresh_bg(a: &Account, root: &Path) {
    let stale = CACHE.lock().get(&a.id).is_none_or(|(t, _)| t.elapsed() > FRESH);
    if !stale || !BUSY.lock().insert(a.id.clone()) {
        return;
    }
    let (a, root) = (a.clone(), root.to_path_buf());
    std::thread::spawn(move || {
        let _ = fetch(&a, &root);
        BUSY.lock().remove(&a.id);
    });
}

/// The model to hand the agent for this account: `want` when the account can run it, otherwise one it can
/// (a free one first) or "" = the agent's own default. The note says what was changed, for the run log.
pub fn fit(a: &Account, root: &Path, want: &str, checks: &std::collections::BTreeMap<String, crate::model_check::Verdict>) -> (String, Option<String>) {
    if !crate::model_check::applies(&crate::model_check::check_kind(a)) {
        return (want.to_string(), None);
    }
    refresh_bg(a, root);
    let known = cached(&a.id).unwrap_or_else(|| {
        // not read yet: the provider's catalog (Vibe's aliases for Mistral) is the best guess
        crate::catalog::vendor(&a.vendor_id()).map(|v| v.models.iter().map(|s| s.to_string()).collect()).unwrap_or_default()
    });
    // 1. a model this account can run at all
    let (m, note) = if a.kind == "mistral" || opencode_prefix(a).is_some() { choose(a, &known, want) } else { (want.to_string(), None) };
    // 2. one that passed the bot-work check (model_check.rs)
    match checked(a, &known, &m, checks, crate::db::now()) {
        Some((better, why)) => (better, Some(why)),
        None => (m, note),
    }
}

/// Replace `m` when it is known to be chat-only (or, for OpenCode, left to its default while checked models exist)
/// by a model that passed the check — a free one first. None = keep `m`.
pub fn checked(a: &Account, known: &[String], m: &str, checks: &std::collections::BTreeMap<String, crate::model_check::Verdict>, now: i64) -> Option<(String, String)> {
    let ck = crate::model_check::check_kind(a);
    let state = |x: &str| crate::model_check::state(checks, &ck, x, now);
    let chat_only = !m.is_empty() && matches!(state(m).as_str(), "chat" | "retry");
    let locked = (!m.is_empty()).then(|| model_locked(&a.id, m, now)).flatten();
    let unset = m.is_empty() && a.kind == "opencode";
    if !chat_only && !unset && locked.is_none() {
        return None;
    }
    let good: Vec<&String> = known.iter().filter(|k| state(k) == "bot" && model_locked(&a.id, k, now).is_none()).collect();
    let best = good.iter().find(|k| crate::catalog::is_free_model(k)).or(good.first())?;
    let why = if let Some(u) = locked {
        format!("โมเดล “{m}” ติดลิมิตเฉพาะตัว (ถึง {})", crate::limits::when_text(u, now))
    } else if chat_only {
        format!("โมเดล “{m}” ใช้แชตได้อย่างเดียว (ไม่ผ่านการตรวจงานบอท)")
    } else {
        "ไม่ได้ระบุโมเดล".to_string()
    };
    Some(((*best).clone(), format!("{why} → ใช้ {best} ที่ผ่านการตรวจแล้ว")))
}

fn choose(a: &Account, known: &[String], want: &str) -> (String, Option<String>) {
    let prefix = opencode_prefix(a);
    let ok = !want.is_empty() && prefix.as_ref().is_none_or(|p| want.starts_with(p.as_str())) && (known.is_empty() && prefix.is_some() || known.iter().any(|m| m == want));
    if ok {
        return (want.to_string(), None);
    }
    let pick = if a.kind == "mistral" {
        String::new() // Vibe's own active model
    } else {
        known.iter().find(|m| crate::catalog::is_free_model(m)).or(known.first()).cloned().unwrap_or_default()
    };
    let note = (!want.is_empty()).then(|| {
        format!("โมเดล “{want}” ใช้กับบัญชี {} ไม่ได้ → ใช้ {}", a.id, if pick.is_empty() { "ค่าเริ่มต้นของโปรแกรม".to_string() } else { pick.clone() })
    });
    (pick, note)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acc(kind: &str, vendor: &str) -> Account {
        Account {
            id: "t".into(),
            kind: kind.into(),
            label: String::new(),
            enabled: true,
            home: String::new(),
            model: "auto".into(),
            effort: "auto".into(),
            vendor: vendor.into(),
            auth: "key".into(),
            base_url: String::new(),
        }
    }

    #[test]
    fn vibe_config_keeps_its_own_models_and_the_owners_settings() {
        let api = vec!["codestral-latest".to_string(), "glm-5-3".to_string(), "bad name".to_string()];
        let b = vibe_block(&api);
        assert!(b.contains("alias = \"glm-5-3\"") && b.contains("name = \"zai-glm-5-3\"")); // Vibe's own entry
        assert_eq!(b.matches("alias = \"glm-5-3\"").count(), 1); // not twice from the API list
        assert!(b.contains("alias = \"codestral-latest\""));
        assert!(!b.contains("bad name"));
        // the other settings stay ahead of the list; a refresh replaces the list
        let first = merge_block("active_model = \"glm-5-3\"\n", &b);
        assert!(first.starts_with("active_model = \"glm-5-3\""));
        let again = merge_block(&first, &vibe_block(&[]));
        assert!(!again.contains("codestral-latest") && again.contains("active_model"));
        // Vibe rewrote the file as [[models]] (markers gone) and a setting table follows: still exactly one list
        let vibe_wrote = "[[models]]\nname = \"a\"\nalias = \"a\"\nsupports_images = true\n\n[tools.bash]\ntimeout = 5\n";
        let m = merge_block(vibe_wrote, &b);
        assert!(!m.contains("alias = \"a\"") && m.contains("[tools.bash]\ntimeout = 5"));
        assert_eq!(m.matches("[[models]]").count(), 5); // Vibe's own 4 + codestral (glm-5-3 is one of the 4)
        assert!(m.find("[tools.bash]").unwrap() < m.find("[[models]]").unwrap());
    }

    #[test]
    fn reads_vibe_aliases_from_its_error() {
        let t = "Reading API keys…\nError: Unknown model: __list_models__. Available: glm-5-3, mistral-large-4-0-3, mistral-medium-3.5, zai-glm-5-3-flash\n";
        assert_eq!(parse_vibe_list(t), vec!["glm-5-3", "mistral-large-4-0-3", "mistral-medium-3.5", "zai-glm-5-3-flash"]);
        assert!(parse_vibe_list("Error: Missing MISTRAL_API_KEY").is_empty());
    }

    #[test]
    fn a_chat_only_model_is_swapped_for_a_checked_one() {
        use crate::model_check::{key, Verdict};
        let v = |s: &str| Verdict { v: s.into(), ts: 0, secs: 1, note: String::new() };
        let or = acc("opencode", "openrouter");
        let known = vec!["openrouter/nano:free".to_string(), "openrouter/big".to_string(), "openrouter/good:free".to_string()];
        let mut c = std::collections::BTreeMap::new();
        c.insert(key("opencode", "openrouter/nano:free"), v("chat"));
        c.insert(key("opencode", "openrouter/good:free"), v("bot"));
        c.insert(key("opencode", "openrouter/big"), v("bot"));
        // chat-only → a checked one, free first
        assert_eq!(checked(&or, &known, "openrouter/nano:free", &c, 1).unwrap().0, "openrouter/good:free");
        // checked or not-yet-checked models are kept
        assert!(checked(&or, &known, "openrouter/big", &c, 1).is_none());
        assert!(checked(&or, &known, "openrouter/new", &c, 1).is_none());
        // nothing passed yet: keep what was asked (no dead end)
        let none = std::collections::BTreeMap::new();
        assert!(checked(&or, &known, "openrouter/nano:free", &none, 1).is_none());
        // Claude never goes through the check
        assert!(checked(&acc("claude", ""), &known, "haiku", &c, 1).is_none());
    }

    #[test]
    fn a_model_with_its_own_limit_steps_aside() {
        use crate::model_check::{key, Verdict};
        let v = |s: &str| Verdict { v: s.into(), ts: 0, secs: 1, note: String::new() };
        let mut or = acc("opencode", "openrouter");
        or.id = "lock-test".into();
        let known = vec!["openrouter/a:free".to_string(), "openrouter/b:free".to_string()];
        CACHE.lock().insert(or.id.clone(), (Instant::now(), known.clone()));
        let mut c = std::collections::BTreeMap::new();
        c.insert(key("opencode", "openrouter/a:free"), v("bot"));
        c.insert(key("opencode", "openrouter/b:free"), v("bot"));
        assert!(model_level_limit("openrouter/a:free is temporarily rate-limited upstream. Please retry shortly"));
        assert!(!model_level_limit("Rate limit exceeded: free-models-per-day"));
        assert_eq!(alternative(&or, "openrouter/a:free", &c, 10).as_deref(), Some("openrouter/b:free"));
        lock_model(&or.id, "openrouter/a:free", 1000);
        assert_eq!(checked(&or, &known, "openrouter/a:free", &c, 10).unwrap().0, "openrouter/b:free");
        // once the lock ends, the model is used again
        assert!(checked(&or, &known, "openrouter/a:free", &c, 2000).is_none());
        // nothing else passed → the account itself has to wait
        lock_model(&or.id, "openrouter/b:free", 1000);
        assert!(alternative(&or, "openrouter/a:free", &c, 10).is_none());
    }

    #[test]
    fn a_model_the_account_cannot_run_is_replaced() {
        let or = acc("opencode", "openrouter");
        let known = vec!["openrouter/a/paid".to_string(), "openrouter/b/x:free".to_string()];
        // a model of another provider (from the shared OpenCode list) → the account's own, free first
        assert_eq!(choose(&or, &known, "opencode/big-pickle").0, "openrouter/b/x:free");
        assert_eq!(choose(&or, &known, "openrouter/a/paid").0, "openrouter/a/paid");
        // Vibe: a name of Mistral's API is not a Vibe alias → Vibe's default
        let mi = acc("mistral", "");
        let aliases = vec!["glm-5-3".to_string()];
        assert_eq!(choose(&mi, &aliases, "devstral-medium-latest").0, "");
        assert_eq!(choose(&mi, &aliases, "glm-5-3").0, "glm-5-3");
        // OpenCode's own login: anything goes
        assert!(opencode_prefix(&acc("opencode", "")).is_none());
    }
}
