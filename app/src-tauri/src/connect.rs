//! Connection health of the AI accounts (ISSUE-099). Tells a sign-in / key / certificate / network failure from a
//! failure of the work itself, turns the agent's raw error into a cause and what to do, and holds ("parks") an account
//! that cannot connect so the bot moves on to another one instead of failing the idea three times.

use crate::db;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub struct Diagnosis {
    /// auth | cert | credit | model | install | network
    pub kind: &'static str,
    pub cause: &'static str,
    pub fix: &'static str,
    /// the one button that helps: login | key | models | install | retry
    pub action: &'static str,
}

const CERT: &[&str] = &["unable_to_get_issuer_cert", "unable to get local issuer", "self signed certificate", "self-signed certificate", "unable to verify the first certificate", "cert_has_expired", "certificate verify failed", "depth_zero_self_signed_cert"];
const AUTH: &[&str] = &[
    "not logged in", "run /login", "claude auth login", "codex login", "invalid api key", "invalid x-api-key", "invalid_api_key", "incorrect api key",
    "authentication_error", "authentication failed", "unauthorized", "oauth token has expired", "token has expired", "token expired", "token is expired",
    "login required", "not authenticated", "missing api key", "no api key", "api key not found", "api key is missing", "status 401", "error 401", "http 401",
    "401 unauthorized", "sign in again", "please log in", "please login", "re-authenticate", "reauthenticate", "refresh token was already used",
];
const CREDIT: &[&str] = &["credit balance is too low", "insufficient credit", "insufficient balance", "insufficient_quota", "payment required", "billing", "status 402", "error 402"];
const MODEL: &[&str] = &["model_not_found", "model not found", "unknown model", "invalid model", "does not exist or you do not have access", "is not a valid model", "no such model"];
const INSTALL: &[&str] = &["is not recognized as an internal or external command", "command not found", "ยังไม่ได้ติดตั้ง"];
const NETWORK: &[&str] = &["enotfound", "econnrefused", "econnreset", "etimedout", "getaddrinfo", "fetch failed", "connection refused", "connection error", "socket hang up", "network error", "network is unreachable", "tunneling socket"];

fn hit(text: &str, words: &[&str]) -> bool {
    words.iter().any(|w| text.contains(w))
}

/// What a failed run's error text says about the connection; None when it does not look like a connection problem.
pub fn diagnose(text: &str, uses_key: bool) -> Option<Diagnosis> {
    let t = text.to_lowercase();
    if hit(&t, CERT) {
        return Some(Diagnosis { kind: "cert", cause: "เครือข่ายนี้ตรวจการเชื่อมต่อแบบเข้ารหัส และ AI ไม่รู้จักใบรับรองของเครือข่าย", fix: "โปรแกรมสั่งให้ AI ใช้ใบรับรองของ Windows ให้แล้ว ลองทดสอบอีกครั้ง ถ้ายังไม่ผ่าน ขอไฟล์ใบรับรอง (CA) จากผู้ดูแลเครือข่าย แล้วตั้งตัวแปร NODE_EXTRA_CA_CERTS ให้ชี้ไปที่ไฟล์นั้น", action: "retry" });
    }
    if hit(&t, AUTH) {
        return Some(if uses_key {
            Diagnosis { kind: "auth", cause: "key ของบัญชีนี้ใช้ไม่ได้ (ผิด หมดอายุ หรือถูกยกเลิก)", fix: "ใส่ key ใหม่ แล้วโปรแกรมจะทดสอบให้เอง", action: "key" }
        } else {
            Diagnosis { kind: "auth", cause: "การล็อกอินของบัญชีนี้หลุดหรือหมดอายุ", fix: "กด “ล็อกอินใหม่” แล้วทำตามในหน้าต่างที่เปิดขึ้น", action: "login" }
        });
    }
    if hit(&t, CREDIT) {
        return Some(Diagnosis { kind: "credit", cause: "เครดิตหรือยอดเงินของบัญชีนี้หมด", fix: "เติมเครดิตที่เว็บของผู้ให้บริการ หรือใช้บัญชีอื่นไปก่อน", action: "retry" });
    }
    if hit(&t, MODEL) {
        return Some(Diagnosis { kind: "model", cause: "บัญชีนี้ไม่มีโมเดลที่เลือกไว้", fix: "เลือกโมเดลอื่นจากรายชื่อของบัญชีนี้", action: "models" });
    }
    if hit(&t, INSTALL) {
        return Some(Diagnosis { kind: "install", cause: "ยังไม่ได้ติดตั้งโปรแกรมของ AI ตัวนี้ หรือหาไม่เจอ", fix: "กด “ติดตั้ง” แล้วทดสอบอีกครั้ง", action: "install" });
    }
    if hit(&t, NETWORK) {
        return Some(Diagnosis { kind: "network", cause: "ต่ออินเทอร์เน็ตไปหาผู้ให้บริการ AI ไม่ได้", fix: "เช็กอินเทอร์เน็ต / VPN / proxy แล้วลองใหม่ โปรแกรมจะลองเองอีกครั้งในไม่กี่นาที", action: "retry" });
    }
    None
}

