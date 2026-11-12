//! Updating the program itself from the project's GitHub Releases.
//!
//! - The check reads the latest release (tag, notes, the `-Setup.exe` asset and its SHA-256 as GitHub reports it).
//! - "Update now" works for a copy installed with the installer: the asset is downloaded from github.com only,
//!   its SHA-256 must match what GitHub reports (no digest, no automatic install), then the installer runs
//!   silently once this program has quit, and the program starts again. Ideas and data live outside the install
//!   folder, so nothing is lost.
//! - A portable copy updates itself too (ISSUE-077): the `-portable.zip` asset is downloaded and checked the same way,
//!   then a hidden script waits for this program to quit, backs up the program files in `bin\`, puts the new ones in
//!   their place (restoring the backup if that fails) and starts the program again. Its `data\` is never touched.
//! - A Scoop copy is updated with `scoop update idea-autopilot`, a development copy by its own build (deploy.ps1).
//! - Automatic updates (setting `auto_update`, off by default) check every 6 hours and apply only when the bot is
//!   idle and no chat session is open.
use serde_json::{json, Value};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

use crate::db;
use crate::state::AppState;

const REPO: &str = "bombxdev/idea-autopilot";
const CHECK_EVERY: i64 = 6 * 3600;
const MAX_BYTES: usize = 250 * 1024 * 1024;

#[derive(Debug, Clone, Default)]
struct Release {
    tag: String,
    notes: String,
    page: String,
    asset_url: String,
    asset_name: String,
    digest: String,
    /// the portable zip of the same release (for a portable copy)
    zip_url: String,
    zip_name: String,
    zip_digest: String,
}

impl Release {
    /// (url, name, sha-256) of the file this kind of copy updates from
    fn asset_for(&self, mode: &str) -> (&str, &str, &str) {
        if mode == "portable" {
            (&self.zip_url, &self.zip_name, &self.zip_digest)
        } else {
            (&self.asset_url, &self.asset_name, &self.digest)
        }
    }
}

/// Copies that update themselves from the release
fn self_updating(mode: &str) -> bool {
    mode == "installer" || mode == "portable"
}

pub fn current() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// installer | scoop | portable | dev
fn install_mode() -> &'static str {
    let Ok(exe) = std::env::current_exe() else { return "portable" };
    let p = exe.display().to_string().to_lowercase().replace('/', "\\");
    if p.contains("\\scoop\\") {
        "scoop"
    } else if p.contains("\\target\\") || p.contains("claude-control-usage") {
        "dev"
    } else if dirs::data_local_dir().map(|d| p.starts_with(&d.join("programs").display().to_string().to_lowercase())).unwrap_or(false) {
        "installer"
    } else {
        "portable"
    }
}

fn client(follow: bool) -> Result<reqwest::Client, String> {
    let policy = if follow {
        // only ever follow to GitHub's own hosts
        reqwest::redirect::Policy::custom(|a| {
            let ok = a.url().host_str().is_some_and(|h| h == "github.com" || h.ends_with(".githubusercontent.com") || h.ends_with(".github.com"));
            if ok && a.previous().len() < 5 { a.follow() } else { a.stop() }
        })
    } else {
        reqwest::redirect::Policy::none()
    };
    reqwest::Client::builder().user_agent("idea-autopilot-updater").timeout(std::time::Duration::from_secs(if follow { 600 } else { 20 })).redirect(policy).build().map_err(|e| e.to_string())
}

async fn latest_release() -> Result<Release, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let resp = client(false)?.get(&url).header("Accept", "application/vnd.github+json").send().await.map_err(|e| format!("ต่อ GitHub ไม่ได้: {e}"))?;
    if resp.status().as_u16() == 404 {
        return Err("ยังไม่พบรุ่นที่เผยแพร่บน GitHub Releases".into());
    }
    if !resp.status().is_success() {
        return Err(format!("GitHub ตอบ {}", resp.status()));
    }
    let v: Value = resp.json().await.map_err(|e| e.to_string())?;
    parse_release(&v)
}

