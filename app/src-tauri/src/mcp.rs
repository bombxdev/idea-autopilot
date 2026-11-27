//! Tools (MCP servers) next to skills.
//!
//! A skill is a recipe the AI reads; an MCP server is a program that gives the AI new tools (REA, a browser, a
//! database). A skill may need one, but installing a skill never registered the tool — and bot runs start Claude with
//! `--strict-mcp-config`, so until now no MCP reached a bot run at all.
//!
//! - The owner's tools live in `data/mcp/servers.json` (command, arguments, environment — local only).
//! - They come from the machine's own Claude Code config (`~/.claude.json`, import in one click), from the official
//!   MCP Registry (registry.modelcontextprotocol.io — free, the install command is shown first), or by hand.
//! - Each tool lists the skills that need it. A bot run gets a tool **only when a skill offered to that run needs it**
//!   (`--mcp-config <file>` with just those): every MCP adds its whole tool list to every turn, so "all tools always"
//!   would cost tokens on every run for nothing.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const REGISTRY: &str = "https://registry.modelcontextprotocol.io/v0/servers";

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Server {
    pub id: String,
    pub label: String,
    pub description: String,
    /// claude-code | registry | manual
    pub source: String,
    /// stdio | http
    pub transport: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub enabled: bool,
    /// skills that need this tool: a run gets it only when one of them is offered
    pub skills: Vec<String>,
    /// offered to every run (for a tool the owner wants always; costs tokens on each turn)
    pub always: bool,
    pub category: String,
    pub registry_name: String,
    pub version: String,
    /// how and when to use it, in a line or two (ISSUE-171): from the registry's description, editable by the owner —
    /// written into the idea's TOOLS.md, so a tool whose own description is thin still gets used at the right time
    pub howto: String,
}

fn dir(root: &Path) -> PathBuf {
    root.join("data").join("mcp")
}

