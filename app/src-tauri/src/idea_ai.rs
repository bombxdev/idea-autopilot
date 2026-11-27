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
    /// the named team that works on this idea (teams.rs); "" = the AI alone. v0.9.0 stored a helper count:
    /// any number above 0 reads as the team made from those days ("my-team")
    #[serde(default, deserialize_with = "team_compat")]
    pub team: String,
    /// accounts to carry on with, in order, when this idea's account hits its limit (ISSUE-176)
    #[serde(default)]
    pub backups: Vec<String>,
}

fn team_compat<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::String(s) => s,
        Value::Number(n) if n.as_u64().unwrap_or(0) > 0 => "my-team".into(),
        _ => String::new(),
    })
}

impl IdeaAi {
    pub fn is_empty(&self) -> bool {
        self.account.is_empty() && self.model_forced().is_none() && self.effort_forced().is_none() && self.skills.is_empty() && self.team.is_empty() && self.backups.is_empty()
    }

    pub fn model_forced(&self) -> Option<&str> {
        Some(self.model.trim()).filter(|m| !m.is_empty() && *m != "auto")
    }

    pub fn effort_forced(&self) -> Option<&str> {
        ["low", "medium", "high", "xhigh", "max"].into_iter().find(|e| *e == self.effort)
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

/// The account an idea is pinned to, when it is pinned — and that account is still there and switched on. A pin to an
/// account the owner switched off (or removed) counts as no pin: the idea goes to whichever account works, instead of
/// waiting for one that never will.
pub fn pinned_account(c: &Connection, id: i64) -> Option<String> {
    let want = get(c, id).map(|a| a.account).filter(|a| !a.is_empty())?;
    crate::accounts::list(c).into_iter().find(|a| a.id == want && a.enabled).map(|a| a.id)
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
        let acct = |id: &str, on: bool| crate::accounts::Account {
            id: id.into(), kind: "codex".into(), label: String::new(), enabled: on, home: String::new(), model: "auto".into(),
            effort: "auto".into(), vendor: String::new(), auth: "login".into(), base_url: String::new(),
        };
        crate::accounts::save(&c, &[acct("codex01", true), acct("off01", false)]).unwrap();
        // pinned to an account that is switched off: as if not pinned
        set(&c, 5, &IdeaAi { account: "off01".into(), model: "auto".into(), effort: "auto".into(), skills: vec![], team: String::new(), backups: vec![] }).unwrap();
        assert!(pinned_account(&c, 5).is_none());
        assert!(get(&c, 3).is_none());
        set(&c, 3, &IdeaAi { account: "codex01".into(), model: "gpt-6.1-sol".into(), effort: "high".into(), skills: vec![], team: String::new(), backups: vec![] }).unwrap();
        let a = get(&c, 3).unwrap();
        assert_eq!((a.model_forced(), a.effort_forced()), (Some("gpt-6.1-sol"), Some("high")));
        assert_eq!(pinned_account(&c, 3).as_deref(), Some("codex01"));
        assert!(pinned_account(&c, 4).is_none());
        // "auto" everywhere and nothing pinned = no entry at all
        set(&c, 3, &IdeaAi { account: String::new(), model: "auto".into(), effort: "auto".into(), skills: vec![], team: String::new(), backups: vec![] }).unwrap();
        assert!(get(&c, 3).is_none());
    }

    #[test]
    fn old_helper_counts_read_as_the_old_team() {
        let a: IdeaAi = serde_json::from_str(r#"{"team":3}"#).unwrap();
        assert_eq!(a.team, "my-team");
        let b: IdeaAi = serde_json::from_str(r#"{"team":0}"#).unwrap();
        assert!(b.team.is_empty() && b.is_empty());
        let c: IdeaAi = serde_json::from_str(r#"{"team":"web-app"}"#).unwrap();
        assert_eq!(c.team, "web-app");
    }
}