fn parse_release(v: &Value) -> Result<Release, String> {
    let tag = v["tag_name"].as_str().ok_or("อ่านรุ่นล่าสุดไม่ได้")?.trim_start_matches('v').to_string();
    let setup = v["assets"].as_array().and_then(|a| a.iter().find(|x| x["name"].as_str().is_some_and(|n| n.ends_with("-Setup.exe"))));
    let zip = v["assets"].as_array().and_then(|a| a.iter().find(|x| x["name"].as_str().is_some_and(|n| n.ends_with("-portable.zip"))));
    Ok(Release {
        zip_url: zip.and_then(|a| a["browser_download_url"].as_str()).unwrap_or("").to_string(),
        zip_name: zip.and_then(|a| a["name"].as_str()).unwrap_or("").to_string(),
        zip_digest: zip.and_then(|a| a["digest"].as_str()).unwrap_or("").trim_start_matches("sha256:").to_lowercase(),
        tag,
        notes: v["body"].as_str().unwrap_or("").chars().take(1500).collect(),
        page: v["html_url"].as_str().unwrap_or("").to_string(),
        asset_url: setup.and_then(|a| a["browser_download_url"].as_str()).unwrap_or("").to_string(),
        asset_name: setup.and_then(|a| a["name"].as_str()).unwrap_or("").to_string(),
        digest: setup.and_then(|a| a["digest"].as_str()).unwrap_or("").trim_start_matches("sha256:").to_lowercase(),
    })
}

pub async fn status(app: &AppHandle, refresh: bool) -> Value {
    let st = app.state::<AppState>();
    let (cached, at) = {
        let c = st.db.lock();
        (db::get_setting(&c, "update_info"), db::get_setting(&c, "update_checked").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0))
    };
    let mut rel: Option<Release> = cached.and_then(|s| serde_json::from_str::<Value>(&s).ok()).and_then(|v| parse_release(&v).ok());
    let mut error = String::new();
    let mut checked = at;
    if refresh || db::now() - at > CHECK_EVERY {
        match latest_release().await {
            Ok(r) => {
                let raw = json!({ "tag_name": r.tag, "body": r.notes, "html_url": r.page, "assets": [
                    { "name": r.asset_name, "browser_download_url": r.asset_url, "digest": r.digest },
                    { "name": r.zip_name, "browser_download_url": r.zip_url, "digest": r.zip_digest }
                ] });
                let c = st.db.lock();
                let _ = db::set_setting(&c, "update_info", &raw.to_string());
                checked = db::now();
                let _ = db::set_setting(&c, "update_checked", &checked.to_string());
                rel = Some(r);
            }
            Err(e) => error = e,
        }
    }
    let r = rel.unwrap_or_default();
    let available = !r.tag.is_empty() && crate::skillspector::is_newer(&r.tag, current());
    let mode = install_mode();
    let auto = db::get_setting(&st.db.lock(), "auto_update").as_deref() == Some("1");
    let (url, _, digest) = r.asset_for(mode);
    json!({
        "current": current(), "latest": r.tag, "available": available, "notes": r.notes, "page": r.page,
        "mode": mode, "can_apply": available && self_updating(mode) && !url.is_empty() && !digest.is_empty(),
        "no_digest": available && self_updating(mode) && !url.is_empty() && digest.is_empty(),
        "auto": auto, "checked": checked, "error": error,
    })
}

fn sha256_hex(b: &[u8]) -> String {
    crate::skills::sha256_hex(b)
}

