//! AI agent backends. The bot only needs "run this prompt headless in this folder, let it write
//! STATUS.json and the report files" — so any agent CLI can do the work, and nobody needs Claude.
//!
//! - Which agent runs: setting `provider` (`auto` = first one installed: Claude, Codex, Gemini,
//!   OpenCode; or one of those ids; or `custom` = the user's own command template).
//! - Which model: the router picks a *tier* (fast / standard / strong); each agent maps tiers to
//!   its own model names (setting `tier_models`). An empty name means "the CLI's own default",
//!   so an agent works out of the box without knowing any model names.
//! - Quota: Claude can be read for real (`Meter::Real`). Every other agent has no readable quota,
//!   so the app meters itself: hours of bot runtime against a weekly budget the user sets
//!   (`Meter::Budget`, see usage::budget_snapshot).
use rusqlite::Connection;
use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::db;

pub const TIER_IDS: [&str; 3] = ["fast", "standard", "strong"];

pub struct Def {
    pub id: &'static str,
    pub label: &'static str,
    pub bin: &'static str,
    /// model names per tier; "" = the CLI's own default
    pub tiers: [&'static str; 3],
    /// can the agent be told how hard to think (the effort the router picks per stage)?
    pub effort: bool,
    pub hint: &'static str,
}

pub const DEFS: &[Def] = &[
    Def {
        id: "claude",
        label: "Claude Code",
        bin: "claude",
        tiers: ["haiku", "sonnet", "opus"],
        effort: true,
        hint: "อ่านโควต้าจริงได้ · เห็นความคืบหน้าละเอียด",
    },
    Def {
        id: "codex",
        label: "Codex CLI (OpenAI)",
        bin: "codex",
        tiers: ["", "", ""],
        effort: true,
        hint: "อ่านโควต้าจริงจาก log ของ Codex (หลังรันอย่างน้อยหนึ่งครั้ง) · ก่อนหน้านั้นใช้งบชั่วโมงที่ตั้งไว้",
    },
    Def {
        id: "gemini",
        label: "Gemini CLI (Google)",
        bin: "gemini",
        tiers: ["", "", ""],
        effort: false,
        hint: "ไม่มีตัวอ่านโควต้า → ใช้งบชั่วโมงที่ตั้งไว้",
    },
    Def {
        id: "mistral",
        label: "Mistral Vibe",
        bin: "vibe",
        tiers: ["", "", ""],
        effort: false,
        hint: "เลือกโมเดลใน config.toml ของ Vibe · ไม่มีตัวอ่านโควต้า → ใช้งบชั่วโมงที่ตั้งไว้",
    },
    Def {
        id: "opencode",
        label: "OpenCode",
        bin: "opencode",
        tiers: ["", "", ""],
        effort: false,
        hint: "เลือกโมเดลได้หลายค่าย (รูปแบบ provider/model)",
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Meter {
    /// the real plan quota (5-hour and weekly windows)
    Real,
    /// the app's own meter: bot runtime against a user-set weekly budget
    Budget,
}

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub id: String,
    pub label: String,
    pub hint: String,
    pub installed: bool,
    pub path: Option<String>,
    /// the model name used for each tier right now ("" = CLI default)
    pub tiers: [String; 3],
    pub defaults: [String; 3],
    /// what the app picks by itself per tier (shown as the hint in Settings)
    pub auto: [String; 3],
    pub effort: bool,
    /// switched on: some account of this AI is enabled (before accounts are set up, every installed AI counts)
    pub active: bool,
}

fn hidden(c: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    let _ = c;
}

/// Full path of an executable we can actually start. On Windows npm-installed CLIs are `.cmd`
/// shims, and the extension-less file that `where` also lists cannot be spawned.
pub fn resolve_bin(bin: &str) -> Option<PathBuf> {
    resolve_on_path(bin).or_else(|| resolve_known_dirs(bin))
}

/// Places agents install to without touching PATH. Claude Desktop, for one, keeps its own copy of
/// Claude Code in %APPDATA%\Claude\claude-code\<version>\<hash>\ — so people who only ever
/// installed the desktop app have no `claude` on PATH but do have a working Claude Code.
fn resolve_known_dirs(bin: &str) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let exts: &[&str] = if cfg!(windows) { &["exe", "cmd", "bat"] } else { &[""] };
    let file = |dir: &Path| -> Option<PathBuf> {
        exts.iter()
            .map(|e| if e.is_empty() { dir.join(bin) } else { dir.join(format!("{bin}.{e}")) })
            .find(|p| p.is_file())
    };
    let mut dirs_to_try: Vec<PathBuf> = vec![
        home.join(".local").join("bin"),
        home.join(".claude").join("local"),
        home.join(".bun").join("bin"),
        home.join(".opencode").join("bin"),
        home.join(".cargo").join("bin"),
    ];
    if let Some(roaming) = dirs::data_dir() {
        dirs_to_try.push(roaming.join("npm"));
    }
    for d in &dirs_to_try {
        if let Some(p) = file(d) {
            return Some(p);
        }
    }
    if bin == "claude" {
        // newest version first: <data>/Claude/claude-code/<version>/<hash>/claude.exe
        let base = dirs::data_dir()?.join("Claude").join("claude-code");
        let mut versions: Vec<PathBuf> = std::fs::read_dir(base).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        versions.sort_by_key(|p| std::cmp::Reverse(version_of(&p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())));
        for v in versions {
            let mut subs: Vec<PathBuf> = std::fs::read_dir(&v).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
            subs.push(v.clone());
            if let Some(p) = subs.iter().find_map(|d| file(d)) {
                return Some(p);
            }
        }
    }
    None
}

fn resolve_on_path(bin: &str) -> Option<PathBuf> {
    let mut c = std::process::Command::new(if cfg!(windows) { "where" } else { "which" });
    c.arg(bin);
    hidden(&mut c);
    let out = c.output().ok().filter(|o| o.status.success())?;
    let paths: Vec<PathBuf> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| PathBuf::from(l.trim()))
        .filter(|p| p.is_file())
        .collect();
    if cfg!(windows) {
        for ext in ["exe", "cmd", "bat", "com"] {
            if let Some(p) = paths.iter().find(|p| p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case(ext))) {
                return Some(p.clone());
            }
        }
        None
    } else {
        paths.into_iter().next()
    }
}

