//! Roles: named AI team members (a job + a persona + the AI/model that plays it), e.g. "บอม — front-end developer,
//! talks like me". They join an idea's team mode as helpers.
//!
//! - One role = one Markdown file in `data/roles/<slug>.md`, in the Claude Code subagent format (front-matter
//!   `name`, `description`, `tools`, `model` + the instructions as the body), so a role exported from here works
//!   as a subagent elsewhere and the big public subagent collections install here as they are. Extra keys
//!   (`title`, `emoji`, `account`, `team`) are ignored by other tools. The persona sits in the body between
//!   `<!-- persona -->` markers, so it can be left out when sharing.
//! - The shop browses public GitHub collections (pinned to a commit, text only, size-capped, scanned like skills,
//!   the exact text the owner previewed is what gets installed). The AI cannot install roles by itself.
//! - Model: `account` empty = played by the lead's own Claude as a subagent (`model` = sonnet / opus / haiku /
//!   inherit); `account` = an account id = the lead hands that role's work to that account (`ask_other_ai`).
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, State};

use crate::skills::{fm_get, gh_get, parse_repo, raw_fetch, scan, sha256_hex, slugify, trust_of, valid_commit, valid_part, MAX_BYTES};
use crate::state::AppState;

type R<T> = Result<T, String>;

/// Popular public collections of agent roles (subagent .md files). Stars come live from GitHub.
pub const CURATED: &[(&str, &str)] = &[
    ("msitarzewski/agency-agents", "The Agency: agents with a full personality — engineering, design, marketing, product, sales, support…"),
    ("wshobson/agents", "Production-ready subagents for development, ops, data and business"),
    ("VoltAgent/awesome-claude-code-subagents", "100+ subagents in categories: core development, languages, infrastructure, quality, data/AI…"),
    ("lst97/claude-code-sub-agents", "Full-stack development subagents"),
    ("iannuttall/claude-agents", "Small set of practical coding agents"),
    ("0xfurai/claude-code-subagents", "Subagents for many languages and frameworks"),
];
const PERSONA_START: &str = "<!-- persona -->";
const PERSONA_END: &str = "<!-- /persona -->";
pub const TIERS: [&str; 4] = ["auto", "fast", "balanced", "strong"];

/// A Claude model alias in a subagent file → a tier (files from public collections and from v0.9.0 say `model:`).
fn tier_of_model(m: &str) -> &'static str {
    match m.trim().to_lowercase().as_str() {
        "haiku" => "fast",
        "sonnet" => "balanced",
        "opus" => "strong",
        _ => "auto",
    }
}

/// The tier as a Claude alias (what Claude Code subagents understand).
pub fn claude_alias(tier: &str) -> &'static str {
    match tier {
        "fast" => "haiku",
        "balanced" => "sonnet",
        "strong" => "opus",
        _ => "inherit",
    }
}

/// The tier as an index for `providers::tier_model` (0 fast · 1 balanced · 2 strong); auto = balanced.
pub fn tier_index(tier: &str) -> usize {
    match tier {
        "fast" => 0,
        "strong" => 2,
        _ => 1,
    }
}

fn clean_tier(t: &str) -> String {
    if TIERS.contains(&t) { t.to_string() } else { "auto".into() }
}
const DEFAULT_TOOLS: &str = "Read, Write, Edit, Bash, Glob, Grep, WebSearch, WebFetch";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Role {
    pub slug: String,
    pub name: String,
    pub title: String,
    pub emoji: String,
    pub description: String,
    pub tools: String,
    /// how strong a model the role wants: auto | fast | balanced | strong — shared with the role, so it works on any
    /// machine whatever AIs it has (each machine maps the tier to its own model)
    pub tier: String,
    /// this machine only (never exported): the account that plays it ("" = the lead's AI) and an exact model ("" = by tier)
    pub account: String,
    pub model_override: String,
    /// v0.9.0's "in the team" flag: read once to build the first named team (teams.rs), no longer written
    pub team: bool,
    pub instructions: String,
    pub persona: String,
    pub source: String,
    pub license: String,
}

pub fn dir(root: &Path) -> PathBuf {
    root.join("data").join("roles")
}