/// Download, verify, then hand over to a hidden script that waits for this program to quit, runs the installer
/// silently and starts the program again.
pub async fn apply(app: &AppHandle) -> Result<(), String> {
    let mode = install_mode();
    if !self_updating(mode) {
        return Err("อัปเดตในโปรแกรมใช้ได้กับเครื่องที่ติดตั้งด้วยตัวติดตั้งหรือแบบพกพา — แบบ Scoop ใช้ scoop update idea-autopilot".into());
    }
    let r = latest_release().await?;
    if !crate::skillspector::is_newer(&r.tag, current()) {
        return Err("เป็นรุ่นล่าสุดอยู่แล้ว".into());
    }
    let (url, name, digest) = r.asset_for(mode);
    let (url, name, digest) = (url.to_string(), name.to_string(), digest.to_string());
    let prefix = format!("https://github.com/{REPO}/releases/download/");
    if !url.starts_with(&prefix) || !(name.ends_with("-Setup.exe") || name.ends_with("-portable.zip")) {
        return Err(if mode == "portable" { "ไม่พบไฟล์แบบพกพา (.zip) ในรุ่นนี้" } else { "ไม่พบไฟล์ติดตั้งในรุ่นนี้" }.into());
    }
    if digest.len() != 64 {
        return Err("GitHub ไม่ได้ให้ค่า SHA-256 ของไฟล์นี้ จึงตรวจความถูกต้องไม่ได้ — โหลดจากหน้า Releases เอง".into());
    }
    let resp = client(true)?.get(&url).send().await.map_err(|e| format!("โหลดไม่ได้: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("โหลดไม่ได้ ({})", resp.status()));
    }
    if resp.content_length().is_some_and(|n| n as usize > MAX_BYTES) {
        return Err("ไฟล์ใหญ่ผิดปกติ".into());
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    if bytes.len() > MAX_BYTES || bytes.len() < 1024 * 1024 {
        return Err("ขนาดไฟล์ผิดปกติ".into());
    }
    if sha256_hex(&bytes) != digest {
        return Err("ไฟล์ที่โหลดไม่ตรงกับค่า SHA-256 ที่ GitHub ให้ไว้ — ไม่ติดตั้ง".into());
    }
    let st = app.state::<AppState>();
    let dir: PathBuf = st.root.join("data").join("updates");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = dir.join(&name);
    std::fs::write(&file, &bytes).map_err(|e| e.to_string())?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let script = dir.join("apply-update.ps1");
    let text = if mode == "portable" {
        portable_script(std::process::id(), &file, &dir, &exe, &r.tag)
    } else {
        let q = |p: &std::path::Path| p.display().to_string().replace('\'', "''");
        format!(
            "Wait-Process -Id {pid} -ErrorAction SilentlyContinue\nStart-Sleep -Seconds 2\nStart-Process -FilePath '{setup}' -ArgumentList '/S' -Wait\nStart-Process -FilePath '{exe}' -ArgumentList '--minimized'\n",
            pid = std::process::id(),
            setup = q(&file),
            exe = q(&exe)
        )
    };
    std::fs::write(&script, text).map_err(|e| e.to_string())?;
    let mut c = std::process::Command::new("powershell");
    c.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden", "-File"]).arg(&script);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0008); // no window, detached from this program
    }
    c.spawn().map_err(|e| format!("เริ่มตัวอัปเดตไม่ได้: {e}"))?;
    st.push_log(format!("[update] กำลังอัปเดตเป็น v{} — โปรแกรมจะปิดแล้วเปิดใหม่เอง", r.tag));
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
        crate::remote::stop_all(&app2);
        app2.exit(0);
    });
    Ok(())
}

/// The hidden script that swaps a portable copy's program files: unpack, check the new program is there, back up the
/// files it replaces, copy the new ones over `bin\` (and the read-me files next to it), put the backup back if
/// anything fails, then start the program. `data\` is never written.
fn portable_script(pid: u32, zip: &std::path::Path, dir: &std::path::Path, exe: &std::path::Path, tag: &str) -> String {
    let q = |p: &std::path::Path| p.display().to_string().replace('\'', "''");
    let bin = exe.parent().unwrap_or(std::path::Path::new("."));
    let home = bin.parent().unwrap_or(bin);
    format!(
        r#"$ErrorActionPreference = 'Stop'
Start-Transcript -Path '{log}' -Force | Out-Null
Wait-Process -Id {pid} -ErrorAction SilentlyContinue
Start-Sleep -Seconds 2
$x = '{unpack}'
Remove-Item $x -Recurse -Force -ErrorAction SilentlyContinue
Expand-Archive -LiteralPath '{zip}' -DestinationPath $x -Force
$new = Get-ChildItem $x -Recurse -Filter 'idea-autopilot.exe' | Select-Object -First 1
if (-not $new) {{ Write-Output 'no program in the zip'; Start-Process -FilePath '{exe}' -ArgumentList '--minimized'; exit 1 }}
$bak = '{backup}'
New-Item -ItemType Directory -Force $bak | Out-Null
$bin = '{bin}'
Get-ChildItem $new.DirectoryName -File | ForEach-Object {{ $t = Join-Path $bin $_.Name; if (Test-Path $t) {{ Copy-Item $t $bak -Force }} }}
try {{
  Copy-Item (Join-Path $new.DirectoryName '*') $bin -Force
  Get-ChildItem (Split-Path $new.DirectoryName -Parent) -File | Where-Object {{ $_.Extension -in '.md', '.txt' }} | Copy-Item -Destination '{home}' -Force
  Write-Output 'updated to v{tag}'
}} catch {{
  Write-Output "update failed: $_ - putting the old files back"
  Copy-Item (Join-Path $bak '*') $bin -Force
}}
Remove-Item $x -Recurse -Force -ErrorAction SilentlyContinue
Start-Process -FilePath '{exe}' -ArgumentList '--minimized'
"#,
        log = q(&dir.join("apply-update.log")),
        unpack = q(&dir.join("portable-new")),
        zip = q(zip),
        exe = q(exe),
        backup = q(&dir.join(format!("backup-before-v{tag}"))),
        bin = q(bin),
        home = q(home),
    )
}

