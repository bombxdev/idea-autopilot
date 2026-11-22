//! Screenshots of web pages for the bots, taken by the app itself (ISSUE-147).
//!
//! A bot that started Edge headless from inside its AI's sandbox (Codex and others) was usually refused
//! ("Access denied"); Edge then crashed with a breakpoint dialog or hung, and the run stalled. Here the app — which is
//! not sandboxed — starts the browser: one at a time, with a fresh profile, no crash reporter or dialogs, a time limit
//! that ends the whole process tree, and one retry. Bots ask for it with `ia shot <url> <file.png>`.
//!
//! Only pages a bot builds are allowed: http(s) on this computer (localhost / 127.0.0.1 / ::1) or a file inside
//! the workspace, and the picture must be written inside the workspace.
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// one browser at a time: several bots asking at once would start several Edges
static BUSY: parking_lot::Mutex<()> = parking_lot::Mutex::new(());
const LIMIT_SECS: u64 = 45;

/// Setting `web_check`: how bots look at web pages. "app" (default): `ia shot`, the app takes the picture.
/// "self": the bot starts Edge headless itself (the old way). "off": bots do not try to look at pages.
pub static MODE: parking_lot::Mutex<String> = parking_lot::Mutex::new(String::new());

pub fn set_mode(v: &str) {
    *MODE.lock() = v.to_string();
}

pub fn mode() -> String {
    let m = MODE.lock().clone();
    if m == "self" || m == "off" { m } else { "app".into() }
}

/// Edge first (always on Windows), then Chrome / Chromium.
pub fn browser() -> Option<PathBuf> {
    let mut c: Vec<PathBuf> = vec![];
    if cfg!(windows) {
        for base in [std::env::var_os("ProgramFiles(x86)"), std::env::var_os("ProgramFiles"), std::env::var_os("LOCALAPPDATA")].into_iter().flatten() {
            let b = PathBuf::from(base);
            c.push(b.join("Microsoft/Edge/Application/msedge.exe"));
            c.push(b.join("Google/Chrome/Application/chrome.exe"));
        }
    } else if cfg!(target_os = "macos") {
        c.push("/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge".into());
        c.push("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".into());
        c.push("/Applications/Chromium.app/Contents/MacOS/Chromium".into());
    } else {
        for n in ["microsoft-edge", "google-chrome", "chromium", "chromium-browser"] {
            c.push(PathBuf::from("/usr/bin").join(n));
        }
    }
    c.into_iter().find(|p| p.is_file())
}

fn inside(path: &Path, dir: &Path) -> bool {
    let norm = |p: &Path| dunce_lower(p);
    norm(path).starts_with(&norm(dir))
}

/// lower-case, forward slashes, no `\\?\` prefix — enough to compare two Windows paths
fn dunce_lower(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    let s = s.trim_start_matches("//?/");
    if cfg!(windows) { s.to_lowercase() } else { s.to_string() }
}