fn valid_slug(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Read a role file (also any subagent .md from elsewhere).
pub fn parse(text: &str, slug: &str) -> R<Role> {
    let t = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let ls: Vec<&str> = t.lines().collect();
    if ls.first().map(|l| l.trim()) != Some("---") {
        return Err("ไฟล์นี้ไม่ใช่ role/agent ที่ถูกรูปแบบ: ต้องขึ้นต้นด้วย --- แล้วมี name กับ description".into());
    }
    let end = ls.iter().skip(1).position(|l| l.trim() == "---").map(|p| p + 1).ok_or("front-matter ไม่ปิดด้วย ---")?;
    let fm = &ls[1..end];
    let name = fm_get(fm, "name");
    let description = fm_get(fm, "description");
    if name.trim().is_empty() || description.trim().is_empty() {
        return Err("role ต้องมีทั้ง name และ description".into());
    }
    let body = ls[end + 1..].join("\n");
    let (instructions, persona) = match (body.find(PERSONA_START), body.find(PERSONA_END)) {
        (Some(a), Some(b)) if b > a => {
            let p = body[a + PERSONA_START.len()..b].trim().trim_start_matches("## Persona").trim().to_string();
            (format!("{}\n{}", body[..a].trim_end(), body[b + PERSONA_END.len()..].trim_start()).trim().to_string(), p)
        }
        _ => (body.trim().to_string(), String::new()),
    };
    if instructions.is_empty() && persona.is_empty() {
        return Err("role ไม่มีเนื้อหา (มีแต่ front-matter)".into());
    }
    let tier = { let t = fm_get(fm, "tier").to_lowercase(); if TIERS.contains(&t.as_str()) { t } else { tier_of_model(&fm_get(fm, "model")).to_string() } };
    let slug = if slug.is_empty() { slugify(&name) } else { slug.to_string() };
    Ok(Role {
        slug: if slug.is_empty() { "role".into() } else { slug },
        name: name.trim().into(),
        title: fm_get(fm, "title"),
        emoji: fm_get(fm, "emoji").chars().take(4).collect(),
        description: description.trim().into(),
        tools: fm_get(fm, "tools"),
        tier,
        account: fm_get(fm, "account"),
        model_override: fm_get(fm, "model_override"),
        team: fm_get(fm, "team") == "true",
        instructions,
        persona,
        source: fm_get(fm, "source"),
        license: fm_get(fm, "license"),
    })
}

fn q(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', " "))
}

/// The role as a file. `with_persona` false leaves the persona out (for sharing the job only);
/// `for_sharing` leaves out what only makes sense on this machine (the account).
pub fn render(r: &Role, with_persona: bool, for_sharing: bool) -> String {
    let mut fm = vec![format!("name: {}", q(&r.name)), format!("description: {}", q(&r.description))];
    if !r.title.is_empty() {
        fm.push(format!("title: {}", q(&r.title)));
    }
    if !r.emoji.is_empty() {
        fm.push(format!("emoji: {}", q(&r.emoji)));
    }
    fm.push(format!("tools: {}", if r.tools.trim().is_empty() { DEFAULT_TOOLS } else { r.tools.trim() }));
    // `model` keeps the file a valid Claude Code subagent; `tier` is what this app reads
    fm.push(format!("tier: {}", clean_tier(&r.tier)));
    fm.push(format!("model: {}", claude_alias(&r.tier)));
    if !for_sharing {
        if !r.account.is_empty() {
            fm.push(format!("account: {}", q(&r.account)));
        }
        if !r.model_override.is_empty() {
            fm.push(format!("model_override: {}", q(&r.model_override)));
        }
        if !r.source.is_empty() {
            fm.push(format!("source: {}", q(&r.source)));
        }
    }
    if !r.license.is_empty() {
        fm.push(format!("license: {}", q(&r.license)));
    }
    let mut out = format!("---\n{}\n---\n\n{}\n", fm.join("\n"), r.instructions.trim());
    if with_persona && !r.persona.trim().is_empty() {
        out.push_str(&format!("\n{PERSONA_START}\n## Persona\n\n{}\n{PERSONA_END}\n", r.persona.trim()));
    }
    out
}

pub fn list(root: &Path) -> Vec<Role> {
    let mut v: Vec<Role> = std::fs::read_dir(dir(root))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let slug = p.file_stem()?.to_str()?.to_string();
            if p.extension()?.to_str()? != "md" || !valid_slug(&slug) {
                return None;
            }
            parse(&std::fs::read_to_string(&p).ok()?, &slug).ok()
        })
        .collect();
    v.sort_by_key(|a| a.name.to_lowercase());
    v
}

