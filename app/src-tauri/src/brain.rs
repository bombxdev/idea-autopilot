//! "Portable brain": everything the user has taught Claude, exported as plain Markdown that any
//! model (or a future Claude account) can read back.
//!
//! Sources: Claude Code memories (`~/.claude/projects/*/memory/*.md`), personal skills
//! (`~/.claude/skills/*`), the idea knowledge base and idea summaries.
//! Output is split into `personal/` and `work/` so company material can be left behind.
//! Classification is rule-based and editable in `<brain>/classification.json`.
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::db;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Classification {
    /// a project/reference memory or a skill mentioning any of these is "work"
    pub work_keywords: Vec<String>,
    /// memory/skill names forced into a set
    pub force_personal: Vec<String>,
    pub force_work: Vec<String>,
}

impl Default for Classification {
    fn default() -> Self {
        Classification {
            // each user lists their own company/project names in <brain>/classification.json
            work_keywords: vec![],
            force_personal: vec![],
            force_work: vec![],
        }
    }
}

#[derive(Debug, Clone)]
struct Memory {
    name: String,
    description: String,
    kind: String,
    body: String,
    project: String,
}

fn parse_memory(text: &str, project: &str, fallback: &str) -> Option<Memory> {
    let rest = text.strip_prefix("---")?;
    let end = rest.find("\n---")?;
    let (front, body) = (&rest[..end], rest[end + 4..].trim());
    let field = |k: &str| {
        front
            .lines()
            .find_map(|l| l.trim().strip_prefix(&format!("{k}:")).map(|v| v.trim().trim_matches('"').to_string()))
    };
    Some(Memory {
        name: field("name").unwrap_or_else(|| fallback.into()),
        description: field("description").unwrap_or_default(),
        kind: field("type").unwrap_or_else(|| "project".into()),
        body: body.to_string(),
        project: project.into(),
    })
}

fn is_work(name: &str, text: &str, kind: &str, c: &Classification) -> bool {
    if c.force_personal.iter().any(|n| n == name) {
        return false;
    }
    if c.force_work.iter().any(|n| n == name) {
        return true;
    }
    // facts about the person and how they like to work are always personal
    if kind == "user" || kind == "feedback" {
        return false;
    }
    let t = text.to_lowercase();
    c.work_keywords.iter().any(|k| t.contains(&k.to_lowercase()))
}

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)?.flatten() {
        let (s, d) = (e.path(), dst.join(e.file_name()));
        if s.is_dir() {
            copy_dir(&s, &d)?;
        } else {
            std::fs::copy(&s, &d)?;
        }
    }
    Ok(())
}

pub fn brain_dir(st: &AppState) -> PathBuf {
    let c = st.db.lock();
    if let Some(d) = db::get_setting(&c, "brain_dir").filter(|s| !s.trim().is_empty()) {
        return PathBuf::from(d);
    }
    drop(c);
    match crate::sync::sync_dir(st) {
        Some(s) => s.join("brain"),
        None => st.root.join("brain"),
    }
}

fn section(title: &str, items: &[&Memory]) -> String {
    let mut s = format!("# {title}\n\n");
    if items.is_empty() {
        s.push_str("_(ยังไม่มี)_\n");
    }
    for m in items {
        s.push_str(&format!("## {}\n_{}_\n\n{}\n\n", m.name, m.description, m.body));
    }
    s
}

const IMPORT_GUIDE: &str = r#"# วิธีนำสมองนี้กลับไปใช้

ไฟล์ทั้งหมดเป็น Markdown ธรรมดา โมเดลไหนก็อ่านได้

## เริ่มเร็วที่สุด (ทุกโมเดล)
คัดลอกเนื้อหา `BOOTSTRAP.md` ไปวางเป็น system prompt / custom instructions / project instructions
(ChatGPT: Custom instructions หรือ Project, Gemini: Gem, Claude.ai: Project instructions หรือใช้ฟีเจอร์ import memory)

