//! What the owner chose for one idea when adding or editing it: which AI account works on it, with which model and
//! effort, and which skills it must use. Empty = automatic (the bot's own choice). Kept as one JSON setting so the idea
//! itself does not change shape.
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

use crate::db;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct IdeaAi {
    /// an account id (claude01, ...); "" = whichever account the bot is using
    #[serde(default)]
    pub account: String,
    /// "" or "auto" = the router decides, else an exact model name
    #[serde(default)]
    pub model: String,
    /// "" or "auto", else low | medium | high | xhigh
    #[serde(default)]
    pub effort: String,
    /// skills named with /name when the idea was added: always offered to the AI for this idea
    #[serde(default)]
    pub skills: Vec<String>,
}

impl IdeaAi {
    pub fn is_empty(&self) -> bool {
        self.account.is_empty() && self.model_forced().is_none() && self.effort_forced().is_none() && self.skills.is_empty()
    }

    pub fn model_forced(&self) -> Option<&str> {
        Some(self.model.trim()).filter(|m| !m.is_empty() && *m != "auto")
    }

    pub fn effort_forced(&self) -> Option<&str> {
        ["low", "medium", "high", "xhigh"].into_iter().find(|e| *e == self.effort)
    }
}

fn load(c: &Connection) -> BTreeMap<String, IdeaAi> {
    db::get_setting(c, "idea_ai").and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn get(c: &Connection, id: i64) -> Option<IdeaAi> {
    load(c).remove(&id.to_string())
}

pub fn set(c: &Connection, id: i64, v: &IdeaAi) -> Result<(), String> {
    let mut all = load(c);
    if v.is_empty() {
        all.remove(&id.to_string());
    } else {
        all.insert(id.to_string(), v.clone());
    }
    db::set_setting(c, "idea_ai", &serde_json::to_string(&all).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

pub fn all_json(c: &Connection) -> Value {
    json!(load(c))
}

/// The account an idea is pinned to, when it is pinned.
pub fn pinned_account(c: &Connection, id: i64) -> Option<String> {
    get(c, id).map(|a| a.account).filter(|a| !a.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        let p = std::env::temp_dir().join(format!("ia-ideaai-{}-{:?}.db", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_file(&p);
        db::open(&p).unwrap()
    }

    #[test]
    fn a_choice_is_kept_per_idea_and_empty_means_automatic() {
        let c = conn();
        assert!(get(&c, 3).is_none());
        set(&c, 3, &IdeaAi { account: "codex01".into(), model: "gpt-6.1-sol".into(), effort: "high".into(), skills: vec![] }).unwrap();
        let a = get(&c, 3).unwrap();
        assert_eq!((a.model_forced(), a.effort_forced()), (Some("gpt-6.1-sol"), Some("high")));
        assert_eq!(pinned_account(&c, 3).as_deref(), Some("codex01"));
        assert!(pinned_account(&c, 4).is_none());
        // "auto" everywhere and nothing pinned = no entry at all
        set(&c, 3, &IdeaAi { account: String::new(), model: "auto".into(), effort: "auto".into(), skills: vec![] }).unwrap();
        assert!(get(&c, 3).is_none());
    }
}
