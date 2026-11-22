//! Lifeline (ISSUE-144): a short diary of when the app starts, quits and panics (`data/app-life.log`), and a small
//! watcher — a second copy of this program started with `--watchdog` — that opens the app again when it vanishes
//! without quitting properly. Before this, an app that was killed or crashed overnight stayed closed until
//! someone noticed in the morning, and nothing said when or why it had stopped.
//!
//! Every proper way out (tray "quit", the updater, switching copies) passes `RunEvent::Exit`, which writes `exit`;
//! `scripts/deploy.ps1` writes `exit deploy` before it stops the app. Anything else is "lost".
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

const LOG: &str = "app-life.log";
/// the watcher gives a deploy/update script this long to start its own new copy before starting one itself
const GRACE_SECS: u64 = 20;
/// at most this many restarts in RESTART_WINDOW_MIN: a copy that dies right after it starts is not restarted forever
const MAX_RESTARTS: usize = 3;
const RESTART_WINDOW_MIN: i64 = 30;
const TIME_FMT: &str = "%Y-%m-%d %H:%M:%S";

fn log_path(root: &Path) -> PathBuf {
    root.join("data").join(LOG)
}

/// Append one line `<local time> <what>`; the file is trimmed to its last 400 lines once it passes 200 KB.
pub fn note(root: &Path, what: &str) {
    let p = log_path(root);
    if std::fs::metadata(&p).map(|m| m.len() > 200_000).unwrap_or(false) {
        if let Ok(s) = std::fs::read_to_string(&p) {
            let lines: Vec<&str> = s.lines().collect();
            let keep = lines[lines.len().saturating_sub(400)..].join("\n");
            let _ = std::fs::write(&p, keep + "\n");
        }
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&p) {
        let _ = writeln!(f, "{} {}", chrono::Local::now().format(TIME_FMT), what.replace('\n', " "));
    }
}

fn lines(root: &Path) -> Vec<String> {
    std::fs::read_to_string(log_path(root)).unwrap_or_default().lines().map(str::to_string).collect()
}

/// (time, rest) of a diary line
fn split(line: &str) -> Option<(chrono::NaiveDateTime, &str)> {
    let t = chrono::NaiveDateTime::parse_from_str(line.get(..19)?, TIME_FMT).ok()?;
    Some((t, line.get(20..).unwrap_or("")))
}

fn kind(rest: &str) -> &str {
    rest.split_whitespace().next().unwrap_or("")
}

/// When this computer last started (local time); None where it is not known.
fn boot_time() -> Option<chrono::NaiveDateTime> {
    #[cfg(windows)]
    {
        let up = unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() };
        return Some(chrono::Local::now().naive_local() - chrono::Duration::milliseconds(up as i64));
    }
    #[cfg(target_os = "linux")]
    {
        let s = std::fs::read_to_string("/proc/uptime").ok()?;
        let up: f64 = s.split_whitespace().next()?.parse().ok()?;
        return Some(chrono::Local::now().naive_local() - chrono::Duration::seconds(up as i64));
    }
    #[allow(unreachable_code)]
    None
}

/// What the diary says about how the previous copy ended, as a sentence for the history — None when it quit
/// properly, or when the computer was shut down or restarted since (that ends every program without a goodbye).
pub fn previous_end(log: &[String], boot: Option<chrono::NaiveDateTime>) -> Option<String> {
    let last = log.iter().rev().find_map(|l| split(l))?;
    let (t, rest) = last;
    if boot.map(|b| t < b).unwrap_or(false) {
        return None;
    }
    let when = t.format("%d/%m %H:%M");
    match kind(rest) {
        "exit" | "restart" => None,
        "lost" => Some(format!("โปรแกรมปิดไปเองเมื่อ {when} โดยไม่ได้กดออก (ไม่ใช่การปิดเครื่อง) — ตัวเฝ้าโปรแกรมเปิดให้ใหม่แล้ว · ดูรายละเอียดใน data/{LOG}")),
        "panic" => Some(format!("โปรแกรมพังเมื่อ {when}: {} · ดูรายละเอียดใน data/{LOG}", rest.trim_start_matches("panic").trim().chars().take(300).collect::<String>())),
        "start" => Some(format!("โปรแกรมรอบที่เปิดเมื่อ {when} ปิดไปโดยไม่ได้กดออก (ไม่ทราบเวลาที่ปิดแน่ชัด) · ดูรายละเอียดใน data/{LOG}")),
        _ => None,
    }
}

