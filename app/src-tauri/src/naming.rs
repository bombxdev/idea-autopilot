//! A short, readable title and an English folder name for a new idea, right after it is added.
//!
//! The bot's "interpret" step also names the idea, but that can be hours away (quota, work hours). Until then the
//! board showed the owner's text cut at 80 characters, often mid-word. Here the cheapest model of the first usable
//! account reads the text once and writes `{title, slug_en}`; without an AI, a cut at a word boundary stands in.

use parking_lot::Mutex;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::{Arc, LazyLock};
use tauri::{AppHandle, Manager};
use tokio::sync::Notify;

use crate::db::{self, Idea};
use crate::state::AppState;
use crate::workspace;

/// Ideas tried in this run of the app (each at most once, so a failing AI is not called again and again).
static TRIED: LazyLock<Mutex<HashSet<i64>>> = LazyLock::new(|| Mutex::new(HashSet::new()));
/// One naming at a time.
static BUSY: LazyLock<Mutex<bool>> = LazyLock::new(|| Mutex::new(false));

/// Longest title shown on a card (Thai or English).
pub const TITLE_MAX: usize = 48;

/// The owner's text as it was first used for a title (first line, 80 characters): still unnamed.
fn raw_title(body: &str) -> String {
    let t = body.trim();
    t.lines().next().unwrap_or(t).chars().take(80).collect()
}

/// Is the folder name still the first automatic one — the id plus whatever Latin words happened to be in the text
/// ("idea-0024-discord" for a Thai idea mentioning Discord)? Then a summary may rename it.
pub fn auto_slug(i: &Idea) -> bool {
    !workspace::has_english_name(&i.slug)
        || [raw_title(&i.body), fallback_title(&i.body)].iter().any(|t| i.slug == workspace::make_slug(i.id, t))
}

/// Has nobody (AI or owner) given this idea a real title yet?
pub fn unnamed(i: &Idea) -> bool {
    let raw = raw_title(&i.body);
    i.title == raw || i.title == fallback_title(&i.body)
}

/// A title without AI: the first sentence, cut only between words, with "…" when shortened.
pub fn fallback_title(body: &str) -> String {
    let t = body.trim();
    let first = t.lines().next().unwrap_or(t).trim();
    // a sentence end inside the limit is the cleanest cut
    let mut s: String = first.chars().take(TITLE_MAX + 1).collect();
    if first.chars().count() <= TITLE_MAX {
        return first.to_string();
    }
    if let Some(i) = s.char_indices().filter(|(_, c)| matches!(c, '.' | '?' | '!' | '。')).map(|(i, _)| i).filter(|i| *i > 12).last() {
        return s[..i].trim().to_string();
    }
    // otherwise the last space (Thai writes spaces between phrases, so this keeps whole phrases)
    s = first.chars().take(TITLE_MAX).collect();
    match s.rfind(' ').filter(|i| *i > 12) {
        Some(i) => format!("{}…", s[..i].trim_end_matches([',', ' '])),
        None => format!("{s}…"),
    }
}

/// Clean what the AI wrote: a title that fits, and 1–5 plain English words for the folder.
fn accept(v: &Value) -> Option<(String, String)> {
    let title = v.get("title")?.as_str()?.trim().trim_matches(['"', '\'', '“', '”']).trim().to_string();
    if title.is_empty() || title.chars().count() > TITLE_MAX || title.contains('\n') {
        return None;
    }
    let slug_en = v.get("slug_en").and_then(Value::as_str).unwrap_or("").trim().to_lowercase();
    Some((title, slug_en))
}

