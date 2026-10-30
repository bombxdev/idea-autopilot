//! Detects whether the user is using Claude right now (only meaningful when Claude is the agent
//! doing the bot's work: they share one plan quota. Other agents have no such overlap): any Claude Code session log under
//! `~/.claude/projects/` written in the last few minutes that doesn't belong to the bot's
//! workspace counts as user activity.
use std::path::Path;
use std::time::{Duration, SystemTime};

const ACTIVE_WINDOW: Duration = Duration::from_secs(10 * 60);

/// Claude Code's project-folder naming: every non-alphanumeric char becomes '-'.
pub fn encode_path(p: &Path) -> String {
    p.to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

pub fn user_active(bot_workspace: &Path) -> bool {
    let Some(home) = dirs::home_dir() else { return false };
    let bot_prefix = encode_path(bot_workspace).to_lowercase();
    let Ok(dirs) = std::fs::read_dir(home.join(".claude").join("projects")) else { return false };
    let cutoff = SystemTime::now() - ACTIVE_WINDOW;
    for d in dirs.flatten() {
        let name = d.file_name().to_string_lossy().to_lowercase();
        if name.starts_with(&bot_prefix) {
            continue;
        }
        let Ok(files) = std::fs::read_dir(d.path()) else { continue };
        for f in files.flatten() {
            if f.path().extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            if let Ok(m) = f.metadata().and_then(|m| m.modified()) {
                if m > cutoff {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_like_claude_code() {
        let p = Path::new(r"D:\Apps\IdeaAutopilot\workspace");
        assert_eq!(encode_path(p), "D--Apps-IdeaAutopilot-workspace");
    }
}
