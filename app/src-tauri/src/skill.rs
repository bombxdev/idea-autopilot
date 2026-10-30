//! Installs/refreshes the Claude Code skill that lets any Claude Code session control the app
//! through the `ia` CLI, with this machine's paths filled in.
use crate::state::AppState;

const TEMPLATE: &str = include_str!("skill_template.md");

pub fn install(st: &AppState) {
    let Some(home) = dirs::home_dir() else { return };
    // only for people who use Claude Code: never create ~/.claude on a machine without it
    if !home.join(".claude").is_dir() {
        return;
    }
    let Some(bin) = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.to_path_buf())) else { return };
    let ia = bin.join("ia.exe");
    if !ia.exists() {
        return; // dev build without the CLI next to it
    }
    let app = std::env::current_exe().unwrap_or_default();
    let body = TEMPLATE
        .replace("{IA_SLASH}", &ia.display().to_string().replace('\\', "/"))
        .replace("{IA}", &ia.display().to_string())
        .replace("{APP}", &app.display().to_string());
    let dir = home.join(".claude").join("skills").join("idea-autopilot");
    let path = dir.join("SKILL.md");
    let current = std::fs::read_to_string(&path).unwrap_or_default();
    // only touch our own skill file
    if !current.is_empty() && !current.contains("name: idea-autopilot") {
        return;
    }
    if current != body {
        let _ = std::fs::create_dir_all(&dir);
        if std::fs::write(&path, body).is_ok() {
            st.push_log("[skill] อัปเดต Claude Code skill idea-autopilot แล้ว".into());
        }
    }
}
