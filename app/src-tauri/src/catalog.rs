//! The AI providers an account can be made for, and how each one is reached and signed in.
//!
//! Every provider is driven by one of the agent programs the app knows (its `runner`): Claude Code, Codex,
//! Gemini CLI, Mistral Vibe or OpenCode. Some providers speak the Anthropic API shape, so Claude Code can
//! drive them through a base URL and a token; OpenCode reaches many more with just an API key. Signing in is
//! either the agent's own login (a browser flow) or an API key / token, which the app keeps in the account's
//! own folder and hands to the agent through the environment variable that provider documents.
//! Addresses and model names follow each provider's documentation at the time of writing: they are starting
//! points the owner can change, not promises.

pub struct Vendor {
    pub id: &'static str,
    pub label: &'static str,
    /// the agent that runs it: claude | codex | gemini | mistral | opencode
    pub runner: &'static str,
    /// "login" = the agent's own sign-in, "key" = an API key / token
    pub auth: &'static [&'static str],
    /// environment variables that receive the key
    pub key_env: &'static [&'static str],
    /// API address for providers reached through Claude Code (empty = the provider's own default)
    pub base_url: &'static str,
    pub models: &'static [&'static str],
    pub note: &'static str,
}

pub const VENDORS: &[Vendor] = &[
    Vendor { id: "anthropic", label: "Anthropic — Claude", runner: "claude", auth: &["login", "key"], key_env: &["ANTHROPIC_API_KEY"], base_url: "", models: &["opus", "sonnet", "haiku"], note: "Claude Code: ล็อกอินบัญชี Claude (Pro/Max) หรือใช้ API key" },
    Vendor { id: "openai", label: "OpenAI — Codex (GPT)", runner: "codex", auth: &["login", "key"], key_env: &["OPENAI_API_KEY"], base_url: "", models: &[], note: "Codex CLI: ล็อกอินด้วย ChatGPT หรือใช้ API key — รายชื่อโมเดลอ่านจาก Codex เอง" },
    Vendor { id: "google", label: "Google — Gemini", runner: "gemini", auth: &["login", "key"], key_env: &["GEMINI_API_KEY"], base_url: "", models: &["gemini-2.5-pro", "gemini-2.5-flash", "gemini-2.5-flash-lite"], note: "Gemini CLI: ล็อกอินด้วย Google หรือใช้ API key" },
    Vendor { id: "mistral", label: "Mistral — Vibe (Devstral)", runner: "mistral", auth: &["key"], key_env: &["MISTRAL_API_KEY"], base_url: "", models: &["mistral-medium-3.5", "mistral-large-4-0-3", "glm-5-3", "zai-glm-5-3-flash"], note: "Mistral Vibe: ใช้ API key จาก console.mistral.ai · ชื่อโมเดลเป็นชื่อย่อของ Vibe เอง (กด “อ่านรายชื่อโมเดลใหม่” เพื่ออ่านจาก Vibe จริง)" },
    Vendor { id: "deepseek", label: "DeepSeek", runner: "claude", auth: &["key"], key_env: &["ANTHROPIC_AUTH_TOKEN"], base_url: "https://api.deepseek.com/anthropic", models: &["deepseek-chat", "deepseek-reasoner"], note: "ผ่าน Claude Code ด้วย API แบบ Anthropic ของ DeepSeek" },
    Vendor { id: "moonshot", label: "Moonshot — Kimi", runner: "claude", auth: &["key"], key_env: &["ANTHROPIC_AUTH_TOKEN"], base_url: "https://api.moonshot.ai/anthropic", models: &["kimi-k2-turbo-preview", "kimi-k2-0905-preview"], note: "ผ่าน Claude Code ด้วย API แบบ Anthropic ของ Moonshot" },
    Vendor { id: "zhipu", label: "Z.ai — GLM", runner: "claude", auth: &["key"], key_env: &["ANTHROPIC_AUTH_TOKEN"], base_url: "https://api.z.ai/api/anthropic", models: &["glm-4.6", "glm-4.5-air"], note: "ผ่าน Claude Code ด้วย API แบบ Anthropic ของ Z.ai" },
    Vendor { id: "openrouter", label: "OpenRouter (หลายร้อยโมเดล)", runner: "opencode", auth: &["key"], key_env: &["OPENROUTER_API_KEY"], base_url: "", models: &[], note: "ผ่าน OpenCode — ชื่อโมเดลรูปแบบ openrouter/<ค่าย>/<โมเดล>" },
    Vendor { id: "xai", label: "xAI — Grok", runner: "opencode", auth: &["key"], key_env: &["XAI_API_KEY"], base_url: "", models: &[], note: "ผ่าน OpenCode — ชื่อโมเดลรูปแบบ xai/<โมเดล>" },
    Vendor { id: "groq", label: "Groq", runner: "opencode", auth: &["key"], key_env: &["GROQ_API_KEY"], base_url: "", models: &[], note: "ผ่าน OpenCode — ชื่อโมเดลรูปแบบ groq/<โมเดล>" },
    Vendor { id: "together", label: "Together AI", runner: "opencode", auth: &["key"], key_env: &["TOGETHER_API_KEY"], base_url: "", models: &[], note: "ผ่าน OpenCode — ชื่อโมเดลรูปแบบ together/<โมเดล>" },
    Vendor { id: "fireworks", label: "Fireworks AI", runner: "opencode", auth: &["key"], key_env: &["FIREWORKS_API_KEY"], base_url: "", models: &[], note: "ผ่าน OpenCode — ชื่อโมเดลรูปแบบ fireworks/<โมเดล>" },
    Vendor { id: "cerebras", label: "Cerebras", runner: "opencode", auth: &["key"], key_env: &["CEREBRAS_API_KEY"], base_url: "", models: &[], note: "ผ่าน OpenCode — ชื่อโมเดลรูปแบบ cerebras/<โมเดล>" },
    Vendor { id: "opencode", label: "OpenCode (ล็อกอินเอง เลือกค่ายใน OpenCode)", runner: "opencode", auth: &["login"], key_env: &[], base_url: "", models: &[], note: "OpenCode จัดการล็อกอินของค่ายต่างๆ เอง (opencode auth login)" },
    Vendor { id: "omniroute", label: "OmniRoute (เกตเวย์ในเครื่อง รวมหลายค่าย)", runner: "claude", auth: &["key"], key_env: &["ANTHROPIC_AUTH_TOKEN"], base_url: "http://localhost:20128", models: &["auto/coding", "auto", "auto/fast", "auto/cheap", "auto/thrifty", "auto/offline", "auto/smart", "auto/subscription"], note: "ผ่าน Claude Code → OmniRoute ที่รันในเครื่อง (localhost:20128) ซึ่งส่งต่อไปยังค่ายที่คุณเชื่อมไว้ใน OmniRoute · key ไม่บังคับ (ใส่เพื่อคัดโมเดลที่เรียกเครื่องมือไม่ได้ออกก่อน — สร้างที่ dashboard ของ OmniRoute → API Manager) · แอปอ่านโควต้าของแต่ละค่ายจาก OmniRoute ในเครื่องให้เอง · ถ้า OmniRoute ยังไม่ได้เปิด แอปเปิดให้เอง" },
    Vendor { id: "custom", label: "API แบบ Anthropic ที่ระบุที่อยู่เอง", runner: "claude", auth: &["key"], key_env: &["ANTHROPIC_AUTH_TOKEN"], base_url: "", models: &[], note: "ใส่ที่อยู่ API (base URL) ของผู้ให้บริการที่รองรับรูปแบบ Anthropic + token + ชื่อโมเดล" },
];