## Claude Code (เครื่องใหม่ / account ใหม่)
1. คัดลอกโฟลเดอร์ใน `skills/` ไปไว้ที่ `~/.claude/skills/`
2. วาง `BOOTSTRAP.md` เป็น `~/.claude/CLAUDE.md` (หรือต่อท้ายไฟล์เดิม) เพื่อให้ทุก session รู้จักคุณ
3. memory รายโปรเจกต์อยู่ใน `memories/<project>/` — คัดลอกกลับไปที่ `~/.claude/projects/<project>/memory/` ได้ถ้าใช้ path เดิม

## ไฟล์
- `PROFILE.md` คุณเป็นใคร ชอบอะไร
- `PRINCIPLES.md` หลักการทำงานที่คุณสอนไว้ (สำคัญที่สุด)
- `PROJECTS.md` บริบทโปรเจกต์ต่างๆ
- `REFERENCES.md` ลิงก์/ระบบภายนอก
- `IDEAS.md` ไอเดียทั้งหมดและผลที่ได้
- `knowledge/` บทความความรู้ที่บอทเรียบเรียงไว้
- `skills/` ทักษะ (Claude Code skills) ที่ใช้ได้กับ agent อื่นที่รองรับ SKILL.md
"#;

fn write_set(dir: &Path, label: &str, mems: &[Memory], skills: &[PathBuf], extra: Option<(&str, &Path)>) -> Result<()> {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir)?;
    let by = |k: &str| mems.iter().filter(|m| m.kind == k).collect::<Vec<_>>();
    let (user, fb, proj, refs) = (by("user"), by("feedback"), by("project"), by("reference"));
    std::fs::write(dir.join("PROFILE.md"), section(&format!("โปรไฟล์ ({label})"), &user))?;
    std::fs::write(dir.join("PRINCIPLES.md"), section(&format!("หลักการทำงานที่สอนไว้ ({label})"), &fb))?;
    std::fs::write(dir.join("PROJECTS.md"), section(&format!("โปรเจกต์ ({label})"), &proj))?;
    std::fs::write(dir.join("REFERENCES.md"), section(&format!("แหล่งอ้างอิง ({label})"), &refs))?;
    std::fs::write(dir.join("IMPORT.md"), IMPORT_GUIDE)?;

    // one compact prompt for any model
    let mut boot = format!(
        "# About me and how I want AI to work with me ({label})\n\n\
         Use this as standing context. Reply in Thai unless asked otherwise.\n\n## Who I am\n"
    );
    for m in &user {
        boot.push_str(&format!("- {}\n", m.body.replace('\n', " ")));
    }
    boot.push_str("\n## Principles I've taught (follow these)\n");
    for m in &fb {
        let first = m.body.split("**Why:**").next().unwrap_or(&m.body).trim().replace('\n', " ");
        boot.push_str(&format!("- **{}** — {}\n", m.description, first));
    }
    if !proj.is_empty() {
        boot.push_str("\n## Projects (details in PROJECTS.md)\n");
        for m in &proj {
            boot.push_str(&format!("- {}: {}\n", m.name, m.description));
        }
    }
    std::fs::write(dir.join("BOOTSTRAP.md"), boot)?;

    for m in mems {
        let d = dir.join("memories").join(&m.project);
        std::fs::create_dir_all(&d)?;
        std::fs::write(
            d.join(format!("{}.md", m.name)),
            format!("---\nname: {}\ndescription: {}\nmetadata:\n  type: {}\n---\n\n{}\n", m.name, m.description, m.kind, m.body),
        )?;
    }
    for s in skills {
        if let Some(n) = s.file_name() {
            copy_dir(s, &dir.join("skills").join(n))?;
        }
    }
    if let Some((name, src)) = extra {
        if src.is_dir() {
            copy_dir(src, &dir.join(name))?;
        }
    }
    Ok(())
}

