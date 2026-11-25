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
    // "." or "./." name the lead's own folder: a helper must get a sub-folder of its own
    let own = Path::new(f).components().any(|c| matches!(c, std::path::Component::Normal(_)));
    if f.is_empty() || !own || f.contains("..") || f.contains(':') || Path::new(f).is_absolute() {
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
    // the lead's run: when it ends — stopped, paused, the quota governor, a chat taking over, or just done —
    // its helpers are stopped too, so nothing keeps spending quota on its own
    let lead_cancel = {
        let rt = st.rt.lock();
        if rt.paused {
            return Err("บอทถูกพักอยู่ — ไม่ส่งงานให้ AI อื่น".into());
        }
        // the quota governor's verdict for this account (scheduler.rs), the same one the bot obeys
        if let Some((false, why)) = rt.acct_allowed.get(&account) {
            return Err(format!("บัญชี {account} ถูกงดใช้ตอนนี้: {why} — เลือกบัญชีอื่นจาก list_other_ais หรือทำส่วนนี้เอง"));
        }
        rt.cancel.clone()
    };
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
    // the role's own choice first (an exact model on this machine, else its tier), then the account's
    let role_def = a["role"].as_str().and_then(|r| crate::roles::find(&st.root, r));
    let model = match &role_def {
        Some(r) if !r.model_override.is_empty() => r.model_override.clone(),
        Some(r) if acct.is_auto_model() => crate::providers::tier_model(&st.db.lock(), &acct.kind, crate::roles::tier_index(&r.tier)),
        _ if acct.is_auto_model() => crate::providers::tier_model(&st.db.lock(), &acct.kind, 1),
        _ => acct.model.clone(),
    };
    let checks = crate::model_check::all(&st.db.lock());
    let (model, _) = crate::acct_models::fit(&acct, &st.root, &model, &checks);
    let report = dir.join("HELPER-REPORT.md");
    let _ = std::fs::remove_file(&report);
    // the owner's team role this AI plays, if the lead named one (roles.rs)
    let role = a["role"].as_str().and_then(|r| crate::roles::delegate_brief(&st.root, r)).map(|b| format!("{b}\n\n")).unwrap_or_default();
    let prompt = format!(
        "{role}You are a helper on a team; the lead is another AI. Do ONLY this part, working ONLY inside this folder (it is your current directory):\n\n{task}\n\nWhen done, write a short report to HELPER-REPORT.md in this folder: what you made (files), how to use it, and anything left or uncertain (under 25 lines)."
    );
    st.push_log(format!("🤝 #{idea_id} ส่งงานส่วนหนึ่งให้ {} ({}): {}", acct.id, acct.kind, task.chars().take(80).collect::<String>()));
    let log = idea_dir.join("logs").join(format!("helper-{}-{}.jsonl", acct.id, crate::db::now()));
    let part = crate::db::team_part_start(&st.db.lock(), idea_id, role_def.as_ref().map(|r| r.slug.as_str()).unwrap_or(""), &acct.id, &model, a["folder"].as_str().unwrap_or(""), &task);
    let _ = std::fs::create_dir_all(idea_dir.join("logs"));
    let env = crate::accounts::env_with_secret(&acct, &st.root);
    let lang = crate::lang::ui();
    let helper_cancel = std::sync::Arc::new(tokio::sync::Notify::new());
    let _watch = lead_cancel.map(|lead| watch_lead(app.clone(), lead, helper_cancel.clone()));
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
        helper_cancel,
        |_| {},
        |_| {},
    ))
    .map_err(|e| format!("{e:#}"))?;
    crate::db::team_part_end(&st.db.lock(), part, &out.status);
    let text = std::fs::read_to_string(&report).unwrap_or_default();
    st.push_log(format!("🤝 #{idea_id} {} ส่งงานกลับแล้ว ({})", acct.id, out.status));
    Ok(serde_json::json!({
        "status": out.status, "account": acct.id, "agent": acct.kind,
        "report": if text.trim().is_empty() { "(ไม่ได้เขียน HELPER-REPORT.md — ดูไฟล์ในโฟลเดอร์เอง)".to_string() } else { text.chars().take(6000).collect() },
        "error": out.error,
    }))
}

