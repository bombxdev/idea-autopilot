//! Installing an AI's command-line program from inside the app (the "Install CLI" button), so an account for it can work.
//!
//! The commands are fixed, the ones each provider documents (npm for Claude Code, Codex, Gemini CLI and OpenCode; uv for
//! Mistral Vibe), and run only when the owner presses the button after seeing the command. Nothing else is installed.
//! When the network cannot reach the default npm registry the owner may name another one (a mirror); it is used only
//! as the last attempt and only if given.
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::sync::LazyLock;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Default)]
struct Job {
    kind: String,
    /// idle | running | ok | error
    state: String,
    lines: Vec<String>,
    message: String,
}

static JOB: LazyLock<Mutex<Job>> = LazyLock::new(|| Mutex::new(Job { state: "idle".into(), ..Default::default() }));

/// tool, arguments (without the tool's name)
pub fn plan(kind: &str) -> Option<(&'static str, Vec<&'static str>)> {
    Some(match kind {
        "claude" => ("npm", vec!["install", "-g", "@anthropic-ai/claude-code"]),
        "codex" => ("npm", vec!["install", "-g", "@openai/codex"]),
        "gemini" => ("npm", vec!["install", "-g", "@google/gemini-cli"]),
        "opencode" => ("npm", vec!["install", "-g", "opencode-ai"]),
        "mistral" => ("uv", vec!["tool", "install", "mistral-vibe"]),
        _ => return None,
    })
}

/// The command as the owner sees it before pressing the button.
pub fn display(kind: &str) -> String {
    plan(kind).map(|(t, a)| format!("{t} {}", a.join(" "))).unwrap_or_default()
}

fn valid_registry(r: &str) -> bool {
    r.starts_with("https://") && r.len() < 200 && r.chars().all(|c| c.is_ascii_alphanumeric() || "-._~:/?=&%".contains(c) && c != '&')
}

fn push(line: &str) {
    let t = line.trim();
    if t.is_empty() {
        return;
    }
    let mut j = JOB.lock();
    j.lines.push(t.chars().take(300).collect());
    let n = j.lines.len();
    if n > 80 {
        j.lines.drain(..n - 80);
    }
}

