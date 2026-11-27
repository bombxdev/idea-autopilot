//! Chat rounds (ISSUE-159): every prompt the owner sends in an idea's chat and what came back, kept whole.
//!
//! The live feed keeps only its last lines in memory and CHAT-LOG.md flattened each message to one 600-character
//! line, so on waking up the owner had the prompts (directives) and a one-line summary, but not the answers.
//! A round starts with the owner's message (🙋) and collects the AI's lines until the next one:
//! - 💬 messages: the last one is the answer (shown whole, as markdown); the ones before are the "along the way" notes
//! - tool lines (🔧 📖 ✏️ 🌐 › …) are only counted, files written are listed
//! - ⚠️ lines become plain-language problems (`plain_problem`), deduplicated
//! - 📝 the summary the AI noted; ✅ the round finished
//! The same feed comes from every AI (Claude, Codex, Gemini, Antigravity…), so rounds look alike for all of them.

use rusqlite::{params, Connection};
use serde_json::{json, Value};

pub fn migrate(c: &Connection) {
    let _ = c.execute_batch(
        "CREATE TABLE IF NOT EXISTS chat_rounds (
            id INTEGER PRIMARY KEY,
            idea_id INTEGER NOT NULL,
            started INTEGER NOT NULL,
            updated INTEGER NOT NULL,
            prompt TEXT NOT NULL DEFAULT '',
            answer TEXT NOT NULL DEFAULT '',
            says TEXT NOT NULL DEFAULT '[]',
            steps INTEGER NOT NULL DEFAULT 0,
            files TEXT NOT NULL DEFAULT '[]',
            problems TEXT NOT NULL DEFAULT '[]',
            note TEXT NOT NULL DEFAULT '',
            done INTEGER NOT NULL DEFAULT 0,
            imported INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS chat_rounds_idea ON chat_rounds(idea_id, started);",
    );
}

const PROMPT_PREFIXES: &[&str] = &["🙋 คุณ:", "🙋 You:", "🙋 你：", "🙋 あなた：", "🙋 나:", "🙋 Bạn:", "🙋 Anda:", "🙋 Tú:", "🙋"];
const MAX_SAYS: usize = 60;
const MAX_ANSWER: usize = 40_000;

fn strip_prefix<'a>(t: &'a str, p: &str) -> &'a str {
    t.strip_prefix(p).unwrap_or(t).trim()
}

fn arr(s: &str) -> Vec<Value> {
    serde_json::from_str::<Vec<Value>>(s).unwrap_or_default()
}

