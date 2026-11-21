//! Team mode: one idea worked on by a lead and several helpers at once (multi-agent).
//!
//! The lead is the run's own model. Its helpers are Claude Code subagents defined with `--agents`: `helper` (Sonnet,
//! ordinary parts), `helper-fast` (Haiku, simple or mechanical parts — searching, collecting, formatting, cheapest),
//! and `reviewer` (the lead's own model, checks the merged work). The lead calls several in one message, so they run
//! in parallel; helpers get no `Task` tool, so they cannot hire helpers of their own (depth 1 — no runaway trees).
//!
//! Limits kept on purpose:
//! - off by default, per idea; the lead decides whether a stage is worth splitting (small or tightly coupled work
//!   stays single — splitting costs more than it saves there);
//! - quota: the team runs inside one agent process, so the governor's mid-run stop (should_kill) stops the whole team
//!   at the ceiling, and the owner's work hours stay protected as for any run;
//! - only Claude accounts on Anthropic (the helper models are Anthropic aliases). Other agents run as before.
use std::path::{Path, PathBuf};

/// Set in RunSpec.env to the `--agents` file; the runner also adds the Task tool (taken out before the agent starts).
pub const TEAM_ENV: &str = "IA_TEAM_AGENTS";
pub const MAX_HELPERS: u32 = 5;

const AGENTS: &str = r#"{
  "helper": {
    "description": "A helper on the team: does ONE independent part of the work given by the lead (code, research, writing) and reports back briefly.",
    "prompt": "You are a helper on a small team. Do only the part the lead gave you, only in the files/folder named in the brief — never edit files outside it. Work fully and carefully, then report in under 10 lines: what you made, where, what is left or uncertain.",
    "model": "sonnet",
    "tools": ["Bash", "Read", "Write", "Edit", "Glob", "Grep", "WebSearch", "WebFetch"]
  },
  "helper-fast": {
    "description": "A cheap, fast helper for simple or mechanical parts: searching, collecting facts or links, renaming, formatting, small edits.",
    "prompt": "You are a fast helper on a small team. Do only the simple part the lead gave you, in the files named in the brief. Be quick and exact. Report in under 6 lines.",
    "model": "haiku",
    "tools": ["Bash", "Read", "Write", "Edit", "Glob", "Grep", "WebSearch", "WebFetch"]
  },
  "reviewer": {
    "description": "Reviews the merged work for mistakes, gaps and parts that do not fit together. Does not rewrite; reports what to fix.",
    "prompt": "You review a small team's merged work. Read it, run it if it can run, and list concrete problems (file:line, what is wrong, how to fix) by importance. Change nothing yourself. Under 20 lines.",
    "model": "inherit",
    "tools": ["Bash", "Read", "Glob", "Grep"]
  }
}"#;

/// Other AIs (Codex, Gemini, another Claude account…) the lead may hand a part to through `ask_other_ai`: every
/// enabled, installed account except the one running the lead.
pub fn other_accounts(c: &rusqlite::Connection, exclude: &str) -> Vec<serde_json::Value> {
    crate::accounts::usable(c)
        .into_iter()
        .filter(|a| a.id != exclude)
        .map(|a| serde_json::json!({ "account": a.id, "agent": a.kind, "vendor": a.vendor_id(), "label": a.label, "model": a.model }))
        .collect()
}

static DELEGATES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// At most this many other-AI jobs at once (on top of the lead's own helpers).
const MAX_DELEGATES: usize = 3;

/// The folder a helper works in: inside the idea folder, never above it.
pub fn helper_dir(idea_dir: &Path, folder: &str) -> Result<PathBuf, String> {
    let f = folder.trim().trim_matches(['/', '\\']);
    if f.is_empty() || f.contains("..") || f.contains(':') || Path::new(f).is_absolute() {
        return Err("folder ต้องเป็นโฟลเดอร์ย่อยในโฟลเดอร์ไอเดีย เช่น parts/api".into());
    }
    Ok(idea_dir.join(f))
}

