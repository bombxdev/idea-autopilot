//! "Portable brain": everything the user has taught Claude, exported as plain Markdown that any
//! model (or a future Claude account) can read back.
//!
//! Sources: Claude Code memories (`~/.claude/projects/*/memory/*.md`), personal skills
//! (`~/.claude/skills/*`), the idea knowledge base and idea summaries.
//! Output is split into a personal and a work set so company material can be left behind.
//! Ideas (and the knowledge articles named after them) go to the set the owner picked for each idea;
//! memories and skills are classified by rules: the work words in settings plus `<brain>/classification.json`.
//! Each set can live in its own folder or git repo (settings `brain_personal` / `brain_work`):
//! a git URL is cloned into `data/brain-repos/<set>` and pushed after every export.
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
            // each user lists their own company/project names (settings, or <brain>/classification.json)
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
    c.work_keywords.iter().any(|k| has_word(&t, &k.to_lowercase()))
}

/// `word` appears in `text` on its own: not glued to a letter or digit ("5s" is not found in "45s").
fn has_word(text: &str, word: &str) -> bool {
    let word = word.trim();
    if word.is_empty() {
        return false;
    }
    let glued = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric());
    // a keyword that ends in "_" or "-" ("sy_") is a prefix: only its word-like ends need a boundary
    let (open_start, open_end) = (!glued(word.chars().next()), !glued(word.chars().next_back()));
    text.match_indices(word).any(|(i, _)| {
        (open_start || !glued(text[..i].chars().next_back())) && (open_end || !glued(text[i + word.len()..].chars().next()))
    })
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
- `IDEAS.md` ไอเดียในชุดนี้และผลที่ได้
- `knowledge/` บทความความรู้ที่บอทเรียบเรียงไว้ (ของไอเดียในชุดนี้)
- `skills/` ทักษะ (Claude Code skills) ที่ใช้ได้กับ agent อื่นที่รองรับ SKILL.md
"#;

/// What one export writes into a set's folder. Only these are replaced, so the folder can be a git
/// repo: its `.git` and anything else the owner keeps there stay put.
const MANAGED_DIRS: &[&str] = &["memories", "skills", "knowledge"];

struct Set<'a> {
    label: &'a str,
    mems: Vec<Memory>,
    skills: Vec<PathBuf>,
    ideas: Vec<&'a db::Idea>,
}

/// A knowledge article belongs to the idea whose number ("idea-0005") starts its file name.
fn article_of(file: &str, slug: &str) -> bool {
    let key: String = slug.split('-').take(2).collect::<Vec<_>>().join("-");
    file.strip_prefix(&key).is_some_and(|rest| rest.starts_with('.') || rest.starts_with('-'))
}

fn write_set(dir: &Path, set: &Set, knowledge: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    for d in MANAGED_DIRS {
        let _ = std::fs::remove_dir_all(dir.join(d));
    }
    let label = set.label;
    let by = |k: &str| set.mems.iter().filter(|m| m.kind == k).collect::<Vec<_>>();
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

    for m in &set.mems {
        let d = dir.join("memories").join(&m.project);
        std::fs::create_dir_all(&d)?;
        std::fs::write(
            d.join(format!("{}.md", m.name)),
            format!("---\nname: {}\ndescription: {}\nmetadata:\n  type: {}\n---\n\n{}\n", m.name, m.description, m.kind, m.body),
        )?;
    }
    for s in &set.skills {
        if let Some(n) = s.file_name() {
            copy_dir(s, &dir.join("skills").join(n))?;
        }
    }

    let articles: Vec<(String, PathBuf)> = std::fs::read_dir(knowledge)
        .map(|r| r.flatten().map(|f| (f.file_name().to_string_lossy().to_string(), f.path())).collect())
        .unwrap_or_default();
    let mut md = String::from("# ไอเดียทั้งหมด\n\n");
    for i in &set.ideas {
        md.push_str(&format!(
            "## {} ({})\n- เป้าหมาย: {}\n- ผล: {}{}\n\n",
            i.title,
            i.status,
            i.intents.join(", "),
            i.summary,
            i.verdict.as_ref().map(|v| format!(" — คำตัดสิน {v}")).unwrap_or_default()
        ));
        for (name, path) in articles.iter().filter(|(n, p)| p.is_file() && article_of(n, &i.slug)) {
            let d = dir.join("knowledge");
            std::fs::create_dir_all(&d)?;
            std::fs::copy(path, d.join(name))?;
        }
    }
    std::fs::write(dir.join("IDEAS.md"), md)?;
    Ok(())
}