/// Background: look for a new version every few hours, tell the owner once per version, and install it when allowed.
pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(90)).await;
        loop {
            let s = status(&app, false).await;
            let st = app.state::<AppState>();
            if s["available"].as_bool() == Some(true) {
                let latest = s["latest"].as_str().unwrap_or("").to_string();
                let told = db::get_setting(&st.db.lock(), "update_notified").unwrap_or_default();
                if told != latest {
                    let _ = db::set_setting(&st.db.lock(), "update_notified", &latest);
                    let _ = app.emit("update-available", &latest);
                    crate::scheduler::notify(&app, "Idea Autopilot", &format!("มีเวอร์ชันใหม่ v{latest} — ดูที่ ตั้งค่า → เกี่ยวกับ"));
                }
                let idle = {
                    let rt = st.rt.lock();
                    rt.current.is_none() && rt.sessions.is_empty()
                };
                if s["auto"].as_bool() == Some(true) && s["can_apply"].as_bool() == Some(true) && idle {
                    if let Err(e) = apply(&app).await {
                        st.push_log(format!("[update] {e}"));
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(CHECK_EVERY as u64)).await;
        }
    });
}

#[tauri::command]
pub async fn update_status(app: AppHandle, refresh: Option<bool>) -> Value {
    status(&app, refresh.unwrap_or(false)).await
}

#[tauri::command]
pub async fn update_apply(app: AppHandle) -> Result<(), String> {
    apply(&app).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_release() {
        let v = json!({
            "tag_name": "v0.4.0", "body": "notes", "html_url": "https://github.com/bombxdev/idea-autopilot/releases/tag/v0.4.0",
            "assets": [
                { "name": "IdeaAutopilot-0.4.0-portable.zip", "browser_download_url": "x" },
                { "name": "IdeaAutopilot-0.4.0-Setup.exe", "browser_download_url": "https://github.com/bombxdev/idea-autopilot/releases/download/v0.4.0/IdeaAutopilot-0.4.0-Setup.exe", "digest": "sha256:ABC123" }
            ]
        });
        let r = parse_release(&v).unwrap();
        assert_eq!((r.tag.as_str(), r.asset_name.as_str(), r.digest.as_str()), ("0.4.0", "IdeaAutopilot-0.4.0-Setup.exe", "abc123"));
        assert!(crate::skillspector::is_newer(&r.tag, "0.3.0") && !crate::skillspector::is_newer(&r.tag, "0.4.0"));
        assert!(parse_release(&json!({})).is_err());
        // a portable copy updates from the portable zip of the same release (ISSUE-077)
        let v = json!({ "tag_name": "v0.5.2", "assets": [
            { "name": "IdeaAutopilot-0.5.2-portable.zip", "browser_download_url": "https://github.com/bombxdev/idea-autopilot/releases/download/v0.5.2/IdeaAutopilot-0.5.2-portable.zip", "digest": "sha256:DEF" },
            { "name": "IdeaAutopilot-0.5.2-Setup.exe", "browser_download_url": "u", "digest": "sha256:ABC" } ] });
        let r = parse_release(&v).unwrap();
        assert_eq!(r.asset_for("portable").1, "IdeaAutopilot-0.5.2-portable.zip");
        assert_eq!(r.asset_for("portable").2, "def");
        assert_eq!(r.asset_for("installer").1, "IdeaAutopilot-0.5.2-Setup.exe");
        assert!(self_updating("portable") && self_updating("installer") && !self_updating("scoop") && !self_updating("dev"));
        let s = portable_script(1, std::path::Path::new("C:/u/a.zip"), std::path::Path::new("C:/P/data/updates"), std::path::Path::new("C:/P/bin/idea-autopilot.exe"), "0.5.2");
        assert!(s.contains("Expand-Archive") && s.contains("backup-before-v0.5.2") && s.contains("C:/P/bin") && !s.contains("data\\.keep"));
    }
}