pub fn find(root: &Path, slug: &str) -> Option<Role> {
    if !valid_slug(slug) {
        return None;
    }
    parse(&std::fs::read_to_string(dir(root).join(format!("{slug}.md"))).ok()?, slug).ok()
}

pub(crate) fn write(root: &Path, r: &Role) -> R<()> {
    if !valid_slug(&r.slug) {
        return Err("ชื่อไฟล์ของ role ใช้ได้เฉพาะ a-z 0-9 และ -".into());
    }
    let text = render(r, true, false);
    if text.len() > MAX_BYTES {
        return Err("role ยาวเกินไป (เกิน 60 KB)".into());
    }
    std::fs::create_dir_all(dir(root)).map_err(|e| e.to_string())?;
    std::fs::write(dir(root).join(format!("{}.md", r.slug)), text).map_err(|e| e.to_string())
}

/// A free slug: the wanted one, or `-2`, `-3`… when another role already has it.
pub(crate) fn free_slug(root: &Path, want: &str) -> String {
    let base = { let s = slugify(want); if s.is_empty() { "role".to_string() } else { s.chars().take(56).collect() } };
    let mut s = base.clone();
    let mut n = 2;
    while dir(root).join(format!("{s}.md")).exists() {
        s = format!("{base}-{n}");
        n += 1;
    }
    s
}

pub fn to_json(r: &Role) -> Value {
    json!({
        "slug": r.slug, "name": r.name, "title": r.title, "emoji": r.emoji, "description": r.description, "tools": r.tools,
        "tier": r.tier, "account": r.account, "model_override": r.model_override, "instructions": r.instructions, "persona": r.persona,
        "source": r.source, "license": r.license,
    })
}

fn from_json(v: &Value) -> Role {
    let s = |k: &str, n: usize| v[k].as_str().unwrap_or("").chars().take(n).collect::<String>();
    Role {
        slug: s("slug", 64),
        name: s("name", 60).trim().into(),
        title: s("title", 80).trim().into(),
        emoji: s("emoji", 4),
        description: s("description", 400).trim().into(),
        tools: s("tools", 300),
        tier: clean_tier(&s("tier", 16)),
        account: s("account", 32),
        model_override: s("model_override", 80),
        team: false,
        instructions: s("instructions", 40_000),
        persona: s("persona", 12_000),
        source: s("source", 300),
        license: s("license", 40),
    }
}

/// Warnings about a persona before it leaves this machine: things that look personal.
pub fn persona_warnings(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let has_email = text.split_whitespace().any(|w| {
        let w = w.trim_matches(|c: char| !c.is_alphanumeric() && c != '@' && c != '.');
        w.contains('@') && w.split('@').nth(1).is_some_and(|d| d.contains('.'))
    });
    if has_email {
        out.push("มีอีเมล".into());
    }
    let digits = |w: &str| w.chars().filter(|c| c.is_ascii_digit()).count();
    if text.split(|c: char| c.is_whitespace() || c == ',').any(|w| digits(w) >= 9 && w.chars().all(|c| c.is_ascii_digit() || "+-() ".contains(c))) {
        out.push("มีตัวเลขยาวที่อาจเป็นเบอร์โทรหรือเลขบัญชี".into());
    }
    if text.contains(":\\") || text.contains("/Users/") || text.contains("/home/") {
        out.push("มี path ในเครื่อง".into());
    }
    if ["password", "รหัสผ่าน", "token", "api key", "apikey", "secret"].iter().any(|k| text.to_lowercase().contains(k)) {
        out.push("มีคำที่อาจเป็นรหัสหรือกุญแจ".into());
    }
    out
}

// ---------- team mode ----------

/// Roles played by the lead's own Claude, as `--agents` entries (helpers cannot hire helpers: no Task tool).
/// Only when the lead is Claude on Anthropic; every other lead hands roles out through `ask_other_ai`.
pub fn subagents(roles: &[Role]) -> Map<String, Value> {
    let mut m = Map::new();
    for r in roles.iter().filter(|r| r.account.is_empty()) {
        let tools: Vec<String> = if r.tools.trim().is_empty() { DEFAULT_TOOLS.split(',').map(|t| t.trim().to_string()).collect() } else {
            r.tools.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty() && t != "Task" && t != "Agent").collect()
        };
        let mut prompt = r.instructions.clone();
        if !r.persona.trim().is_empty() {
            prompt.push_str(&format!("\n\n## Your persona\n{}", r.persona.trim()));
        }
        prompt.push_str("\n\nYou are on a small team: do only the part the lead gave you, only in the files/folder named in the brief, then report briefly (what you made, where, what is left).");
        let model = if r.model_override.is_empty() { claude_alias(&r.tier).to_string() } else { r.model_override.clone() };
        m.insert(
            format!("role-{}", r.slug),
            json!({ "description": format!("{} — {}: {}", r.name, if r.title.is_empty() { "team member" } else { &r.title }, r.description), "prompt": prompt, "model": model, "tools": tools }),
        );
    }
    m
}

