//! Interface text in languages other than Thai and English.
//!
//! Thai (the source) and English (written by hand, shipped as `i18n/en.json`) need nothing. Any other language is
//! translated once, by the owner's own AI, from the English strings and kept in `data/i18n/<lang>.json`; the page
//! then swaps the text (see uitrans.svelte.js). It costs a few model calls on the fastest tier (about 20 batches of 50
//! short strings) and only for a language the owner actually picks. Until it is done the page shows English.
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Notify;

use crate::state::AppState;

const SOURCE: &str = include_str!("../../src/lib/i18n/source.json");
const ENGLISH: &str = include_str!("../../src/lib/i18n/en.json");
const BATCH: usize = 50;

#[derive(Clone, Default)]
struct Job {
    running: bool,
    done: usize,
    error: String,
}

static JOBS: LazyLock<Mutex<HashMap<String, Job>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn templates() -> Vec<(String, String)> {
    let src: Vec<String> = serde_json::from_str(SOURCE).unwrap_or_default();
    let en: HashMap<String, String> = serde_json::from_str(ENGLISH).unwrap_or_default();
    src.into_iter().filter_map(|k| en.get(&k).cloned().map(|v| (k, v))).collect()
}

fn cache_path(root: &std::path::Path, lang: &str) -> PathBuf {
    root.join("data").join("i18n").join(format!("{lang}.json"))
}

fn read_cache(root: &std::path::Path, lang: &str) -> BTreeMap<String, String> {
    std::fs::read_to_string(cache_path(root, lang)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn placeholders(s: &str) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'{' {
            if let Some(j) = s[i..].find('}') {
                let inner = &s[i + 1..i + j];
                if !inner.is_empty() && inner.chars().all(|c| c.is_ascii_digit()) {
                    v.push(format!("{{{inner}}}"));
                }
                i += j;
            }
        }
        i += 1;
    }
    v.sort();
    v
}

/// A translation is kept only if it still carries every {0}, {1} of the original.
fn acceptable(english: &str, translated: &str) -> bool {
    !translated.trim().is_empty() && placeholders(english) == placeholders(translated)
}

pub fn dict(app: &AppHandle, lang: &str) -> Value {
    let st = app.state::<AppState>();
    let total = templates().len();
    let map = read_cache(&st.root, lang);
    let job = JOBS.lock().get(lang).cloned().unwrap_or_default();
    let status = if job.running {
        "translating"
    } else if map.len() >= total.saturating_sub(5) && total > 0 {
        "ready"
    } else if map.is_empty() {
        "missing"
    } else {
        "partial"
    };
    json!({ "status": status, "map": map, "done": map.len().max(job.done), "total": total, "error": job.error })
}