/// Run one part with another account, wait for it, and return its report (the `ask_other_ai` tool, via `ia team-mcp`).
pub fn delegate(app: &tauri::AppHandle, a: &serde_json::Value) -> Result<serde_json::Value, String> {
    use tauri::Manager;
    let st = app.state::<crate::state::AppState>();
    let idea_id = a["idea"].as_i64().ok_or("ต้องมี idea")?;
    let account = a["account"].as_str().ok_or("ต้องมี account")?.to_string();
    let task = a["task"].as_str().filter(|t| !t.trim().is_empty()).ok_or("ต้องมี task")?.to_string();
    if st.rt.lock().paused {
        return Err("บอทถูกพักอยู่ — ไม่ส่งงานให้ AI อื่น".into());
    }
    let (acct, idea) = {
        let c = st.db.lock();
        let acct = crate::accounts::usable(&c).into_iter().find(|x| x.id == account).ok_or("ไม่พบบัญชีนี้ หรือบัญชีปิดอยู่ (ดู list_other_ais)")?;
        (acct, crate::db::get_idea(&c, idea_id).map_err(|e| e.to_string())?)
    };
    let idea_dir = crate::workspace::idea_dir(&st.ws, &idea.slug);
    let dir = helper_dir(&idea_dir, a["folder"].as_str().unwrap_or(""))?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    if DELEGATES.fetch_add(1, std::sync::atomic::Ordering::SeqCst) >= MAX_DELEGATES {
        DELEGATES.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        return Err(format!("ส่งงานให้ AI อื่นพร้อมกันได้ไม่เกิน {MAX_DELEGATES} งาน — รองานที่ส่งไปก่อนเสร็จ"));
    }
    struct Release;
    impl Drop for Release {
        fn drop(&mut self) {
            DELEGATES.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let _release = Release;
    let model = if acct.is_auto_model() { crate::providers::tier_model(&st.db.lock(), &acct.kind, 1) } else { acct.model.clone() };
    let checks = crate::model_check::all(&st.db.lock());
    let (model, _) = crate::acct_models::fit(&acct, &st.root, &model, &checks);
    let report = dir.join("HELPER-REPORT.md");
    let _ = std::fs::remove_file(&report);
    let prompt = format!(
        "You are a helper on a team; the lead is another AI. Do ONLY this part, working ONLY inside this folder (it is your current directory):\n\n{task}\n\nWhen done, write a short report to HELPER-REPORT.md in this folder: what you made (files), how to use it, and anything left or uncertain (under 25 lines)."
    );
    st.push_log(format!("🤝 #{idea_id} ส่งงานส่วนหนึ่งให้ {} ({}): {}", acct.id, acct.kind, task.chars().take(80).collect::<String>()));
    let log = idea_dir.join("logs").join(format!("helper-{}-{}.jsonl", acct.id, crate::db::now()));
    let _ = std::fs::create_dir_all(idea_dir.join("logs"));
    let env = crate::accounts::env_with_secret(&acct, &st.root);
    let lang = crate::lang::ui();
    let out = tauri::async_runtime::block_on(crate::runner::run(
        crate::runner::RunSpec {
            cwd: &dir,
            prompt,
            provider: acct.kind.clone(),
            model,
            effort: "medium".into(),
            tools: "Bash,Read,Write,Edit,Glob,Grep,WebSearch,WebFetch".into(),
            custom_cmd: String::new(),
            timeout_mins: 30,
            log_path: &log,
            lang: &lang,
            env,
        },
        std::sync::Arc::new(tokio::sync::Notify::new()),
        |_| {},
        |_| {},
    ))
    .map_err(|e| format!("{e:#}"))?;
    let text = std::fs::read_to_string(&report).unwrap_or_default();
    st.push_log(format!("🤝 #{idea_id} {} ส่งงานกลับแล้ว ({})", acct.id, out.status));
    Ok(serde_json::json!({
        "status": out.status, "account": acct.id, "agent": acct.kind,
        "report": if text.trim().is_empty() { "(ไม่ได้เขียน HELPER-REPORT.md — ดูไฟล์ในโฟลเดอร์เอง)".to_string() } else { text.chars().take(6000).collect() },
        "error": out.error,
    }))
}

/// The `ia team-mcp` server for a team run: lets the lead call other vendors' AIs.
pub fn team_server(idea_id: i64, exclude: &str) -> Option<(String, serde_json::Value)> {
    let ia = crate::video::ia_exe()?;
    Some(("team".into(), serde_json::json!({ "command": ia.to_string_lossy(), "args": ["team-mcp", "--idea", idea_id.to_string(), "--exclude", exclude] })))
}

pub fn prompt_others() -> &'static str {
    "\n- นอกจากผู้ช่วยข้างบน ยังส่งงานให้ **AI ค่ายอื่น** ได้ด้วยเครื่องมือ `list_other_ais` (ดูว่ามีบัญชีไหน) และ `ask_other_ai` (account, folder, task) — ใช้เมื่อค่ายอื่นน่าจะทำส่วนนั้นได้ดีกว่าหรือถูกกว่า (เช่น Codex เขียน/แก้โค้ดเป็นชุด Gemini อ่านเอกสารยาว) งานนี้ช้ากว่าผู้ช่วยปกติ ให้ folder แยกกัน และเขียน task ให้ครบในตัว (มันไม่เห็นอะไรนอกจาก task)\n"
}

/// The `--agents` file (written once per change).
pub fn agents_file(root: &Path) -> Option<PathBuf> {
    let p = root.join("data").join("team").join("agents.json");
    if std::fs::read_to_string(&p).ok().as_deref() != Some(AGENTS) {
        std::fs::create_dir_all(p.parent()?).ok()?;
        std::fs::write(&p, AGENTS).ok()?;
    }
    Some(p)
}

/// Can this run use a team? Claude on Anthropic, a stage with real work, and the owner turned it on.
pub fn applies(helpers: u32, provider: &str, vendor_needs_model_name: bool, stage: &str) -> bool {
    helpers > 0 && provider == "claude" && !vendor_needs_model_name && !matches!(stage, "replan" | "verdict")
}

/// Added to the lead's prompt.
pub fn prompt(helpers: u32) -> String {
    format!(
        "\n\n## ทำงานเป็นทีม (เจ้าของเปิดไว้)\n\n\
         คุณเป็น**หัวหน้าทีม** เรียกผู้ช่วยได้ด้วยเครื่องมือ Task พร้อมกันไม่เกิน **{helpers}** ตัว:\n\
         - `helper` — งานทั่วไป (เขียนโค้ด ค้นคว้า เขียนเอกสาร)\n\
         - `helper-fast` — งานง่าย/งานซ้ำ (ค้นหา รวบรวมข้อมูล จัดรูปแบบ) ถูกและเร็วกว่า ใช้ตัวนี้เมื่อพอ\n\
         - `reviewer` — ตรวจงานที่รวมแล้ว ไม่แก้เอง\n\n\
         กติกา:\n\
         1. แบ่งงานเฉพาะเมื่อมีส่วนที่**ทำแยกกันได้จริง**และแต่ละส่วนใหญ่พอ ถ้างานเล็กหรือผูกกันแน่น (แก้ไฟล์เดียวกัน ต้องรอผลกัน) ให้ทำเองคนเดียว — การแบ่งมีต้นทุน\n\
         2. เรียกผู้ช่วยหลายตัว**ในข้อความเดียว**เพื่อให้ทำพร้อมกัน แต่ละตัวได้ไฟล์/โฟลเดอร์ของตัวเอง ห้ามสองตัวแก้ไฟล์เดียวกัน\n\
         3. brief สั้นแต่ครบ: เป้าหมาย ไฟล์ที่ทำได้ สิ่งที่ต้องส่งกลับ — ผู้ช่วยไม่เห็นบทสนทนานี้\n\
         4. รวมงานเอง ตรวจว่าเข้ากันได้ (ใช้ `reviewer` เมื่องานใหญ่) แล้วเขียนใน PROGRESS.md ว่าแบ่งงานอย่างไร ใครทำอะไร\n"
    )
}

/// The runner's part: with a team, the Task tool is added to `--tools` and the helpers file is passed.
pub fn apply_args(args: &mut Vec<String>, agents: &str) {
    if let Some(i) = args.iter().position(|a| a == "--tools") {
        if let Some(t) = args.get_mut(i + 1) {
            if !t.split(',').any(|x| x == "Task") {
                *t = if t.is_empty() { "Task".into() } else { format!("{t},Task") };
            }
        }
    }
    args.extend(["--agents".to_string(), agents.to_string()]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers_file_is_valid_and_helpers_cannot_hire() {
        let v: serde_json::Value = serde_json::from_str(AGENTS).unwrap();
        for (name, a) in v.as_object().unwrap() {
            let tools: Vec<&str> = a["tools"].as_array().unwrap().iter().map(|t| t.as_str().unwrap()).collect();
            assert!(!tools.contains(&"Task"), "{name} could start helpers of its own");
            assert!(a["model"].is_string());
        }
    }

    #[test]
    fn only_where_it_can_work() {
        assert!(applies(3, "claude", false, "build"));
        assert!(!applies(0, "claude", false, "build"));
        assert!(!applies(3, "codex", false, "build"));
        assert!(!applies(3, "claude", true, "build")); // StepFun/DeepSeek through Claude Code: no Anthropic helper models
        assert!(!applies(3, "claude", false, "replan"));
    }

    #[test]
    fn task_tool_is_added_once() {
        let mut a: Vec<String> = ["-p", "--tools", "Read,Write", "--model", "opus"].iter().map(|s| s.to_string()).collect();
        apply_args(&mut a, "x.json");
        apply_args(&mut a, "x.json");
        assert_eq!(a[2], "Read,Write,Task");
    }
}
