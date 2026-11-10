//! OmniRoute (github.com/cheaper-inference/OmniRoute): a gateway on this machine that routes one endpoint to the many
//! providers the owner connected in it (free tiers first, with fallback). The bot reaches it through Claude Code, as
//! OmniRoute documents: `ANTHROPIC_BASE_URL=http://localhost:20128` (the root, no /v1) + `ANTHROPIC_AUTH_TOKEN`.
//! Claude Code keeps the agent part (files, commands); OmniRoute picks the model (`auto/coding`, a combo, or
//! `provider/model`).
//!
//! - A key is optional: a local OmniRoute answers /v1/messages without one, but its model list (/v1/models) needs a key
//!   made in its dashboard (API Manager). Without a key the account sends a placeholder token.
//! - When the address is this machine and nothing answers there, the app starts it (`omniroute serve --daemon`).
//! - Its management API (quota, connections, call logs, the model catalog) is read the way its own CLI does on this
//!   machine: the `x-omniroute-cli-token` header, an HMAC of the machine id that OmniRoute accepts from loopback only
//!   (docs/security/CLI_TOKEN.md) — so nothing has to be set up. A remote OmniRoute needs a key with the `manage` scope.
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use crate::accounts::Account;

pub const VENDOR: &str = "omniroute";
pub const DEFAULT_BASE: &str = "http://localhost:20128";
/// sent when the owner gave no key (Claude Code needs some token, or it falls back to its own sign-in)
pub const NO_KEY: &str = "omniroute-local";

pub fn is(a: &Account) -> bool {
    a.vendor_id() == VENDOR
}

/// The gateway's address for this account (root, no trailing slash, no /v1).
pub fn base(a: &Account) -> String {
    let b = if a.base_url.trim().is_empty() { DEFAULT_BASE } else { a.base_url.trim() };
    b.trim_end_matches('/').trim_end_matches("/v1").trim_end_matches('/').to_string()
}

/// host:port of an http(s) address
fn host_port(base: &str) -> Option<(String, u16)> {
    let (scheme, rest) = base.split_once("://")?;
    let hp = rest.split('/').next()?;
    let (h, p) = match hp.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse().ok()?),
        None => (hp.to_string(), if scheme == "https" { 443 } else { 80 }),
    };
    Some((h, p))
}

fn local(base: &str) -> bool {
    host_port(base).is_some_and(|(h, _)| matches!(h.as_str(), "localhost" | "127.0.0.1" | "[::1]"))
}

/// Does something answer at this address?
fn port_open(base: &str) -> bool {
    let Some((h, p)) = host_port(base) else { return false };
    let Ok(addrs) = (h.as_str(), p).to_socket_addrs() else { return false };
    addrs.into_iter().any(|ad| TcpStream::connect_timeout(&ad, Duration::from_millis(600)).is_ok())
}

/// Is the gateway ready to work (its /api/health says ok)? An open port alone can be a server still starting up.
pub fn up(base: &str) -> bool {
    if !port_open(base) {
        return false;
    }
    match send(base, "/api/health", &[], None, 4) {
        Ok((200, v)) => v["status"].as_str().is_none_or(|s| s == "ok" || s == "healthy"),
        // an older OmniRoute without /api/health: the open port has to do
        Ok((404, _)) => true,
        _ => false,
    }
}

/// One blocking GET (or POST with `body`) → (status, JSON). Call off the async runtime.
fn send(base: &str, path: &str, headers: &[(String, String)], body: Option<&Value>, secs: u64) -> Result<(u16, Value), String> {
    let url = format!("{base}{path}");
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| e.to_string())?;
    rt.block_on(async {
        let c = reqwest::Client::builder().timeout(Duration::from_secs(secs)).build().map_err(|e| e.to_string())?;
        let mut r = match body {
            Some(b) => c.post(&url).json(b),
            None => c.get(&url),
        };
        for (k, v) in headers {
            r = r.header(k.as_str(), v.as_str());
        }
        let r = r.send().await.map_err(|e| format!("ติดต่อ OmniRoute ไม่ได้: {e}"))?;
        let code = r.status().as_u16();
        let v = r.json::<Value>().await.unwrap_or(Value::Null);
        Ok((code, v))
    })
}

// ---------- the machine's own pass to the management API (as OmniRoute's CLI does it) ----------