/// Run one attempt, streaming its output into the job; Ok(true) = exit code 0.
fn attempt(tool: &std::path::Path, args: &[String], env: &[(&str, &str)]) -> Result<bool, String> {
    let mut c = std::process::Command::new(tool);
    c.args(args).envs(env.iter().copied()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).stdin(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    let mut child = c.spawn().map_err(|e| format!("เริ่มคำสั่งไม่ได้: {e}"))?;
    let out = child.stdout.take();
    let err = child.stderr.take();
    let t1 = std::thread::spawn(move || {
        if let Some(o) = out {
            for l in BufReader::new(o).lines().map_while(Result::ok) {
                push(&l);
            }
        }
    });
    let t2 = std::thread::spawn(move || {
        if let Some(o) = err {
            for l in BufReader::new(o).lines().map_while(Result::ok) {
                push(&l);
            }
        }
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(600);
    let code = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s.success(),
            Ok(None) if std::time::Instant::now() > deadline => {
                let _ = child.kill();
                return Err("ใช้เวลาเกิน 10 นาที".into());
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(200)),
            Err(e) => return Err(e.to_string()),
        }
    };
    let _ = t1.join();
    let _ = t2.join();
    Ok(code)
}

fn network_trouble(lines: &[String]) -> bool {
    lines.iter().any(|l| {
        let u = l.to_uppercase();
        ["ETIMEDOUT", "ECONNRESET", "ENOTFOUND", "UNABLE_TO_GET_ISSUER_CERT", "SELF_SIGNED", "CERT_", "EAI_AGAIN", "FETCH FAILED", "ERROR SENDING REQUEST", "CERTIFICATE"].iter().any(|w| u.contains(w))
    })
}

fn run(app: &AppHandle, kind: &str, registry: &str) {
    let finish = |state: &str, message: &str| {
        let mut j = JOB.lock();
        j.state = state.into();
        j.message = message.into();
        drop(j);
        let _ = app.emit("cli-install", kind);
    };
    let Some((tool, base)) = plan(kind) else { return finish("error", "ไม่รู้จักโปรแกรมนี้") };
    let Some(path) = crate::providers::resolve_bin(tool) else {
        let how = if tool == "npm" { "ต้องมี Node.js (npm) ก่อน: winget install OpenJS.NodeJS.LTS แล้วเปิดโปรแกรมนี้ใหม่" } else { "ต้องมี uv ก่อน: winget install --id astral-sh.uv แล้วเปิดโปรแกรมนี้ใหม่" };
        return finish("error", how);
    };
    let args: Vec<String> = base.iter().map(|s| s.to_string()).collect();
    push(&format!("$ {}", display(kind)));
    // 1: as is · 2: Node told to use the Windows certificate store (networks that inspect HTTPS) · 3: the registry the owner named
    let mut tries: Vec<(Vec<String>, Vec<(&str, &str)>, &str)> = vec![(args.clone(), vec![("UV_NATIVE_TLS", "1")], "")];
    if tool == "npm" {
        tries.push((args.clone(), vec![("NODE_OPTIONS", "--use-system-ca")], "ใช้ใบรับรองของ Windows"));
        if !registry.is_empty() {
            let mut a = args.clone();
            a.push(format!("--registry={registry}"));
            tries.push((a, vec![("NODE_OPTIONS", "--use-system-ca")], "ใช้ registry ที่ระบุ"));
        }
    }
    let mut ok = false;
    for (n, (a, env, note)) in tries.iter().enumerate() {
        if n > 0 {
            let seen: Vec<String> = JOB.lock().lines.clone();
            if !network_trouble(&seen) {
                break; // not a network problem: another attempt would fail the same way
            }
            push(&format!("— ลองอีกครั้ง ({note})"));
        }
        match attempt(&path, a, env) {
            Ok(true) => {
                ok = true;
                break;
            }
            Ok(false) => {}
            Err(e) => {
                push(&e);
                break;
            }
        }
    }
    crate::providers::forget_found();
    let runner = if kind == "mistral" { "mistral" } else { kind };
    if ok && crate::providers::installed(runner) {
        finish("ok", "ติดตั้งแล้ว — ตอนนี้ล็อกอินหรือใส่ key แล้วกด “ทดสอบ” ได้");
    } else if ok {
        finish("error", "ติดตั้งแล้วแต่ยังหาโปรแกรมไม่เจอใน PATH — ปิดแล้วเปิดโปรแกรมนี้ใหม่");
    } else {
        finish("error", "ติดตั้งไม่สำเร็จ — ดูข้อความด้านล่าง หรือรันคำสั่งเองใน terminal");
    }
}

#[tauri::command]
pub fn cli_install_start(app: AppHandle, kind: String, registry: Option<String>) -> Result<(), String> {
    if plan(&kind).is_none() {
        return Err("ไม่รู้จักโปรแกรมนี้".into());
    }
    let registry = registry.unwrap_or_default().trim().to_string();
    if !registry.is_empty() && !valid_registry(&registry) {
        return Err("ที่อยู่ registry ต้องขึ้นต้นด้วย https:// และไม่มีอักขระแปลก".into());
    }
    {
        let mut j = JOB.lock();
        if j.state == "running" {
            return Err("กำลังติดตั้งอีกตัวอยู่ รอให้เสร็จก่อน".into());
        }
        *j = Job { kind: kind.clone(), state: "running".into(), ..Default::default() };
    }
    std::thread::spawn(move || run(&app, &kind, &registry));
    Ok(())
}

#[tauri::command]
pub fn cli_install_status() -> Value {
    let j = JOB.lock().clone();
    json!({ "kind": j.kind, "state": j.state, "lines": j.lines, "message": j.message })
}

/// The command shown on the button's confirmation.
#[tauri::command]
pub fn cli_install_command(kind: String) -> String {
    display(&kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_documented_commands_exist() {
        assert_eq!(display("codex"), "npm install -g @openai/codex");
        assert_eq!(display("mistral"), "uv tool install mistral-vibe");
        assert!(plan("custom").is_none());
        for k in crate::accounts::KINDS {
            assert!(plan(k).is_some(), "{k}");
        }
    }

    #[test]
    fn a_registry_must_be_a_plain_https_address() {
        assert!(valid_registry("https://registry.npmmirror.com"));
        assert!(!valid_registry("http://x"));
        assert!(!valid_registry("https://x.com & calc"));
        assert!(!valid_registry("https://x.com\"y"));
    }

    #[test]
    fn network_problems_are_recognised() {
        assert!(network_trouble(&["npm error code ETIMEDOUT".into()]));
        assert!(network_trouble(&["UNABLE_TO_GET_ISSUER_CERT_LOCALLY".into()]));
        assert!(!network_trouble(&["npm error code EACCES".into()]));
    }
}