/// What the lead is told about its team. `lead_account` plays the roles bound to "the lead's AI" when the lead is
/// not Claude on Anthropic (`subagents_ok` false): then every role goes through `ask_other_ai`.
pub fn team_prompt(team_name: &str, roles: &[Role], lead_account: &str, subagents_ok: bool) -> String {
    if roles.is_empty() {
        return String::new();
    }
    let mut s = format!("\n\n## Your team: {team_name}\nGive parts to the member whose role fits; brief them like any helper (they see only your brief). Members:\n");
    for r in roles {
        let who = if r.title.is_empty() { r.description.clone() } else { format!("{} — {}", r.title, r.description) };
        if r.account.is_empty() && subagents_ok {
            s.push_str(&format!("- {} (subagent `role-{}`): {}\n", r.name, r.slug, who));
        } else {
            let acct = if r.account.is_empty() { lead_account } else { &r.account };
            s.push_str(&format!("- {} (call `ask_other_ai` with account `{}`, role `{}`): {}\n", r.name, acct, r.slug, who));
        }
    }
    s
}

/// The brief another AI gets when it plays a role (`ask_other_ai` with `role`).
pub fn delegate_brief(root: &Path, slug: &str) -> Option<String> {
    let r = find(root, slug)?;
    let mut s = format!("You play the team role \"{}\"{}.\n{}\n", r.name, if r.title.is_empty() { String::new() } else { format!(" ({})", r.title) }, r.instructions.trim());
    if !r.persona.trim().is_empty() {
        s.push_str(&format!("\n## Your persona\n{}\n", r.persona.trim()));
    }
    Some(s)
}

// ---------- shop ----------

fn role_path_ok(p: &str) -> bool {
    let lower = p.to_lowercase();
    let file = lower.rsplit('/').next().unwrap_or("");
    lower.ends_with(".md")
        && p.len() <= 300
        && !lower.starts_with('.')
        && !lower.contains("/.")
        && !["readme.md", "contributing.md", "claude.md", "agents.md", "license.md", "changelog.md", "security.md", "code_of_conduct.md", "architecture.md"].contains(&file)
        && !file.starts_with("contributing")
        && !file.starts_with("readme")
        && !lower.starts_with("docs/")
        && !lower.contains("/docs/")
        && !lower.contains("examples/")
        && !lower.contains("templates/")
        && !lower.contains("/commands/")
        && !lower.contains("/skills/")
        && !lower.ends_with("/skill.md")
        && p.split('/').all(|seg| !seg.is_empty() && seg != ".." && seg.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ' ' | '@' | '+')))
}

/// The category a file sits in, from its folder ("categories/01-core-development/x.md" → "core-development").
fn folder_category(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    let folder = if parts.len() >= 2 { parts[parts.len() - 2] } else { "" };
    let folder = if folder == "agents" && parts.len() >= 3 { parts[parts.len() - 3] } else { folder };
    folder.trim_start_matches(|c: char| c.is_ascii_digit() || c == '-').to_string()
}