/// The machine id the way node-machine-id reads it (raw, lowercased).
fn machine_id() -> Option<String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let out = std::process::Command::new("reg")
            .args(["query", r"HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Cryptography", "/v", "MachineGuid"])
            .creation_flags(0x0800_0000)
            .output()
            .ok()?;
        let t = String::from_utf8_lossy(&out.stdout).to_string();
        let id: String = t.split("REG_SZ").nth(1)?.chars().filter(|c| !c.is_whitespace()).collect();
        (!id.is_empty()).then(|| id.to_lowercase())
    }
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("ioreg").args(["-rd1", "-c", "IOPlatformExpertDevice"]).output().ok()?;
        let t = String::from_utf8_lossy(&out.stdout).to_string();
        let line = t.split("IOPlatformUUID").nth(1)?.lines().next()?.to_string();
        let id: String = line.chars().filter(|c| !c.is_whitespace() && *c != '=' && *c != '"').collect();
        (!id.is_empty()).then(|| id.to_lowercase())
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        ["/var/lib/dbus/machine-id", "/etc/machine-id"]
            .iter()
            .find_map(|p| std::fs::read_to_string(p).ok())
            .map(|s| s.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase())
            .filter(|s| !s.is_empty())
    }
}

/// OMNIROUTE_CLI_SALT when the owner rotated it (environment, then ~/.omniroute/.env), else OmniRoute's default.
fn salt() -> String {
    if let Ok(s) = std::env::var("OMNIROUTE_CLI_SALT") {
        if !s.trim().is_empty() {
            return s.trim().to_string();
        }
    }
    let env = dirs::home_dir().map(|h| h.join(".omniroute").join(".env")).and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
    env.lines()
        .filter_map(|l| l.trim().strip_prefix("OMNIROUTE_CLI_SALT="))
        .map(|v| v.trim().trim_matches('"').trim_matches('\'').to_string())
        .find(|v| !v.is_empty())
        .unwrap_or_else(|| "omniroute-cli-auth-v1".into())
}

fn sha256(data: &[u8]) -> Vec<u8> {
    let h = crate::skills::sha256_hex(data);
    (0..h.len()).step_by(2).filter_map(|i| u8::from_str_radix(&h[i..i + 2], 16).ok()).collect()
}

fn hmac_sha256_hex(key: &[u8], msg: &[u8]) -> String {
    let mut k = if key.len() > 64 { sha256(key) } else { key.to_vec() };
    k.resize(64, 0);
    let mut inner: Vec<u8> = k.iter().map(|b| b ^ 0x36).collect();
    inner.extend_from_slice(msg);
    let mut outer: Vec<u8> = k.iter().map(|b| b ^ 0x5c).collect();
    outer.extend_from_slice(&sha256(&inner));
    crate::skills::sha256_hex(&outer)
}

static CLI_TOKEN: LazyLock<Option<String>> = LazyLock::new(|| machine_id().map(|id| hmac_sha256_hex(id.as_bytes(), salt().as_bytes())));

/// Headers that open the management API: this machine's pass for a local OmniRoute, plus the account's key (a key
/// with the `manage` scope is what opens a remote one).
fn mgmt_headers(a: &Account, key: Option<&str>) -> Vec<(String, String)> {
    let mut h = vec![];
    if local(&base(a)) {
        if let Some(t) = CLI_TOKEN.as_ref() {
            h.push(("x-omniroute-cli-token".to_string(), t.clone()));
        }
    }
    if let Some(k) = key.filter(|k| !k.trim().is_empty()) {
        h.push(("authorization".to_string(), format!("Bearer {}", k.trim())));
    }
    h
}

/// A management read. Err says how to make it work.
fn mgmt(a: &Account, key: Option<&str>, path: &str) -> Result<Value, String> {
    let b = base(a);
    match send(&b, path, &mgmt_headers(a, key), None, 15)? {
        (200, v) => Ok(v),
        (401 | 403, _) if local(&b) => Err("OmniRoute ไม่ให้แอปอ่านข้อมูลการจัดการ (สิทธิ์ของโปรแกรมในเครื่องถูกปิดไว้) — วิธีแก้: สร้าง key ที่ dashboard → API Manager ติ๊กสิทธิ์ manage แล้ววางในบัญชีนี้".into()),
        (401 | 403, _) => Err("OmniRoute ที่อยู่เครื่องอื่นต้องใช้ key ที่มีสิทธิ์ manage จึงอ่านโควต้า/ประวัติได้ — วิธีแก้: dashboard → API Manager → สร้าง key ติ๊ก manage แล้ววางในบัญชีนี้".into()),
        (c, _) => Err(format!("OmniRoute ตอบ HTTP {c}")),
    }
}

