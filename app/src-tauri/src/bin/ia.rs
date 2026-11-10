//! `ia` — command-line control for Idea Autopilot (talks to the running app on 127.0.0.1).
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::Duration;

const HELP: &str = r#"ia — สั่ง Idea Autopilot จาก command line

  ia status                     สถานะบอท โควต้า และงานที่ทำอยู่
  ia add "ข้อความ"              จดไอเดีย (! นำหน้า = ด่วน, ? = ขอแค่ research)
  ia list [--all]               รายการไอเดีย
  ia show <id>                  รายละเอียด + คำถาม + สิ่งที่ต้องทำเอง
  ia report <id> [knowledge|plan|progress|run]   อ่านเอกสารของไอเดีย (ค่าเริ่มต้น: report)
  ia questions                  คำถามที่รอคุณตอบ
  ia answer <qid> <คำตอบ|เลขตัวเลือก>
  ia go|deeper|alt|stop|urgent|archive|restore|retry|now <id> [ข้อความเพิ่ม]
  ia say <id> "คำสั่ง"           สั่งงานสั้นๆ ให้ไอเดียนั้น
  ia play <id>                  เปิดโปรแกรมที่สร้างเสร็จ
  ia pause | resume | stop      พักบอท / ทำต่อ / หยุดงานที่ทำอยู่
  ia turbo [ชั่วโมง]             ใช้โควต้าเต็มที่ชั่วคราว (0 = ปิด)
  ia digest                     สรุปล่าสุด
  ia day <YYYY-MM-DD> off|work|half|clear [หมายเหตุ]   ตั้งวันหยุด/วันทำงาน
  ia ui [morning|board|quota|library|settings] [id]  เปิดหน้าต่างไปหน้านั้น
  ia log [วัน]                  บันทึกย้อนหลัง: ทำอะไร ไอเดียไหน กี่โมง ผลเป็นยังไง (ค่าเริ่มต้น 1 วัน)
  ia runlog <run id>            ดู log ของรอบงานนั้น
  ia brain                      export สมองพกพา (ส่วนตัว / งาน) ตอนนี้เลย
  ia remote <id> [off]          เปิด/ปิด session "Idea#<id> : ชื่อไอเดีย" ในแอป Claude (คุยต่อจากมือถือ)
  ia handoff <id> <agent> "ข้อความ"  ส่งงานให้ AI อื่นผ่าน Orca (codex, gemini, …) · ia agents = ดูว่ามีตัวไหนบ้าง
  ia ask <id> "ข้อความ"         สั่ง AI แก้/ทำต่อในไอเดียนั้น (ส่งเข้า session เดิม ถ้าไม่มีจะเปิดให้)
  ia hold <id> [off]            บอกบอทว่ากำลังคุยไอเดียนี้อยู่ (บอทจะเว้นไว้ 3 ชม.) / ปล่อยคืน
  ia note <id> "สรุป"            บันทึกความคืบหน้าลงไอเดีย (แจ้งเจ้าของทาง Telegram/แอป)
  ia reqs <id>                  คำสั่งของเจ้าของในไอเดียนั้น (รายการที่ต้องทำให้ครบ) และสถานะ
  ia req <เลข> done|no "รายละเอียด"  ทำคำสั่งนั้นเสร็จแล้ว (บอกว่าทำอะไร ตรวจยังไง) / ทำตามที่ขอไม่ได้ (บอกเหตุผล)
  ia models                     โมเดลที่ระบบเลือกแต่ละขั้น + สถิติที่ใช้ตัดสิน + เวอร์ชันโมเดลล่าสุด
  ia sync                       สถานะ sync หลายเครื่อง
  ia set <key> <value>          ตั้งค่า เช่น ia set sync_dir "D:\my-sync-repo" (key: sync_dir, brain_dir, extra_inbox,
                                reserve_per_hour, model_default, autostart)
  ia takeover                   ย้ายเครื่องหลักมาที่เครื่องนี้

  เพิ่ม --json ท้ายคำสั่งเพื่อดูผลแบบ JSON"#;

fn data_dir() -> PathBuf {
    if let Ok(p) = std::env::var("IDEA_AUTOPILOT_HOME") {
        return PathBuf::from(p).join("data");
    }
    if let Ok(exe) = std::env::current_exe() {
        for anc in exe.ancestors().skip(1) {
            if anc.join("data").join("api.json").exists() {
                return anc.join("data");
            }
        }
    }
    dirs::home_dir().unwrap_or_default().join("IdeaAutopilot").join("data")
}