/// A raw error line → what it means, in plain Thai, or None when it is not recognised.
pub fn plain_problem(raw: &str) -> Option<String> {
    let l = raw.to_lowercase();
    let module = |key: &str| {
        raw.split(key).nth(1).map(|r| r.trim_start_matches([' ', ':', '\'', '"']).split(['\'', '"', ' ', ')']).next().unwrap_or("").to_string())
    };
    let m = if l.contains("modulenotfounderror") || l.contains("no module named") {
        let name = module("No module named").or_else(|| module("no module named")).filter(|n| !n.is_empty());
        format!("ขาดไลบรารี Python{} ที่สคริปต์ต้องใช้ — AI ต้องติดตั้งก่อนหรือเลือกวิธีอื่น", name.map(|n| format!(" ({n})")).unwrap_or_default())
    } else if l.contains("cannot find module") || l.contains("err_module_not_found") {
        "ขาดแพ็กเกจ Node.js ที่โค้ดต้องใช้ — ต้องติดตั้ง (npm install) ก่อน".into()
    } else if l.contains("prompt is too long") || l.contains("context length") || l.contains("context_length") || l.contains("too many tokens") {
        "ข้อความที่ส่งให้ AI ยาวเกินที่รับได้ (ไฟล์แนบหรือประวัติยาวไป) — เริ่มแชตใหม่หรือสั่งให้อ่านเฉพาะส่วน".into()
    } else if l.contains("is not recognized as") || l.contains("command not found") || l.contains("not found in path") {
        "คำสั่งที่ AI เรียกไม่มีในเครื่องนี้ — ต้องติดตั้งโปรแกรมนั้นหรือใช้วิธีอื่น".into()
    } else if l.contains("permission denied") || l.contains("access is denied") || l.contains("operation not permitted") || l.contains("sandbox") {
        "ไม่มีสิทธิ์เขียน/รันในตำแหน่งนั้น (ข้อจำกัดความปลอดภัยของ AI หรือไฟล์ถูกล็อก) — งานส่วนนั้นอาจไม่ได้บันทึก".into()
    } else if l.contains("index.lock") || (l.contains("git") && l.contains("lock")) {
        "git ถูกล็อกอยู่ (มีโปรแกรมอื่นใช้ git ค้าง) — commit ไม่ได้ แต่ไฟล์งานยังอยู่".into()
    } else if l.contains("timed out") || l.contains("timeout") || l.contains("หมดเวลา") {
        "ขั้นนั้นใช้เวลานานเกินกำหนดจนถูกหยุด — อาจต้องแบ่งงานให้เล็กลง".into()
    } else if l.contains("rate limit") || l.contains("429") || l.contains("usage limit") || l.contains("quota") {
        "บัญชี AI ชนลิมิตการใช้งาน — รอรอบรีเซ็ตหรือสลับไปบัญชีอื่น".into()
    } else if l.contains("401") || l.contains("unauthorized") || l.contains("invalid api key") || l.contains("not logged in") || l.contains("oauth") {
        "บัญชี AI ล็อกอินหลุดหรือ key ใช้ไม่ได้ — ต้องล็อกอินใหม่ในหน้าบัญชี".into()
    } else if l.contains("403") || l.contains("forbidden") || l.contains("access denied") {
        "เว็บ/บริการปฏิเสธการเข้าถึง (403) — อาจต้องล็อกอินหรือไม่อนุญาตให้บอทเข้า".into()
    } else if l.contains("404") || l.contains("no such file") || l.contains("does not exist") || l.contains("filenotfounderror") || l.contains("cannot find the path") {
        "หาไฟล์หรือหน้าเว็บที่อ้างถึงไม่เจอ — อาจพิมพ์ชื่อผิดหรือยังไม่ได้สร้าง".into()
    } else if l.contains("econnrefused") || l.contains("enotfound") || l.contains("network") || l.contains("connection") || l.contains("certificate") || l.contains("ssl") {
        "เชื่อมต่อเน็ต/เว็บไม่ได้ (เครือข่าย, proxy หรือใบรับรอง) — ลองใหม่ภายหลัง".into()
    } else if l.contains("out of memory") || l.contains("memoryerror") || l.contains("killed") {
        "หน่วยความจำไม่พอ งานถูกหยุดกลางทาง — ต้องลดขนาดงาน".into()
    } else if l.contains("syntaxerror") || l.contains("traceback") || l.contains("exception") || l.contains("error:") {
        "โค้ดที่ AI เขียนรันแล้วพัง — AI มักแก้แล้วลองใหม่เอง ดูคำตอบท้ายรอบว่าแก้ได้ไหม".into()
    } else if l.contains("exit") && (l.contains("คำสั่งไม่สำเร็จ") || l.contains("exit code") || l.contains("exit status")) {
        "คำสั่งหนึ่งรันไม่ผ่าน — AI มักลองวิธีอื่นต่อเอง ดูคำตอบท้ายรอบ".into()
    } else {
        return None;
    };
    Some(m)
}

/// A ⚠️ feed line as the owner should read it: plain meaning first, the raw error kept short after it.
pub fn plain_feed_line(text: &str) -> String {
    let Some(rest) = text.strip_prefix("⚠️") else { return text.to_string() };
    let raw = rest.trim();
    match plain_problem(raw) {
        Some(p) if !raw.is_empty() => format!("⚠️ {p} · {}", raw.chars().take(90).collect::<String>()),
        _ => text.to_string(),
    }
}

fn is_tool(t: &str) -> bool {
    // (a 📝 summary is taken before this; "📝 แก้ไฟล์" is a step)
    !(t.starts_with("💬") || t.starts_with("🙋") || t.starts_with("⚠️") || t.starts_with("✅"))
}

fn open_round(c: &Connection, idea_id: i64) -> Option<i64> {
    c.query_row("SELECT id FROM chat_rounds WHERE idea_id=?1 AND done=0 ORDER BY id DESC LIMIT 1", [idea_id], |r| r.get(0)).ok()
}

fn ensure_round(c: &Connection, idea_id: i64, now: i64) -> Option<i64> {
    open_round(c, idea_id).or_else(|| {
        c.execute("INSERT INTO chat_rounds(idea_id,started,updated) VALUES(?1,?2,?2)", params![idea_id, now]).ok()?;
        Some(c.last_insert_rowid())
    })
}