/// How long an account is held for each kind of problem before the bot tries it again by itself.
pub fn hold_secs(kind: &str) -> i64 {
    match kind {
        "network" | "cert" => 15 * 60,
        _ => 6 * 3600,
    }
}

pub fn json_of(d: &Diagnosis) -> Value {
    json!({ "kind": d.kind, "cause": d.cause, "fix": d.fix, "action": d.action })
}

fn load(c: &Connection) -> BTreeMap<String, Value> {
    db::get_setting(c, "acct_problems").and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}
fn save(c: &Connection, m: &BTreeMap<String, Value>) {
    if let Ok(s) = serde_json::to_string(m) {
        let _ = db::set_setting(c, "acct_problems", &s);
    }
}

/// Hold this account until `until` (it is tried again after that) and remember why.
pub fn park(c: &Connection, account: &str, d: &Diagnosis, until: i64) {
    let mut m = load(c);
    let mut v = json_of(d);
    v["until"] = json!(until);
    m.insert(account.to_string(), v);
    save(c, &m);
}

/// The account works again (a test passed, the owner logs in again or gave a new key).
pub fn unpark(c: &Connection, account: &str) {
    let mut m = load(c);
    if m.remove(account).is_some() {
        save(c, &m);
    }
}

/// The problem holding this account now ({kind, cause, fix, action, until}); an elapsed one is dropped.
pub fn parked(c: &Connection, account: &str, now: i64) -> Option<Value> {
    let v = load(c).get(account)?.clone();
    if v["until"].as_i64().unwrap_or(0) > now {
        Some(v)
    } else {
        unpark(c, account);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_errors_become_a_cause_and_one_action() {
        let d = |t: &str, key: bool| diagnose(t, key).map(|d| (d.kind, d.action));
        assert_eq!(d("exit Some(1): Invalid API key · Please run /login", false), Some(("auth", "login")));
        assert_eq!(d("401 Unauthorized: invalid x-api-key", true), Some(("auth", "key")));
        assert_eq!(d("Error: refresh token was already used", false), Some(("auth", "login")));
        assert_eq!(d("request failed: UNABLE_TO_GET_ISSUER_CERT_LOCALLY fetch failed", false), Some(("cert", "retry")));
        assert_eq!(d("getaddrinfo ENOTFOUND api.anthropic.com", false), Some(("network", "retry")));
        assert_eq!(d("Your credit balance is too low to access the API", true), Some(("credit", "retry")));
        assert_eq!(d("model_not_found: gpt-9", false), Some(("model", "models")));
        assert_eq!(d("'codex' is not recognized as an internal or external command", false), Some(("install", "install")));
        // a failure of the work itself is not a connection problem
        assert!(d("exit Some(1): test suite failed: 3 assertions", false).is_none());
        assert!(d("TypeError: cannot read properties of undefined", false).is_none());
    }

    #[test]
    fn a_parked_account_comes_back_when_time_is_up_or_it_is_fixed() {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);").unwrap();
        let d = diagnose("not logged in", false).unwrap();
        park(&c, "claude01", &d, 1000);
        assert_eq!(parked(&c, "claude01", 500).unwrap()["action"], "login");
        assert!(parked(&c, "claude01", 1001).is_none());
        park(&c, "claude01", &d, 1000);
        unpark(&c, "claude01");
        assert!(parked(&c, "claude01", 500).is_none());
    }
}