async fn one_batch(app: &AppHandle, lang: &str, items: &[(String, String)], n: usize) -> Result<BTreeMap<String, String>, String> {
    let st = app.state::<AppState>();
    let dir = st.root.join("data").join("i18n").join(format!("work-{lang}"));
    std::fs::create_dir_all(dir.join("logs")).map_err(|e| e.to_string())?;
    let input: BTreeMap<String, &String> = items.iter().enumerate().map(|(i, (_, en))| (i.to_string(), en)).collect();
    std::fs::write(dir.join("in.json"), serde_json::to_string_pretty(&input).unwrap_or_default()).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(dir.join("out.json"));
    let prompt = format!(
        "Translate the user-interface strings in in.json (English) into {language}. Write the result to out.json in this folder: \
         a JSON object with exactly the same keys, each value being the translation. Rules: keep placeholders like {{0}} and {{1}} exactly as they are \
         (they may move to where the grammar needs them); keep emoji and symbols such as · — → ✓ ✕ ⚡ unchanged; keep product and technical names in Latin letters \
         (Claude, Codex, Gemini, OpenCode, Mistral, SkillSpector, Telegram, GitHub, Orca, Idea Autopilot, Turbo, token, session, API, CLI, effort, SKILL.md, \
         front-matter, base64); use short, natural wording as in a software interface. Do nothing else and write no other files.",
        language = crate::lang::name(lang)
    );
    let (provider, model, env) = {
        let c = st.db.lock();
        let acct = crate::accounts::usable(&c).into_iter().next();
        let kind = acct.as_ref().map(|a| a.kind.clone()).unwrap_or_else(|| crate::providers::chosen(&c));
        let model = crate::providers::tier_model(&c, &kind, 0);
        (kind, model, acct.as_ref().map(|a| crate::accounts::env_with_secret(a, &st.root)).unwrap_or_default())
    };
    let log = dir.join("logs").join(format!("batch-{n}.log"));
    let out = crate::runner::run(
        crate::runner::RunSpec {
            cwd: &dir,
            prompt,
            provider,
            model,
            effort: "low".into(),
            tools: "Read,Write".into(),
            custom_cmd: String::new(),
            timeout_mins: 8,
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
    let raw = std::fs::read_to_string(dir.join("out.json")).map_err(|_| "the AI did not write out.json".to_string())?;
    let v: Value = serde_json::from_str(raw.trim_start_matches('\u{feff}')).map_err(|e| format!("out.json is not valid JSON: {e}"))?;
    let mut got = BTreeMap::new();
    for (i, (thai, en)) in items.iter().enumerate() {
        if let Some(t) = v.get(i.to_string()).and_then(Value::as_str) {
            if acceptable(en, t) {
                got.insert(thai.clone(), t.trim().to_string());
            }
        }
    }
    Ok(got)
}

/// Translate every string this language still lacks, batch by batch; progress is saved after each batch.
pub fn start(app: &AppHandle, lang: &str) -> Result<(), String> {
    let lang = crate::lang::normalize(lang);
    if lang == "th" || lang == "en" {
        return Err("ภาษานี้มีคำแปลอยู่แล้ว".into());
    }
    {
        let mut jobs = JOBS.lock();
        if jobs.values().any(|j| j.running) {
            return Err("กำลังแปลภาษาอื่นอยู่ รอให้เสร็จก่อน".into());
        }
        jobs.insert(lang.clone(), Job { running: true, ..Default::default() });
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let st = app.state::<AppState>();
        let all = templates();
        let mut map = read_cache(&st.root, &lang);
        let todo: Vec<(String, String)> = all.iter().filter(|(k, _)| !map.contains_key(k)).cloned().collect();
        let total = all.len();
        let mut failures = 0;
        for (n, chunk) in todo.chunks(BATCH).enumerate() {
            let mut result = one_batch(&app, &lang, chunk, n).await;
            if result.is_err() {
                result = one_batch(&app, &lang, chunk, n).await; // once more
            }
            match result {
                Ok(got) => {
                    map.extend(got);
                    let _ = std::fs::create_dir_all(st.root.join("data").join("i18n"));
                    let _ = std::fs::write(cache_path(&st.root, &lang), serde_json::to_string(&map).unwrap_or_default());
                    JOBS.lock().insert(lang.clone(), Job { running: true, done: map.len(), error: String::new() });
                }
                Err(e) => {
                    failures += 1;
                    st.push_log(format!("[แปลหน้าจอ {lang}] ชุดที่ {} ไม่สำเร็จ: {e}", n + 1));
                    JOBS.lock().insert(lang.clone(), Job { running: true, done: map.len(), error: e });
                    if failures >= 3 {
                        break;
                    }
                }
            }
            let _ = app.emit("ui-translate", &lang);
        }
        let err = JOBS.lock().get(&lang).map(|j| j.error.clone()).unwrap_or_default();
        JOBS.lock().insert(lang.clone(), Job { running: false, done: map.len(), error: if map.len() + 5 >= total { String::new() } else { err } });
        let _ = std::fs::remove_dir_all(st.root.join("data").join("i18n").join(format!("work-{lang}")));
        let _ = app.emit("ui-translate", &lang);
    });
    Ok(())
}

#[tauri::command]
pub fn ui_dict(app: AppHandle, lang: String) -> Value {
    dict(&app, &crate::lang::normalize(&lang))
}

#[tauri::command]
pub fn ui_translate_start(app: AppHandle, lang: String) -> Result<(), String> {
    start(&app, &lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_files_line_up() {
        let t = templates();
        let all: Vec<String> = serde_json::from_str(SOURCE).unwrap();
        // every Thai template has an English text (python scripts/extract-i18n.py --check lists the missing ones)
        assert_eq!(t.len(), all.len(), "a template has no English text: run scripts/extract-i18n.py --check");
        assert!(t.len() > 900);
        for (th, en) in &t {
            assert_eq!(placeholders(th), placeholders(en), "{th} | {en}");
        }
    }

    #[test]
    fn a_translation_must_keep_its_placeholders() {
        assert!(acceptable("Step {0}/{1}", "Paso {0}/{1}"));
        assert!(acceptable("{1} of {0}", "{0} sur {1}"));
        assert!(!acceptable("Step {0}/{1}", "Paso {0}"));
        assert!(!acceptable("Step {0}", ""));
        assert!(!acceptable("Stop", "{0}"));
    }
}
