//! Readable names for the models that really ran: `claude-opus-5-5-20260101` -> "Opus 5.5",
//! `gpt-6-astra` -> "GPT-6 Astra", `gemini-2.5-pro` -> "Gemini 2.5 Pro". Rules only, no lookups:
//! a model the rules do not know is shown title-cased instead of being hidden or guessed at.

const FAMILIES: [&str; 3] = ["opus", "sonnet", "haiku"];

fn title(word: &str) -> String {
    match word.to_lowercase().as_str() {
        "gpt" => "GPT".into(),
        "gemini" => "Gemini".into(),
        "glm" => "GLM".into(),
        "llama" => "Llama".into(),
        w if w.chars().all(|c| c.is_ascii_digit() || c == '.') => w.to_string(),
        w => {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        }
    }
}

/// 8-digit build dates (20260101) and the "[1m]" context suffix are not part of the name.
fn clean(id: &str) -> &str {
    id.split('[').next().unwrap_or(id).trim()
}

fn is_date(tok: &str) -> bool {
    tok.len() == 8 && tok.chars().all(|c| c.is_ascii_digit())
}

fn claude(id: &str) -> Option<String> {
    let toks: Vec<&str> = id.split('-').filter(|t| !t.is_empty() && *t != "claude" && !is_date(t)).collect();
    let fam = toks.iter().position(|t| FAMILIES.contains(&t.to_lowercase().as_str()))?;
    // version digits sit after the family (claude-opus-5-5) or before it (claude-3-5-sonnet)
    let nums: Vec<&str> = toks.iter().filter(|t| t.len() <= 2 && t.chars().all(|c| c.is_ascii_digit())).copied().collect();
    let name = title(toks[fam]);
    Some(if nums.is_empty() { name } else { format!("{name} {}", nums.join(".")) })
}

fn generic(id: &str) -> String {
    let toks: Vec<&str> = id.split(['-', '_', '/']).filter(|t| !t.is_empty() && !is_date(t)).collect();
    // "gpt" + "6" reads better as GPT-6
    let mut out = String::new();
    for (i, t) in toks.iter().enumerate() {
        let w = title(t);
        if i == 0 {
            out = w;
        } else if i == 1 && toks[0].eq_ignore_ascii_case("gpt") && w.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            out.push('-');
            out.push_str(&w);
        } else {
            out.push(' ');
            out.push_str(&w);
        }
    }
    out
}

/// Name for a run: the concrete model id when known, else the model name that was asked for, else the tier.
pub fn label(provider: &str, model_id: Option<&str>, requested: &str) -> String {
    let id = model_id.map(clean).filter(|s| !s.is_empty()).unwrap_or("");
    // "<agent>-default" means the agent chose its own model and did not say which
    if id.is_empty() || id.ends_with("-default") {
        let r = clean(requested);
        let prov = crate::providers::label(if provider.is_empty() { "claude" } else { provider });
        return match r {
            "" | "auto" => format!("{prov} (ค่าเริ่มต้น)"),
            "fast" | "standard" | "strong" => format!("{prov} ({})", tier_th(r)),
            "haiku" | "sonnet" | "opus" => format!("{} (ล่าสุด)", title(r)),
            other => claude(other).unwrap_or_else(|| generic(other)),
        };
    }
    claude(id).unwrap_or_else(|| generic(id))
}

fn tier_th(t: &str) -> &'static str {
    match t {
        "fast" => "ระดับเร็ว",
        "strong" => "ระดับแรง",
        _ => "ระดับกลาง",
    }
}

/// "Opus 5.5 · medium"
pub fn with_effort(label: &str, effort: Option<&str>) -> String {
    match effort.map(str::trim).filter(|e| !e.is_empty()) {
        Some(e) => format!("{label} · {e}"),
        None => label.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_models_the_way_people_say_them() {
        assert_eq!(label("claude", Some("claude-opus-5-5"), "strong"), "Opus 5.5");
        assert_eq!(label("claude", Some("claude-sonnet-4-8-20260101"), "standard"), "Sonnet 4.8");
        assert_eq!(label("claude", Some("claude-3-5-sonnet-20241022"), ""), "Sonnet 3.5");
        assert_eq!(label("claude", Some("claude-haiku-5"), "fast"), "Haiku 5");
        assert_eq!(label("claude", Some("claude-opus-5-5[1m]"), ""), "Opus 5.5");
        assert_eq!(label("codex", Some("gpt-6-astra"), "strong"), "GPT-6 Astra");
        assert_eq!(label("codex", Some("gpt-5.1-codex-max"), ""), "GPT-5.1 Codex Max");
        assert_eq!(label("gemini", Some("gemini-2.5-pro"), ""), "Gemini 2.5 Pro");
    }

    #[test]
    fn falls_back_without_hiding_anything() {
        assert_eq!(label("claude", None, "opus"), "Opus (ล่าสุด)");
        assert_eq!(label("claude", None, "claude-opus-4-8"), "Opus 4.8");
        assert_eq!(label("codex", Some("codex-default"), "standard"), format!("{} (ระดับกลาง)", crate::providers::label("codex")));
        assert_eq!(with_effort("Opus 5.5", Some("medium")), "Opus 5.5 · medium");
        assert_eq!(with_effort("Opus 5.5", None), "Opus 5.5");
    }
}