/// Look for the agents again at the next question (after one was just installed).
pub fn forget_found() {
    *CACHE.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

static CACHE: Mutex<Option<(Instant, Vec<(&'static str, Option<PathBuf>)>)>> = Mutex::new(None);

/// Which agent CLIs are on this machine (re-checked every 30 s, so installing one needs no restart).
fn found() -> Vec<(&'static str, Option<PathBuf>)> {
    let mut g = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((t, v)) = g.as_ref() {
        if t.elapsed() < Duration::from_secs(30) {
            return v.clone();
        }
    }
    let v: Vec<_> = DEFS.iter().map(|d| (d.id, resolve_bin(d.bin))).collect();
    *g = Some((Instant::now(), v.clone()));
    v
}

pub fn def(id: &str) -> Option<&'static Def> {
    DEFS.iter().find(|d| d.id == id)
}

pub fn installed(id: &str) -> bool {
    found().iter().any(|(i, p)| *i == id && p.is_some())
}

fn tier_overrides(c: &Connection) -> Value {
    db::get_setting(c, "tier_models").and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null)
}

pub fn tier_model(c: &Connection, provider: &str, tier: usize) -> String {
    let Some(d) = def(provider) else { return String::new() };
    let tier = tier.min(2);
    tier_overrides(c)
        .pointer(&format!("/{provider}/{}", TIER_IDS[tier]))
        .and_then(Value::as_str)
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| {
            // Claude's family names already mean "newest"; other agents are asked what they offer
            if provider == "claude" { d.tiers[tier].to_string() } else { discovered(provider)[tier].clone() }
        })
}

/// The agent that will do the work: the user's choice when it is usable, otherwise the first
/// installed one. `none` means nothing can run yet (the app then shows the setup screen).
pub fn chosen(c: &Connection) -> String {
    let want = db::get_setting(c, "provider").unwrap_or_else(|| "auto".into());
    if want == "custom" && !db::get_setting(c, "custom_cmd").unwrap_or_default().trim().is_empty() {
        return "custom".into();
    }
    if installed(&want) {
        return want;
    }
    if !db::get_setting(c, "custom_cmd").unwrap_or_default().trim().is_empty() && want != "auto" {
        return "custom".into();
    }
    found().iter().find(|(_, p)| p.is_some()).map(|(i, _)| i.to_string()).unwrap_or_else(|| "none".into())
}