pub async fn shop_search(query: &str, sort: &str) -> R<Value> {
    let q = query.trim();
    let mut hits: BTreeMap<String, Value> = BTreeMap::new();
    let mut last_err = None;
    // the well-known collections, with their stars, in ONE request (unauthenticated GitHub allows 60 an hour)
    let repos_q = CURATED.iter().map(|(n, _)| format!("repo:{n}")).collect::<Vec<_>>().join(" ");
    let stars: BTreeMap<String, Value> = match reqwest::Url::parse_with_params("https://api.github.com/search/repositories", &[("q", repos_q.as_str()), ("per_page", "20")]) {
        Ok(url) => match gh_get(url.as_str()).await {
            Ok(v) => v["items"].as_array().into_iter().flatten().map(|it| (it["full_name"].as_str().unwrap_or("").to_lowercase(), it["stargazers_count"].clone())).collect(),
            Err(x) => {
                last_err = Some(x);
                BTreeMap::new()
            }
        },
        Err(_) => BTreeMap::new(),
    };
    for (name, desc) in CURATED {
        let owner = name.split('/').next().unwrap_or("");
        if q.is_empty() || format!("{name} {desc}").to_lowercase().contains(&q.to_lowercase()) {
            let st = stars.get(&name.to_lowercase()).cloned().unwrap_or(Value::Null);
            hits.insert(name.to_string(), json!({ "repo": name, "description": desc, "stars": st, "trust": trust_of(owner), "listed": "curated" }));
        }
    }
    let since = |days: i64| (chrono::Utc::now() - chrono::Duration::days(days)).format("%Y-%m-%d").to_string();
    let age = match sort {
        "new" => format!(" created:>{}", since(90)),
        "trending" => format!(" created:>{}", since(365)),
        _ => String::new(),
    };
    for topic in ["claude-code-subagents", "subagents", "ai-agents-personas"] {
        let search = if q.is_empty() { format!("topic:{topic}{age}") } else { format!("{q} topic:{topic}{age}") };
        let Ok(url) = reqwest::Url::parse_with_params("https://api.github.com/search/repositories", &[("q", search.as_str()), ("sort", "stars"), ("per_page", "20")]) else { continue };
        match gh_get(url.as_str()).await {
            Ok(v) => {
                for it in v["items"].as_array().cloned().unwrap_or_default() {
                    let full = it["full_name"].as_str().unwrap_or("").to_string();
                    if parse_repo(&full).is_none() {
                        continue;
                    }
                    let created = it["created_at"].as_str().unwrap_or("").to_string();
                    let days = chrono::DateTime::parse_from_rfc3339(&created).map(|d| (chrono::Utc::now() - d.with_timezone(&chrono::Utc)).num_days().max(1)).unwrap_or(365);
                    hits.entry(full.clone()).or_insert_with(|| {
                        json!({ "repo": full, "description": it["description"], "stars": it["stargazers_count"], "license": it["license"]["spdx_id"], "created": created,
                                "stars_per_day": (it["stargazers_count"].as_f64().unwrap_or(0.0) / days as f64 * 10.0).round() / 10.0,
                                "trust": trust_of(it["owner"]["login"].as_str().unwrap_or("")), "listed": "github-search" })
                    });
                }
            }
            Err(x) => last_err = Some(x),
        }
    }
    let mut list: Vec<Value> = hits.into_values().collect();
    match sort {
        "new" => list.sort_by(|a, b| b["created"].as_str().unwrap_or("").cmp(a["created"].as_str().unwrap_or(""))),
        "trending" => list.sort_by(|a, b| b["stars_per_day"].as_f64().unwrap_or(0.0).partial_cmp(&a["stars_per_day"].as_f64().unwrap_or(0.0)).unwrap_or(std::cmp::Ordering::Equal)),
        _ => list.sort_by_key(|v| std::cmp::Reverse(v["stars"].as_i64().unwrap_or(0))),
    }
    if list.is_empty() {
        if let Some(x) = last_err {
            return Err(x);
        }
    }
    Ok(json!({ "repos": list, "note": last_err }))
}