/// The page may be a local server or a file in the workspace — nothing on the internet.
pub fn url_ok(url: &str, ws: &Path) -> Result<(), String> {
    let lower = url.to_lowercase();
    if let Some(rest) = lower.strip_prefix("http://").or_else(|| lower.strip_prefix("https://")) {
        let host = rest.split(['/', '?', '#']).next().unwrap_or("");
        let host = if host.starts_with('[') { host.split(']').next().unwrap_or("").trim_start_matches('[') } else { host.split(':').next().unwrap_or("") };
        return if matches!(host, "localhost" | "127.0.0.1" | "::1") { Ok(()) } else { Err(format!("ถ่ายได้เฉพาะหน้าเว็บในเครื่องนี้ (localhost / 127.0.0.1) หรือไฟล์ในโฟลเดอร์งาน — ไม่ใช่ {host}")) };
    }
    if let Some(rest) = url.strip_prefix("file:///").or_else(|| url.strip_prefix("file://")) {
        let p = PathBuf::from(percent_decode(rest.split(['?', '#']).next().unwrap_or("")));
        if p.components().any(|c| matches!(c, std::path::Component::ParentDir)) || !inside(&p, ws) {
            return Err("ไฟล์ต้องอยู่ในโฟลเดอร์งาน (workspace)".into());
        }
        return Ok(());
    }
    Err("ใช้ได้เฉพาะ http(s)://localhost… หรือ file:///… ในโฟลเดอร์งาน".into())
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Arguments for one try.
fn args(profile: &Path, url: &str, out: &Path, w: u32, h: u32, wait_ms: u32) -> Vec<String> {
    let mut a: Vec<String> = [
        "--headless=new",
        "--disable-gpu",
        "--no-first-run",
        "--no-default-browser-check",
        "--disable-extensions",
        "--disable-breakpad",
        "--disable-crash-reporter",
        "--noerrdialogs",
        "--hide-scrollbars",
        "--mute-audio",
        "--disable-background-networking",
        "--disable-sync",
        "--disable-features=msEdgeSidebarV2,EdgeCollections,Translate",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    a.push(format!("--user-data-dir={}", profile.display()));
    a.push(format!("--window-size={w},{h}"));
    if wait_ms > 0 {
        a.push(format!("--virtual-time-budget={wait_ms}"));
    }
    a.push(format!("--screenshot={}", out.display()));
    a.push(url.to_string());
    a
}

/// Take the picture. `ws` = the workspace folder.
pub fn shot(ws: &Path, a: &Value) -> Result<Value, String> {
    if mode() == "off" {
        return Err("เจ้าของปิดการดูหน้าเว็บของบอทไว้ (ตั้งค่า → ระบบ) — ข้ามการตรวจหน้าจอ แล้วบอกใน REPORT.md ว่ายังไม่ได้ดู".into());
    }
    let url = a.get("url").and_then(Value::as_str).ok_or("ต้องระบุ url")?.trim().to_string();
    let out = PathBuf::from(a.get("out").and_then(Value::as_str).ok_or("ต้องระบุไฟล์ภาพปลายทาง")?);
    let w = a.get("width").and_then(Value::as_u64).unwrap_or(1280).clamp(200, 4000) as u32;
    let h = a.get("height").and_then(Value::as_u64).unwrap_or(800).clamp(200, 8000) as u32;
    let wait_ms = a.get("wait_ms").and_then(Value::as_u64).unwrap_or(2000).min(20_000) as u32;
    url_ok(&url, ws)?;
    if !out.is_absolute() || out.components().any(|c| matches!(c, std::path::Component::ParentDir)) || !inside(&out, ws) {
        return Err("ไฟล์ภาพต้องอยู่ในโฟลเดอร์งาน (workspace) และเป็น path เต็ม".into());
    }
    if !out.extension().map(|e| e.eq_ignore_ascii_case("png")).unwrap_or(false) {
        return Err("ไฟล์ภาพต้องลงท้าย .png".into());
    }
    if let Some(d) = out.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("สร้างโฟลเดอร์ไม่ได้: {e}"))?;
    }
    let exe = browser().ok_or("ไม่พบ Microsoft Edge หรือ Chrome ในเครื่อง")?;
    let _one = BUSY.lock();
    let mut last = String::new();
    for attempt in 1..=2 {
        let profile = std::env::temp_dir().join(format!("ia-webshot-{}-{attempt}-{}", std::process::id(), crate::db::now()));
        let _ = std::fs::remove_file(&out);
        let mut c = std::process::Command::new(&exe);
        c.args(args(&profile, &url, &out, w, h, wait_ms)).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let started = std::time::Instant::now();
        let got = match c.spawn() {
            Ok(mut child) => {
                // msedge.exe returns at once and a child process does the work: the picture appearing (and
                // no longer growing) is the real "done"
                let deadline = started + std::time::Duration::from_secs(LIMIT_SECS);
                let mut prev = 0u64;
                let mut ok = false;
                while std::time::Instant::now() < deadline {
                    let n = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
                    if n > 0 && n == prev {
                        ok = true;
                        break;
                    }
                    prev = n;
                    std::thread::sleep(std::time::Duration::from_millis(400));
                }
                let _ = child.try_wait();
                ok
            }
            Err(e) => {
                last = format!("เปิดเบราว์เซอร์ไม่ได้: {e}");
                false
            }
        };
        // nothing of this browser may stay behind (a hung one used to keep the bot waiting)
        end_browser(&profile);
        remove_dir_retry(&profile);
        if got {
            return Ok(json!({ "file": out.display().to_string(), "width": w, "height": h, "seconds": started.elapsed().as_secs_f32(), "attempt": attempt, "browser": exe.display().to_string() }));
        }
        if last.is_empty() || attempt == 2 {
            last = format!("ไม่ได้ภาพภายใน {LIMIT_SECS} วินาที");
        }
    }
    Err(format!("ถ่ายหน้าเว็บไม่สำเร็จ 2 ครั้ง ({last}) — ตรวจว่าเซิร์ฟเวอร์ของหน้าเว็บยังเปิดอยู่ ถ้ายังไม่ได้ ให้บอกใน REPORT.md ว่ายังไม่ได้ดูหน้าจอ แล้วทำอย่างอื่นต่อ"))
}