pub fn label(id: &str) -> String {
    match id {
        "custom" => "คำสั่งของคุณเอง".into(),
        "none" => "ยังไม่มี AI".into(),
        _ => def(id).map(|d| d.label.to_string()).unwrap_or_else(|| id.to_string()),
    }
}

fn claude_logged_in() -> bool {
    let Some(h) = dirs::home_dir() else { return false };
    h.join(".claude").join(".credentials.json").is_file() || h.join(".claude").join("usage-cache.json").is_file()
}

/// How quota is measured for `provider`: setting `meter` = auto | real | budget.
pub fn meter(c: &Connection, provider: &str) -> Meter {
    let real_possible = provider == "claude" && claude_logged_in();
    match db::get_setting(c, "meter").as_deref() {
        Some("budget") => Meter::Budget,
        Some("real") if real_possible => Meter::Real,
        _ if real_possible => Meter::Real,
        _ => Meter::Budget,
    }
}

fn path_is_some_marker(f: &[(&'static str, Option<PathBuf>)], id: &str) -> bool {
    f.iter().any(|(i, p)| *i == id && p.is_some())
}

pub fn statuses(c: &Connection) -> Vec<Status> {
    let f = found();
    let configured = db::get_setting(c, "ai_accounts").is_some();
    let on: Vec<String> = crate::accounts::list(c).into_iter().filter(|a| a.enabled).map(|a| a.kind).collect();
    DEFS.iter()
        .map(|d| {
            let path = f.iter().find(|(i, _)| *i == d.id).and_then(|(_, p)| p.clone());
            Status {
                id: d.id.into(),
                label: d.label.into(),
                hint: d.hint.into(),
                installed: path.is_some(),
                path: path.map(|p| p.display().to_string()),
                tiers: [0, 1, 2].map(|t| tier_model(c, d.id, t)),
                defaults: d.tiers.map(String::from),
                auto: if d.id == "claude" { d.tiers.map(String::from) } else { discovered(d.id) },
                effort: d.effort,
                active: path_is_some_marker(&f, d.id) && (!configured || on.iter().any(|k| k == d.id)),
            }
        })
        .collect()
}

/// Everything the UI needs to show the agent picker and the quota mode.
pub fn summary(c: &Connection) -> Value {
    let p = chosen(c);
    json!({
        "active": p,
        "active_label": label(&p),
        "setting": db::get_setting(c, "provider").unwrap_or_else(|| "auto".into()),
        "meter": meter(c, &p),
        "list": statuses(c),
        "custom_cmd": db::get_setting(c, "custom_cmd").unwrap_or_default(),
        "claude": installed("claude"),
    })
}

/// How to start one headless run.
#[derive(Debug)]
pub struct Invocation {
    pub program: PathBuf,
    pub args: Vec<String>,
    /// a whole command line for the shell (custom provider); `program`/`args` are ignored
    pub shell_line: Option<String>,
    /// send the prompt on stdin (otherwise the agent is told to read `prompt_file`)
    pub stdin_prompt: bool,
    pub claude_stream: bool,
}

/// Setting `codex_approval`: how Codex runs deal with tools that need a person's approval (Computer Use drives the real
/// Chrome and asks for it). "headless" (default): Computer Use is switched off for the run, so Codex does not burn turns on
/// a request nobody can answer and checks web pages with headless Edge (see the rules in AGENTS.md). "full": Codex may use
/// Computer Use; every prompt is skipped and the folder sandbox is dropped for these runs (dangerous, needs the Codex app open).
static CODEX_FULL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_codex_approval(v: &str) {
    CODEX_FULL.store(v == "full", std::sync::atomic::Ordering::Relaxed);
}

/// The arguments of the approval mode for one Codex run; `first` = a new conversation (resume does not take `-s`).
pub fn codex_approval_args(first: bool) -> Vec<String> {
    let s = |v: &str| v.to_string();
    if CODEX_FULL.load(std::sync::atomic::Ordering::Relaxed) {
        vec![s("--dangerously-bypass-approvals-and-sandbox")]
    } else {
        let mut a = if first { vec![s("-s"), s("workspace-write")] } else { vec![s("-c"), s("sandbox_mode=workspace-write")] };
        a.extend([s("-c"), s("sandbox_workspace_write.network_access=true"), s("-c"), s("features.computer_use=false")]);
        a
    }
}

const READ_FILE_MSG: &str = "Read the file logs/prompt.md in the current folder and carry out the task described there.";

pub fn build(provider: &str, model: &str, effort: &str, tools: &str, custom: &str, prompt_file: &Path) -> Result<Invocation, String> {
    let bin = |id: &str| resolve_bin(def(id).map(|d| d.bin).unwrap_or(id)).ok_or_else(|| format!("ไม่พบ {id} ในเครื่อง (ติดตั้งแล้วอยู่ใน PATH ไหม?)"));
    let s = |v: &str| v.to_string();
    match provider {
        "claude" => {
            let mut args: Vec<String> = ["-p", "--output-format", "stream-json", "--verbose", "--permission-mode", "bypassPermissions", "--setting-sources", "project,local"]
                .iter()
                .map(|x| s(x))
                .collect();
            if !model.is_empty() {
                args.extend([s("--model"), s(model)]);
            }
            args.extend([
                s("--effort"),
                s(effort),
                // no MCP servers and only the tools the stage needs: ~58% less context per turn
                s("--strict-mcp-config"),
                s("--tools"),
                s(tools),
            ]);
            Ok(Invocation { program: bin("claude")?, args, shell_line: None, stdin_prompt: true, claude_stream: true })
        }
        "codex" => {
            // workspace-write sandbox (files only in the idea folder) + network so research works
            let mut args: Vec<String> = ["exec", "--skip-git-repo-check", "--json", "--color", "never"].iter().map(|x| s(x)).collect();
            args.extend(codex_approval_args(true));
            if !model.is_empty() {
                args.extend([s("-m"), s(model)]);
            }
            if let Some(e) = effort_for("codex", model, effort) {
                // unquoted on purpose: codex reads a non-TOML value as a plain string
                args.extend([s("-c"), format!("model_reasoning_effort={e}")]);
            }
            args.push(s("-"));
            Ok(Invocation { program: bin("codex")?, args, shell_line: None, stdin_prompt: true, claude_stream: false })
        }
        "gemini" => {
            let mut args: Vec<String> = vec![s("-p"), s("Carry out the task given on stdin."), s("--yolo")];
            if !model.is_empty() {
                args.extend([s("-m"), s(model)]);
            }
            Ok(Invocation { program: bin("gemini")?, args, shell_line: None, stdin_prompt: true, claude_stream: false })
        }
        "opencode" => {
            let mut args: Vec<String> = vec![s("run")];
            // OpenCode 2.x: without this the background service answers with its own login, not this account's key
            if crate::acct_models::opencode_standalone() {
                args.push(s("--standalone"));
            }
            if !model.is_empty() {
                args.extend([s("-m"), s(model)]);
            }
            args.push(s(READ_FILE_MSG));
            Ok(Invocation { program: bin("opencode")?, args, shell_line: None, stdin_prompt: false, claude_stream: false })
        }
        "mistral" => {
            // programmatic mode: the prompt is the argument of -p; --trust = the idea folder for this run only (no prompt);
            // --model takes one of Vibe's own aliases (acct_models::fit keeps other names out), empty = Vibe's active model
            // --auto-approve: headless Vibe otherwise refuses shell commands (it could write code but not run or test it) —
            // the same freedom Claude Code gets with bypassPermissions
            let mut args: Vec<String> = vec![s("-p"), s(READ_FILE_MSG), s("--trust"), s("--auto-approve")];
            if !model.is_empty() {
                args.extend([s("--model"), s(model)]);
            }
            Ok(Invocation { program: bin("vibe")?, args, shell_line: None, stdin_prompt: false, claude_stream: false })
        }
        "custom" => {
            let tpl = custom.trim();
            if tpl.is_empty() {
                return Err("ยังไม่ได้ใส่คำสั่งของคุณเอง (ตั้งค่า → AI ที่ใช้)".into());
            }
            let line = tpl
                .replace("{prompt_file}", &prompt_file.display().to_string())
                .replace("{prompt_file_rel}", "logs/prompt.md")
                .replace("{model}", model)
                .replace("{stdin}", "");
            Ok(Invocation { program: PathBuf::new(), args: vec![], shell_line: Some(line), stdin_prompt: tpl.contains("{stdin}"), claude_stream: false })
        }
        "none" => Err("ยังไม่มี AI ในเครื่อง — ติดตั้ง Claude Code, Codex, Gemini CLI หรือ OpenCode อย่างใดอย่างหนึ่ง หรือใส่คำสั่งของคุณเองในหน้าตั้งค่า".into()),
        other => Err(format!("ไม่รู้จัก AI: {other}")),
    }
}


// ---------- model discovery: tiers for agents that list their own models ----------
//
// Claude resolves its family names (haiku / sonnet / opus) to the newest release by itself. For
// the others the app reads what the agent itself offers — Codex keeps a models cache with a
// description and priority per model, OpenCode lists models on request — and sorts them into the
// three tiers by what they say about themselves ("fast and affordable", "frontier", …). When a new
// model ships the agent's list changes, and so does the choice, with no update of this app.
// Anything the user types in Settings wins over this.

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub slug: String,
    /// slug + display name + description, lower case
    pub text: String,
    /// lower = preferred/newer, as the agent ranks its own models (999 = unknown)
    pub priority: i64,
    pub efforts: Vec<String>,
}

/// Codex's own model list for the login in `home` (empty = the machine's normal Codex login).
pub fn codex_models_in(home: &str) -> Vec<ModelInfo> {
    let dir = if home.is_empty() { std::env::var("CODEX_HOME").map(PathBuf::from).ok().or_else(|| dirs::home_dir().map(|h| h.join(".codex"))) } else { Some(PathBuf::from(home)) };
    let Some(raw) = dir.and_then(|h| std::fs::read_to_string(h.join("models_cache.json")).ok()) else { return vec![] };
    parse_codex_cache(&raw)
}

fn codex_models() -> Vec<ModelInfo> {
    let home = std::env::var("CODEX_HOME").map(PathBuf::from).ok().or_else(|| dirs::home_dir().map(|h| h.join(".codex")));
    let Some(raw) = home.and_then(|h| std::fs::read_to_string(h.join("models_cache.json")).ok()) else { return vec![] };
    parse_codex_cache(&raw)
}

pub fn parse_codex_cache(raw: &str) -> Vec<ModelInfo> {
    let Ok(v) = serde_json::from_str::<Value>(raw) else { return vec![] };
    v.get("models")
        .and_then(Value::as_array)
        .map(|ms| {
            ms.iter()
                .filter(|m| m.get("visibility").and_then(Value::as_str) != Some("hide"))
                .filter_map(|m| {
                    let slug = m.get("slug")?.as_str()?.to_string();
                    let s = |k: &str| m.get(k).and_then(Value::as_str).unwrap_or("");
                    Some(ModelInfo {
                        text: format!("{slug} {} {}", s("display_name"), s("description")).to_lowercase(),
                        priority: m.get("priority").and_then(Value::as_i64).unwrap_or(999),
                        efforts: m
                            .get("supported_reasoning_levels")
                            .and_then(Value::as_array)
                            .map(|a| a.iter().filter_map(|e| e.get("effort").and_then(Value::as_str).map(String::from)).collect())
                            .unwrap_or_default(),
                        slug,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

static OPENCODE: Mutex<Option<(Instant, Vec<ModelInfo>)>> = Mutex::new(None);
static OPENCODE_BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// `opencode models` lists provider/model per line. Slow, so it runs in the background and the
/// last answer is used meanwhile (empty on the very first call → the agent's default model).
fn opencode_models() -> Vec<ModelInfo> {
    let (stale, cached) = {
        let g = OPENCODE.lock().unwrap_or_else(|e| e.into_inner());
        match g.as_ref() {
            Some((t, v)) => (t.elapsed() > Duration::from_secs(1800), v.clone()),
            None => (true, vec![]),
        }
    };
    if stale && !OPENCODE_BUSY.swap(true, std::sync::atomic::Ordering::SeqCst) {
        std::thread::spawn(|| {
            let list = resolve_bin("opencode")
                .and_then(|bin| {
                    let mut c = std::process::Command::new(bin);
                    c.arg("models");
                    if crate::acct_models::opencode_standalone() {
                        c.arg("--standalone");
                    }
                    hidden(&mut c);
                    c.output().ok()
                })
                .map(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .lines()
                        .map(str::trim)
                        .filter(|l| l.contains('/'))
                        .map(|l| ModelInfo { slug: l.to_string(), text: l.to_lowercase(), priority: 999, efforts: vec![] })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            *OPENCODE.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), list));
            OPENCODE_BUSY.store(false, std::sync::atomic::Ordering::SeqCst);
        });
    }
    cached
}

pub fn catalog(provider: &str) -> Vec<ModelInfo> {
    match provider {
        "codex" => codex_models(),
        "opencode" => opencode_models(),
        _ => vec![],
    }
}

const NOT_CHAT: [&str; 9] = ["review", "embed", "audio", "image", "realtime", "tts", "whisper", "moderation", "transcribe"];
const FAST_WORDS: [&str; 13] = ["mini", "nano", "lite", "flash", "haiku", "fast", "affordable", "efficient", "lighter", "easier", "small", "cheap", "quick"];
const STRONG_WORDS: [&str; 7] = ["opus", "pro", "max", "ultra", "frontier", "flagship", "largest"];
const STRONG_PHRASES: [&str; 3] = ["most demanding", "most capable", "most advanced"];

/// fast / standard / strong from what a model says about itself; None = not a coding chat model.
fn tier_of_text(text: &str) -> Option<usize> {
    if NOT_CHAT.iter().any(|w| text.contains(w)) {
        return None;
    }
    let tokens: Vec<&str> = text.split(|c: char| !c.is_ascii_alphanumeric()).collect();
    let hit = |ws: &[&str]| ws.iter().filter(|w| tokens.contains(w)).count();
    let fast = hit(&FAST_WORDS);
    let strong = hit(&STRONG_WORDS) + STRONG_PHRASES.iter().filter(|p| text.contains(**p)).count();
    Some(if strong > fast { 2 } else if fast > strong { 0 } else { 1 })
}

fn version_of(slug: &str) -> Vec<i64> {
    slug.split(|c: char| !c.is_ascii_digit()).filter_map(|n| n.parse().ok()).collect()
}

/// Best model per tier: not marked older/previous, then the agent's own priority, then the
/// highest version number. An empty string = nothing recognised → the agent's default model.
pub fn classify(models: &[ModelInfo]) -> [String; 3] {
    let mut best: [Option<&ModelInfo>; 3] = [None, None, None];
    let key = |m: &ModelInfo| {
        let old = ["older", "previous", "legacy", "deprecated"].iter().any(|w| m.text.contains(w));
        (old, m.priority, std::cmp::Reverse(version_of(&m.slug)))
    };
    for m in models {
        let Some(t) = tier_of_text(&m.text) else { continue };
        if best[t].map(|b| key(m) < key(b)).unwrap_or(true) {
            best[t] = Some(m);
        }
    }
    best.map(|b| b.map(|m| m.slug.clone()).unwrap_or_default())
}

/// The reasoning level to pass for this run, or None when the agent can't take one. Asks the
/// agent's own model list what the chosen model accepts and falls back one step at a time.
pub fn effort_for(provider: &str, model: &str, effort: &str) -> Option<String> {
    if !def(provider).is_some_and(|d| d.effort) {
        return None;
    }
    if provider != "codex" {
        return Some(effort.to_string());
    }
    let models = catalog("codex");
    let entry = models.iter().find(|m| m.slug == model).or_else(|| if model.is_empty() { models.iter().min_by_key(|m| m.priority) } else { None });
    let ladder = ["low", "medium", "high", "xhigh"];
    let want = ladder.iter().position(|e| *e == effort)?;
    let allowed: Vec<String> = entry.map(|m| m.efforts.clone()).filter(|e| !e.is_empty()).unwrap_or_else(|| ladder[..3].iter().map(|s| s.to_string()).collect());
    (0..=want).rev().map(|i| ladder[i]).find(|e| allowed.iter().any(|a| a == e)).map(String::from)
}

/// What the app would pick on its own for each tier right now ("" = agent default).
pub fn discovered(provider: &str) -> [String; 3] {
    classify(&catalog(provider))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CODEX_CACHE: &str = r#"{"models":[
      {"slug":"gpt-6.1-sol","display_name":"GPT-6.1-Sol","description":"Latest workhorse model for coding and everyday work.","priority":0,"visibility":"list","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"high"},{"effort":"xhigh"}]},
      {"slug":"gpt-6.1-terra","display_name":"GPT-6.1 Terra","description":"GPT-6.1 Terra Codex model.","priority":1,"visibility":"list"},
      {"slug":"gpt-6-astra","display_name":"GPT-6-Astra","description":"Frontier intelligence for the most demanding work.","priority":3,"visibility":"list"},
      {"slug":"gpt-6-luna","display_name":"GPT-6-Luna","description":"Fast and affordable model for easier tasks.","priority":5,"visibility":"list"},
      {"slug":"gpt-5.6-luna","display_name":"GPT-5.6-Luna","description":"Older fast and efficient model.","priority":8,"visibility":"list"},
      {"slug":"gpt-reserve","display_name":"GPT-Reserve","description":"Fast and affordable agentic coding model.","priority":4,"visibility":"hide"},
      {"slug":"codex-auto-review","display_name":"Codex Auto Review","description":"Automatic approval review model for Codex.","priority":43,"visibility":"list"}]}"#;

    #[test]
    fn tiers_come_from_what_the_agent_says_about_its_models() {
        let t = classify(&parse_codex_cache(CODEX_CACHE));
        assert_eq!(t, ["gpt-6-luna".to_string(), "gpt-6.1-sol".to_string(), "gpt-6-astra".to_string()]);
    }

    #[test]
    fn a_new_release_is_picked_up_without_code_changes() {
        let mut ms = parse_codex_cache(CODEX_CACHE);
        ms.push(ModelInfo { slug: "next-mini-2".into(), text: "next-mini-2 a mini model".into(), priority: -1, efforts: vec![] });
        assert_eq!(classify(&ms)[0], "next-mini-2");
        // names alone work too (OpenCode lists only provider/model)
        let oc: Vec<ModelInfo> = ["google/gemini-3-flash", "google/gemini-3-pro", "google/gemini-2-pro"]
            .iter()
            .map(|s| ModelInfo { slug: s.to_string(), text: s.to_string(), priority: 999, efforts: vec![] })
            .collect();
        let t = classify(&oc);
        assert_eq!(t[0], "google/gemini-3-flash");
        assert_eq!(t[2], "google/gemini-3-pro");
    }

    #[test]
    fn unknown_agent_has_no_tiers_and_no_effort() {
        assert_eq!(classify(&[]), [String::new(), String::new(), String::new()]);
        assert_eq!(effort_for("gemini", "", "high"), None);
        assert_eq!(effort_for("claude", "opus", "high").as_deref(), Some("high"));
    }

    #[test]
    fn custom_template_fills_placeholders() {
        let i = build("custom", "m1", "low", "", "myagent --model {model} --file {prompt_file}", Path::new("C:/x/logs/prompt.md")).unwrap();
        assert_eq!(i.shell_line.unwrap(), "myagent --model m1 --file C:/x/logs/prompt.md");
    }

    #[test]
    fn codex_approval_mode_picks_the_arguments() {
        set_codex_approval("headless");
        let a = codex_approval_args(true);
        assert!(a.contains(&"features.computer_use=false".to_string()) && a.contains(&"workspace-write".to_string()));
        assert!(codex_approval_args(false).contains(&"sandbox_mode=workspace-write".to_string()));
        set_codex_approval("full");
        assert_eq!(codex_approval_args(true), vec!["--dangerously-bypass-approvals-and-sandbox".to_string()]);
        set_codex_approval("");
        assert!(codex_approval_args(true).contains(&"features.computer_use=false".to_string()), "default is the easy mode");
    }

    #[test]
    fn nothing_installed_is_a_clear_error() {
        assert!(build("none", "", "low", "", "", Path::new("p")).unwrap_err().contains("ยังไม่มี AI"));
        assert!(build("custom", "", "low", "", "  ", Path::new("p")).is_err());
    }

    #[test]
    fn tier_defaults_and_overrides() {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);").unwrap();
        assert_eq!(tier_model(&c, "claude", 2), "opus");
        assert_eq!(tier_model(&c, "gemini", 1), "");
        db::set_setting(&c, "tier_models", r#"{"gemini":{"standard":"my-model"}}"#).unwrap();
        assert_eq!(tier_model(&c, "gemini", 1), "my-model");
        assert_eq!(tier_model(&c, "gemini", 0), "");
    }
}
