//! AI accounts: several logins of the same AI (claude01, claude02, ...) or of different ones, each with
//! its own name, on/off switch, model/effort choice and quota.
//!
//! - An account with an empty `home` uses the AI's normal login on this machine. One with a `home` keeps
//!   its login in its own folder (`<app data>/accounts/<id>`), pointed to through the environment variable
//!   each CLI documents for that (Claude `CLAUDE_CONFIG_DIR`, Codex `CODEX_HOME`, Gemini `GEMINI_CLI_HOME`,
//!   Mistral Vibe `VIBE_HOME`, OpenCode `XDG_*`). That is how two accounts of one AI stay apart.
//! - The bot works with the first account (in the order listed) that is switched on, installed and
//!   allowed by its own quota; when that one reaches its limit the next one takes over.
//! - A dedicated bot account (its own `home`) does not share its quota with the owner's own use, so the
//!   reserve for work hours and "the owner is working" never hold it back.
//! - Switching an account off pauses it; removing one forgets it, and deletes its own login folder (the
//!   machine's normal login is never touched).
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::db;

pub const KINDS: [&str; 5] = ["claude", "codex", "gemini", "opencode", "mistral"];
pub const EFFORTS: [&str; 4] = ["low", "medium", "high", "xhigh"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Account {
    /// the name the owner calls it by (letters, digits, - and _), e.g. claude01
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub label: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// "" = the AI's normal login on this machine
    #[serde(default)]
    pub home: String,
    /// "auto" (the router decides) or an exact model name
    #[serde(default = "auto")]
    pub model: String,
    /// "auto" or low | medium | high | xhigh
    #[serde(default = "auto")]
    pub effort: String,
    /// the provider (catalog.rs); empty = the agent's own, e.g. Anthropic for Claude Code
    #[serde(default)]
    pub vendor: String,
    /// "login" (the agent's own sign-in) or "key" (an API key / token kept in the account's folder)
    #[serde(default = "login")]
    pub auth: String,
    /// API address override for providers reached through Claude Code
    #[serde(default)]
    pub base_url: String,
}

fn login() -> String {
    "login".into()
}

fn yes() -> bool {
    true
}
fn auto() -> String {
    "auto".into()
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 32 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

impl Account {
    pub fn title(&self) -> String {
        if self.label.trim().is_empty() { self.id.clone() } else { self.label.clone() }
    }

    /// Does the owner's own use of the AI draw on the same quota as this account?
    pub fn shared_with_owner(&self) -> bool {
        self.kind == "claude" && self.home.is_empty()
    }

    pub fn is_auto_model(&self) -> bool {
        self.model.trim().is_empty() || self.model == "auto"
    }

    pub fn vendor_id(&self) -> String {
        if self.vendor.is_empty() { crate::catalog::native(&self.kind).to_string() } else { self.vendor.clone() }
    }

    pub fn uses_key(&self) -> bool {
        self.auth == "key"
    }

    pub fn forced_effort(&self) -> Option<&str> {
        EFFORTS.iter().find(|e| **e == self.effort).copied()
    }
}

/// Saved accounts. Until the owner has configured any, one account for the AI the app already used (the
/// `provider` setting, else the first installed), named after the AI, so everything works as before. Other AIs
/// on the machine are not switched on behind the owner's back; they are added on purpose.
pub fn list(c: &Connection) -> Vec<Account> {
    if let Some(s) = db::get_setting(c, "ai_accounts") {
        if let Ok(v) = serde_json::from_str::<Vec<Account>>(&s) {
            return v;
        }
    }
    let main = crate::providers::chosen(c);
    KINDS
        .iter()
        .filter(|k| **k == main && crate::providers::installed(k))
        .map(|k| Account { id: (*k).into(), kind: (*k).into(), label: String::new(), enabled: true, home: String::new(), model: auto(), effort: auto(), vendor: String::new(), auth: login(), base_url: String::new() })
        .collect()
}

pub fn save(c: &Connection, v: &[Account]) -> Result<(), String> {
    let mut seen = std::collections::BTreeSet::new();
    for a in v {
        if !valid_id(&a.id) {
            return Err(format!("ชื่อ “{}” ใช้ไม่ได้ (ใช้ a-z 0-9 - _ ไม่เกิน 32 ตัว)", a.id));
        }
        if !KINDS.contains(&a.kind.as_str()) {
            return Err(format!("ไม่รู้จักค่าย AI: {}", a.kind));
        }
        if !seen.insert(a.id.clone()) {
            return Err(format!("ชื่อ “{}” ซ้ำกัน", a.id));
        }
    }
    db::set_setting(c, "ai_accounts", &serde_json::to_string(v).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

pub fn find(c: &Connection, id: &str) -> Option<Account> {
    list(c).into_iter().find(|a| a.id == id)
}

/// Accounts the bot may use right now: switched on, and the AI installed.
pub fn usable(c: &Connection) -> Vec<Account> {
    list(c).into_iter().filter(|a| a.enabled && crate::providers::installed(&a.kind)).collect()
}

pub fn home_dir(root: &Path, id: &str) -> PathBuf {
    root.join("data").join("accounts").join(id)
}

/// Environment that points a CLI at this account's own login. Empty for the machine's normal login.
pub fn env_for(a: &Account) -> Vec<(String, String)> {
    if a.home.is_empty() {
        return vec![];
    }
    let h = &a.home;
    let join = |s: &str| Path::new(h).join(s).display().to_string();
    match a.kind.as_str() {
        "claude" => vec![("CLAUDE_CONFIG_DIR".into(), h.clone())],
        "codex" => vec![("CODEX_HOME".into(), h.clone())],
        "gemini" => vec![("GEMINI_CLI_HOME".into(), h.clone())],
        "mistral" => vec![("VIBE_HOME".into(), h.clone())],
        "opencode" => vec![("XDG_DATA_HOME".into(), join("data")), ("XDG_CONFIG_HOME".into(), join("config"))],
        _ => vec![],
    }
}

fn secret_path(root: &Path, id: &str) -> PathBuf {
    home_dir(root, id).join("key")
}

/// An API key / token lives only in the account's own folder, never in settings or the database, and is never sent
/// back to the interface.
pub fn set_secret(root: &Path, id: &str, key: &str) -> Result<(), String> {
    if !valid_id(id) {
        return Err("ชื่อบัญชีไม่ถูกต้อง".into());
    }
    let key = key.trim();
    if key.is_empty() || key.len() > 4096 || key.chars().any(|c| c.is_control()) {
        return Err("key ว่างหรือรูปแบบไม่ถูกต้อง".into());
    }
    std::fs::create_dir_all(home_dir(root, id)).map_err(|e| e.to_string())?;
    std::fs::write(secret_path(root, id), key).map_err(|e| e.to_string())
}

pub fn clear_secret(root: &Path, id: &str) {
    if valid_id(id) {
        let _ = std::fs::remove_file(secret_path(root, id));
    }
}

pub fn has_secret(root: &Path, id: &str) -> bool {
    valid_id(id) && secret_path(root, id).is_file()
}

pub(crate) fn secret(root: &Path, id: &str) -> Option<String> {
    std::fs::read_to_string(secret_path(root, id)).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Everything a run of this account needs in its environment: its own login folder, and its key when it signs in with one.
pub fn env_with_secret(a: &Account, root: &Path) -> Vec<(String, String)> {
    let mut env = env_for(a);
    if crate::omniroute::is(a) {
        // the gateway on this machine: start it when it is down; a key is optional there
        if let Err(why) = crate::omniroute::ensure_running(a) {
            eprintln!("omniroute: {why}");
        }
        let k = secret(root, &a.id).unwrap_or_else(|| crate::omniroute::NO_KEY.to_string());
        env.push(("ANTHROPIC_AUTH_TOKEN".into(), k));
        env.push(("ANTHROPIC_BASE_URL".into(), crate::omniroute::base(a)));
    } else if a.uses_key() {
        if let (Some(v), Some(k)) = (crate::catalog::vendor(&a.vendor_id()), secret(root, &a.id)) {
            env.extend(crate::catalog::key_env(v, &k, &a.base_url));
        }
    }
    env
}

/// What to type in a terminal to log this account in (shown in a window the owner finishes in the browser).
pub fn login_args(kind: &str) -> &'static [&'static str] {
    match kind {
        "claude" => &["auth", "login"],
        "codex" => &["login"],
        "opencode" => &["auth", "login"],
        // these start their own first-run login when they have none
        _ => &[],
    }
}

/// A free name for a new account of `kind`: claude01, claude02, ...
pub fn next_id(existing: &[Account], kind: &str) -> String {
    (1..).map(|n| format!("{kind}{n:02}")).find(|c| !existing.iter().any(|a| a.id == *c)).unwrap_or_else(|| kind.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acc(id: &str, kind: &str, home: &str) -> Account {
        Account { id: id.into(), kind: kind.into(), label: String::new(), enabled: true, home: home.into(), model: auto(), effort: auto(), vendor: String::new(), auth: login(), base_url: String::new() }
    }

    #[test]
    fn names_and_validation() {
        assert!(valid_id("claude01") && valid_id("work_2-b"));
        assert!(!valid_id("") && !valid_id("a b") && !valid_id("../x") && !valid_id(&"x".repeat(33)));
        let v = vec![acc("claude01", "claude", ""), acc("claude02", "claude", "x")];
        assert_eq!(next_id(&v, "claude"), "claude03");
        assert_eq!(next_id(&v, "codex"), "codex01");
    }

    #[test]
    fn each_cli_gets_its_own_login_folder() {
        assert!(env_for(&acc("a", "claude", "")).is_empty());
        assert_eq!(env_for(&acc("a", "claude", "D:/h"))[0], ("CLAUDE_CONFIG_DIR".into(), "D:/h".into()));
        assert_eq!(env_for(&acc("a", "codex", "D:/h"))[0].0, "CODEX_HOME");
        assert_eq!(env_for(&acc("a", "mistral", "D:/h"))[0].0, "VIBE_HOME");
        assert_eq!(env_for(&acc("a", "gemini", "D:/h"))[0].0, "GEMINI_CLI_HOME");
        assert_eq!(env_for(&acc("a", "opencode", "D:/h")).len(), 2);
    }

    #[test]
    fn only_the_owners_own_claude_login_shares_quota_with_the_owner() {
        assert!(acc("claude", "claude", "").shared_with_owner());
        assert!(!acc("claude02", "claude", "D:/h").shared_with_owner());
        assert!(!acc("codex", "codex", "").shared_with_owner());
    }

    #[test]
    fn save_rejects_duplicates_and_unknown_kinds() {
        let p = std::env::temp_dir().join(format!("ia-acc-{}-{:?}.db", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_file(&p);
        let c = db::open(&p).unwrap();
        assert!(save(&c, &[acc("a", "claude", ""), acc("a", "codex", "")]).is_err());
        assert!(save(&c, &[acc("a", "nope", "")]).is_err());
        save(&c, &[acc("claude01", "claude", ""), acc("claude02", "claude", "D:/x")]).unwrap();
        assert_eq!(list(&c).len(), 2);
        assert_eq!(find(&c, "claude02").unwrap().home, "D:/x");
    }
}