static LAST_START: Mutex<Option<Instant>> = Mutex::new(None);

/// Make sure the gateway answers: start a local one when it is down (at most once every 2 minutes) and wait for it.
/// Err says what to do when it cannot.
pub fn ensure_running(a: &Account) -> Result<(), String> {
    let b = base(a);
    if up(&b) {
        return Ok(());
    }
    if !local(&b) {
        return Err(format!("ติดต่อ OmniRoute ที่ {b} ไม่ได้"));
    }
    let bin = crate::providers::resolve_bin("omniroute").ok_or("OmniRoute ไม่ได้เปิดอยู่ และไม่พบโปรแกรม omniroute ในเครื่อง — ติดตั้งด้วย npm install -g omniroute")?;
    {
        let mut last = LAST_START.lock();
        if last.is_some_and(|t| t.elapsed() < Duration::from_secs(120)) {
            return Err("OmniRoute ยังไม่ตอบ (เพิ่งสั่งเปิดไป) — รอสักครู่แล้วลองใหม่".into());
        }
        *last = Some(Instant::now());
    }
    let mut c = std::process::Command::new(bin);
    c.args(["serve", "--daemon", "--no-open", "--no-tray"]);
    if let Some((_, p)) = host_port(&b) {
        c.args(["--port", &p.to_string()]);
    }
    c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    c.spawn().map_err(|e| format!("เปิด OmniRoute ไม่ได้: {e}"))?;
    let end = Instant::now() + Duration::from_secs(40);
    while Instant::now() < end {
        std::thread::sleep(Duration::from_millis(800));
        if up(&b) {
            return Ok(());
        }
    }
    Err("สั่งเปิด OmniRoute แล้วแต่ยังไม่ตอบภายใน 40 วินาที — ลองเปิดเองด้วย omniroute serve".into())
}

/// Models OmniRoute itself says cannot call tools (so they cannot do bot work), per account — filled when the list is
/// read with a key. None = it did not say (then every model goes through the real check).
static NO_TOOLS: LazyLock<Mutex<HashMap<String, Vec<String>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
/// Why the list could not be pre-filtered (shown under the model list), per account.
static NOTES: LazyLock<Mutex<HashMap<String, String>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn no_tools(id: &str, model: &str) -> bool {
    NO_TOOLS.lock().get(id).is_some_and(|l| l.iter().any(|m| m == model))
}

pub fn note(id: &str) -> Option<String> {
    NOTES.lock().get(id).cloned()
}

/// A routing alias or a combo (OmniRoute picks the model itself) rather than one provider's model.
fn is_alias(m: &str) -> bool {
    m.starts_with("auto") || !m.contains('/')
}

fn bearer(k: &str) -> Vec<(String, String)> {
    vec![("authorization".to_string(), format!("Bearer {}", k.trim()))]
}

/// The models the gateway offers. Blocking; call off the async runtime.
/// - with a key: its /v1/models (what that key may use), and which of them can call tools (/api/v1/models/query)
/// - without: the catalog of the providers connected in it (management API, this machine's pass)
pub fn models(a: &Account, key: Option<&str>) -> Result<Vec<String>, String> {
    ensure_running(a)?;
    let b = base(a);
    let key = key.filter(|k| !k.trim().is_empty());
    let mut ids: Vec<String> = match key {
        Some(k) => match send(&b, "/v1/models", &bearer(k), None, 20)? {
            (401, _) => return Err("OmniRoute ไม่รับ key นี้ (401) — สร้าง key ใหม่ที่ dashboard → API Manager".to_string()),
            (200, v) => v["data"].as_array().map(|l| l.iter().filter_map(|m| m["id"].as_str().map(String::from)).collect()).unwrap_or_default(),
            (c, _) => return Err(format!("OmniRoute ตอบ HTTP {c}")),
        },
        None => {
            let v = mgmt(a, None, "/api/models").map_err(|e| format!("{e} · หรือใส่ key ของ OmniRoute เพื่ออ่านรายชื่อโมเดล — ระหว่างนี้ใช้ auto/coding ได้"))?;
            v["models"]
                .as_array()
                .map(|l| l.iter().filter(|m| m["available"].as_bool() != Some(false)).filter_map(|m| m["fullModel"].as_str().map(String::from)).collect())
                .unwrap_or_default()
        }
    };
    // the routing aliases first (they pick a working model by themselves), then the rest by name
    ids.sort_by_key(|m| (!m.starts_with("auto"), m.clone()));
    ids.dedup();
    for al in ["auto/coding", "auto"] {
        if !ids.iter().any(|m| m == al) {
            ids.insert(0, al.to_string());
        }
    }
    if ids.iter().all(|m| m.starts_with("auto")) {
        return Err("OmniRoute ไม่มีโมเดลให้ใช้ — เชื่อมผู้ให้บริการใน dashboard ของ OmniRoute ก่อน".into());
    }
    // which of them can call tools (read/write files, run commands): the others cannot do bot work at all
    let note = match key {
        None => Some("ใส่ key ของ OmniRoute (dashboard → API Manager) เพื่อให้แอปคัดโมเดลที่เรียกเครื่องมือไม่ได้ออกก่อน — ตอนนี้ทุกโมเดลต้องลองรันตรวจทีละตัว (เปลืองโควต้ากว่า)".to_string()),
        Some(k) => match tool_models(&b, k) {
            Some(ok) if !ok.is_empty() => {
                let no: Vec<String> = ids.iter().filter(|m| !is_alias(m) && !ok.contains(*m)).cloned().collect();
                NO_TOOLS.lock().insert(a.id.clone(), no);
                None
            }
            _ => Some("OmniRoute ไม่บอกว่าโมเดลไหนเรียกเครื่องมือได้ — ทุกโมเดลจะถูกลองรันตรวจทีละตัวแทน".to_string()),
        },
    };
    match note {
        Some(n) => {
            NO_TOOLS.lock().remove(&a.id);
            NOTES.lock().insert(a.id.clone(), n)
        }
        None => NOTES.lock().remove(&a.id),
    };
    Ok(ids)
}