/// The role files of one collection, with their categories (from folders).
pub async fn shop_browse(repo: &str) -> R<Value> {
    let (o, r) = parse_repo(repo).ok_or("ชื่อ repo ไม่ถูกต้อง (ใช้รูปแบบ owner/repo)")?;
    let info = gh_get(&format!("https://api.github.com/repos/{o}/{r}")).await?;
    let branch = info["default_branch"].as_str().unwrap_or("main");
    if !branch.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/')) {
        return Err("ชื่อ branch แปลก — ไม่รองรับ".into());
    }
    let commit = gh_get(&format!("https://api.github.com/repos/{o}/{r}/commits/{branch}")).await?;
    let sha = commit["sha"].as_str().unwrap_or("").to_string();
    if !valid_commit(&sha) {
        return Err("อ่าน commit ล่าสุดไม่ได้".into());
    }
    let tree = gh_get(&format!("https://api.github.com/repos/{o}/{r}/git/trees/{sha}?recursive=1")).await?;
    let items = tree["tree"].as_array().cloned().unwrap_or_default();
    // collections that keep agents in `agents/` folders also hold commands and skills — take only the agents then
    let agents_dirs = items.iter().any(|it| it["path"].as_str().is_some_and(|p| p.contains("/agents/") || p.starts_with("agents/")));
    let mut roles = Vec::new();
    for it in items {
        let path = it["path"].as_str().unwrap_or("");
        let in_agents = path.rsplit_once('/').is_some_and(|(d, _)| d == "agents" || d.ends_with("/agents"));
        if it["type"] == "blob" && role_path_ok(path) && (!agents_dirs || in_agents) {
            let size = it["size"].as_u64().unwrap_or(0);
            let name = path.rsplit('/').next().unwrap_or(path).trim_end_matches(".md").replace(['-', '_'], " ");
            roles.push(json!({ "path": path, "name": name, "category": folder_category(path), "size": size, "too_big": size as usize > MAX_BYTES }));
        }
    }
    let total = roles.len();
    roles.truncate(600);
    Ok(json!({
        "repo": format!("{o}/{r}"), "commit": sha, "description": info["description"], "stars": info["stargazers_count"], "license": info["license"]["spdx_id"],
        "trust": trust_of(&o), "url": format!("https://github.com/{o}/{r}"), "roles": roles, "total": total, "truncated": tree["truncated"].as_bool().unwrap_or(false) || total > 600,
    }))
}

async fn fetch(repo: &str, commit: &str, path: &str) -> R<(String, String, String)> {
    let (o, r) = parse_repo(repo).ok_or("ชื่อ repo ไม่ถูกต้อง")?;
    if !valid_part(&o) || !valid_part(&r) || !role_path_ok(path) {
        return Err("ที่อยู่ของ role ไม่ถูกต้อง".into());
    }
    let text = raw_fetch(&o, &r, commit, path).await?;
    let license = gh_get(&format!("https://api.github.com/repos/{o}/{r}")).await.ok().and_then(|v| v["license"]["spdx_id"].as_str().map(String::from)).unwrap_or_default();
    Ok((text, format!("https://github.com/{o}/{r}/blob/{commit}/{path}"), license))
}

fn preview_of(text: &str, source: &str, license: &str) -> R<Value> {
    let r = parse(text, "")?;
    let sc = scan(text);
    Ok(json!({
        "role": to_json(&r), "text": text, "sha256": sha256_hex(text.as_bytes()), "size": text.len(), "verdict": sc.verdict, "findings": sc.findings,
        "domains": sc.domains, "source": source, "license": license, "needs_accept": sc.verdict == "warn",
        "note": "ติดตั้งเฉพาะข้อความของไฟล์นี้ — role เป็นคำแนะนำให้ AI ไม่เหนือกฎของบอท",
    }))
}

fn install_text(root: &Path, text: &str, source: &str, license: &str, accept: bool) -> R<Value> {
    let sc = scan(text);
    match sc.verdict {
        "danger" => return Err(format!("บล็อกการติดตั้ง: {}", sc.findings.iter().map(|f| f.what.clone()).collect::<Vec<_>>().join(", "))),
        "warn" if !accept => return Err("NEEDS_ACCEPT: อ่านเนื้อหาเต็มแล้วติ๊กยืนยันก่อนติดตั้ง".into()),
        _ => {}
    }
    let mut r = parse(text, "")?;
    r.slug = free_slug(root, &r.name);
    r.source = source.into();
    r.license = license.into();
    r.team = false;
    r.account.clear(); // an account name from someone else's machine means nothing here
    write(root, &r)?;
    Ok(to_json(&r))
}

// ---------- Tauri commands ----------

#[tauri::command]
pub fn roles_list(st: State<AppState>) -> Value {
    json!({ "roles": list(&st.root).iter().map(to_json).collect::<Vec<_>>(), "tiers": TIERS, "curated": CURATED.iter().map(|(r, d)| json!({ "repo": r, "description": d })).collect::<Vec<_>>() })
}