pub fn is_git_url(v: &str) -> bool {
    let v = v.trim();
    v.starts_with("https://") || v.starts_with("http://") || v.starts_with("git@") || v.starts_with("ssh://")
}

fn bound(st: &AppState, set: &str) -> Option<String> {
    db::get_setting(&st.db.lock(), &format!("brain_{set}")).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Where one set ("personal" | "work") is written: the folder or repo bound in settings, else `<brain>/<set>`.
pub fn set_dir(st: &AppState, set: &str) -> PathBuf {
    match bound(st, set) {
        Some(url) if is_git_url(&url) => st.root.join("data").join("brain-repos").join(set),
        Some(path) => PathBuf::from(path),
        None => brain_dir(st).join(set),
    }
}

/// Get a bound git repo ready to write into: clone it the first time, else pull.
fn prepare_repo(st: &AppState, set: &str, dir: &Path) -> Result<()> {
    let url = bound(st, set).unwrap_or_default();
    if is_git_url(&url) && !dir.join(".git").exists() {
        if let Some(parent) = dir.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let _ = std::fs::remove_dir_all(dir);
        crate::sync::git_run(None, &["clone", "-q", &url, &dir.to_string_lossy()])?;
    } else if dir.join(".git").exists() {
        crate::sync::git_pull(dir)?;
    }
    Ok(())
}

fn within(child: &Path, parent: &Path) -> bool {
    let norm = |p: &Path| p.to_string_lossy().replace('\\', "/").trim_end_matches('/').to_lowercase();
    let (c, p) = (norm(child), norm(parent));
    c == p || c.starts_with(&format!("{p}/"))
}

/// The work words from settings, plus those in classification.json next to the export and in the one an
/// earlier version kept in the app folder (so moving the export into the sync folder loses none).
fn work_keywords(st: &AppState, cls: &Classification) -> Vec<String> {
    let mut words: Vec<String> = db::get_setting(&st.db.lock(), "brain_work_keywords")
        .unwrap_or_default()
        .split([',', '\n', ';'])
        .map(|w| w.trim().to_string())
        .filter(|w| !w.is_empty())
        .collect();
    let legacy: Option<Classification> =
        std::fs::read_to_string(st.root.join("brain").join("classification.json")).ok().and_then(|s| serde_json::from_str(&s).ok());
    for w in cls.work_keywords.iter().chain(legacy.iter().flat_map(|l| l.work_keywords.iter())) {
        if !w.trim().is_empty() && !words.iter().any(|x| x.eq_ignore_ascii_case(w.trim())) {
            words.push(w.trim().to_string());
        }
    }
    words
}

/// The work words in use right now (settings + classification files), for the settings page.
pub fn effective_keywords(st: &AppState) -> Vec<String> {
    let cls: Classification = std::fs::read_to_string(brain_dir(st).join("classification.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    work_keywords(st, &cls)
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
    let cls = Classification { work_keywords: work_keywords(st, &cls), ..cls };

    let (pdir, wdir) = (set_dir(st, "personal"), set_dir(st, "work"));
    if within(&pdir, &wdir) || within(&wdir, &pdir) {
        anyhow::bail!("ชุดส่วนตัวกับชุดงานต้องอยู่คนละที่ — แก้ในหน้าตั้งค่า › สมองพกพา");
    }
    // the sync folder may be a personal git repo: company material only goes there under brain/work/ (ignored)
    if let Some(sd) = crate::sync::sync_dir(st) {
        if within(&wdir, &sd) && !within(&wdir, &sd.join("brain").join("work")) {
            anyhow::bail!("ที่เก็บชุดงานอยู่ในโฟลเดอร์ sync — จะถูก push ไปกับข้อมูลส่วนตัว เลือกที่เก็บอื่น");
        }
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

    let ideas = db::list_ideas(&st.db.lock())?;
    let live: Vec<&db::Idea> = ideas.iter().filter(|i| i.status != "deleted").collect();
    let personal = Set { label: "ส่วนตัว", mems: personal, skills: pskills, ideas: live.iter().copied().filter(|i| i.scope != "work").collect() };
    let work = Set { label: "งาน", mems: work, skills: wskills, ideas: live.iter().copied().filter(|i| i.scope == "work").collect() };

    let knowledge = st.ws.join("knowledge");
    let sync = crate::sync::sync_dir(st);
    let mut notes = Vec::new();
    for (key, set, dir) in [("personal", &personal, &pdir), ("work", &work, &wdir)] {
        if let Err(e) = prepare_repo(st, key, dir) {
            notes.push(format!("{}: ดึง repo ไม่สำเร็จ ({e:#})", set.label));
        }
        write_set(dir, set, &knowledge)?;
        // a repo of its own (not the sync folder, which pushes itself): commit and push what changed
        let in_sync = sync.as_ref().is_some_and(|sd| within(dir, sd));
        if dir.join(".git").exists() && !in_sync {
            if let Err(e) = crate::sync::git_commit_push(dir, &format!("brain export from {}", crate::sync::machine())) {
                notes.push(format!("{}: push ไม่สำเร็จ ({e:#})", set.label));
            }
        }
    }
    for n in &notes {
        st.push_log(format!("[brain] {n}"));
    }

    std::fs::write(
        out.join("README.md"),
        format!(
            "# สมองพกพา\n\nexport ล่าสุด: {}\n\n- ส่วนตัว: `{}` — เอาไปใช้ที่ไหนก็ได้\n- งานบริษัท: `{}` — เลือกเองว่าจะเอาไปด้วยหรือไม่\n\n\
             ไอเดียแยกตามที่เลือกตอนจด (ส่วนตัว / งาน) · memory และ skill แยกด้วยคำของบริษัท (หน้าตั้งค่า และ `classification.json`)\n\n\
             วิธีนำกลับไปใช้: ดู `IMPORT.md` ในแต่ละชุด\n\n\
             ส่วนตัว: {} memory, {} skill, {} ไอเดีย · งาน: {} memory, {} skill, {} ไอเดีย\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M"),
            pdir.display(),
            wdir.display(),
            personal.mems.len(),
            personal.skills.len(),
            personal.ideas.len(),
            work.mems.len(),
            work.skills.len(),
            work.ideas.len()
        ),
    )?;
    db::set_setting(&st.db.lock(), "brain_exported", &db::now().to_string())?;
    if notes.is_empty() {
        Ok(out.to_string_lossy().into())
    } else {
        Ok(format!("{} ({})", out.to_string_lossy(), notes.join(" · ")))
    }
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
        let c = Classification { work_keywords: vec!["5s".into(), "sy_".into()], ..Default::default() };
        assert!(is_work("a", "the 5S dashboard", "project", &c));
        assert!(!is_work("b", "written 45s after", "project", &c), "a word inside another is not a match");
        assert!(is_work("c", "table SY_Menu", "project", &c), "a prefix keyword ending in _ still matches");
    }

    #[test]
    fn articles_follow_their_idea_and_folders_nest() {
        assert!(article_of("idea-0005-bilibili-zscaler-vpn.md", "idea-0005-bilibili-zscaler-vpn"));
        assert!(article_of("idea-0005-old-name.md", "idea-0005-renamed"));
        assert!(!article_of("idea-0050-other.md", "idea-0005"));
        assert!(within(Path::new("D:\\data\\brain\\work"), Path::new("d:/data")));
        assert!(!within(Path::new("D:/database"), Path::new("D:/data")));
        assert!(is_git_url("https://github.com/me/brain.git") && is_git_url("git@github.com:me/b.git") && !is_git_url("D:/brain"));
    }
}