/// The models that can call tools, by OmniRoute's own catalog (100 per answer: asked per provider when it is full).
fn tool_models(base: &str, key: &str) -> Option<std::collections::HashSet<String>> {
    let ask = |owned_by: Option<&str>| -> Option<Vec<(String, String)>> {
        let mut body = json!({"capabilities": ["tool_calling"], "kind": "chat", "limit": 100});
        if let Some(o) = owned_by {
            body["owned_by"] = json!(o);
        }
        match send(base, "/api/v1/models/query", &bearer(key), Some(&body), 20).ok()? {
            (200, v) => Some(v["data"].as_array()?.iter().filter_map(|m| Some((m["id"].as_str()?.to_string(), m["owned_by"].as_str().unwrap_or("").to_string()))).collect()),
            _ => None,
        }
    };
    let first = ask(None)?;
    let mut ok: std::collections::HashSet<String> = first.iter().map(|x| x.0.clone()).collect();
    if first.len() >= 100 {
        let owners: std::collections::BTreeSet<String> = first.iter().map(|x| x.1.clone()).filter(|o| !o.is_empty()).collect();
        for o in owners {
            ok.extend(ask(Some(&o)).unwrap_or_default().into_iter().map(|x| x.0));
        }
    }
    Some(ok)
}

// ---------- what OmniRoute knows about its providers: quota, broken connections, a subscription used twice ----------

/// The latest reading per account (see `refresh`).
static STATUS: LazyLock<Mutex<HashMap<String, Value>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn status(id: &str) -> Option<Value> {
    STATUS.lock().get(id).cloned()
}

/// Why this account must not run now (a subscription the app already uses is connected in OmniRoute too).
pub fn blocked(id: &str) -> Option<String> {
    STATUS.lock().get(id).and_then(|v| v["block"].as_str().map(String::from)).filter(|s| !s.is_empty())
}

pub fn dup_key(id: &str) -> String {
    format!("omniroute_dup_ok:{id}")
}

/// Read quota and connections from OmniRoute (blocking) and keep them. `others` = all the app's accounts.
pub fn refresh(a: &Account, key: Option<&str>, others: &[Account], dup_ok: bool) {
    let v = read_status(a, key, others, dup_ok);
    STATUS.lock().insert(a.id.clone(), v);
}