/// Create (no slug) or change a role.
#[tauri::command]
pub fn role_save(st: State<AppState>, role: Value) -> R<Value> {
    let mut r = from_json(&role);
    if r.name.is_empty() {
        return Err("ตั้งชื่อ role ก่อน".into());
    }
    if r.description.is_empty() {
        r.description = if r.title.is_empty() { r.name.clone() } else { r.title.clone() };
    }
    if r.instructions.trim().is_empty() {
        r.instructions = format!("You are {}, {}. {}", r.name, if r.title.is_empty() { "a member of the owner's AI team" } else { &r.title }, r.description);
    }
    let sc = scan(&render(&r, true, false));
    if sc.verdict == "danger" {
        return Err(format!("บันทึกไม่ได้: {}", sc.findings.iter().map(|f| f.what.clone()).collect::<Vec<_>>().join(", ")));
    }
    if r.slug.is_empty() || find(&st.root, &r.slug).is_none() {
        r.slug = free_slug(&st.root, &r.name);
    }
    write(&st.root, &r)?;
    Ok(to_json(&r))
}

#[tauri::command]
pub fn role_delete(st: State<AppState>, slug: String) -> R<()> {
    if !valid_slug(&slug) {
        return Err("ชื่อ role ไม่ถูกต้อง".into());
    }
    std::fs::remove_file(dir(&st.root).join(format!("{slug}.md"))).map_err(|e| e.to_string())
}

/// The file to share: shown in full first; `save` writes it to data/exports and opens the folder.
#[tauri::command]
pub fn role_export(app: AppHandle, st: State<AppState>, slug: String, with_persona: bool, save: bool) -> R<Value> {
    let r = find(&st.root, &slug).ok_or("ไม่พบ role")?;
    let text = render(&r, with_persona, true);
    let warnings = if with_persona { persona_warnings(&r.persona) } else { Vec::new() };
    let path = if save { Some(crate::commands::save_export(app, st, format!("{slug}.agent.md"), text.clone())?) } else { None };
    Ok(json!({ "text": text, "warnings": warnings, "path": path }))
}

/// A role file from a friend or the community (pasted or opened): previewed and scanned first.
#[tauri::command]
pub fn role_check_text(text: String) -> R<Value> {
    if text.len() > MAX_BYTES {
        return Err("ไฟล์ใหญ่เกินไป".into());
    }
    preview_of(&text, "", "")
}

#[tauri::command]
pub fn role_import_text(st: State<AppState>, text: String, accept: bool) -> R<Value> {
    if text.len() > MAX_BYTES {
        return Err("ไฟล์ใหญ่เกินไป".into());
    }
    let r = install_text(&st.root, &text, "", "", accept)?;
    st.push_log(format!("[roles] นำเข้า role: {}", r["name"].as_str().unwrap_or("")));
    Ok(r)
}

#[tauri::command]
pub async fn roles_shop_search(query: String, sort: Option<String>) -> R<Value> {
    shop_search(&query, sort.as_deref().unwrap_or("popular")).await
}

#[tauri::command]
pub async fn roles_shop_browse(repo: String) -> R<Value> {
    shop_browse(&repo).await
}

#[tauri::command]
pub async fn roles_shop_preview(repo: String, commit: String, path: String) -> R<Value> {
    let (text, source, license) = fetch(&repo, &commit, &path).await?;
    preview_of(&text, &source, &license)
}