/// Queue the ideas that still carry their raw text as a title; called from the scheduler's tick.
pub fn tick(app: &AppHandle) {
    let st = app.state::<AppState>();
    if *BUSY.lock() {
        return;
    }
    // not one the bot is working inside right now (its folder would move under it): it comes up again after the run
    let running = st.rt.lock().current.as_ref().map(|c| c.idea_id);
    let next = db::list_ideas(&st.db.lock())
        .unwrap_or_default()
        .into_iter()
        .filter(|i| matches!(i.status.as_str(), "inbox" | "active") && (unnamed(i) || auto_slug(i)) && Some(i.id) != running && !TRIED.lock().contains(&i.id))
        .next();
    let Some(idea) = next else { return };
    TRIED.lock().insert(idea.id);
    *BUSY.lock() = true;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let res = name_one(&app, &idea).await;
        let st = app.state::<AppState>();
        let (title, slug_en) = match res {
            Ok(v) => v,
            Err(e) => {
                st.push_log(format!("[ตั้งชื่อไอเดีย] #{} ใช้ AI ไม่ได้ ({e}) — ใช้ประโยคแรกแทน", idea.id));
                (fallback_title(&idea.body), String::new())
            }
        };
        if let Err(e) = apply(&st, idea.id, &title, &slug_en) {
            st.push_log(format!("[ตั้งชื่อไอเดีย] #{}: {e:#}", idea.id));
        }
        *BUSY.lock() = false;
        st.wake.notify_one();
    });
}

/// Again for an idea whose text the owner just changed.
pub fn retry(id: i64) {
    TRIED.lock().remove(&id);
}

async fn name_one(app: &AppHandle, idea: &Idea) -> Result<(String, String), String> {
    let st = app.state::<AppState>();
    let dir = st.root.join("data").join("naming").join(format!("idea-{}", idea.id));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("logs")).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("idea.txt"), &idea.body).map_err(|e| e.to_string())?;
    let lang = crate::lang::name(&crate::scheduler::idea_lang(&st, idea));
    let prompt = format!(
        "Read idea.txt (an idea someone jotted down). Write out.json containing only {{\"title\": \"...\", \"slug_en\": \"...\"}}. \
         title: a short headline in {lang} that sums up the whole idea so it is understood in one read — what is wanted, not \
         the first words of the text; at most {TITLE_MAX} characters, whole words only, no ending punctuation, no quotes. \
         slug_en: 2-4 plain English words naming the same thing, lowercase, for a folder name (e.g. \"discord english writing helper\"). \
         Do nothing else and write no other files."
    );
    let (provider, model, env) = {
        let c = st.db.lock();
        let acct = crate::accounts::usable(&c).into_iter().next();
        let kind = acct.as_ref().map(|a| a.kind.clone()).unwrap_or_else(|| crate::providers::chosen(&c));
        // the smallest model of that AI: naming needs no more
        let model = crate::providers::tier_model(&c, &kind, 0);
        (kind, model, acct.as_ref().map(|a| crate::accounts::env_with_secret(a, &st.root)).unwrap_or_default())
    };
    let log = dir.join("logs").join("naming.log");
    let out = crate::runner::run(
        crate::runner::RunSpec {
            cwd: &dir,
            prompt,
            provider,
            model,
            effort: "low".into(),
            tools: "Read,Write".into(),
            custom_cmd: String::new(),
            timeout_mins: 3,
            log_path: &log,
            lang: "en",
            env,
        },
        Arc::new(Notify::new()),
        |_| {},
        |_| {},
    )
    .await
    .map_err(|e| format!("{e:#}"))?;
    if out.status != "ok" {
        return Err(out.error.unwrap_or_else(|| out.status.clone()));
    }
    let raw = std::fs::read_to_string(dir.join("out.json")).map_err(|_| "ไม่ได้เขียน out.json".to_string())?;
    let v: Value = serde_json::from_str(raw.trim_start_matches('\u{feff}')).map_err(|e| format!("out.json อ่านไม่ได้: {e}"))?;
    let _ = std::fs::remove_dir_all(&dir);
    accept(&v).ok_or_else(|| "ชื่อที่ได้ยาวเกินหรือว่าง".into())
}