pub fn list(root: &Path) -> Vec<Server> {
    std::fs::read_to_string(dir(root).join("servers.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save(root: &Path, v: &[Server]) -> Result<()> {
    std::fs::create_dir_all(dir(root))?;
    std::fs::write(dir(root).join("servers.json"), serde_json::to_string_pretty(v)?)?;
    Ok(())
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 48 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn slug(s: &str) -> String {
    let t: String = s.rsplit('/').next().unwrap_or(s).chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    let t = t.trim_matches('-').to_string();
    let t: String = t.chars().take(40).collect();
    if t.is_empty() { "tool".into() } else { t }
}

/// The server as Claude Code's `mcpServers` entry.
pub fn claude_entry(s: &Server) -> Value {
    if s.transport == "http" {
        let mut v = json!({ "type": "http", "url": s.url });
        if !s.headers.is_empty() {
            v["headers"] = json!(s.headers);
        }
        v
    } else {
        let mut v = json!({ "command": s.command, "args": s.args });
        if !s.env.is_empty() {
            v["env"] = json!(s.env);
        }
        v
    }
}

/// Skills whose text names this tool ("mcp__rea__…", "REA MCP"): linked on import so it works without setup.
fn auto_skills(root: &Path, id: &str) -> Vec<String> {
    let id = id.to_lowercase();
    let mut out = vec![];
    let Ok(rd) = std::fs::read_dir(crate::skills::dir(root)) else { return out };
    for e in rd.flatten() {
        let Ok(t) = std::fs::read_to_string(e.path().join("SKILL.md")) else { continue };
        let t = t.to_lowercase();
        if t.contains(&format!("mcp__{id}__")) || t.contains(&format!("{id} mcp")) || t.contains(&format!("mcp server {id}")) {
            out.push(e.file_name().to_string_lossy().into_owned());
        }
    }
    out
}

fn upsert(root: &Path, mut s: Server) -> Result<Server> {
    if !valid_id(&s.id) {
        bail!("ชื่อเครื่องมือใช้ได้แค่ a-z 0-9 - _");
    }
    let mut v = list(root);
    if s.skills.is_empty() {
        s.skills = auto_skills(root, &s.id);
    }
    if s.category.is_empty() {
        s.category = crate::skills::category_of(&format!("{} {} {}", s.id, s.label, s.description)).to_string();
    }
    match v.iter_mut().find(|x| x.id == s.id) {
        Some(x) => *x = s.clone(),
        None => v.push(s.clone()),
    }
    save(root, &v)?;
    Ok(s)
}

// ---------- from this machine's Claude Code ----------

fn claude_json() -> Option<Value> {
    let p = dirs::home_dir()?.join(".claude.json");
    serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()
}

/// Tools Claude Code on this machine already has (user-wide), that the app could use too. Values are not returned.
pub fn local(root: &Path) -> Vec<Value> {
    let have: Vec<String> = list(root).into_iter().map(|s| s.id).collect();
    let Some(c) = claude_json() else { return vec![] };
    c["mcpServers"]
        .as_object()
        .map(|m| {
            m.iter()
                .map(|(k, v)| {
                    let shown = match v["url"].as_str() {
                        Some(u) => u.to_string(),
                        None => std::iter::once(v["command"].as_str().unwrap_or("")).chain(v["args"].as_array().into_iter().flatten().filter_map(|a| a.as_str())).collect::<Vec<_>>().join(" "),
                    };
                    json!({ "id": k, "command": shown, "env_names": v["env"].as_object().map(|e| e.keys().cloned().collect::<Vec<_>>()).unwrap_or_default(), "imported": have.contains(k) })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn import_local(root: &Path, id: &str) -> Result<Server> {
    let c = claude_json().context("ไม่พบการตั้งค่าของ Claude Code ในเครื่องนี้")?;
    let v = c["mcpServers"].get(id).context("ไม่พบเครื่องมือนี้ในการตั้งค่าของ Claude Code")?;
    let strs = |x: &Value| -> BTreeMap<String, String> { x.as_object().map(|m| m.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect()).unwrap_or_default() };
    let http = v["url"].is_string();
    let id_ok: String = id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
    upsert(
        root,
        Server {
            id: id_ok,
            label: id.to_string(),
            source: "claude-code".into(),
            transport: if http { "http".into() } else { "stdio".into() },
            command: v["command"].as_str().unwrap_or("").into(),
            args: v["args"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default(),
            env: strs(&v["env"]),
            url: v["url"].as_str().unwrap_or("").into(),
            headers: strs(&v["headers"]),
            enabled: true,
            ..Default::default()
        },
    )
}

// ---------- the official MCP Registry ----------

fn pkg_command(p: &Value) -> Option<(String, Vec<String>)> {
    let id = p["identifier"].as_str()?;
    let ver = p["version"].as_str().unwrap_or("");
    let win = cfg!(windows);
    match p["registryType"].as_str()? {
        // Claude Code on Windows needs npx through cmd /c
        "npm" => {
            let spec = if ver.is_empty() { id.to_string() } else { format!("{id}@{ver}") };
            Some(if win { ("cmd".into(), vec!["/c".into(), "npx".into(), "-y".into(), spec]) } else { ("npx".into(), vec!["-y".into(), spec]) })
        }
        "pypi" => Some(("uvx".into(), vec![if ver.is_empty() { id.to_string() } else { format!("{id}=={ver}") }])),
        _ => None,
    }
}

/// One registry entry as the shop shows it: how it would be installed (exact command) and what it asks for.
fn shop_item(s: &Value, published: &str) -> Value {
    let name = s["name"].as_str().unwrap_or("");
    let desc = s["description"].as_str().unwrap_or("");
    let mut ways = vec![];
    for (i, p) in s["packages"].as_array().into_iter().flatten().enumerate() {
        if p["transport"]["type"].as_str().unwrap_or("stdio") != "stdio" {
            continue;
        }
        if let Some((c, a)) = pkg_command(p) {
            let env: Vec<Value> = p["environmentVariables"].as_array().into_iter().flatten().map(|e| json!({ "name": e["name"], "description": e["description"], "required": e["isRequired"].as_bool().unwrap_or(false), "secret": e["isSecret"].as_bool().unwrap_or(false), "default": e["default"] })).collect();
            ways.push(json!({ "kind": "package", "index": i, "command": format!("{c} {}", a.join(" ")), "env": env }));
        }
    }
    for (i, r) in s["remotes"].as_array().into_iter().flatten().enumerate() {
        if let Some(u) = r["url"].as_str().filter(|u| u.starts_with("https://")) {
            let hdr: Vec<Value> = r["headers"].as_array().into_iter().flatten().map(|h| json!({ "name": h["name"], "description": h["description"], "required": h["isRequired"].as_bool().unwrap_or(false), "secret": h["isSecret"].as_bool().unwrap_or(false) })).collect();
            ways.push(json!({ "kind": "remote", "index": i, "command": u, "env": hdr }));
        }
    }
    json!({
        "name": name, "title": s["title"], "description": desc, "version": s["version"], "repo": s["repository"]["url"],
        "category": crate::skills::category_of(&format!("{name} {desc}")), "published": published, "ways": ways,
    })
}

/// Well-known tools, each from the company or project that makes it: (registry name, category, GitHub repo for
/// stars). "Popular" = these, most stars first; everything else is found by searching the registry.
pub const POPULAR: &[(&str, &str, &str)] = &[
    ("io.github.github/github-mcp-server", "code", "github/github-mcp-server"),
    ("io.github.upstash/context7", "docs", "upstash/context7"),
    ("io.github.ChromeDevTools/chrome-devtools-mcp", "test", "ChromeDevTools/chrome-devtools-mcp"),
    ("io.github.wonderwhy-er/desktop-commander", "devops", "wonderwhy-er/DesktopCommanderMCP"),
    ("io.github.oraios/serena", "code", "oraios/serena"),
    ("io.github.GLips/Figma-Context-MCP", "design", "GLips/Figma-Context-MCP"),
    ("io.github.firecrawl/firecrawl-mcp-server", "research", "firecrawl/firecrawl-mcp-server"),
    ("io.github.brave/brave-search-mcp-server", "research", "brave/brave-search-mcp-server"),
    ("io.github.tavily-ai/tavily-mcp", "research", "tavily-ai/tavily-mcp"),
    ("ai.exa/exa", "research", "exa-labs/exa-mcp-server"),
    ("com.notion/mcp", "business", "makenotion/notion-mcp-server"),
    ("com.supabase/mcp", "data", "supabase-community/supabase-mcp"),
    ("io.github.mongodb-js/mongodb-mcp-server", "data", "mongodb-js/mongodb-mcp-server"),
    ("com.atlassian/atlassian-mcp-server", "business", "atlassian/atlassian-mcp-server"),
    ("app.linear/linear", "business", ""),
    ("com.stripe/mcp", "business", "stripe/agent-toolkit"),
    ("co.huggingface/hf-mcp-server", "agents", "huggingface/hf-mcp-server"),
    ("io.github.arben-adm/mcp-sequential-thinking", "agents", "arben-adm/mcp-sequential-thinking"),
];

static POPULAR_CACHE: std::sync::Mutex<Option<(std::time::Instant, Value)>> = std::sync::Mutex::new(None);

async fn registry_entry(name: String) -> Option<Value> {
    let mut url = reqwest::Url::parse(REGISTRY).ok()?;
    url.query_pairs_mut().append_pair("search", &name).append_pair("version", "latest").append_pair("limit", "20");
    let c = reqwest::Client::builder().timeout(std::time::Duration::from_secs(20)).build().ok()?;
    let v: Value = c.get(url).header("User-Agent", "idea-autopilot").send().await.ok()?.json().await.ok()?;
    v["servers"].as_array()?.iter().find(|x| x["server"]["name"].as_str() == Some(name.as_str())).cloned()
}

/// The well-known tools with their GitHub stars, most first (kept for 6 hours: GitHub allows few requests).
pub async fn popular() -> Result<Value, String> {
    if let Some((t, v)) = POPULAR_CACHE.lock().unwrap().as_ref() {
        if t.elapsed() < std::time::Duration::from_secs(6 * 3600) {
            return Ok(v.clone());
        }
    }
    let mut jobs = tokio::task::JoinSet::new();
    for (i, (name, _, _)) in POPULAR.iter().enumerate() {
        let n = name.to_string();
        jobs.spawn(async move { (i, registry_entry(n).await) });
    }
    // stars: a few repos per search (a query may be at most 256 characters)
    let mut stars: BTreeMap<String, i64> = BTreeMap::new();
    let repos: Vec<&str> = POPULAR.iter().map(|p| p.2).filter(|r| !r.is_empty()).collect();
    for chunk in repos.chunks(7) {
        let q = chunk.iter().map(|r| format!("repo:{r}")).collect::<Vec<_>>().join(" ");
        if let Ok(url) = reqwest::Url::parse_with_params("https://api.github.com/search/repositories", &[("q", q.as_str()), ("per_page", "20")]) {
            if let Ok(v) = crate::skills::gh_get(url.as_str()).await {
                for it in v["items"].as_array().into_iter().flatten() {
                    stars.insert(it["full_name"].as_str().unwrap_or("").to_lowercase(), it["stargazers_count"].as_i64().unwrap_or(0));
                }
            }
        }
    }
    let mut items = Vec::new();
    while let Some(Ok((i, entry))) = jobs.join_next().await {
        let Some(x) = entry else { continue };
        let (_, cat, repo) = POPULAR[i];
        let mut it = shop_item(&x["server"], x["_meta"]["io.modelcontextprotocol.registry/official"]["publishedAt"].as_str().unwrap_or(""));
        if it["ways"].as_array().is_none_or(|w| w.is_empty()) {
            continue;
        }
        it["category"] = json!(cat);
        it["stars"] = stars.get(&repo.to_lowercase()).map(|s| json!(s)).unwrap_or(Value::Null);
        it["popular"] = json!(true);
        items.push(it);
    }
    items.sort_by_key(|v| std::cmp::Reverse(v["stars"].as_i64().unwrap_or(-1)));
    let out = json!({ "servers": items, "note": if stars.is_empty() { "อ่านจำนวนดาวจาก GitHub ไม่ได้ตอนนี้ — เรียงตามรายการ" } else { "" } });
    if !items_empty(&out) {
        *POPULAR_CACHE.lock().unwrap() = Some((std::time::Instant::now(), out.clone()));
    }
    Ok(out)
}

fn items_empty(v: &Value) -> bool {
    v["servers"].as_array().is_none_or(|a| a.is_empty())
}

/// Search the registry (newest first when nothing is typed: "try the new ones first").
pub async fn search(query: &str) -> Result<Value, String> {
    let mut url = reqwest::Url::parse(REGISTRY).map_err(|e| e.to_string())?;
    url.query_pairs_mut().append_pair("limit", "60").append_pair("version", "latest");
    if !query.trim().is_empty() {
        url.query_pairs_mut().append_pair("search", query.trim());
    }
    let c = reqwest::Client::builder().timeout(std::time::Duration::from_secs(20)).build().map_err(|e| e.to_string())?;
    let v: Value = c.get(url).header("User-Agent", "idea-autopilot").send().await.map_err(|e| format!("ค้นหาไม่ได้: {e}"))?.json().await.map_err(|e| e.to_string())?;
    let mut items: Vec<Value> = v["servers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|x| x["_meta"]["io.modelcontextprotocol.registry/official"]["status"].as_str().unwrap_or("active") == "active")
        .map(|x| shop_item(&x["server"], x["_meta"]["io.modelcontextprotocol.registry/official"]["publishedAt"].as_str().unwrap_or("")))
        .filter(|x| x["ways"].as_array().is_some_and(|w| !w.is_empty()))
        .collect();
    items.sort_by(|a, b| b["published"].as_str().unwrap_or("").cmp(a["published"].as_str().unwrap_or("")));
    Ok(json!({ "servers": items }))
}

/// Install exactly the way the owner picked (and saw): a package run by npx/uvx, or a remote https address.
pub async fn install(root: &Path, name: &str, kind: &str, index: usize, values: BTreeMap<String, String>) -> Result<Server> {
    let mut url = reqwest::Url::parse(REGISTRY).map_err(|e| anyhow::anyhow!("{e}"))?;
    url.query_pairs_mut().append_pair("search", name).append_pair("version", "latest").append_pair("limit", "20");
    let c = reqwest::Client::builder().timeout(std::time::Duration::from_secs(20)).build()?;
    let v: Value = c.get(url).header("User-Agent", "idea-autopilot").send().await?.json().await?;
    let s = v["servers"].as_array().into_iter().flatten().map(|x| &x["server"]).find(|x| x["name"].as_str() == Some(name)).context("ไม่พบเครื่องมือนี้ใน registry")?.clone();
    let values: BTreeMap<String, String> = values.into_iter().filter(|(_, v)| !v.trim().is_empty()).collect();
    let mut srv = Server {
        id: slug(name),
        label: s["title"].as_str().unwrap_or(name.rsplit('/').next().unwrap_or(name)).into(),
        description: s["description"].as_str().unwrap_or("").into(),
        source: "registry".into(),
        registry_name: name.into(),
        version: s["version"].as_str().unwrap_or("").into(),
        enabled: true,
        ..Default::default()
    };
    match kind {
        "package" => {
            let p = s["packages"].get(index).context("ไม่พบวิธีติดตั้งนี้")?;
            let (cmd, args) = pkg_command(p).context("ติดตั้งแบบนี้ยังไม่รองรับ")?;
            for e in p["environmentVariables"].as_array().into_iter().flatten() {
                let n = e["name"].as_str().unwrap_or("");
                if e["isRequired"].as_bool().unwrap_or(false) && !values.contains_key(n) {
                    bail!("ต้องใส่ {n}");
                }
            }
            srv.transport = "stdio".into();
            srv.command = cmd;
            srv.args = args;
            srv.env = values;
        }
        "remote" => {
            let r = s["remotes"].get(index).context("ไม่พบวิธีติดตั้งนี้")?;
            srv.transport = "http".into();
            srv.url = r["url"].as_str().filter(|u| u.starts_with("https://")).context("ที่อยู่ต้องเป็น https")?.into();
            srv.headers = values;
        }
        _ => bail!("ไม่รู้จักวิธีติดตั้งนี้"),
    }
    upsert(root, srv)
}

pub fn set(root: &Path, id: &str, field: &str, value: &str) -> Result<()> {
    let mut v = list(root);
    let s = v.iter_mut().find(|s| s.id == id).context("ไม่พบเครื่องมือนี้")?;
    let on = matches!(value, "1" | "true");
    match field {
        "enabled" => s.enabled = on,
        "always" => s.always = on,
        "howto" => s.howto = value.chars().take(600).collect(),
        // comma-separated skill slugs
        "skills" => s.skills = value.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect(),
        _ => bail!("ไม่รู้จักฟิลด์นี้"),
    }
    save(root, &v)
}

pub fn remove(root: &Path, id: &str) -> Result<()> {
    let mut v = list(root);
    v.retain(|s| s.id != id);
    save(root, &v)
}

/// The tools one run gets from its offered skills alone (tests; runs use `for_idea`, which adds the picked tools).
#[cfg(test)]
pub fn for_run(root: &Path, offered: &[String]) -> Vec<Server> {
    for_idea(root, &[], offered)
}

#[cfg(test)]
pub fn run_config(root: &Path, offered: &[String], key: &str, extra: &[(String, Value)]) -> Option<PathBuf> {
    config_for(root, &for_run(root, offered), key, extra)
}

// ---------- tools picked for an idea (ISSUE-171) ----------
// Before this a tool reached a bot run only through a skill that needs it (or "always"), and chats got none at all:
// an installed tool was mostly never seen by any AI. Now the owner picks tools with "/" like skills; a picked tool
// stays with the idea and goes to its bot runs and to every chat AI, with TOOLS.md saying how to use each.

fn idea_key(id: i64) -> String {
    format!("idea_tools_{id}")
}

/// The tools picked for this idea (ids).
pub fn idea_tools(c: &rusqlite::Connection, id: i64) -> Vec<String> {
    crate::db::get_setting(c, &idea_key(id)).map(|v| v.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or_default()
}

pub fn add_idea_tools(c: &rusqlite::Connection, id: i64, ids: &[String]) {
    let mut cur = idea_tools(c, id);
    for x in ids {
        if !cur.contains(x) {
            cur.push(x.clone());
        }
    }
    let _ = crate::db::set_setting(c, &idea_key(id), &cur.join(","));
}

/// What one run or chat of an idea gets: enabled tools that are "always", needed by an offered skill, or picked.
pub fn for_idea(root: &Path, picked: &[String], offered: &[String]) -> Vec<Server> {
    list(root)
        .into_iter()
        .filter(|s| s.enabled && (s.always || s.skills.iter().any(|k| offered.contains(k)) || picked.contains(&s.id)))
        .collect()
}

/// Write the `--mcp-config` file for these tools (+ extra entries); None when there is nothing to give.
pub fn config_for(root: &Path, servers: &[Server], key: &str, extra: &[(String, Value)]) -> Option<PathBuf> {
    if servers.is_empty() && extra.is_empty() {
        return None;
    }
    let mut m = Map::new();
    for s in servers {
        m.insert(s.id.clone(), claude_entry(s));
    }
    for (k, v) in extra {
        m.insert(k.clone(), v.clone());
    }
    let p = dir(root).join(format!("run-{key}.json"));
    std::fs::create_dir_all(dir(root)).ok()?;
    std::fs::write(&p, serde_json::to_string_pretty(&json!({ "mcpServers": m })).ok()?).ok()?;
    Some(p)
}

/// TOOLS.md in the idea folder: which tools this AI has and when to use each (removed when there are none).
pub fn write_tools_md(idea_dir: &Path, servers: &[Server]) {
    let p = idea_dir.join("TOOLS.md");
    if servers.is_empty() {
        let _ = std::fs::remove_file(p);
        return;
    }
    let mut s = String::from(
        "# Tools (MCP) you have in this run\n\nThe app gave you these MCP servers; their tools show up in your tool list under the server name. \
         Use one when the task fits what it is for — prefer it over writing your own script for the same job.\n\n",
    );
    for t in servers {
        let how = if t.howto.trim().is_empty() { t.description.trim() } else { t.howto.trim() };
        s.push_str(&format!("## {} (`{}`)\n{}\n\n", if t.label.is_empty() { &t.id } else { &t.label }, t.id, if how.is_empty() { "(no description)" } else { how }));
    }
    let _ = std::fs::write(p, s);
}

/// "/tool-<id>" in a message: the tools it names and the message without those names.
pub fn tool_tokens(root: &Path, text: &str) -> (Vec<Server>, String) {
    let all = list(root);
    let mut found: Vec<Server> = vec![];
    let mut rest = text.to_string();
    for (s, e, name) in crate::skills::slash_tokens(text).into_iter().rev() {
        let Some(id) = name.strip_prefix("tool-") else { continue };
        let Some(t) = all.iter().find(|t| t.id.to_lowercase() == id && t.enabled) else { continue };
        rest.replace_range(s..e, "");
        if !found.iter().any(|f| f.id == t.id) {
            found.insert(0, t.clone());
        }
    }
    let rest = rest.lines().map(|l| l.split(' ').filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ")).collect::<Vec<_>>().join("\n");
    (found, rest.trim().to_string())
}

/// The line that asks the AI to use the picked tools.
pub fn tool_request(tools: &[Server]) -> String {
    let names = tools.iter().map(|t| format!("\"{}\" (MCP server `{}`)", if t.label.is_empty() { &t.id } else { &t.label }, t.id)).collect::<Vec<_>>().join(", ");
    format!("Use the tool {names} for this request — see TOOLS.md for what it is for. If it is not in your tool list (a chat opened before it was picked), say so: the owner reopens the chat to load it.")
}

/// Tools for the "/" menu, next to the skills.
pub fn slash_items(root: &Path) -> Vec<Value> {
    list(root)
        .into_iter()
        .filter(|s| s.enabled)
        .map(|s| json!({ "slug": format!("tool-{}", s.id.to_lowercase()), "name": if s.label.is_empty() { s.id.clone() } else { s.label.clone() }, "description": if s.howto.is_empty() { s.description.clone() } else { s.howto.clone() }, "kind": "tool" }))
        .collect()
}

fn toml_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into()) // a JSON string is a valid TOML basic string
}

fn toml_table(m: &Map<String, Value>) -> String {
    let parts: Vec<String> = m.iter().filter_map(|(k, v)| v.as_str().map(|v| format!("{} = {}", toml_str(k), toml_str(v)))).collect();
    format!("{{ {} }}", parts.join(", "))
}

/// Give the run's tools (a `--mcp-config` file in Claude's shape) to whichever agent runs it, in that agent's own way:
/// Claude `--mcp-config`, Codex `-c mcp_servers.<id>.…`, Gemini a project `.gemini/settings.json` + an allow-list,
/// OpenCode a config file named by `OPENCODE_CONFIG`. Other agents: nothing (they run without tools as before).
pub fn apply_to_run(provider: &str, cfg: &Path, cwd: &Path, args: &mut Vec<String>, env: &mut Vec<(String, String)>, run_env: &[(String, String)]) {
    let Some(servers) = std::fs::read_to_string(cfg).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()).and_then(|v| v["mcpServers"].as_object().cloned()) else { return };
    if servers.is_empty() {
        return;
    }
    match provider {
        // Qwen Code takes the same {"mcpServers": …} file as Claude
        "claude" | "qwen" => args.extend(["--mcp-config".to_string(), cfg.to_string_lossy().into_owned()]),
        "codex" => {
            let mut flags = vec![];
            for (id, s) in &servers {
                let mut kv = vec![];
                if let Some(u) = s["url"].as_str() {
                    kv.push(format!("mcp_servers.{id}.url={}", toml_str(u)));
                } else {
                    kv.push(format!("mcp_servers.{id}.command={}", toml_str(s["command"].as_str().unwrap_or(""))));
                    let a: Vec<String> = s["args"].as_array().into_iter().flatten().filter_map(|x| x.as_str()).map(toml_str).collect();
                    kv.push(format!("mcp_servers.{id}.args=[{}]", a.join(", ")));
                    if let Some(e) = s["env"].as_object().filter(|e| !e.is_empty()) {
                        kv.push(format!("mcp_servers.{id}.env={}", toml_table(e)));
                    }
                }
                for x in kv {
                    flags.push("-c".to_string());
                    flags.push(x);
                }
            }
            // the prompt marker "-" stays last
            let at = args.iter().rposition(|a| a == "-").unwrap_or(args.len());
            args.splice(at..at, flags);
        }
        "gemini" => {
            let mut m = Map::new();
            for (id, s) in &servers {
                let v = match s["url"].as_str() {
                    Some(u) => json!({ "httpUrl": u, "headers": s["headers"] }),
                    None => json!({ "command": s["command"], "args": s["args"], "env": s["env"] }),
                };
                m.insert(id.clone(), v);
            }
            let p = cwd.join(".gemini").join("settings.json");
            let mut cur: Value = std::fs::read_to_string(&p).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_else(|| json!({}));
            cur["mcpServers"] = Value::Object(m);
            if std::fs::create_dir_all(cwd.join(".gemini")).is_ok() {
                let _ = std::fs::write(&p, serde_json::to_string_pretty(&cur).unwrap_or_default());
            }
            args.push("--allowed-mcp-server-names".into());
            args.extend(servers.keys().cloned());
        }
        "opencode" => {
            let mut m = Map::new();
            for (id, s) in &servers {
                let v = match s["url"].as_str() {
                    Some(u) => json!({ "type": "remote", "url": u, "headers": s["headers"], "enabled": true }),
                    None => {
                        let mut cmd = vec![s["command"].clone()];
                        cmd.extend(s["args"].as_array().cloned().unwrap_or_default());
                        json!({ "type": "local", "command": cmd, "environment": s["env"], "enabled": true })
                    }
                };
                m.insert(id.clone(), v);
            }
            let p = cfg.with_extension("opencode.json");
            if std::fs::write(&p, serde_json::to_string_pretty(&json!({ "$schema": "https://opencode.ai/config.json", "mcp": m })).unwrap_or_default()).is_ok() {
                env.push(("OPENCODE_CONFIG".into(), p.to_string_lossy().into_owned()));
            }
        }
        // Antigravity: the workspace's .agents/mcp_config.json (remote servers are `serverUrl` there)
        "antigravity" => {
            let mut m = Map::new();
            for (id, s) in &servers {
                let v = match s["url"].as_str() {
                    Some(u) => json!({ "serverUrl": u, "headers": s["headers"] }),
                    None => json!({ "command": s["command"], "args": s["args"], "env": s["env"] }),
                };
                m.insert(id.clone(), v);
            }
            set_agy(cwd, Some(m));
        }
        // Vibe reads tools only from its config.toml: written into the account's own VIBE_HOME (never the machine's ~/.vibe)
        "mistral" => {
            if let Some((_, home)) = run_env.iter().find(|(k, _)| k == "VIBE_HOME") {
                set_vibe(Path::new(home), Some(&servers));
            }
        }
        _ => {}
    }
}

/// Names of the tools the app put into this Vibe home, so they (and only they) are replaced or taken out next time,
/// even after Vibe rewrote its config.toml (it keeps no comments).
fn vibe_managed(home: &Path) -> PathBuf {
    home.join("ia-mcp-names.json")
}

fn vibe_server(id: &str, s: &Value) -> toml::Value {
    let mut t = toml::map::Map::new();
    t.insert("name".into(), toml::Value::String(id.into()));
    match s["url"].as_str() {
        Some(u) => {
            t.insert("transport".into(), toml::Value::String("streamable-http".into()));
            t.insert("url".into(), toml::Value::String(u.into()));
        }
        None => {
            t.insert("transport".into(), toml::Value::String("stdio".into()));
            t.insert("command".into(), toml::Value::String(s["command"].as_str().unwrap_or("").into()));
            let a: Vec<toml::Value> = s["args"].as_array().into_iter().flatten().filter_map(|x| x.as_str()).map(|x| toml::Value::String(x.into())).collect();
            t.insert("args".into(), toml::Value::Array(a));
            if let Some(e) = s["env"].as_object().filter(|e| !e.is_empty()) {
                let m: toml::map::Map<String, toml::Value> = e.iter().filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), toml::Value::String(v.into())))).collect();
                t.insert("env".into(), toml::Value::Table(m));
            }
        }
    }
    toml::Value::Table(t)
}

/// Put this run's tools into a Vibe account's own config.toml, or take them out (None). Everything else in the file is
/// kept; the first time, the file is copied to config.toml.ia-backup. A file that does not read as TOML is left alone.
pub fn set_vibe(home: &Path, servers: Option<&Map<String, Value>>) {
    let p = home.join("config.toml");
    let managed: Vec<String> = std::fs::read_to_string(vibe_managed(home)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let want: Vec<(String, Value)> = servers.map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default();
    if managed.is_empty() && want.is_empty() {
        return;
    }
    let text = std::fs::read_to_string(&p).unwrap_or_default();
    let Ok(mut doc) = text.parse::<toml::Table>() else { return };
    let bak = home.join("config.toml.ia-backup");
    if p.is_file() && !bak.exists() {
        let _ = std::fs::copy(&p, &bak);
    }
    let ours: Vec<&str> = managed.iter().map(String::as_str).chain(want.iter().map(|(k, _)| k.as_str())).collect();
    let mut list: Vec<toml::Value> = doc.get("mcp_servers").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    list.retain(|s| !s.get("name").and_then(|n| n.as_str()).is_some_and(|n| ours.contains(&n)));
    for (id, s) in &want {
        list.push(vibe_server(id, s));
    }
    if list.is_empty() {
        doc.remove("mcp_servers");
    } else {
        doc.insert("mcp_servers".into(), toml::Value::Array(list));
    }
    if let Ok(out) = toml::to_string(&doc) {
        let _ = std::fs::create_dir_all(home);
        let _ = std::fs::write(&p, out);
        let names: Vec<&String> = want.iter().map(|(k, _)| k).collect();
        let _ = std::fs::write(vibe_managed(home), serde_json::to_string(&names).unwrap_or_default());
    }
}

/// A run without tools: drop what an earlier run left in the idea's `.gemini/settings.json` (Gemini would load it).
/// The tools of an Antigravity run in `<idea>/.agents/mcp_config.json` (None = take the app's out again).
pub fn set_agy(cwd: &Path, servers: Option<Map<String, Value>>) {
    let p = cwd.join(".agents").join("mcp_config.json");
    let mut cur: Value = std::fs::read_to_string(&p).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_else(|| json!({}));
    match servers {
        Some(m) => cur["mcpServers"] = Value::Object(m),
        None => {
            if cur.as_object_mut().and_then(|o| o.remove("mcpServers")).is_none() {
                return;
            }
        }
    }
    if std::fs::create_dir_all(cwd.join(".agents")).is_ok() {
        let _ = std::fs::write(&p, serde_json::to_string_pretty(&cur).unwrap_or_default());
    }
}

pub fn clear_gemini(cwd: &Path) {
    let p = cwd.join(".gemini").join("settings.json");
    let Some(mut v) = std::fs::read_to_string(&p).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()) else { return };
    if v.as_object_mut().and_then(|o| o.remove("mcpServers")).is_some() {
        let _ = std::fs::write(&p, serde_json::to_string_pretty(&v).unwrap_or_default());
    }
}

/// The settings page list: values of secrets are never sent back.
pub fn list_json(root: &Path) -> Value {
    let v: Vec<Value> = list(root)
        .into_iter()
        .map(|s| {
            let shown = if s.transport == "http" { s.url.clone() } else { format!("{} {}", s.command, s.args.join(" ")) };
            json!({
                "id": s.id, "label": s.label, "description": s.description, "source": s.source, "transport": s.transport, "command": shown,
                "env_names": s.env.keys().chain(s.headers.keys()).cloned().collect::<Vec<_>>(), "enabled": s.enabled, "always": s.always,
                "skills": s.skills, "category": s.category, "version": s.version, "registry_name": s.registry_name, "howto": s.howto,
            })
        })
        .collect();
    json!(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("ia-mcp-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn a_run_gets_only_the_tools_its_skills_need() {
        let root = tmp("run");
        let rea = Server { id: "rea".into(), command: "cmd.exe".into(), args: vec!["/c".into(), "rea".into()], enabled: true, skills: vec!["reverse-engineer-anything".into()], ..Default::default() };
        let web = Server { id: "web".into(), transport: "http".into(), url: "https://x.test/mcp".into(), enabled: true, always: true, ..Default::default() };
        let off = Server { id: "off".into(), command: "x".into(), enabled: false, always: true, ..Default::default() };
        save(&root, &[rea, web, off]).unwrap();
        let ids = |v: Vec<Server>| v.into_iter().map(|s| s.id).collect::<Vec<_>>();
        assert_eq!(ids(for_run(&root, &["impeccable".into()])), vec!["web"]);
        assert_eq!(ids(for_run(&root, &["reverse-engineer-anything".into()])), vec!["rea", "web"]);
        let p = run_config(&root, &["reverse-engineer-anything".into()], "t", &[]).unwrap();
        let v: Value = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
        assert_eq!(v["mcpServers"]["rea"]["command"], "cmd.exe");
        assert_eq!(v["mcpServers"]["web"]["type"], "http");
        set(&root, "web", "always", "0").unwrap();
        assert!(run_config(&root, &[], "t2", &[]).is_none());
        assert!(run_config(&root, &[], "t3", &[("team".into(), json!({ "command": "ia" }))]).is_some());
        // a tool picked for an idea goes to it without any skill, and "/tool-<id>" names it in a message (ISSUE-171)
        assert_eq!(ids(for_idea(&root, &["rea".into()], &[])), vec!["rea"]);
        let (found, rest) = tool_tokens(&root, "/tool-rea แกะไฟล์นี้หน่อย");
        assert_eq!(ids(found), vec!["rea"]);
        assert_eq!(rest, "แกะไฟล์นี้หน่อย");
        let d = root.join("idea");
        std::fs::create_dir_all(&d).unwrap();
        write_tools_md(&d, &for_idea(&root, &["rea".into()], &[]));
        assert!(std::fs::read_to_string(d.join("TOOLS.md")).unwrap().contains("`rea`"));
        write_tools_md(&d, &[]);
        assert!(!d.join("TOOLS.md").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn each_agent_gets_the_tools_its_own_way() {
        let root = tmp("apply");
        std::fs::create_dir_all(&root).unwrap();
        let cfg = root.join("run.json");
        std::fs::write(&cfg, r#"{"mcpServers":{"rea":{"command":"cmd.exe","args":["/c","rea \"x\""],"env":{"K":"v"}},"web":{"type":"http","url":"https://x.test/mcp"}}}"#).unwrap();
        let base = |p: &str| -> (Vec<String>, Vec<(String, String)>) {
            let mut a: Vec<String> = if p == "codex" { vec!["exec".into(), "-".into()] } else { vec!["-p".into(), "go".into()] };
            let mut e = vec![];
            apply_to_run(p, &cfg, &root, &mut a, &mut e, &[]);
            (a, e)
        };
        let (a, _) = base("claude");
        assert_eq!(a[a.len() - 2], "--mcp-config");
        let (a, _) = base("codex");
        assert_eq!(a.last().unwrap(), "-");
        assert!(a.contains(&r#"mcp_servers.rea.args=["/c", "rea \"x\""]"#.to_string()), "{a:?}");
        assert!(a.contains(&r#"mcp_servers.rea.env={ "K" = "v" }"#.to_string()));
        assert!(a.contains(&r#"mcp_servers.web.url="https://x.test/mcp""#.to_string()));
        let (a, _) = base("gemini");
        assert!(a.ends_with(&["--allowed-mcp-server-names".to_string(), "rea".to_string(), "web".to_string()]));
        let g: Value = serde_json::from_str(&std::fs::read_to_string(root.join(".gemini").join("settings.json")).unwrap()).unwrap();
        assert_eq!(g["mcpServers"]["web"]["httpUrl"], "https://x.test/mcp");
        let (_, e) = base("opencode");
        let oc: Value = serde_json::from_str(&std::fs::read_to_string(&e[0].1).unwrap()).unwrap();
        assert_eq!(oc["mcp"]["rea"]["command"][0], "cmd.exe");
        assert_eq!(oc["mcp"]["web"]["type"], "remote");
        let (a, e) = base("mistral");
        assert_eq!(a.len(), 2);
        assert!(e.is_empty());
        // Vibe: a managed block in the account's own config.toml, the rest untouched, and taken out again
        let home = root.join("vibe");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join("config.toml"), "active_model = \"m\"\n\n[[models]]\nname = \"m\"\n\n[[mcp_servers]]\nname = \"mine\"\ntransport = \"stdio\"\ncommand = \"x\"\n").unwrap();
        let mut a2 = vec![];
        let mut e2 = vec![];
        apply_to_run("mistral", &cfg, &root, &mut a2, &mut e2, &[("VIBE_HOME".into(), home.to_string_lossy().into_owned())]);
        let t = std::fs::read_to_string(home.join("config.toml")).unwrap();
        assert!(t.contains("active_model") && t.contains("[[models]]") && t.contains("name = \"rea\"") && t.contains("name = \"mine\"") && t.contains("transport = \"streamable-http\""), "{t}");
        // run again (as if Vibe had rewritten the file): no duplicates
        apply_to_run("mistral", &cfg, &root, &mut a2, &mut e2, &[("VIBE_HOME".into(), home.to_string_lossy().into_owned())]);
        assert_eq!(std::fs::read_to_string(home.join("config.toml")).unwrap().matches("name = \"rea\"").count(), 1);
        assert!(home.join("config.toml.ia-backup").is_file());
        set_vibe(&home, None);
        let t2 = std::fs::read_to_string(home.join("config.toml")).unwrap();
        assert!(!t2.contains("\"rea\"") && !t2.contains("\"web\"") && t2.contains("name = \"mine\"") && t2.contains("[[models]]"), "{t2}");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// VIBE_TEST_HOME=<a copy of a Vibe home> cargo test --lib -- --ignored vibe_real, then let Vibe read it
    #[test]
    #[ignore]
    fn vibe_real_home() {
        let home = PathBuf::from(std::env::var("VIBE_TEST_HOME").unwrap());
        let m: Map<String, Value> = serde_json::from_str(r#"{"rea":{"command":"cmd.exe","args":["/c","echo","x"],"env":{"K":"v"}},"web":{"url":"https://x.test/mcp"}}"#).unwrap();
        set_vibe(&home, Some(&m));
    }

    #[test]
    fn registry_packages_become_commands() {
        let npm = json!({ "registryType": "npm", "identifier": "@x/srv", "version": "1.2.0" });
        let (c, a) = pkg_command(&npm).unwrap();
        assert!(a.contains(&"@x/srv@1.2.0".to_string()));
        assert!(if cfg!(windows) { c == "cmd" } else { c == "npx" });
        assert_eq!(pkg_command(&json!({ "registryType": "pypi", "identifier": "srv", "version": "2" })).unwrap(), ("uvx".into(), vec!["srv==2".into()]));
        assert!(pkg_command(&json!({ "registryType": "oci", "identifier": "img" })).is_none());
        assert_eq!(slug("io.github.someone/My Tool"), "my-tool");
    }

    /// Talks to the registry and GitHub: `cargo test mcp::tests::live -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live_popular_tools_resolve_in_the_registry() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let v = rt.block_on(popular()).unwrap();
        let items = v["servers"].as_array().unwrap();
        for it in items {
            println!("{} ★{} {}", it["name"], it["stars"], it["ways"].as_array().unwrap().len());
        }
        assert!(items.len() >= POPULAR.len() - 4, "only {} of {} found", items.len(), POPULAR.len());
    }
}