/// One feed line of an idea's chat (any AI) → its round.
pub fn on_feed(c: &Connection, idea_id: i64, text: &str, now: i64) {
    let t = text.trim();
    if t.is_empty() {
        return;
    }
    if t.starts_with("🙋") {
        let _ = c.execute("UPDATE chat_rounds SET done=1 WHERE idea_id=?1 AND done=0", [idea_id]);
        let p = PROMPT_PREFIXES.iter().find(|p| t.starts_with(**p)).map(|p| strip_prefix(t, p)).unwrap_or(t);
        let _ = c.execute("INSERT INTO chat_rounds(idea_id,started,updated,prompt) VALUES(?1,?2,?2,?3)", params![idea_id, now, p]);
        return;
    }
    let Some(rid) = ensure_round(c, idea_id, now) else { return };
    let (answer, says, files, problems): (String, String, String, String) = match c.query_row(
        "SELECT answer,says,files,problems FROM chat_rounds WHERE id=?1",
        [rid],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    ) {
        Ok(x) => x,
        Err(_) => return,
    };
    if let Some(m) = t.strip_prefix("💬") {
        let m = m.trim();
        // the previous last message was a step along the way
        let mut s = arr(&says);
        if !answer.is_empty() && s.last().and_then(|v| v.as_str()) != Some(answer.as_str()) {
            s.push(json!(answer.chars().take(1500).collect::<String>()));
            let n = s.len();
            if n > MAX_SAYS {
                s.drain(..n - MAX_SAYS);
            }
        }
        let a: String = m.chars().take(MAX_ANSWER).collect();
        let _ = c.execute("UPDATE chat_rounds SET answer=?2, says=?3, updated=?4 WHERE id=?1", params![rid, a, Value::Array(s).to_string(), now]);
    } else if let Some(n) = t.strip_prefix("📝 สรุป:").or_else(|| t.strip_prefix("📝 Summary:")) {
        let _ = c.execute("UPDATE chat_rounds SET note=?2, updated=?3 WHERE id=?1", params![rid, n.trim(), now]);
    } else if t.starts_with("⚠️") {
        let raw = t.trim_start_matches("⚠️").trim();
        let plain = plain_problem(raw).unwrap_or_else(|| raw.chars().take(160).collect());
        let mut p = arr(&problems);
        if !p.iter().any(|x| x["text"].as_str() == Some(plain.as_str())) {
            p.push(json!({ "text": plain, "raw": raw.chars().take(300).collect::<String>() }));
        }
        let _ = c.execute("UPDATE chat_rounds SET problems=?2, updated=?3 WHERE id=?1", params![rid, Value::Array(p).to_string(), now]);
    } else if t.starts_with("✅") {
        let _ = c.execute("UPDATE chat_rounds SET done=1, updated=?2 WHERE id=?1", params![rid, now]);
    } else if is_tool(t) {
        // "📝 แก้ไฟล์: X" / "✏️ แก้ไฟล์ X": remember which files changed
        let mut f = arr(&files);
        let file = ["📝 แก้ไฟล์:", "✏️ แก้ไฟล์", "📝 Edited:", "✏️ Edit"].iter().find_map(|p| t.strip_prefix(p)).map(|x| x.trim().to_string());
        if let Some(name) = file.filter(|n| !n.is_empty() && n.len() < 200) {
            if !f.iter().any(|x| x.as_str() == Some(name.as_str())) && f.len() < 40 {
                f.push(json!(name));
            }
        }
        let _ = c.execute("UPDATE chat_rounds SET steps=steps+1, files=?2, updated=?3 WHERE id=?1", params![rid, Value::Array(f).to_string(), now]);
    }
}

/// The newest chat rounds of all ideas, light, for the phone's "ทำล่าสุด" (ISSUE-164): before this the phone listed
/// bot runs only, so a day of chat work never showed. Short text only — the phone keeps little (ISSUE-160).
pub fn recent_light(c: &Connection, n: i64) -> Vec<Value> {
    let Ok(mut q) = c.prepare(
        "SELECT idea_id,started,updated,prompt,answer,note,problems,done FROM chat_rounds
         WHERE prompt<>'' OR answer<>'' OR note<>'' ORDER BY updated DESC, id DESC LIMIT ?1",
    ) else {
        return vec![];
    };
    let cut = |s: &str, n: usize| -> String {
        let t: String = s.lines().filter(|l| !l.trim_start().starts_with("➡️")).collect::<Vec<_>>().join(" ");
        let t = t.replace(['#', '*', '`', '|', '>'], " ").split_whitespace().collect::<Vec<_>>().join(" ");
        if t.chars().count() > n { format!("{}…", t.chars().take(n).collect::<String>()) } else { t }
    };
    q.query_map([n], |r| {
        let answer: String = r.get(4)?;
        let note: String = r.get(5)?;
        Ok(json!({
            "idea_id": r.get::<_, i64>(0)?, "started": r.get::<_, i64>(1)?, "updated": r.get::<_, i64>(2)?,
            "prompt": cut(&r.get::<_, String>(3)?, 160),
            "result": cut(if note.is_empty() { &answer } else { &note }, 300),
            "problems": arr(&r.get::<_, String>(6)?).len(), "done": r.get::<_, i64>(7)? != 0,
        }))
    })
    .map(|it| it.flatten().collect())
    .unwrap_or_default()
}