/// Stops a helper when the run that asked for it is over: the lead's cancel signal is gone from the runtime
/// (the run ended) or replaced by another run's. Checked every second; dropping the guard ends the watch.
pub struct LeadWatch(std::sync::Arc<std::sync::atomic::AtomicBool>);

impl Drop for LeadWatch {
    fn drop(&mut self) {
        self.0.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

fn watch_lead(app: tauri::AppHandle, lead: std::sync::Arc<tokio::sync::Notify>, helper: std::sync::Arc<tokio::sync::Notify>) -> LeadWatch {
    use tauri::Manager;
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let d = done.clone();
    std::thread::spawn(move || {
        while !d.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let st = app.state::<crate::state::AppState>();
            if lead_gone(st.rt.lock().cancel.as_ref(), &lead) {
                helper.notify_one();
                return;
            }
        }
    });
    LeadWatch(done)
}

/// The lead's run is over when the runtime holds no cancel signal, or another run's.
pub fn lead_gone(now: Option<&std::sync::Arc<tokio::sync::Notify>>, lead: &std::sync::Arc<tokio::sync::Notify>) -> bool {
    now.map(|c| !std::sync::Arc::ptr_eq(c, lead)).unwrap_or(true)
}

/// The `ia team-mcp` server for a team run: lets the lead call other vendors' AIs.
pub fn team_server(idea_id: i64, exclude: &str) -> Option<(String, serde_json::Value)> {
    let ia = crate::video::ia_exe()?;
    Some(("team".into(), serde_json::json!({ "command": ia.to_string_lossy(), "args": ["team-mcp", "--idea", idea_id.to_string(), "--exclude", exclude] })))
}

pub fn prompt_others() -> &'static str {
    "\n- นอกจากผู้ช่วยข้างบน ยังส่งงานให้ **AI ค่ายอื่น** ได้ด้วยเครื่องมือ `list_other_ais` (ดูว่ามีบัญชีไหน) และ `ask_other_ai` (account, folder, task) — ใช้เมื่อค่ายอื่นน่าจะทำส่วนนั้นได้ดีกว่าหรือถูกกว่า (เช่น Codex เขียน/แก้โค้ดเป็นชุด Gemini อ่านเอกสารยาว) งานนี้ช้ากว่าผู้ช่วยปกติ ให้ folder แยกกัน และเขียน task ให้ครบในตัว (มันไม่เห็นอะไรนอกจาก task)\n"
}

/// The `--agents` file (written once per change): the built-in helpers plus the team's roles that the lead's own
/// Claude plays (roles.rs).
pub fn agents_file(root: &Path, roles: &[crate::roles::Role]) -> Option<PathBuf> {
    let mut all: serde_json::Map<String, serde_json::Value> = serde_json::from_str(AGENTS).ok()?;
    all.extend(crate::roles::subagents(roles));
    let text = serde_json::to_string_pretty(&all).ok()?;
    let p = root.join("data").join("team").join("agents.json");
    if std::fs::read_to_string(&p).ok().as_deref() != Some(text.as_str()) {
        std::fs::create_dir_all(p.parent()?).ok()?;
        std::fs::write(&p, &text).ok()?;
    }
    Some(p)
}

/// Can this run use a team? Claude on Anthropic, a stage with real work, and the owner turned it on.
/// AIs that can lead a team: they take tools (MCP), so they can hand parts out with `ask_other_ai` (mcp.rs).
const LEADS: [&str; 7] = ["claude", "codex", "gemini", "antigravity", "qwen", "opencode", "mistral"];

