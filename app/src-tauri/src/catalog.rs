//! The AI providers an account can be made for, and how each one is reached and signed in.
//!
//! Every provider is driven by one of the agent programs the app knows (its `runner`): Claude Code, Codex,
//! Gemini CLI, Mistral Vibe or OpenCode. Some providers speak the Anthropic API shape, so Claude Code can
//! drive them through a base URL and a token; OpenCode reaches many more with just an API key. Signing in is
//! either the agent's own login (a browser flow) or an API key / token, which the app keeps in the account's
//! own folder and hands to the agent through the environment variable that provider documents.
//! Addresses and model names follow each provider's documentation at the time of writing: they are starting
//! points the owner can change, not promises.

/// The Claude models in the order and with the names Claude Code's own model picker uses, so the app and the Claude
/// app say the same thing (ISSUE-097): (id passed to `--model`, name shown, "main" = the first list, "more" = under
/// "more models", note). Update this list when Claude Code's picker changes.
pub const CLAUDE_MODELS: &[(&str, &str, &str, &str)] = &[
    ("claude-opus-5-5", "Opus 5.5", "main", ""),
    ("claude-sonnet-5-5", "Sonnet 5.5", "main", "ค่าเริ่มต้นของ Claude Code"),
    ("claude-fable-5-2", "Fable 5.2", "main", ""),
    ("claude-haiku-5-5", "Haiku 5.5", "main", ""),
    ("claude-fable-5-1", "Fable 5.1", "more", ""),
    ("claude-haiku-4-5", "Haiku 4.5", "more", ""),
    ("claude-sonnet-5", "Sonnet 5", "more", ""),
    ("claude-opus-5", "Opus 5", "more", ""),
    ("claude-fable-5", "Fable 5", "more", ""),
    ("claude-opus-4-8", "Opus 4.8", "more", ""),
    ("claude-opus-4-7", "Opus 4.7", "more", ""),
    ("claude-opus-4-6", "Opus 4.6", "more", ""),
    ("claude-sonnet-4-6", "Sonnet 4.6", "more", ""),
];
/// Claude Code's effort levels (its picker: Low, Medium, High, Extra, Max)
pub const CLAUDE_EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

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
    Vendor { id: "antigravity", label: "Google — Antigravity CLI", runner: "antigravity", auth: &["login"], key_env: &[], base_url: "", models: &["gemini-4-hydrogen-high", "gemini-4-hydrogen-medium", "gemini-3.8-flash-medium", "gemini-3.8-flash-low", "claude-sonnet-4-6", "claude-opus-4-6-thinking", "gpt-oss-120b-medium"], note: "Antigravity CLI (agy): ล็อกอินบัญชี Google (ฟรีมีโควต้ารายสัปดาห์) · แอปอ่านโควต้าจริงจาก agy /usage · โมเดล Claude/GPT ใช้โควต้ากลุ่มแยกจาก Gemini · กด “อ่านรายชื่อโมเดลใหม่” เพื่ออ่านจาก agy จริง" },
    Vendor { id: "qwencloud", label: "QwenCloud — Token Plan (Qwen Code)", runner: "qwen", auth: &["key"], key_env: &["OPENAI_API_KEY"], base_url: "https://token-plan.maas.qwencloudapi.com/compatible-mode/v1", models: &["qwen3.8-max", "qwen3.7-plus", "qwen3.8-flash", "glm-5.3", "deepseek-v4-pro-0813", "deepseek-v4.1-flash"], note: "ผ่าน Qwen Code (qwen) ด้วย key ของ QwenCloud Token Plan (ต่างประเทศ) · ถ้าใช้ QwenCloud แบบจ่ายตามใช้ เปลี่ยนที่อยู่ API เป็น https://maas.qwencloudapi.com/compatible-mode/v1 · ไม่มี Qwen ฟรีแล้ว (Qwen OAuth ฟรีปิดไปเมื่อ เม.ย. 2026)" },
    Vendor { id: "qwen-coding", label: "Alibaba Model Studio — Coding Plan (Qwen Code)", runner: "qwen", auth: &["key"], key_env: &["OPENAI_API_KEY"], base_url: "https://coding-intl.dashscope.aliyuncs.com/v1", models: &["qwen3.7-plus", "qwen3.6-plus", "qwen3.5-plus", "kimi-k2.5", "glm-5", "MiniMax-M2.5"], note: "ผ่าน Qwen Code ด้วย key ของ Coding Plan (ขึ้นต้น sk-sp-) · ต่างประเทศ coding-intl… / จีนใช้ https://coding.dashscope.aliyuncs.com/v1 · โควต้านับเป็นคำขอ (รอบ 5 ชม./สัปดาห์/เดือน) ดูได้ในหน้า console เท่านั้น" },
    Vendor { id: "dashscope", label: "Alibaba Model Studio — จ่ายตามใช้ (Qwen Code)", runner: "qwen", auth: &["key"], key_env: &["OPENAI_API_KEY"], base_url: "https://dashscope-intl.aliyuncs.com/compatible-mode/v1", models: &["qwen3.7-max", "qwen3.7-plus", "qwen3.6-flash", "qwen3-coder-plus"], note: "ผ่าน Qwen Code ด้วย API key ของ Model Studio (DashScope) ภูมิภาคสิงคโปร์ · จีนใช้ https://dashscope.aliyuncs.com/compatible-mode/v1 · คิดตามโทเคน" },
    Vendor { id: "qwen", label: "Qwen Code (ตั้งค่าใน Qwen Code เอง)", runner: "qwen", auth: &["login"], key_env: &[], base_url: "", models: &[], note: "ใช้ผู้ให้บริการที่ตั้งไว้ใน Qwen Code เอง (/auth หรือ settings.json) — รายชื่อโมเดลอ่านจาก settings.json ของ Qwen Code · ถ้าล็อกอินแยก ให้เปิด qwen ในหน้าต่างล็อกอินแล้วตั้ง /auth" },
    Vendor { id: "mistral", label: "Mistral — Vibe (Devstral)", runner: "mistral", auth: &["key"], key_env: &["MISTRAL_API_KEY"], base_url: "", models: &["mistral-medium-3.5", "mistral-large-4-0-3", "glm-5-3", "zai-glm-5-3-flash"], note: "Mistral Vibe: ใช้ API key จาก console.mistral.ai · ชื่อโมเดลเป็นชื่อย่อของ Vibe เอง (กด “อ่านรายชื่อโมเดลใหม่” เพื่ออ่านจาก Vibe จริง)" },
    Vendor { id: "deepseek", label: "DeepSeek", runner: "claude", auth: &["key"], key_env: &["ANTHROPIC_AUTH_TOKEN"], base_url: "https://api.deepseek.com/anthropic", models: &["deepseek-chat", "deepseek-reasoner"], note: "ผ่าน Claude Code ด้วย API แบบ Anthropic ของ DeepSeek" },
    Vendor { id: "moonshot", label: "Moonshot — Kimi", runner: "claude", auth: &["key"], key_env: &["ANTHROPIC_AUTH_TOKEN"], base_url: "https://api.moonshot.ai/anthropic", models: &["kimi-k2-turbo-preview", "kimi-k2-0905-preview"], note: "ผ่าน Claude Code ด้วย API แบบ Anthropic ของ Moonshot" },
    Vendor { id: "zhipu", label: "Z.ai — GLM", runner: "claude", auth: &["key"], key_env: &["ANTHROPIC_AUTH_TOKEN"], base_url: "https://api.z.ai/api/anthropic", models: &["glm-4.6", "glm-4.5-air"], note: "ผ่าน Claude Code ด้วย API แบบ Anthropic ของ Z.ai" },
    Vendor { id: "stepfun", label: "StepFun — Step", runner: "claude", auth: &["key"], key_env: &["ANTHROPIC_AUTH_TOKEN"], base_url: "https://api.stepfun.ai", models: &["step-5-preview", "step-3.7-flash", "step-3.5-flash", "step-3.5-flash-2603"], note: "ผ่าน Claude Code ด้วย API แบบ Anthropic ของ StepFun · key จาก platform.stepfun.ai/interface-key · ถ้าใช้แพ็กเกจ Step Plan ให้เปลี่ยนที่อยู่ API เป็น https://api.stepfun.ai/step_plan" },
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
        "antigravity" => "antigravity",
        "qwen" => "qwen",
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
        out.push(("ANTHROPIC_BASE_URL".into(), anthropic_base(base)));
    }
    // Qwen Code with a key: its OpenAI-compatible mode (runner adds `--auth-type openai`), no settings.json needed
    if v.runner == "qwen" {
        if !base.is_empty() {
            out.push(("OPENAI_BASE_URL".into(), base.trim_end_matches('/').to_string()));
        }
    }
    out
}