/// Call once at start-up: writes `start`, keeps panics in the diary, and returns how the previous copy ended
/// when that was not a proper quit.
pub fn started(root: &Path) -> Option<String> {
    let before = previous_end(&lines(root), boot_time());
    note(root, &format!("start pid={} v{}", std::process::id(), env!("CARGO_PKG_VERSION")));
    let r = root.to_path_buf();
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let at = info.location().map(|l| format!(" ({}:{})", l.file(), l.line())).unwrap_or_default();
        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_default();
        let thread = std::thread::current().name().unwrap_or("?").to_string();
        note(&r, &format!("panic [{thread}] {msg}{at}"));
        default(info);
    }));
    before
}

/// Restarts in the last RESTART_WINDOW_MIN minutes
fn recent_restarts(log: &[String], now: chrono::NaiveDateTime) -> usize {
    log.iter()
        .filter_map(|l| split(l))
        .filter(|(t, rest)| kind(rest) == "restart" && now - *t < chrono::Duration::minutes(RESTART_WINDOW_MIN))
        .count()
}

/// Is a copy of the app answering on its local port (data/api.json)?
fn app_answers(root: &Path) -> bool {
    let port = std::fs::read_to_string(root.join("data").join("api.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v["port"].as_u64());
    let Some(port) = port else { return false };
    std::net::TcpStream::connect_timeout(&([127, 0, 0, 1], port as u16).into(), Duration::from_secs(2)).is_ok()
}

#[cfg(windows)]
fn wait_for_exit(pid: u32) {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, WaitForSingleObject, INFINITE, PROCESS_SYNCHRONIZE};
    unsafe {
        let h = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if h.is_null() {
            return;
        }
        WaitForSingleObject(h, INFINITE);
        CloseHandle(h);
    }
}

#[cfg(not(windows))]
fn wait_for_exit(pid: u32) {
    while std::process::Command::new("kill").args(["-0", &pid.to_string()]).status().map(|s| s.success()).unwrap_or(false) {
        std::thread::sleep(Duration::from_secs(3));
    }
}

/// Start `exe args` on its own: not tied to this process, its console or a job that closes with it.
fn spawn_free(exe: &Path, args: &[String]) -> std::io::Result<()> {
    let mut c = std::process::Command::new(exe);
    c.args(args).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const NO_WINDOW_NEW_GROUP: u32 = 0x0800_0200;
        const BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
        // leave a job (a terminal or script that kills everything it started when it closes); a job that does not
        // allow that refuses the start, so try once more inside it
        c.creation_flags(NO_WINDOW_NEW_GROUP | BREAKAWAY_FROM_JOB);
        if c.spawn().is_ok() {
            return Ok(());
        }
        c.creation_flags(NO_WINDOW_NEW_GROUP);
    }
    c.spawn().map(|_| ())
}

/// Start the watcher for this copy. Only for the installed program: not in development builds, not on phones.
pub fn spawn_watchdog(root: &Path) {
    if cfg!(debug_assertions) || cfg!(any(target_os = "android", target_os = "ios")) {
        return;
    }
    let Ok(exe) = std::env::current_exe() else { return };
    let args = vec!["--watchdog".to_string(), std::process::id().to_string(), root.display().to_string()];
    if let Err(e) = spawn_free(&exe, &args) {
        note(root, &format!("watchdog-failed {e}"));
    }
}