fn read_status(a: &Account, key: Option<&str>, others: &[Account], dup_ok: bool) -> Value {
    if !up(&base(a)) {
        return json!({"error": format!("OmniRoute ที่ {} ไม่ตอบ — แอปจะสั่งเปิดให้ตอนจะใช้งาน", base(a))});
    }
    let quota = match mgmt(a, key, "/api/usage/quota") {
        Ok(v) => v,
        Err(e) => return json!({"error": e}),
    };
    let rows: Vec<Value> = quota["providers"]
        .as_array()
        .map(|l| {
            l.iter()
                .map(|p| json!({"provider": p["provider"], "name": p["name"], "pct": p["percentRemaining"], "used": p["quotaUsed"], "total": p["quotaTotal"], "reset": p["resetAt"]}))
                .collect()
        })
        .unwrap_or_default();
    let conns: Vec<Value> = mgmt(a, key, "/api/providers").ok().and_then(|v| v["connections"].as_array().cloned()).unwrap_or_default();
    let (block, warnings) = check_connections(&conns, others, dup_ok);
    json!({"quota": rows, "warnings": warnings, "block": block, "dup_ok": dup_ok})
}

/// (why the account must wait, notes) from OmniRoute's provider connections.
fn check_connections(conns: &[Value], others: &[Account], dup_ok: bool) -> (String, Vec<String>) {
    let mut warnings: Vec<String> = vec![];
    // connections that stopped working (a login that expired): OmniRoute skips them until the owner signs in again
    let broken: Vec<String> = conns
        .iter()
        .filter(|x| x["testStatus"].as_str().is_some_and(|s| matches!(s, "expired" | "error" | "banned" | "invalid" | "unavailable")))
        .map(|x| format!("{} ({})", x["provider"].as_str().unwrap_or("?"), x["name"].as_str().unwrap_or("")))
        .collect();
    if !broken.is_empty() {
        warnings.push(format!("การเชื่อมต่อใน OmniRoute ใช้ไม่ได้แล้ว: {} — OmniRoute จะข้ามไป · วิธีแก้: dashboard → Providers → ล็อกอินใหม่ หรือลบทิ้ง", broken.join(", ")));
    }
    // a subscription the app also runs directly: its quota would be spent from two sides, past the app's caps
    let mut block = String::new();
    for (prov, kind, name) in [("claude", "claude", "Claude"), ("codex", "codex", "ChatGPT (Codex)")] {
        let theirs: Vec<&Value> = conns
            .iter()
            .filter(|x| x["provider"] == prov && x["authType"] == "oauth" && x["isActive"].as_bool() != Some(false) && x["testStatus"].as_str() != Some("expired"))
            .collect();
        let ours = others.iter().any(|o| o.enabled && o.kind == kind && !o.uses_key());
        if theirs.is_empty() || !ours {
            continue;
        }
        let who = theirs.iter().filter_map(|x| x["name"].as_str()).collect::<Vec<_>>().join(", ");
        if dup_ok {
            warnings.push(format!("OmniRoute เชื่อมแพ็กเกจ {name} ({who}) ไว้ — คุณยืนยันแล้วว่าเป็นคนละบัญชีกับในแอป"));
        } else if block.is_empty() {
            // in short pieces joined by " · " (each piece is translated on its own)
            block = [
                format!("OmniRoute เชื่อมแพ็กเกจ {name} ({who}) ไว้ ซึ่งแอปก็ใช้แพ็กเกจนี้อยู่โดยตรง — ถ้าเป็นบัญชีเดียวกัน โควต้าจะถูกใช้สองทางและแอปคุมเพดานไม่ได้"),
                "จึงพักบัญชี OmniRoute นี้ไว้".to_string(),
                format!("วิธีแก้: ปิดการเชื่อมต่อ {prov} ใน dashboard ของ OmniRoute (Providers) หรือปิดบัญชีนี้ในแอป"),
                "ถ้าเป็นคนละบัญชีกัน กด “เป็นคนละบัญชี — ใช้ต่อ”".to_string(),
            ]
            .join(" · ");
        }
    }
    (block, warnings)
}

/// Which provider/model OmniRoute really sent a run's requests to (its call log between `from` and `to`, unix
/// seconds) — e.g. "groq/openai/gpt-oss-120b ×12 · opencode-zen/big-pickle ×2". Blocking.
pub fn served(a: &Account, key: Option<&str>, from: i64, to: i64) -> Option<String> {
    count_served(&mgmt(a, key, "/api/usage/call-logs?limit=300").ok()?, from, to)
}