/// What the owner asked in the newest round ("" when the AI carried on by itself).
pub fn last_prompt(c: &Connection, idea_id: i64) -> String {
    c.query_row("SELECT prompt FROM chat_rounds WHERE idea_id=?1 ORDER BY id DESC LIMIT 1", [idea_id], |r| r.get(0)).unwrap_or_default()
}

/// The prompts the AI suggested next ("➡️ …" lines at the end of its answer).
pub fn next_prompts(answer: &str) -> Vec<String> {
    answer
        .lines()
        .filter_map(|l| l.trim().trim_start_matches(['-', '*', ' ']).strip_prefix("➡️"))
        .map(|l| l.trim().trim_matches('"').trim().to_string())
        .filter(|l| !l.is_empty())
        .take(3)
        .collect()
}

/// Old ideas: rebuild rounds once from CHAT-LOG.md (each line was cut at 600 characters, newlines flattened).
fn import_chat_log(c: &Connection, idea_id: i64, dir: &std::path::Path) {
    let Ok(s) = std::fs::read_to_string(dir.join("CHAT-LOG.md")) else { return };
    // entries: "- [HH:MM] text", later lines of the same message indented by two spaces
    let mut day = String::new();
    let mut entries: Vec<(i64, String)> = vec![];
    for line in s.lines() {
        if let Some(d) = line.strip_prefix("## ") {
            day = d.trim().to_string();
            continue;
        }
        if let (Some(rest), false) = (line.strip_prefix("  "), entries.is_empty()) {
            let last = entries.last_mut().unwrap();
            last.1.push('\n');
            last.1.push_str(rest);
            continue;
        }
        let Some(rest) = line.strip_prefix("- [") else { continue };
        let Some((hm, text)) = rest.split_once("] ") else { continue };
        let ts = chrono::NaiveDateTime::parse_from_str(&format!("{day} {hm}"), "%Y-%m-%d %H:%M")
            .ok()
            .and_then(|d| d.and_local_timezone(chrono::Local).single())
            .map(|d| d.timestamp())
            .unwrap_or(0);
        entries.push((ts, text.to_string()));
    }
    let n = entries.len();
    for (ts, text) in entries {
        on_feed(c, idea_id, &text, ts);
    }
    if n > 0 {
        let _ = c.execute("UPDATE chat_rounds SET done=1, imported=1 WHERE idea_id=?1", [idea_id]);
    }
}