pub fn vendor(id: &str) -> Option<&'static Vendor> {
    VENDORS.iter().find(|v| v.id == id)
}

/// The vendor an account of this agent is, when nothing else was chosen.
pub fn native(runner: &str) -> &'static str {
    match runner {
        "claude" => "anthropic",
        "codex" => "openai",
        "gemini" => "google",
        "mistral" => "mistral",
        _ => "opencode",
    }
}

/// Does the agent behind this vendor need a model name that is not one of the agent's own aliases?
pub fn needs_model_name(v: &Vendor) -> bool {
    v.runner == "claude" && v.id != "anthropic"
}

/// Environment for an account that signs in with a key: the key under each variable the provider documents, and its
/// API address when it has one (`base_override` wins).
pub fn key_env(v: &Vendor, key: &str, base_override: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = v.key_env.iter().map(|k| ((*k).to_string(), key.to_string())).collect();
    let base = if base_override.trim().is_empty() { v.base_url } else { base_override.trim() };
    if v.runner == "claude" && !base.is_empty() {
        out.push(("ANTHROPIC_BASE_URL".into(), base.to_string()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_vendor_is_driven_by_a_known_agent_and_ids_are_unique() {
        let mut ids = std::collections::BTreeSet::new();
        for v in VENDORS {
            assert!(ids.insert(v.id), "duplicate {}", v.id);
            assert!(crate::accounts::KINDS.contains(&v.runner), "{}", v.id);
            assert!(!v.auth.is_empty());
            if v.auth.contains(&"key") && v.id != "opencode" {
                assert!(!v.key_env.is_empty(), "{} has no key variable", v.id);
            }
        }
        assert_eq!(native("claude"), "anthropic");
    }

    #[test]
    fn a_key_goes_to_the_documented_variables_and_base_url() {
        let ds = vendor("deepseek").unwrap();
        let e = key_env(ds, "k", "");
        assert!(e.contains(&("ANTHROPIC_AUTH_TOKEN".into(), "k".into())));
        assert!(e.contains(&("ANTHROPIC_BASE_URL".into(), "https://api.deepseek.com/anthropic".into())));
        let c = key_env(vendor("custom").unwrap(), "k", "https://example.test/anthropic");
        assert!(c.contains(&("ANTHROPIC_BASE_URL".into(), "https://example.test/anthropic".into())));
        assert_eq!(key_env(vendor("mistral").unwrap(), "k", ""), vec![("MISTRAL_API_KEY".to_string(), "k".to_string())]);
        assert!(needs_model_name(ds) && !needs_model_name(vendor("anthropic").unwrap()));
    }
}

/// A model the provider lists at no charge: OpenRouter's `:free` variants and OpenCode's `-free` ones. Only what the
/// name itself says; a provider's free *tier* (rate-limited use of a paid model) is not marked.
pub fn is_free_model(id: &str) -> bool {
    let l = id.to_lowercase();
    l.ends_with(":free") || l.ends_with("-free") || l.contains("/free/")
}

#[cfg(test)]
mod free_tests {
    use super::*;

    #[test]
    fn free_models_are_recognised_by_name() {
        assert!(is_free_model("openrouter/meta-llama/llama-3.3-70b-instruct:free"));
        assert!(is_free_model("opencode/grok-code-free"));
        assert!(!is_free_model("openrouter/anthropic/claude-sonnet-4"));
    }
}