fn count_served(logs: &Value, from: i64, to: i64) -> Option<String> {
    let mut n: Vec<(String, usize)> = vec![];
    for x in logs.as_array()? {
        if x["path"].as_str().unwrap_or("").starts_with("/api/") {
            continue; // OmniRoute's own tests and management calls
        }
        let Some(t) = x["timestamp"].as_str().and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok()).map(|d| d.timestamp()) else { continue };
        if t < from - 5 || t > to + 5 {
            continue;
        }
        let p = x["provider"].as_str().unwrap_or("?");
        let m = x["model"].as_str().unwrap_or("?");
        let name = if m.starts_with(&format!("{p}/")) { m.to_string() } else { format!("{p}/{m}") };
        match n.iter_mut().find(|e| e.0 == name) {
            Some(e) => e.1 += 1,
            None => n.push((name, 1)),
        }
    }
    if n.is_empty() {
        return None;
    }
    n.sort_by(|a, b| b.1.cmp(&a.1));
    Some(n.iter().take(3).map(|(m, c)| format!("{m} ×{c}")).collect::<Vec<_>>().join(" · "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses() {
        assert_eq!(host_port("http://localhost:20128"), Some(("localhost".into(), 20128)));
        assert_eq!(host_port("https://omni.example.com/v1"), Some(("omni.example.com".into(), 443)));
        assert!(local("http://127.0.0.1:20128"));
        assert!(!local("https://omni.example.com"));
        let mut a = crate::accounts::Account {
            id: "o".into(), kind: "claude".into(), label: String::new(), enabled: true, home: String::new(), model: "auto".into(),
            effort: "auto".into(), vendor: VENDOR.into(), auth: "key".into(), base_url: String::new(),
        };
        assert_eq!(base(&a), DEFAULT_BASE);
        a.base_url = "http://localhost:20128/v1/".into();
        assert_eq!(base(&a), "http://localhost:20128");
    }

    #[test]
    fn the_machine_pass_is_hmac_sha256() {
        // RFC 4231 test case 2
        assert_eq!(hmac_sha256_hex(b"Jefe", b"what do ya want for nothing?"), "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843");
    }

    #[test]
    fn a_subscription_used_twice_holds_the_account() {
        let acc = |kind: &str, auth: &str| crate::accounts::Account {
            id: kind.into(), kind: kind.into(), label: String::new(), enabled: true, home: String::new(), model: "auto".into(),
            effort: "auto".into(), vendor: String::new(), auth: auth.into(), base_url: String::new(),
        };
        let conns = vec![
            json!({"provider": "claude", "authType": "oauth", "isActive": true, "testStatus": "active", "name": "me@x.com"}),
            json!({"provider": "kiro", "authType": "oauth", "isActive": false, "testStatus": "expired", "name": "kiro"}),
        ];
        let (block, warn) = check_connections(&conns, &[acc("claude", "login")], false);
        assert!(block.contains("Claude") && warn.iter().any(|w| w.contains("kiro")));
        // the owner said it is another account → a note, not a hold
        let (block, warn) = check_connections(&conns, &[acc("claude", "login")], true);
        assert!(block.is_empty() && warn.iter().any(|w| w.contains("คนละบัญชี")));
        // the app runs Claude with a key (not the subscription) or not at all → nothing to hold
        assert!(check_connections(&conns, &[acc("claude", "key")], false).0.is_empty());
        // an expired Claude login in OmniRoute is not used by it
        let expired = vec![json!({"provider": "claude", "authType": "oauth", "isActive": false, "testStatus": "expired", "name": "me"})];
        assert!(check_connections(&expired, &[acc("claude", "login")], false).0.is_empty());
        assert!(is_alias("auto/coding") && is_alias("my-combo") && !is_alias("groq/llama"));
    }

    #[test]
    fn a_runs_requests_are_counted_by_model() {
        let logs = json!([
            {"path": "/v1/messages", "timestamp": "2026-11-06T09:00:10Z", "provider": "groq", "model": "openai/gpt-oss-120b"},
            {"path": "/v1/messages", "timestamp": "2026-11-06T09:00:20Z", "provider": "groq", "model": "openai/gpt-oss-120b"},
            {"path": "/v1/messages", "timestamp": "2026-11-06T09:00:30Z", "provider": "opencode-zen", "model": "big-pickle"},
            {"path": "/api/providers/test", "timestamp": "2026-11-06T09:00:30Z", "provider": "groq", "model": "connection-test"},
            {"path": "/v1/messages", "timestamp": "2026-11-06T12:00:00Z", "provider": "groq", "model": "other"},
        ]);
        let from = chrono::DateTime::parse_from_rfc3339("2026-11-06T09:00:00Z").unwrap().timestamp();
        assert_eq!(count_served(&logs, from, from + 60).as_deref(), Some("groq/openai/gpt-oss-120b ×2 · opencode-zen/big-pickle ×1"));
        assert!(count_served(&logs, from + 9000, from + 9001).is_none());
    }
}