pub fn export(st: &AppState) -> Result<String> {
    let out = brain_dir(st);
    std::fs::create_dir_all(&out)?;
    let cls_path = out.join("classification.json");
    let cls: Classification = std::fs::read_to_string(&cls_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    if !cls_path.exists() {
        std::fs::write(&cls_path, serde_json::to_string_pretty(&cls)?)?;
    }

    let home = dirs::home_dir().unwrap_or_default().join(".claude");
    let bot_prefix = crate::activity::encode_path(&st.ws).to_lowercase();
    let mut personal = Vec::new();
    let mut work = Vec::new();
    if let Ok(projects) = std::fs::read_dir(home.join("projects")) {
        for p in projects.flatten() {
            let project = p.file_name().to_string_lossy().to_string();
            if project.to_lowercase().starts_with(&bot_prefix) {
                continue;
            }
            let Ok(files) = std::fs::read_dir(p.path().join("memory")) else { continue };
            for f in files.flatten() {
                let path = f.path();
                let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                if path.extension().and_then(|e| e.to_str()) != Some("md") || stem == "MEMORY" {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&path) else { continue };
                let Some(m) = parse_memory(&text, &project, &stem) else { continue };
                let probe = format!("{} {} {}", project, m.description, m.body);
                if is_work(&m.name, &probe, &m.kind, &cls) { work.push(m) } else { personal.push(m) }
            }
        }
    }

    let (mut pskills, mut wskills) = (Vec::new(), Vec::new());
    if let Ok(skills) = std::fs::read_dir(home.join("skills")) {
        for s in skills.flatten() {
            let name = s.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name == "synced" || !s.path().join("SKILL.md").exists() {
                continue;
            }
            let text = std::fs::read_to_string(s.path().join("SKILL.md")).unwrap_or_default();
            if is_work(&name, &format!("{name} {text}"), "skill", &cls) { wskills.push(s.path()) } else { pskills.push(s.path()) }
        }
    }

    write_set(&out.join("personal"), "ส่วนตัว", &personal, &pskills, Some(("knowledge", &st.ws.join("knowledge"))))?;
    write_set(&out.join("work"), "งาน", &work, &wskills, None)?;

    // ideas and their outcomes belong to the personal set
    let ideas = db::list_ideas(&st.db.lock())?;
    let mut md = String::from("# ไอเดียทั้งหมด\n\n");
    for i in ideas.iter().filter(|i| i.status != "deleted") {
        md.push_str(&format!(
            "## {} ({})\n- เป้าหมาย: {}\n- ผล: {}{}\n\n",
            i.title,
            i.status,
            i.intents.join(", "),
            i.summary,
            i.verdict.as_ref().map(|v| format!(" — คำตัดสิน {v}")).unwrap_or_default()
        ));
    }
    std::fs::write(out.join("personal").join("IDEAS.md"), md)?;
    std::fs::write(
        out.join("README.md"),
        format!(
            "# สมองพกพา\n\nexport ล่าสุด: {}\n\n- `personal/` เรื่องส่วนตัว — เอาไปใช้ที่ไหนก็ได้\n- `work/` เรื่องงานบริษัท — เลือกเองว่าจะเอาไปด้วยหรือไม่\n- แก้การแยกหมวดได้ที่ `classification.json`\n\nวิธีนำกลับไปใช้: ดู `personal/IMPORT.md`\n\nส่วนตัว: {} memory, {} skill · งาน: {} memory, {} skill\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M"),
            personal.len(),
            pskills.len(),
            work.len(),
            wskills.len()
        ),
    )?;
    db::set_setting(&st.db.lock(), "brain_exported", &db::now().to_string())?;
    Ok(out.to_string_lossy().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_classifies() {
        let m = parse_memory("---\nname: x\ndescription: \"d\"\nmetadata:\n  type: feedback\n---\n\nbody acme", "p", "f").unwrap();
        assert_eq!((m.name.as_str(), m.kind.as_str(), m.description.as_str()), ("x", "feedback", "d"));
        let c = Classification { work_keywords: vec!["acme".into(), "erp_".into()], ..Default::default() };
        assert!(!is_work("x", "acme", "feedback", &c), "feedback is always personal");
        assert!(is_work("y", "ERP_Application migration", "project", &c));
        assert!(!is_work("z", "usage widget", "project", &c));
    }
}