/// `--watchdog <pid> <root>`: wait for that copy to end; if it did not quit properly, open the app again.
pub fn watch(args: &[String]) {
    let (Some(pid), Some(root)) = (args.first().and_then(|p| p.parse::<u32>().ok()), args.get(1)) else { return };
    let root = PathBuf::from(root);
    wait_for_exit(pid);
    std::thread::sleep(Duration::from_millis(300));
    let log = lines(&root);
    if let Some((_, rest)) = log.iter().rev().find_map(|l| split(l)) {
        // quit properly (or another copy already started and wrote its own line)
        if kind(rest) == "exit" || !rest.contains(&format!("pid={pid} ")) && kind(rest) == "start" {
            return;
        }
    }
    note(&root, &format!("lost pid={pid} ended without quitting"));
    std::thread::sleep(Duration::from_secs(GRACE_SECS));
    let log = lines(&root);
    // a deploy or update script, or the owner, opened a new copy meanwhile
    if app_answers(&root) || log.iter().rev().find_map(|l| split(l)).map(|(_, r)| kind(r) == "start").unwrap_or(false) {
        return;
    }
    if recent_restarts(&log, chrono::Local::now().naive_local()) >= MAX_RESTARTS {
        note(&root, &format!("gave-up restarted {MAX_RESTARTS} times in {RESTART_WINDOW_MIN} min; open the app by hand"));
        return;
    }
    let Ok(exe) = std::env::current_exe() else { return };
    match spawn_free(&exe, &["--minimized".to_string()]) {
        Ok(()) => note(&root, "restart by watchdog"),
        Err(e) => note(&root, &format!("restart-failed {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> chrono::NaiveDateTime {
        chrono::NaiveDateTime::parse_from_str(s, TIME_FMT).unwrap()
    }

    #[test]
    fn how_the_last_copy_ended() {
        let l = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        // quit properly / deploy / nothing yet
        assert_eq!(previous_end(&l(&["2026-11-22 00:47:00 start pid=1 v1", "2026-11-22 01:00:00 exit"]), None), None);
        assert_eq!(previous_end(&l(&["2026-11-22 00:47:00 start pid=1 v1", "2026-11-22 01:00:00 exit deploy"]), None), None);
        assert_eq!(previous_end(&[], None), None);
        // lost, with the watcher's time
        let lost = previous_end(&l(&["2026-11-22 00:47:00 start pid=1 v1", "2026-11-22 00:57:10 lost pid=1 ended without quitting"]), None).unwrap();
        assert!(lost.contains("22/11 00:57") && lost.contains("ปิดไปเอง"), "{lost}");
        // a panic keeps its message
        let p = previous_end(&l(&["2026-11-22 00:47:00 start pid=1 v1", "2026-11-22 00:50:00 panic [main] boom (src/x.rs:1)"]), None).unwrap();
        assert!(p.contains("boom"), "{p}");
        // no goodbye and no watcher line
        assert!(previous_end(&l(&["2026-11-22 00:47:00 start pid=1 v1"]), None).unwrap().contains("ไม่ทราบเวลา"));
        // the computer restarted since: not the app's fault
        assert_eq!(previous_end(&l(&["2026-11-22 00:47:00 start pid=1 v1", "2026-11-22 00:57:10 lost pid=1"]), Some(t("2026-11-22 06:00:00"))), None);
        // the watcher already opened it again: its line is last
        assert_eq!(previous_end(&l(&["2026-11-22 00:57:10 lost pid=1", "2026-11-22 00:57:30 restart by watchdog"]), None), None);
    }

    #[test]
    fn stops_restarting_a_copy_that_keeps_dying() {
        let now = t("2026-11-22 01:00:00");
        let log: Vec<String> = ["00:20:00", "00:40:00", "00:50:00", "00:55:00"].iter().map(|h| format!("2026-11-22 {h} restart by watchdog")).collect();
        assert_eq!(recent_restarts(&log, now), 3, "the 00:20 one is older than 30 minutes");
        assert_eq!(recent_restarts(&log[..1], now), 0);
    }

    #[test]
    fn diary_lines_are_written_and_read_back() {
        let root = std::env::temp_dir().join(format!("ia-lifeline-{}", std::process::id()));
        std::fs::create_dir_all(root.join("data")).unwrap();
        note(&root, "start pid=7 v0");
        note(&root, "exit");
        let l = lines(&root);
        assert_eq!(l.len(), 2);
        assert_eq!(split(&l[1]).map(|(_, r)| kind(r).to_string()).as_deref(), Some("exit"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