/// End every browser process started with this profile folder (the launcher's children are not its tree).
fn end_browser(profile: &Path) {
    let key = profile.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if key.is_empty() {
        return;
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let ps = format!(
            "Get-CimInstance Win32_Process -Filter \"name='msedge.exe' or name='chrome.exe'\" | Where-Object {{ $_.CommandLine -like '*{key}*' }} | ForEach-Object {{ Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }}"
        );
        let mut c = std::process::Command::new("powershell");
        c.args(["-NoProfile", "-NonInteractive", "-Command", &ps]).creation_flags(0x0800_0000);
        let _ = crate::runner::output_within(&mut c, 20);
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("pkill").args(["-f", &key]).status();
    }
}

fn remove_dir_retry(d: &Path) {
    for _ in 0..10 {
        if std::fs::remove_dir_all(d).is_ok() || !d.exists() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_local_pages_and_workspace_files() {
        let ws = Path::new("D:/x/workspace");
        assert!(url_ok("http://localhost:5173/", ws).is_ok());
        assert!(url_ok("http://127.0.0.1:8000/a?b=1", ws).is_ok());
        assert!(url_ok("https://[::1]:3000", ws).is_ok());
        assert!(url_ok("https://example.com", ws).is_err());
        assert!(url_ok("http://localhost.evil.com/", ws).is_err());
        assert!(url_ok("file:///D:/x/workspace/ideas/idea-0001/index.html", ws).is_ok());
        assert!(url_ok("file:///D:/x/workspace/ideas/../../secret.html", ws).is_err());
        assert!(url_ok("file:///C:/Windows/win.ini", ws).is_err());
        assert!(url_ok("javascript:alert(1)", ws).is_err());
    }

    #[test]
    fn picture_must_land_in_the_workspace() {
        let ws = Path::new("D:/x/workspace");
        let bad = shot(ws, &json!({ "url": "http://localhost:1/", "out": "C:/Windows/a.png" }));
        assert!(bad.unwrap_err().contains("โฟลเดอร์งาน"));
        let rel = shot(ws, &json!({ "url": "http://localhost:1/", "out": "a.png" }));
        assert!(rel.is_err());
    }

    /// Takes a real picture of a file page with the installed Edge: `cargo test --lib webshot -- --ignored`
    #[test]
    #[ignore]
    fn takes_a_real_picture() {
        let ws = std::env::temp_dir().join(format!("ia-ws-{}", std::process::id()));
        std::fs::create_dir_all(&ws).unwrap();
        let page = ws.join("p.html");
        std::fs::write(&page, "<html><body style='background:#1e90ff'><h1>Idea Autopilot</h1></body></html>").unwrap();
        let out = ws.join("shots").join("p.png");
        let url = format!("file:///{}", page.display().to_string().replace('\\', "/"));
        let r = shot(&ws, &json!({ "url": url, "out": out.display().to_string(), "width": 390, "height": 844 })).unwrap();
        assert!(std::fs::metadata(&out).unwrap().len() > 1000, "{r}");
        // nothing of the browser is left running and the profile folder is gone
        let left = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", "@(Get-CimInstance Win32_Process -Filter \"name='msedge.exe'\" | Where-Object { $_.CommandLine -like '*ia-webshot-*' }).Count"])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&left.stdout).trim(), "0", "{r}");
        let _ = std::fs::remove_dir_all(&ws);
    }
}