/// Rounds of an idea, newest first, with the suggested next prompts.
pub fn list(c: &Connection, idea_id: i64, dir: &std::path::Path, limit: i64) -> Vec<Value> {
    let none: i64 = c.query_row("SELECT COUNT(*) FROM chat_rounds WHERE idea_id=?1", [idea_id], |r| r.get(0)).unwrap_or(0);
    if none == 0 {
        import_chat_log(c, idea_id, dir);
    }
    let Ok(mut q) = c.prepare(
        "SELECT id,started,updated,prompt,answer,says,steps,files,problems,note,done,imported FROM chat_rounds
         WHERE idea_id=?1 AND (prompt<>'' OR answer<>'' OR note<>'') ORDER BY started DESC, id DESC LIMIT ?2",
    ) else {
        return vec![];
    };
    q.query_map(params![idea_id, limit], |r| {
        let answer: String = r.get(4)?;
        Ok(json!({
            "id": r.get::<_, i64>(0)?, "started": r.get::<_, i64>(1)?, "updated": r.get::<_, i64>(2)?,
            "prompt": r.get::<_, String>(3)?, "next": next_prompts(&answer), "answer": answer,
            "says": arr(&r.get::<_, String>(5)?), "steps": r.get::<_, i64>(6)?, "files": arr(&r.get::<_, String>(7)?),
            "problems": arr(&r.get::<_, String>(8)?), "note": r.get::<_, String>(9)?,
            "done": r.get::<_, i64>(10)? != 0, "imported": r.get::<_, i64>(11)? != 0,
        }))
    })
    .map(|it| it.flatten().collect())
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        migrate(&c);
        c
    }

    #[test]
    fn a_round_keeps_the_prompt_the_whole_answer_and_plain_problems() {
        let c = db();
        let d = std::env::temp_dir();
        on_feed(&c, 7, "🙋 คุณ: เทียบ 3 ตัวให้หน่อย", 100);
        on_feed(&c, 7, "💬 จะอ่านรายงานก่อน", 101);
        on_feed(&c, 7, "🔧 python compare.py", 102);
        on_feed(&c, 7, "⚠️ ModuleNotFoundError: No module named 'fitz'", 103);
        on_feed(&c, 7, "⚠️ ModuleNotFoundError: No module named 'fitz'", 104);
        on_feed(&c, 7, "📝 แก้ไฟล์: REPORT.md", 105);
        on_feed(&c, 7, "💬 ได้ผลแล้ว\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n➡️ ทำ dashboard ให้ดูง่าย\n➡️ \"ลองกับ PDF สแกน\"", 106);
        on_feed(&c, 7, "📝 สรุป: เทียบเสร็จ", 107);
        on_feed(&c, 7, "✅ จบรอบนี้", 108);
        on_feed(&c, 7, "🙋 คุณ: ต่อเลย", 200);
        let r = list(&c, 7, &d, 10);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0]["prompt"], "ต่อเลย");
        let a = &r[1];
        assert_eq!(a["prompt"], "เทียบ 3 ตัวให้หน่อย");
        assert!(a["answer"].as_str().unwrap().contains("| 1 | 2 |"), "answer kept whole, with newlines");
        assert_eq!(a["says"], json!(["จะอ่านรายงานก่อน"]));
        assert_eq!(a["steps"], 2);
        assert_eq!(a["files"], json!(["REPORT.md"]));
        assert_eq!(a["problems"].as_array().unwrap().len(), 1);
        assert!(a["problems"][0]["text"].as_str().unwrap().contains("ขาดไลบรารี Python (fitz)"));
        assert_eq!(a["next"], json!(["ทำ dashboard ให้ดูง่าย", "ลองกับ PDF สแกน"]));
        assert_eq!(a["note"], "เทียบเสร็จ");
        assert!(a["done"].as_bool().unwrap());
    }

    #[test]
    fn the_phone_gets_recent_rounds_short() {
        let c = db();
        on_feed(&c, 4, "🙋 คุณ: ทำ dashboard", 10);
        on_feed(&c, 4, &format!("💬 **เสร็จ** {}\n➡️ ทำต่อ", "ก".repeat(500)), 20);
        on_feed(&c, 9, "🙋 คุณ: สรุปให้หน่อย", 30);
        let r = recent_light(&c, 30);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0]["idea_id"], 9, "newest first");
        let res = r[1]["result"].as_str().unwrap();
        assert!(res.starts_with("เสร็จ") && !res.contains("➡️") && res.chars().count() <= 301);
    }

    #[test]
    fn raw_errors_read_as_what_they_mean() {
        assert!(plain_problem("exit Some(1): Prompt is too long").unwrap().contains("ยาวเกิน"));
        assert!(plain_problem("'node' is not recognized as an internal or external command").unwrap().contains("ไม่มีในเครื่อง"));
        assert!(plain_problem("fatal: Unable to create '.git/index.lock': File exists").unwrap().contains("git ถูกล็อก"));
        assert!(plain_problem("something odd happened").is_none());
        assert!(plain_feed_line("⚠️ HTTP 429 rate limit").starts_with("⚠️ บัญชี AI ชนลิมิต"));
        assert_eq!(plain_feed_line("💬 hi"), "💬 hi");
    }

    #[test]
    fn an_old_chat_log_becomes_rounds_once() {
        let c = db();
        let dir = std::env::temp_dir().join(format!("ia_rounds_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("CHAT-LOG.md"), "# log\n\n## 2026-11-26\n\n- [20:00] 🙋 คุณ: ทดสอบ\n- [20:01] 💬 กำลังทำ\n- [20:05] 💬 เสร็จแล้ว ➡️ ต่อ\n").unwrap();
        let r = list(&c, 3, &dir, 10);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0]["answer"], "เสร็จแล้ว ➡️ ต่อ");
        assert!(r[0]["imported"].as_bool().unwrap());
        assert_eq!(list(&c, 3, &dir, 10).len(), 1, "imported only once");
    }
}