/// Claude Code adds `/v1/messages` to the address itself, so a full endpoint pasted from a provider's docs
/// (…/v1/chat/completions, …/v1/messages, …/v1) went to …/chat/completions/v1/messages and every run failed with
/// "There's an issue with the selected model" (ISSUE-109): cut it back to the base.
pub fn anthropic_base(url: &str) -> String {
    let mut b = url.trim().to_string();
    loop {
        let t = b.trim_end_matches('/');
        let low = t.to_ascii_lowercase();
        let cut = ["/chat/completions", "/completions", "/messages", "/v1"].iter().find(|s| low.ends_with(*s)).map(|s| t.len() - s.len());
        match cut {
            Some(n) => b = t[..n].to_string(),
            None => return t.to_string(),
        }
    }
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
        // Qwen Code: the key and the vendor's (or the owner's) OpenAI-compatible address
        let q = key_env(vendor("dashscope").unwrap(), "k", "");
        assert!(q.contains(&("OPENAI_API_KEY".into(), "k".into())) && q.contains(&("OPENAI_BASE_URL".into(), "https://dashscope-intl.aliyuncs.com/compatible-mode/v1".into())));
        assert!(key_env(vendor("qwen-coding").unwrap(), "k", "https://coding.dashscope.aliyuncs.com/v1/").contains(&("OPENAI_BASE_URL".into(), "https://coding.dashscope.aliyuncs.com/v1".into())));
        assert!(needs_model_name(ds) && !needs_model_name(vendor("anthropic").unwrap()));
        // StepFun: Claude Code appends /v1/messages to the base; a Step Plan account overrides it
        let sf = vendor("stepfun").unwrap();
        assert!(key_env(sf, "k", "").contains(&("ANTHROPIC_BASE_URL".into(), "https://api.stepfun.ai".into())));
        assert!(key_env(sf, "k", "https://api.stepfun.ai/step_plan").contains(&("ANTHROPIC_BASE_URL".into(), "https://api.stepfun.ai/step_plan".into())));
        // a full endpoint pasted from the docs is cut back to the base Claude Code expects (ISSUE-109)
        assert!(key_env(sf, "k", "https://api.stepfun.ai/step_plan/v1/chat/completions").contains(&("ANTHROPIC_BASE_URL".into(), "https://api.stepfun.ai/step_plan".into())));
        assert_eq!(anthropic_base("https://x.test/anthropic/v1/messages/"), "https://x.test/anthropic");
        assert_eq!(anthropic_base("https://api.deepseek.com/anthropic"), "https://api.deepseek.com/anthropic");
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
