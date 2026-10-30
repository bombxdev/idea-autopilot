//! Telegram remote control: long-polls the Bot API (no public URL needed), maps chat
//! messages onto `api::dispatch`, and pushes notifications + new questions to the bound chat.
//! The first chat that messages the bot after a token is set becomes the owner; others are ignored.
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;
use tauri::{AppHandle, Manager};

use crate::api::dispatch;
use crate::db;
use crate::state::AppState;

const HELP: &str = "สั่ง Idea Autopilot จาก Telegram

พิมพ์อะไรก็ได้ = จดไอเดีย (! นำหน้า = ด่วน, ? = ขอแค่ research)
ตอบกลับ (reply) ข้อความของไอเดีย = สั่งงานไอเดียนั้น

/status  สถานะบอทและโควต้า
/list  รายการไอเดีย
/q  คำถามที่รอคุณตอบ
/show <id>  รายละเอียดไอเดีย
/report <id>  อ่านรายงาน
/go /deeper /alt /stop /now <id> [ข้อความ]
/say <id> <คำสั่ง>
/ask <id> <คำสั่ง>  ให้ AI แก้/ทำต่อทันที (session Idea#id)
/pause  /resume  /turbo [ชม.]
/digest  สรุปล่าสุด
/log [วัน]  บอททำอะไรไปบ้าง";

fn setting(app: &AppHandle, k: &str) -> Option<String> {
    db::get_setting(&app.state::<AppState>().db.lock(), k).filter(|v| !v.trim().is_empty())
}

fn set(app: &AppHandle, k: &str, v: &str) {
    let _ = db::set_setting(&app.state::<AppState>().db.lock(), k, v);
}

fn s(v: &Value) -> String {
    v.as_str().map(String::from).unwrap_or_default()
}

fn cut(t: &str, n: usize) -> String {
    if t.chars().count() <= n { t.to_string() } else { format!("{}…", t.chars().take(n).collect::<String>()) }
}

struct Tg {
    http: reqwest::Client,
    token: String,
}

impl Tg {
    async fn call(&self, method: &str, body: Value, timeout: u64) -> Result<Value, String> {
        let r: Value = self
            .http
            .post(format!("https://api.telegram.org/bot{}/{method}", self.token))
            .json(&body)
            .timeout(Duration::from_secs(timeout))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        if r["ok"].as_bool() == Some(true) {
            Ok(r["result"].clone())
        } else {
            Err(s(&r["description"]))
        }
    }

    async fn send(&self, chat: i64, text: &str, buttons: Option<Value>) -> Option<i64> {
        let mut b = json!({ "chat_id": chat, "text": cut(text, 4000), "disable_web_page_preview": true });
        if let Some(k) = buttons {
            b["reply_markup"] = json!({ "inline_keyboard": k });
        }
        self.call("sendMessage", b, 20).await.ok().and_then(|m| m["message_id"].as_i64())
    }
}

/// Fire-and-forget push to the bound chat (used by desktop notifications).
pub fn push(app: &AppHandle, text: String) {
    let (Some(token), Some(chat)) = (setting(app, "telegram_token"), setting(app, "telegram_chat")) else { return };
    let Ok(chat) = chat.parse::<i64>() else { return };
    tauri::async_runtime::spawn(async move {
        let _ = Tg { http: reqwest::Client::new(), token }.send(chat, &text, None).await;
    });
}

pub async fn run_loop(app: AppHandle) {
    let http = reqwest::Client::new();
    let mut offset: i64 = 0;
    let mut cur_token = String::new();
    // message_id → idea id, so a reply to a card steers that idea
    let mut threads: HashMap<i64, i64> = HashMap::new();
    loop {
        let follower = app.state::<AppState>().rt.lock().role == "follower";
        let Some(token) = setting(&app, "telegram_token").filter(|_| !follower) else {
            tokio::time::sleep(Duration::from_secs(30)).await;
            continue;
        };
        if token != cur_token {
            cur_token = token.clone();
            offset = 0;
        }
        let tg = Tg { http: http.clone(), token };
        let chat: Option<i64> = setting(&app, "telegram_chat").and_then(|c| c.parse().ok());
        if let Some(c) = chat {
            push_questions(&app, &tg, c, &mut threads).await;
        }
        let ups = match tg.call("getUpdates", json!({ "offset": offset, "timeout": 25, "allowed_updates": ["message", "callback_query"] }), 40).await {
            Ok(v) => v,
            Err(e) => {
                set(&app, "telegram_status", &format!("error: {e}"));
                tokio::time::sleep(Duration::from_secs(30)).await;
                continue;
            }
        };
        set(&app, "telegram_status", "ok");
        for u in ups.as_array().into_iter().flatten() {
            offset = offset.max(u["update_id"].as_i64().unwrap_or(0) + 1);
            if let Some(cb) = u.get("callback_query") {
                let from = cb["message"]["chat"]["id"].as_i64();
                if from.is_some() && from == chat {
                    let reply = on_button(&app, &s(&cb["data"]));
                    let _ = tg.call("answerCallbackQuery", json!({ "callback_query_id": cb["id"], "text": cut(&reply, 190) }), 10).await;
                    if let Some(c) = from {
                        tg.send(c, &reply, None).await;
                    }
                }
                continue;
            }
            let m = &u["message"];
            let Some(from) = m["chat"]["id"].as_i64() else { continue };
            let text = s(&m["text"]);
            let owner = match chat {
                Some(c) => c,
                None => {
                    // first chat to talk to a fresh bot becomes its owner
                    set(&app, "telegram_chat", &from.to_string());
                    let name = [s(&m["chat"]["first_name"]), s(&m["chat"]["username"])].join(" ");
                    set(&app, "telegram_owner", name.trim());
                    tg.send(from, &format!("ผูกกับ Idea Autopilot แล้ว ✓\n\n{HELP}"), None).await;
                    if text.starts_with('/') {
                        continue;
                    }
                    from
                }
            };
            if from != owner {
                continue;
            }
            let reply_to = m["reply_to_message"]["message_id"].as_i64().and_then(|r| threads.get(&r).copied());
            let (out, buttons, idea) = on_text(&app, &text, reply_to);
            if let Some(mid) = tg.send(owner, &out, buttons).await {
                if let Some(i) = idea {
                    threads.insert(mid, i);
                }
            }
        }
    }
}

/// Send each open question not yet pushed, with one button per option.
async fn push_questions(app: &AppHandle, tg: &Tg, chat: i64, threads: &mut HashMap<i64, i64>) {
    let last: i64 = setting(app, "telegram_last_q").and_then(|v| v.parse().ok()).unwrap_or(0);
    let qs = db::questions(&app.state::<AppState>().db.lock(), None, true).unwrap_or_default();
    // questions are off unless switched on in Settings: Telegram then only gets status and progress
    if setting(app, "telegram_questions").as_deref() != Some("1") {
        if let Some(max) = qs.iter().map(|q| q.id).max().filter(|m| *m > last) {
            set(app, "telegram_last_q", &max.to_string());
        }
        return;
    }
    for q in qs.into_iter().filter(|q| q.id > last) {
        let title = db::get_idea(&app.state::<AppState>().db.lock(), q.idea_id).map(|i| i.title).unwrap_or_default();
        let guess = q.guess.as_deref().map(|g| format!("\n(ใช้ “{g}” ไปก่อน งานไม่หยุดรอ)")).unwrap_or_default();
        let buttons: Vec<Value> = q
            .options
            .iter()
            .enumerate()
            .map(|(n, o)| json!([{ "text": cut(o, 60), "callback_data": format!("a:{}:{}", q.id, n + 1) }]))
            .collect();
        let msg = format!("🙋 #{} {}\n{}{guess}", q.idea_id, title, q.question);
        if let Some(mid) = tg.send(chat, &msg, Some(json!(buttons))).await {
            threads.insert(mid, q.idea_id);
        }
        set(app, "telegram_last_q", &q.id.to_string());
    }
}

fn on_button(app: &AppHandle, data: &str) -> String {
    let p: Vec<&str> = data.split(':').collect();
    let r = match p.as_slice() {
        ["a", qid, n] => dispatch(app, "answer", json!({ "id": qid.parse::<i64>().unwrap_or(0), "answer": n })).map(|_| "รับคำตอบแล้ว ✓".to_string()),
        ["x", id, action] => dispatch(app, "action", json!({ "id": id.parse::<i64>().unwrap_or(0), "action": action })).map(|_| format!("สั่ง {action} แล้ว ✓")),
        _ => Err("ปุ่มนี้หมดอายุแล้ว".into()),
    };
    r.unwrap_or_else(|e| format!("ผิดพลาด: {e}"))
}

fn idea_buttons(id: i64) -> Value {
    json!([[
        { "text": "ไปต่อ", "callback_data": format!("x:{id}:go") },
        { "text": "ขุดลึก", "callback_data": format!("x:{id}:deeper") },
        { "text": "อีกแนว", "callback_data": format!("x:{id}:alt") },
        { "text": "พอแค่นี้", "callback_data": format!("x:{id}:stop") },
    ]])
}

/// Returns (reply text, inline buttons, idea the reply is about).
fn on_text(app: &AppHandle, text: &str, reply_to: Option<i64>) -> (String, Option<Value>, Option<i64>) {
    let text = text.trim();
    if text.is_empty() {
        return ("ส่งเป็นข้อความนะครับ".into(), None, None);
    }
    if !text.starts_with('/') {
        if let Some(id) = reply_to {
            return match dispatch(app, "action", json!({ "id": id, "action": "say", "text": text })) {
                Ok(_) => (format!("สั่งงาน #{id} แล้ว ✓"), None, Some(id)),
                Err(e) => (format!("ผิดพลาด: {e}"), None, None),
            };
        }
        return match dispatch(app, "add", json!({ "text": text })) {
            Ok(v) if v.as_i64() == Some(0) => ("ส่งไปเครื่องหลักแล้ว จะเข้าคิวรอบ sync ถัดไป".into(), None, None),
            Ok(v) => (format!("จดแล้ว #{v} ✓ AI จะเริ่มคิดเมื่อมีโควต้าว่าง"), None, v.as_i64()),
            Err(e) => (format!("ผิดพลาด: {e}"), None, None),
        };
    }
    let mut parts = text.splitn(3, char::is_whitespace);
    // "/status@MyBot" → "status"
    let cmd = parts.next().unwrap_or("").trim_start_matches('/').split('@').next().unwrap_or("").to_lowercase();
    let a1 = parts.next().map(str::trim).filter(|x| !x.is_empty());
    let a2 = parts.next().map(str::trim).filter(|x| !x.is_empty());
    let num = a1.and_then(|x| x.trim_start_matches(['#', 'q', 'Q']).parse::<i64>().ok());
    let need_id = || num.ok_or_else(|| "ต้องระบุเลขไอเดีย เช่น /show 3".to_string());
    let done = |r: Result<Value, String>, msg: &str| r.map(|_| msg.to_string());

    let r: Result<(String, Option<Value>, Option<i64>), String> = (|| match cmd.as_str() {
        "start" | "help" => Ok((HELP.into(), None, None)),
        "status" | "st" => dispatch(app, "status", Value::Null).map(|v| (fmt_status(&v), None, None)),
        "list" | "ls" => dispatch(app, "ideas", Value::Null).map(|v| (fmt_list(&v), None, None)),
        "q" | "questions" => {
            // re-push every open question as a card
            set(app, "telegram_last_q", "0");
            app.state::<AppState>().wake.notify_one();
            let n = db::questions(&app.state::<AppState>().db.lock(), None, true).map(|q| q.len()).unwrap_or(0);
            Ok((if n == 0 { "ไม่มีคำถามรอคุณ".into() } else { format!("มี {n} คำถาม กำลังส่งให้…") }, None, None))
        }
        "show" => {
            let id = need_id()?;
            dispatch(app, "idea", json!({ "id": id })).map(|v| (fmt_idea(&v), Some(idea_buttons(id)), Some(id)))
        }
        "report" => {
            let id = need_id()?;
            let v = dispatch(app, "idea", json!({ "id": id }))?;
            let t = v["files"]["report"].as_str().or(v["files"]["knowledge"].as_str()).unwrap_or("ยังไม่มีรายงาน");
            Ok((t.to_string(), Some(idea_buttons(id)), Some(id)))
        }
        "go" | "deeper" | "alt" | "stop" | "now" | "say" | "urgent" | "retry" | "archive" if num.is_some() || cmd != "stop" => {
            let id = need_id()?;
            let action = if cmd == "now" { "run_now" } else { cmd.as_str() };
            done(dispatch(app, "action", json!({ "id": id, "action": action, "text": a2 })), &format!("สั่ง {cmd} #{id} แล้ว ✓")).map(|m| (m, None, Some(id)))
        }
        "ask" => {
            let id = need_id()?;
            let t = a2.clone().filter(|t| !t.trim().is_empty()).ok_or("ต้องมีข้อความ เช่น /ask 14 ทำปุ่มให้ใหญ่ขึ้น")?;
            done(dispatch(app, "ask", json!({ "id": id, "text": t })), &format!("ส่งให้ AI ทำ #{id} แล้ว ✓ ดูความคืบหน้าในแอป")).map(|m| (m, None, Some(id)))
        }
        "stop" => done(dispatch(app, "stop", Value::Null), "หยุดงานที่ทำอยู่แล้ว").map(|m| (m, None, None)),
        "pause" => done(dispatch(app, "pause", Value::Null), "พักบอทแล้ว ⏸").map(|m| (m, None, None)),
        "resume" => done(dispatch(app, "resume", Value::Null), "บอททำงานต่อแล้ว ▶").map(|m| (m, None, None)),
        "turbo" => {
            let h = a1.and_then(|x| x.parse::<f64>().ok()).unwrap_or(2.0);
            done(dispatch(app, "turbo", json!({ "hours": h })), &format!("Turbo {h} ชม. ✓")).map(|m| (m, None, None))
        }
        "digest" => dispatch(app, "digest", Value::Null).map(|v| (fmt_digest(&v), None, None)),
        "log" => dispatch(app, "activity", json!({ "days": num.unwrap_or(1) })).map(|v| (fmt_log(&v), None, None)),
        _ => Err(format!("ไม่รู้จักคำสั่ง /{cmd} — พิมพ์ /help")),
    })();
    r.unwrap_or_else(|e| (format!("ผิดพลาด: {e}"), None, None))
}

fn pct(v: &Value) -> String {
    v.as_f64().map(|x| format!("{x:.0}%")).unwrap_or("—".into())
}

fn fmt_status(v: &Value) -> String {
    let d = &v["decision"];
    let state = if v["paused"].as_bool() == Some(true) { "⏸ พักอยู่" } else if d["allow"].as_bool() == Some(true) { "▶ ทำงานได้" } else { "⏳ รออยู่" };
    let mut o = format!(
        "{state} — {}\nโควต้าสัปดาห์ {} (กันให้งาน {}, บอทใช้ได้อีก {})\n5 ชม.: {} / เพดาน {}",
        s(&d["reason"]), pct(&d["week_pct"]), pct(&d["reserve"]), pct(&d["bot_budget"]), pct(&d["five_pct"]), pct(&d["five_cap"])
    );
    if let Some(c) = v["current"].as_object() {
        o += &format!("\nกำลังทำ: #{} {} — {}", c["idea_id"], s(&c["title"]), s(&c["stage"]));
    }
    let q = v["open_questions"].as_u64().unwrap_or(0);
    if q > 0 {
        o += &format!("\n🙋 คำถามรอคุณ {q} ข้อ (/q)");
    }
    o
}

fn fmt_list(v: &Value) -> String {
    let mut o = String::new();
    for i in v["ideas"].as_array().into_iter().flatten() {
        let st = s(&i["status"]);
        if st == "archived" || st == "deleted" {
            continue;
        }
        let label = match st.as_str() {
            "inbox" => "รอตีความ",
            "active" => "กำลังทำ",
            "ready" => "พร้อมใช้ ▶",
            "done" => "เสร็จ",
            "failed" => "ติดปัญหา",
            x => x,
        };
        let urgent = if i["priority"].as_i64().unwrap_or(0) > 0 { "⚡" } else { "" };
        o += &format!("#{} {urgent}{} — {label}\n", i["id"], cut(&s(&i["title"]), 60));
    }
    if o.is_empty() { "ยังไม่มีไอเดีย พิมพ์มาได้เลย".into() } else { o }
}

fn fmt_idea(v: &Value) -> String {
    let i = &v["idea"];
    let mut o = format!("#{} {} — {}", i["id"], s(&i["title"]), s(&i["status"]));
    if let Some(x) = i["verdict"].as_str() {
        o += &format!("\nคำตัดสิน: {x}");
    }
    let recipe: Vec<String> = i["recipe"].as_array().into_iter().flatten().map(s).collect();
    let idx = i["stage_idx"].as_u64().unwrap_or(0) as usize;
    let steps: Vec<String> = recipe.iter().enumerate().map(|(n, r)| if n < idx { format!("✓{r}") } else if n == idx { format!("[{r}]") } else { r.clone() }).collect();
    o += &format!("\nขั้นตอน: {}\n\n{}", steps.join(" → "), s(&i["summary"]));
    for n in i["needs"].as_array().into_iter().flatten() {
        o += &format!("\n• ต้องการจากคุณ: {}", s(n));
    }
    o + "\n\nตอบกลับข้อความนี้เพื่อสั่งงานไอเดียนี้"
}

fn fmt_digest(v: &Value) -> String {
    let b = &v["body"];
    let mut o = s(&b["headline"]);
    for (k, label) in [("ready", "▶ พร้อมลองเล่น"), ("finished", "✓ เสร็จ"), ("progressed", "… คืบหน้า")] {
        for i in b[k].as_array().into_iter().flatten() {
            o += &format!("\n{label} #{} {} — {}", i["id"], s(&i["title"]), cut(&s(&i["summary"]), 140));
        }
    }
    o
}

fn fmt_log(v: &Value) -> String {
    let st = &v["stats"];
    let mut o = format!(
        "รอบงาน {} (สำเร็จ {}, ไม่สำเร็จ {}) · {} นาที · โควต้า {:.1}%",
        st["runs"], st["ok"], st["failed"], st["busy_secs"].as_i64().unwrap_or(0) / 60, st["pct"].as_f64().unwrap_or(0.0)
    );
    let runs: Vec<Value> = v["runs"].as_array().cloned().unwrap_or_default();
    for r in runs.iter().rev().take(15) {
        let t = r["started"].as_i64().and_then(|t| chrono::DateTime::from_timestamp(t, 0)).map(|d| d.with_timezone(&chrono::Local).format("%H:%M").to_string()).unwrap_or_default();
        o += &format!("\n{t} #{} {} · {} → {}", r["idea_id"], cut(&s(&r["title"]), 40), s(&r["stage"]), s(&r["status"]));
    }
    o
}