#[tauri::command]
pub async fn roles_shop_install(app: AppHandle, repo: String, commit: String, path: String, sha256: String, accept: bool) -> R<Value> {
    let (text, source, license) = fetch(&repo, &commit, &path).await?;
    // exactly what the owner reviewed, nothing else
    if sha256_hex(text.as_bytes()) != sha256 {
        return Err("ไฟล์ไม่ตรงกับที่คุณเปิดดู — ดูตัวอย่างใหม่อีกครั้ง".into());
    }
    let st = app.state::<AppState>();
    let r = install_text(&st.root, &text, &source, &license, accept)?;
    st.push_log(format!("[roles] ติดตั้ง role จาก {source}"));
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Role {
        Role {
            slug: "bom".into(), name: "บอม".into(), title: "Front-end developer".into(), emoji: "🧑‍💻".into(),
            description: "Builds the UI".into(), tools: DEFAULT_TOOLS.into(), tier: "balanced".into(), account: "codex01".into(), model_override: String::new(), team: false,
            instructions: "You build clean, accessible UIs.".into(), persona: "พูดตรง สั้น ชอบลองของใหม่".into(), source: String::new(), license: String::new(),
        }
    }

    #[test]
    fn a_role_survives_the_round_trip_and_sharing_drops_local_bits() {
        let r = sample();
        assert_eq!(parse(&render(&r, true, false), "bom").unwrap(), r);
        let shared = parse(&render(&r, true, true), "bom").unwrap();
        assert_eq!(shared.persona, r.persona);
        assert!(shared.account.is_empty() && shared.tier == "balanced");
        let job_only = parse(&render(&r, false, true), "bom").unwrap();
        assert!(job_only.persona.is_empty());
        assert_eq!(job_only.instructions, r.instructions);
    }

    #[test]
    fn public_subagent_files_read_as_roles() {
        let t = "---\nname: frontend-developer\ndescription: \"Use when building frontends.\"\ntools: Read, Write, Edit, Bash\nmodel: sonnet\n---\n\nYou are a senior frontend developer.";
        let r = parse(t, "").unwrap();
        assert_eq!((r.slug.as_str(), r.tier.as_str(), r.tools.as_str()), ("frontend-developer", "balanced", "Read, Write, Edit, Bash"));
        // a model name this tool does not know becomes "auto"; the file stays a Claude subagent (model: alias)
        assert_eq!(parse(&t.replace("model: sonnet", "model: gpt-9"), "").unwrap().tier, "auto");
        assert!(render(&r, true, true).contains("model: sonnet"));
        assert!(parse("no front matter", "").is_err());
    }

    #[test]
    fn team_split_between_subagents_and_other_ais() {
        let mut local = sample();
        local.account.clear();
        local.slug = "ann".into();
        let other = sample();
        let m = subagents(&[local.clone(), other.clone()]);
        assert_eq!(m.len(), 1);
        let a = &m["role-ann"];
        assert!(a["prompt"].as_str().unwrap().contains("พูดตรง"));
        assert!(!a["tools"].as_array().unwrap().iter().any(|t| t == "Task"));
        let p = team_prompt("Web", &[local.clone(), other.clone()], "claude01", true);
        assert!(p.contains("subagent `role-ann`") && p.contains("account `codex01`"));
        // a lead that is not Claude hands every role out, the lead's own ones to its own account
        let p = team_prompt("Web", &[local, other], "gemini01", false);
        assert!(!p.contains("subagent") && p.contains("account `gemini01`, role `ann`") && p.contains("account `codex01`"));
    }

    #[test]
    fn only_role_files_are_offered_from_a_collection() {
        assert!(role_path_ok("engineering/engineering-frontend-developer.md"));
        assert!(role_path_ok("categories/01-core-development/api-designer.md"));
        assert!(!role_path_ok("README.md") && !role_path_ok("docs/guide.md") && !role_path_ok(".github/x.md") && !role_path_ok("a/../b.md") && !role_path_ok("x.txt"));
        assert_eq!(folder_category("categories/01-core-development/api-designer.md"), "core-development");
        assert_eq!(folder_category("plugins/python/agents/python-pro.md"), "python");
    }

    #[test]
    fn personal_bits_are_flagged_before_sharing() {
        assert!(persona_warnings("ติดต่อ me@example.com").iter().any(|w| w.contains("อีเมล")));
        assert!(!persona_warnings("ชอบกาแฟ พูดตรง").iter().any(|_| true));
        assert!(!persona_warnings("โทร 0812345678").is_empty());
    }

    /// Talks to GitHub: `cargo test roles::tests::live -- --ignored`
    #[test]
    #[ignore]
    fn live_collections_browse_preview_and_parse() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let found = rt.block_on(shop_search("", "popular")).unwrap();
        assert!(found["repos"].as_array().unwrap().len() >= CURATED.len());
        for repo in ["msitarzewski/agency-agents", "VoltAgent/awesome-claude-code-subagents", "wshobson/agents"] {
            let b = rt.block_on(shop_browse(repo)).unwrap();
            let roles = b["roles"].as_array().unwrap();
            assert!(roles.len() > 20, "{repo}: {}", roles.len());
            let commit = b["commit"].as_str().unwrap();
            let mut ok = 0;
            for r in roles.iter().filter(|r| !r["too_big"].as_bool().unwrap()).take(6) {
                let (text, _, _) = rt.block_on(fetch(repo, commit, r["path"].as_str().unwrap())).unwrap();
                if parse(&text, "").is_ok() { ok += 1; }
            }
            println!("{repo}: {} roles, {ok}/6 parsed, first {}", roles.len(), roles[0]["path"]);
            assert!(ok >= 4, "{repo} parsed only {ok}");
        }
    }
}