/// Set the title (only if still unnamed: the owner or the bot may have named it meanwhile) and the folder name.
fn apply(st: &AppState, id: i64, title: &str, slug_en: &str) -> anyhow::Result<()> {
    let mut idea = db::get_idea(&st.db.lock(), id)?;
    // the title only while still unnamed: the owner or the bot may have named it meanwhile
    if unnamed(&idea) && idea.title != title {
        db::set_title(&st.db.lock(), id, title)?;
        idea.title = title.to_string();
        workspace::rewrite_idea_md(&workspace::idea_dir(&st.ws, &idea.slug), title, &idea.body);
        db::log_event(&st.db.lock(), "rename", Some(id), &format!("ตั้งหัวข้อ: {title}"));
    }
    // the folder: an automatic name gets the summary's English name
    if !slug_en.is_empty() && auto_slug(&idea) {
        // not under a bot run that is inside the folder right now (the next naming of the idea renames it then)
        let running = st.rt.lock().current.as_ref().map(|c| c.idea_id) == Some(id);
        if let Some(new) = workspace::slug_with_english(id, slug_en).filter(|_| !running) {
            if new != idea.slug {
                crate::scheduler::move_folder(st, &mut idea, &new)?;
            }
        }
    }
    st.rt.lock().sync_dirty = true;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_idea_is_cut_between_words_not_inside_one() {
        let body = "ปกติผมต้องคุยกับเพื่อน เป็นภาษาอังกฤษผ่านทางแอพ discord แต่มีปัญหาว่าผมต้องสลับไปมาระหว่าง google translate และ discord";
        let t = fallback_title(body);
        assert!(t.ends_with('…'), "{t}");
        assert!(t.chars().count() <= TITLE_MAX + 1);
        let shown = t.trim_end_matches('…');
        assert!(body.starts_with(shown) && body[shown.len()..].starts_with(' '), "cut inside a word: {t}");
        assert_eq!(fallback_title("short idea"), "short idea");
        assert_eq!(fallback_title("Build a small CLI tool. It should also have a GUI later on and many more things"), "Build a small CLI tool");
    }

    #[test]
    fn only_a_title_that_fits_is_taken() {
        let ok = serde_json::json!({"title": "“ผู้ช่วยเขียนอังกฤษลอยบน Discord”", "slug_en": "Discord English Helper"});
        assert_eq!(accept(&ok), Some(("ผู้ช่วยเขียนอังกฤษลอยบน Discord".into(), "discord english helper".into())));
        assert_eq!(accept(&serde_json::json!({"title": "ก".repeat(TITLE_MAX + 1)})), None);
        assert_eq!(accept(&serde_json::json!({"title": "  "})), None);
    }

    #[test]
    fn a_folder_named_from_stray_latin_words_counts_as_automatic() {
        let mut i: Idea = serde_json::from_value(serde_json::json!({
            "id": 24, "slug": "idea-0024-discord", "title": "ผู้ช่วยอังกฤษลอยบน Discord",
            "body": "ปกติผมต้องคุยกับเพื่อน เป็นภาษาอังกฤษผ่านทางแอพ discord แต่มีปัญหา", "intents": [], "recipe": [], "stage_idx": 0,
            "status": "active", "priority": 0, "research_only": false, "summary": "", "verdict": null, "run_cmd": null,
            "knowledge": false, "needs": [], "repeat_count": 0, "fail_count": 0, "sort": 0.0, "created": 0, "updated": 0
        }))
        .unwrap();
        assert!(auto_slug(&i));
        i.slug = "idea-0024".into();
        assert!(auto_slug(&i));
        i.slug = "idea-0024-discord-english-helper".into();
        assert!(!auto_slug(&i));
    }

    #[test]
    fn a_title_from_the_raw_text_counts_as_unnamed() {
        let body = "ปกติผมต้องคุยกับเพื่อน เป็นภาษาอังกฤษผ่านทางแอพ discord แต่มีปัญหาว่าผมต้องสลับไปมาระหว่าง google translate";
        let mut i: Idea = serde_json::from_value(serde_json::json!({
            "id": 24, "slug": "idea-0024", "title": raw_title(body), "body": body, "intents": [], "recipe": [], "stage_idx": 0,
            "status": "inbox", "priority": 0, "research_only": false, "summary": "", "verdict": null, "run_cmd": null,
            "knowledge": false, "needs": [], "repeat_count": 0, "fail_count": 0, "sort": 0.0, "created": 0, "updated": 0
        }))
        .unwrap();
        assert!(unnamed(&i));
        i.title = fallback_title(body);
        assert!(unnamed(&i));
        i.title = "ผู้ช่วยเขียนอังกฤษบน Discord".into();
        assert!(!unnamed(&i));
    }
}