/// Whether a team can work on this run, and why not when it cannot (shown in the log — never silent).
pub fn applies(provider: &str, stage: &str, ia_ready: bool) -> Result<(), &'static str> {
    if matches!(stage, "replan" | "verdict") {
        return Err("ขั้นนี้สั้นและต้องคิดต่อเนื่อง — ทำคนเดียว");
    }
    if !LEADS.contains(&provider) {
        return Err("AI ตัวนี้รับเครื่องมือ (MCP) ไม่ได้ จึงสั่งงานสมาชิกไม่ได้ — ทำคนเดียว");
    }
    if !ia_ready {
        return Err("ไม่พบ ia (ตัวกลางส่งงานให้สมาชิก) — ทำคนเดียว");
    }
    Ok(())
}

/// Claude on Anthropic can also run helpers inside its own run (subagents, fast); every other lead hands all
/// parts out through `ask_other_ai`.
pub fn subagents_ok(provider: &str, vendor_needs_model_name: bool) -> bool {
    provider == "claude" && !vendor_needs_model_name
}

/// The lead's brief when it is not Claude on Anthropic: every part goes through `ask_other_ai`.
pub fn prompt_any(helpers: u32) -> String {
    format!(
        "\n\n## ทำงานเป็นทีม (เจ้าของเลือกทีมไว้)\n\n\
         คุณเป็น**หัวหน้าทีม** ส่งงานให้สมาชิกด้วยเครื่องมือ `ask_other_ai` (account, role, folder, task) พร้อมกันได้ไม่เกิน **{helpers}** งาน:\n\
         1. แบ่งเฉพาะส่วนที่**ทำแยกกันได้จริง**และใหญ่พอ งานเล็กหรือผูกกันแน่นให้ทำเอง — การแบ่งมีต้นทุน\n\
         2. แต่ละส่วนได้ folder ของตัวเอง (เช่น parts/ui) ห้ามสองส่วนแก้ไฟล์เดียวกัน เรียกหลายส่วนต่อกันได้เลยไม่ต้องรอทีละตัวถ้าเครื่องมือของคุณเรียกพร้อมกันได้\n\
         3. task ต้องครบในตัว (เป้าหมาย ไฟล์ที่ต้องทำ วิธีตรวจ) สมาชิกไม่เห็นอะไรนอกจาก task\n\
         4. รวมงานเอง ตรวจว่าเข้ากันได้ แล้วเขียนใน PROGRESS.md ว่าแบ่งงานอย่างไร ใครทำอะไร\n"
    )
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
    fn helper_never_gets_the_leads_own_folder() {
        let root = Path::new("idea");
        for bad in ["", ".", "./", "./.", ".\\", "../x", "C:/x"] {
            assert!(helper_dir(root, bad).is_err(), "{bad:?}");
        }
        assert_eq!(helper_dir(root, "parts/api").unwrap(), root.join("parts/api"));
        assert_eq!(helper_dir(root, "./parts").unwrap(), root.join("./parts"));
    }

    #[test]
    fn helper_stops_when_the_lead_run_is_over() {
        let lead = std::sync::Arc::new(tokio::sync::Notify::new());
        let other = std::sync::Arc::new(tokio::sync::Notify::new());
        assert!(!lead_gone(Some(&lead), &lead));
        assert!(lead_gone(None, &lead));
        assert!(lead_gone(Some(&other), &lead));
    }

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
    fn any_ai_that_takes_tools_can_lead() {
        for lead in ["claude", "codex", "gemini", "antigravity", "qwen", "opencode", "mistral"] {
            assert!(applies(lead, "build", true).is_ok(), "{lead}");
        }
        assert!(applies("custom", "build", true).is_err()); // "my own command": no tools, so it works alone — and says so
        assert!(applies("codex", "replan", true).is_err());
        assert!(applies("gemini", "build", false).is_err());
        // only Claude on Anthropic runs helpers inside its own run; StepFun/DeepSeek through Claude Code hand parts out
        assert!(subagents_ok("claude", false) && !subagents_ok("claude", true) && !subagents_ok("codex", false));
    }

    #[test]
    fn task_tool_is_added_once() {
        let mut a: Vec<String> = ["-p", "--tools", "Read,Write", "--model", "opus"].iter().map(|s| s.to_string()).collect();
        apply_args(&mut a, "x.json");
        apply_args(&mut a, "x.json");
        assert_eq!(a[2], "Read,Write,Task");
    }
}