fn call(cmd: &str, args: Value) -> Result<Value, String> {
    let cfg: Value = std::fs::read_to_string(data_dir().join("api.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .ok_or("ไม่พบ data/api.json — เปิด Idea Autopilot อย่างน้อยหนึ่งครั้งก่อน")?;
    let port = cfg["port"].as_u64().unwrap_or(47821);
    let token = cfg["token"].as_str().unwrap_or("");
    let body = json!({ "cmd": cmd, "args": args }).to_string();
    let mut s = TcpStream::connect(("127.0.0.1", port as u16)).map_err(|_| "Idea Autopilot ไม่ได้เปิดอยู่ (ต่อพอร์ตไม่ได้)")?;
    s.set_read_timeout(Some(Duration::from_secs(30))).ok();
    write!(
        s,
        "POST /cmd HTTP/1.1\r\nHost: 127.0.0.1\r\nX-Token: {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).map_err(|e| e.to_string())?;
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4).unwrap_or(raw.len());
    let head = String::from_utf8_lossy(&raw[..split]).to_ascii_lowercase();
    // big replies come chunked (hex size, CRLF, data, CRLF ... 0): join on bytes, a Thai char can span chunks
    let body = if head.contains("transfer-encoding: chunked") { dechunk(&raw[split..]) } else { raw[split..].to_vec() };
    let json_part = String::from_utf8_lossy(&body);
    let v: Value = serde_json::from_str(json_part.trim()).map_err(|_| format!("ตอบกลับผิดรูปแบบ: {}", json_part.chars().take(300).collect::<String>()))?;
    if let Some(e) = v.get("error") {
        return Err(e.as_str().unwrap_or("error").to_string());
    }
    Ok(v["ok"].clone())
}

fn dechunk(mut rest: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    while let Some(eol) = rest.windows(2).position(|w| w == b"\r\n") {
        let n = usize::from_str_radix(String::from_utf8_lossy(&rest[..eol]).trim(), 16).unwrap_or(0);
        let data = &rest[eol + 2..];
        if n == 0 || data.len() < n {
            break;
        }
        out.extend_from_slice(&data[..n]);
        rest = data[n..].strip_prefix(b"\r\n").unwrap_or(&data[n..]);
    }
    out
}

fn s(v: &Value) -> String {
    v.as_str().map(String::from).unwrap_or_default()
}

fn pct(v: &Value) -> String {
    v.as_f64().map(|x| format!("{x:.0}%")).unwrap_or("—".into())
}

fn local(ts: &Value) -> String {
    ts.as_i64()
        .and_then(|t| chrono::DateTime::from_timestamp(t, 0))
        .map(|d| d.with_timezone(&chrono::Local).format("%a %d/%m %H:%M").to_string())
        .unwrap_or("—".into())
}

fn status_label(st: &str) -> &str {
    match st {
        "inbox" => "รอตีความ",
        "active" => "กำลังทำ",
        "ready" => "พร้อมใช้ ▶",
        "done" => "เสร็จแล้ว",
        "failed" => "ติดปัญหา",
        "archived" => "เก็บแล้ว",
        x => x,
    }
}

fn print_status(v: &Value) {
    let d = &v["decision"];
    println!("บอท: {}", if v["paused"].as_bool() == Some(true) { "พักอยู่ (pause)" } else if d["allow"].as_bool() == Some(true) { "ทำงานได้" } else { "รออยู่" });
    println!("เหตุผล: {}", s(&d["reason"]));
    println!(
        "โควต้าสัปดาห์: {} (กันให้งาน {}, บอทใช้ได้อีก {}) · 5 ชม.: {} / เพดาน {} · รีเซ็ต {}",
        pct(&d["week_pct"]),
        pct(&d["reserve"]),
        pct(&d["bot_budget"]),
        pct(&d["five_pct"]),
        pct(&d["five_cap"]),
        local(&v["snap"]["week_reset"])
    );
    if let Some(c) = v["current"].as_object() {
        println!("กำลังทำ: #{} {} — ขั้น {}", c["idea_id"], s(&c["title"]), s(&c["stage"]));
        for l in v["live_log"].as_array().into_iter().flatten().rev().take(5).collect::<Vec<_>>().into_iter().rev() {
            println!("   {}", s(l).chars().take(140).collect::<String>());
        }
    } else {
        println!("กำลังทำ: —");
    }
    let q = v["open_questions"].as_u64().unwrap_or(0);
    if q > 0 {
        println!("คำถามรอคุณ: {q} ข้อ (ia questions)");
    }
}

fn print_list(v: &Value, all: bool) {
    let qs = v["questions"].as_array().cloned().unwrap_or_default();
    for i in v["ideas"].as_array().into_iter().flatten() {
        let st = s(&i["status"]);
        if !all && (st == "archived" || st == "deleted") {
            continue;
        }
        let nq = qs.iter().filter(|q| q["idea_id"] == i["id"]).count();
        let verdict = i["verdict"].as_str().map(|x| format!(" [{x}]")).unwrap_or_default();
        let urgent = if i["priority"].as_i64().unwrap_or(0) > 0 { "⚡" } else { "" };
        println!(
            "#{:<3} {urgent}{}{verdict} — {}{}",
            i["id"],
            s(&i["title"]),
            status_label(&st),
            if nq > 0 { format!(" · 🙋 {nq} คำถาม") } else { String::new() }
        );
        let sum = s(&i["summary"]);
        if !sum.is_empty() {
            println!("     {}", sum.chars().take(160).collect::<String>());
        }
    }
}

fn print_questions(qs: &[Value], titles: &Value) {
    if qs.is_empty() {
        println!("ไม่มีคำถามรอคุณ");
    }
    for q in qs {
        let title = titles["ideas"]
            .as_array()
            .and_then(|a| a.iter().find(|i| i["id"] == q["idea_id"]))
            .map(|i| s(&i["title"]))
            .unwrap_or_default();
        println!("Q{} [#{} {}] {}", q["id"], q["idea_id"], title, s(&q["question"]));
        for (n, o) in q["options"].as_array().into_iter().flatten().enumerate() {
            let mark = if o == &q["guess"] { "  ← ใช้ไปก่อน" } else { "" };
            println!("   {}. {}{mark}", n + 1, s(o));
        }
    }
}

fn run() -> Result<(), String> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let as_json = args.iter().any(|a| a == "--json");
    let all = args.iter().any(|a| a == "--all");
    args.retain(|a| a != "--json" && a != "--all");
    let cmd = args.first().cloned().unwrap_or_default();
    let arg = |n: usize| args.get(n).cloned();
    let rest = |n: usize| if args.len() > n { Some(args[n..].join(" ")) } else { None };
    let num = |n: usize| arg(n).and_then(|x| x.trim_start_matches(['#', 'Q', 'q']).parse::<i64>().ok()).ok_or("ต้องระบุเลข id");

    let out = match cmd.as_str() {
        "" | "help" | "-h" | "--help" => {
            println!("{HELP}");
            return Ok(());
        }
        "status" | "st" => call("status", Value::Null)?,
        "add" | "a" => call("add", json!({ "text": rest(1).ok_or("ต้องมีข้อความไอเดีย")? }))?,
        "list" | "ls" => call("ideas", Value::Null)?,
        "show" | "report" => call("idea", json!({ "id": num(1)? }))?,
        "questions" | "q" => call("ideas", Value::Null)?,
        "answer" => call("answer", json!({ "id": num(1)?, "answer": rest(2).ok_or("ต้องมีคำตอบ")? }))?,
        "go" | "deeper" | "alt" | "stop-idea" | "urgent" | "archive" | "restore" | "retry" | "now" | "say" => {
            let action = match cmd.as_str() {
                "now" => "run_now",
                "stop-idea" => "stop",
                x => x,
            };
            call("action", json!({ "id": num(1)?, "action": action, "text": rest(2) }))?
        }
        "play" => call("run", json!({ "id": num(1)? }))?,
        "pause" => call("pause", Value::Null)?,
        "resume" => call("resume", Value::Null)?,
        "stop" => match arg(1).and_then(|x| x.parse::<i64>().ok()) {
            Some(id) => call("action", json!({ "id": id, "action": "stop" }))?,
            None => call("stop", Value::Null)?,
        },
        "turbo" => call("turbo", json!({ "hours": arg(1).and_then(|x| x.parse::<f64>().ok()).unwrap_or(2.0) }))?,
        "digest" => call("digest", Value::Null)?,
        "day" => {
            let kind = arg(2).ok_or("ต้องระบุ off|work|half|clear")?;
            call("set_day", json!({ "date": arg(1).ok_or("ต้องระบุวันที่")?, "kind": if kind == "clear" { Value::Null } else { json!(kind) }, "note": rest(3) }))?
        }
        "brain" => call("brain", Value::Null)?,
        "log" => call("activity", json!({ "days": arg(1).and_then(|x| x.parse::<i64>().ok()).unwrap_or(1) }))?,
        "runlog" => call("runlog", json!({ "id": num(1)? }))?,
        "sync" => call("sync", Value::Null)?,
        "models" => call("models", Value::Null)?,
        "remote" => call("remote", json!({ "id": num(1)?, "on": arg(2).map(|x| x != "off").unwrap_or(true) }))?,
        "handoff" => call("handoff", json!({ "id": num(1)?, "agent": arg(2).ok_or("ต้องระบุ agent เช่น codex")?, "text": rest(3).ok_or("ต้องมีข้อความคำสั่ง")? }))?,
        "agents" => call("agents", Value::Null)?,
        "ask" => call("ask", json!({ "id": num(1)?, "text": rest(2).ok_or("ต้องมีข้อความคำสั่ง")? }))?,
        "hold" => call("hold", json!({ "id": num(1)?, "on": arg(2).map(|x| x != "off").unwrap_or(true) }))?,
        "note" => call("note", json!({ "id": num(1)?, "text": rest(2).ok_or("ต้องมีข้อความสรุป")? }))?,
        "reqs" => call("reqs", json!({ "id": num(1)? }))?,
        "req" => call("req", json!({ "rid": num(1)?, "state": arg(2).ok_or("ต้องระบุ done หรือ no")?, "note": rest(3).unwrap_or_default() }))?,
        "set" => call("set", json!({ "key": arg(1).ok_or("ต้องมี key")?, "value": rest(2).unwrap_or_default() }))?,
        "takeover" => call("takeover", Value::Null)?,
        "ui" => call("ui", json!({ "page": arg(1), "id": arg(2).and_then(|x| x.parse::<i64>().ok()) }))?,
        other => return Err(format!("ไม่รู้จักคำสั่ง “{other}” — ดู ia help")),
    };

    if as_json {
        println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
        return Ok(());
    }
    match cmd.as_str() {
        "status" | "st" => print_status(&out),
        "list" | "ls" => print_list(&out, all),
        "questions" | "q" => {
            let qs = out["questions"].as_array().cloned().unwrap_or_default();
            print_questions(&qs, &out);
        }
        "add" | "a" if out.as_i64() == Some(0) => println!("ส่งไปเครื่องหลักแล้ว — จะเข้าคิวในรอบ sync ถัดไป"),
        "add" | "a" => println!("จดแล้ว #{} — AI จะเริ่มคิดเมื่อมีโควต้าว่าง", out),
        "brain" => println!("export สมองแล้วที่ {}", s(&out)),
        "models" => {
            let v: Vec<String> = out["versions"].as_object().into_iter().flatten().map(|(k, v)| format!("{k}={}", s(v))).collect();
            println!("เวอร์ชันล่าสุดที่เห็น: {}", if v.is_empty() { "—".into() } else { v.join(", ") });
            for (stage, p) in out["stages"].as_object().into_iter().flatten() {
                let st: Vec<String> = p["stats"].as_object().into_iter().flatten()
                    .map(|(t, x)| format!("{t} {}/{}", x["good"], x["runs"])).collect();
                println!("{stage:<10} ตาราง {:<6} → ใช้ {:<6} [{}] {}", s(&p["base"]), s(&p["chosen"]), st.join(", "), s(&p["note"]));
            }
        }
        "runlog" => {
            for l in out.as_array().into_iter().flatten() {
                println!("{}", s(l));
            }
        }
        "log" => {
            let stt = &out["stats"];
            println!(
                "รอบงาน {} (สำเร็จ {}, ไม่สำเร็จ {}) · บอททำงานรวม {} นาที · โควต้า {:.1}% · {:.1}M token",
                stt["runs"], stt["ok"], stt["failed"],
                stt["busy_secs"].as_i64().unwrap_or(0) / 60,
                stt["pct"].as_f64().unwrap_or(0.0),
                stt["tokens"].as_f64().unwrap_or(0.0) / 1e6
            );
            let mut runs: Vec<Value> = out["runs"].as_array().cloned().unwrap_or_default();
            runs.reverse();
            for r in &runs {
                println!(
                    "{}  {:>3} นาที  #{} {} · {} · {}·{} → {}{}",
                    local(&r["started"]),
                    r["secs"].as_i64().unwrap_or(0) / 60,
                    r["idea_id"], s(&r["title"]), s(&r["stage"]), s(&r["model"]),
                    r["effort"].as_str().unwrap_or("-"), s(&r["status"]),
                    r["summary"].as_str().map(|x| format!(" — {}", x.chars().take(90).collect::<String>())).unwrap_or_default()
                );
            }
            if let Some(w) = out["waits"].as_object().filter(|w| !w.is_empty()) {
                let parts: Vec<String> = w.iter().map(|(k, v)| format!("{k} {} นาที", v)).collect();
                println!("เวลาตามสถานะบอท: {}", parts.join(" · "));
            }
            for e in out["eta"].as_array().into_iter().flatten() {
                println!("คิว #{} {} — เหลือ {} ขั้น ~{} นาทีงานบอท", e["id"], s(&e["title"]), e["stages_left"], e["secs"].as_i64().unwrap_or(0) / 60);
            }
        }
        "sync" => println!(
            "โหมด: {} · เครื่องนี้: {} · เครื่องหลัก: {} · heartbeat {} · โฟลเดอร์: {}{}",
            s(&out["mode"]),
            s(&out["machine"]),
            out["leader"].as_str().unwrap_or("—"),
            local(&out["heartbeat"]),
            out["sync_dir"].as_str().unwrap_or("(ยังไม่ตั้ง)"),
            if out["git"].as_bool() == Some(true) { " (git)" } else { "" }
        ),
        "show" => {
            let i = &out["idea"];
            println!("#{} {} — {}", i["id"], s(&i["title"]), status_label(&s(&i["status"])));
            if let Some(v) = i["verdict"].as_str() {
                println!("คำตัดสิน: {v}");
            }
            let intents: Vec<String> = i["intents"].as_array().into_iter().flatten().map(s).collect();
            println!("เป้าหมาย: {}", intents.join(", "));
            let recipe: Vec<String> = i["recipe"].as_array().into_iter().flatten().map(s).collect();
            let idx = i["stage_idx"].as_u64().unwrap_or(0) as usize;
            let steps: Vec<String> = recipe
                .iter()
                .enumerate()
                .map(|(n, r)| if n < idx { format!("✓{r}") } else if n == idx { format!("[{r}]") } else { r.clone() })
                .collect();
            println!("ขั้นตอน: {}", steps.join(" → "));
            println!("สรุป: {}", s(&i["summary"]));
            for n in i["needs"].as_array().into_iter().flatten() {
                println!("ต้องการจากคุณ: {}", s(n));
            }
            let open: Vec<Value> = out["questions"].as_array().into_iter().flatten().filter(|q| q["answer"].is_null()).cloned().collect();
            if !open.is_empty() {
                println!();
                print_questions(&open, &json!({ "ideas": [i] }));
            }
            println!("\nโฟลเดอร์: {}", s(&out["dir"]));
        }
        "report" => {
            let key = match arg(2).as_deref() {
                Some("knowledge") => "knowledge",
                Some("plan") => "plan",
                Some("progress") => "progress",
                Some("run") => "how_to_run",
                Some("idea") => "idea",
                _ => "report",
            };
            match out["files"][key].as_str() {
                Some(t) => println!("{t}"),
                None => println!("ยังไม่มีเอกสารนี้"),
            }
        }
        "digest" => {
            let b = &out["body"];
            println!("{}", s(&b["headline"]));
            for (k, label) in [("ready", "พร้อมลองเล่น"), ("finished", "เสร็จ"), ("progressed", "คืบหน้า")] {
                for i in b[k].as_array().into_iter().flatten() {
                    println!("  [{label}] #{} {} — {}", i["id"], s(&i["title"]), s(&i["summary"]));
                }
            }
        }
        "reqs" => {
            let v = out.as_array().cloned().unwrap_or_default();
            if v.is_empty() {
                println!("ยังไม่มีคำสั่งที่ต้องติ๊ก");
            }
            for r in v {
                let mark = match r["state"].as_str().unwrap_or("") {
                    "done" => "[~] AI ว่าเสร็จ รอเจ้าของยืนยัน",
                    "ok" => "[x] เจ้าของยืนยันแล้ว",
                    "no" => "[!] ทำไม่ได้",
                    _ => "[ ] รอทำ",
                };
                let note = r["note"].as_str().filter(|n| !n.is_empty()).map(|n| format!(" — {n}")).unwrap_or_default();
                println!("#{} {mark}: {}{note}", r["id"], r["text"].as_str().unwrap_or(""));
            }
        }
        _ => println!("เรียบร้อย"),
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("ผิดพลาด: {e}");
        std::process::exit(1);
    }
}
