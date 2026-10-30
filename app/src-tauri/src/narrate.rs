//! Turns Claude's tool calls and messages into short plain-Thai lines for people, with rules
//! only (no tokens): "แก้ไฟล์ app.js" instead of `› Edit D:\...\app.js`. Commands are described
//! by the `description` Claude already writes for them; raw code is never shown.
use serde_json::Value;

fn file_name(input: &Value, key: &str) -> String {
    input
        .get(key)
        .and_then(Value::as_str)
        .map(|p| p.rsplit(['/', '\\']).next().unwrap_or(p).to_string())
        .unwrap_or_default()
}

fn short(s: &str, n: usize) -> String {
    let one: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > n {
        format!("{}…", one.chars().take(n).collect::<String>())
    } else {
        one
    }
}

fn host(url: &str) -> String {
    url.split("//").nth(1).unwrap_or(url).split('/').next().unwrap_or(url).trim_start_matches("www.").to_string()
}

/// One line for a tool call, or None for tools not worth showing.
pub fn tool(name: &str, input: &Value) -> Option<String> {
    let s = |k: &str| input.get(k).and_then(Value::as_str).unwrap_or("");
    let line = match name {
        "Read" => format!("📖 อ่านไฟล์ {}", file_name(input, "file_path")),
        "Edit" | "MultiEdit" => format!("✏️ แก้ไฟล์ {}", file_name(input, "file_path")),
        "Write" => format!("📝 เขียนไฟล์ {}", file_name(input, "file_path")),
        "NotebookEdit" => format!("✏️ แก้ notebook {}", file_name(input, "notebook_path")),
        "Bash" | "PowerShell" => match s("description") {
            "" => "⚙️ รันคำสั่งในเครื่อง".into(),
            d => format!("⚙️ {}", short(d, 120)),
        },
        "Grep" => format!("🔎 ค้นหา “{}” ในโค้ด", short(s("pattern"), 40)),
        "Glob" => "🗂 หาไฟล์ที่เกี่ยวข้อง".into(),
        "WebSearch" => format!("🌐 ค้นเว็บ: {}", short(s("query"), 100)),
        "WebFetch" => format!("🌐 อ่านหน้าเว็บ {}", host(s("url"))),
        "Task" | "Agent" => format!("🧑‍💻 ให้ผู้ช่วยไปทำ: {}", short(s("description"), 80)),
        "TodoWrite" => {
            let now = input.get("todos").and_then(Value::as_array).and_then(|t| {
                t.iter().find(|x| x.get("status").and_then(Value::as_str) == Some("in_progress"))
            });
            match now.and_then(|x| x.get("activeForm").or(x.get("content"))).and_then(Value::as_str) {
                Some(t) => format!("📋 ขั้นตอนตอนนี้: {}", short(t, 100)),
                None => "📋 วางลำดับงาน".into(),
            }
        }
        "ToolSearch" | "TaskOutput" | "BashOutput" | "KillShell" | "Monitor" => return None,
        n if n.starts_with("mcp__") => format!("🔌 ใช้เครื่องมือ {}", n.rsplit("__").next().unwrap_or(n).replace('_', " ")),
        n => format!("🛠 ใช้เครื่องมือ {n}"),
    };
    Some(line)
}

/// Lines for one assistant message's content blocks (text the AI says + its tool calls).
pub fn assistant(parts: &[Value]) -> Vec<String> {
    let mut out = Vec::new();
    for p in parts {
        match p.get("type").and_then(Value::as_str) {
            Some("text") => {
                let t = p.get("text").and_then(Value::as_str).unwrap_or("").trim();
                if !t.is_empty() {
                    out.push(format!("💬 {}", short(t, 400)));
                }
            }
            Some("thinking") => {
                let t = p.get("thinking").and_then(Value::as_str).unwrap_or("").trim();
                if !t.is_empty() {
                    out.push(format!("🤔 {}", short(t, 240)));
                }
            }
            Some("tool_use") => {
                let name = p.get("name").and_then(Value::as_str).unwrap_or("?");
                if let Some(l) = tool(name, p.get("input").unwrap_or(&Value::Null)) {
                    out.push(l);
                }
            }
            _ => {}
        }
    }
    out
}

/// What the owner typed (from the app, the phone or the Claude app). None for tool results and
/// system/meta messages.
pub fn user(v: &Value) -> Option<String> {
    if v.get("isMeta").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let c = v.pointer("/message/content")?;
    let text = match c {
        Value::String(s) => s.clone(),
        Value::Array(a) => a
            .iter()
            .filter(|p| p.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|p| p.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join(" "),
        _ => return None,
    };
    let t = text.trim();
    if t.is_empty() || t.starts_with('<') || t.starts_with("[Request interrupted") {
        return None;
    }
    Some(format!("🙋 คุณ: {}", short(t, 300)))
}

/// A failed tool call shows up as a tool_result with is_error.
pub fn had_error(v: &Value) -> bool {
    v.pointer("/message/content").and_then(Value::as_array).is_some_and(|a| {
        a.iter().any(|p| p.get("type").and_then(Value::as_str) == Some("tool_result") && p.get("is_error").and_then(Value::as_bool) == Some(true))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hides_code() {
        let l = tool("Bash", &json!({ "command": "cd x && npm test", "description": "Run the test suite" })).unwrap();
        assert_eq!(l, "⚙️ Run the test suite");
        assert_eq!(tool("Edit", &json!({ "file_path": "D:\\a\\src\\app.js" })).unwrap(), "✏️ แก้ไฟล์ app.js");
        assert!(tool("ToolSearch", &json!({})).is_none());
    }

    #[test]
    fn owner_text_only() {
        assert_eq!(user(&json!({ "message": { "content": "แก้ปุ่มให้ใหญ่ขึ้น" } })).unwrap(), "🙋 คุณ: แก้ปุ่มให้ใหญ่ขึ้น");
        assert!(user(&json!({ "message": { "content": [{ "type": "tool_result", "content": "ok" }] } })).is_none());
        assert!(user(&json!({ "isMeta": true, "message": { "content": "x" } })).is_none());
    }
}
