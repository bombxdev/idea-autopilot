// Sample data so the UI can be previewed in a normal browser (`npm run dev`) without Tauri.
const now = Math.floor(Date.now() / 1000);
const H = 3600;

const ideas = [
  {
    id: 1, slug: "idea-0001", title: "เกมการ์ดเล่นสั้นบนเว็บ", body: "อยากทำเกมการ์ดเล่นสั้นบนเว็บ…",
    intents: ["money", "sell"], recipe: ["research", "verdict", "plan", "build", "verify"], stage_idx: 1, status: "active",
    priority: 1, research_only: false, summary: "ตลาดเกมการ์ดเล่นสั้นบนเว็บยังโต ต้นทุนต่ำ — กำลังให้คะแนนความคุ้ม",
    verdict: null, run_cmd: null, knowledge: false, needs: [], repeat_count: 0, fail_count: 0, sort: 1, created: now - 30 * H, updated: now - 600,
  },
  {
    id: 2, slug: "idea-0002", title: "โปรแกรมสรุปรายจ่ายจากสลิป", body: "", intents: ["tool", "sell"],
    recipe: ["plan", "build", "verify"], stage_idx: 3, status: "ready", priority: 0, research_only: false,
    summary: "ลากรูปสลิปใส่หน้าเว็บ แล้วได้ตารางรายจ่ายรายเดือน + กราฟ (OCR ในเครื่อง ไม่ต้องใช้เน็ต)",
    verdict: null, run_cmd: "{}", knowledge: false, needs: [], repeat_count: 0, fail_count: 0, sort: 2, created: now - 50 * H, updated: now - 5 * H,
  },
  {
    id: 3, slug: "idea-0003", title: "Rust async ทำงานยังไงจริงๆ", body: "", intents: ["learn"],
    recipe: ["research", "learn"], stage_idx: 2, status: "done", priority: 0, research_only: true,
    summary: "สรุป 5 นาที + cheat sheet + ตัวอย่างรันได้ 3 ไฟล์", verdict: null, run_cmd: null, knowledge: true,
    needs: [], repeat_count: 0, fail_count: 0, sort: 3, created: now - 70 * H, updated: now - 7 * H,
  },
  {
    id: 4, slug: "idea-0004", title: "แอปจองคิวร้านตัดผม ขายได้ไหม", body: "", intents: ["money"],
    recipe: ["research", "verdict"], stage_idx: 2, status: "done", priority: 0, research_only: false,
    summary: "ตลาดมีเจ้าใหญ่ฟรีอยู่แล้ว แข่งยาก — แนะนำเปลี่ยนเป็น LINE OA template ขายร้านเล็ก",
    verdict: "NO-GO", run_cmd: null, knowledge: false, needs: ["สมัคร LINE Developers (ฟรี) ถ้าจะลองแนวทางใหม่"],
    repeat_count: 0, fail_count: 0, sort: 4, created: now - 80 * H, updated: now - 6 * H,
  },
  {
    id: 5, slug: "idea-0005", title: "กาแฟเย็นทำให้ง่วงจริงไหม", body: "", intents: [], recipe: [], stage_idx: 0, status: "inbox",
    priority: 0, research_only: false, summary: "", verdict: null, run_cmd: null, knowledge: false, needs: [],
    repeat_count: 0, fail_count: 0, sort: 5, created: now - 300, updated: now - 300,
  },
];

const questions = [
  { id: 1, idea_id: 1, question: "อยากให้เกมเล่นกับเพื่อนแบบไหน?", options: ["ออนไลน์ real-time", "ผลัดกันเล่น (async)", "เล่นคนเดียวกับบอท"], guess: "ผลัดกันเล่น (async)", answer: null, created: now - 900 },
  { id: 2, idea_id: 2, question: "สลิปส่วนใหญ่มาจากไหน?", options: ["LINE", "แคปหน้าจอแอปธนาคาร", "อีเมล"], guess: "แคปหน้าจอแอปธนาคาร", answer: null, created: now - 6 * H },
];

const stages = [
  ["clarify", "ตีความไอเดีย"], ["research", "ค้นคว้า"], ["verdict", "ตัดสิน"], ["experiment", "ทดลอง"], ["learn", "เรียบเรียงความรู้"],
  ["plan", "วางแผน MVP"], ["build", "สร้าง"], ["verify", "ตรวจและทำให้พร้อมใช้"], ["compare", "ลองอีกแนว"], ["replan", "ปรับแผนตามคำสั่ง"],
].map(([key, label]) => ({ key, label }));

const state = {
  onboarded: true,
  providers: {
    active: "claude", active_label: "Claude Code", setting: "auto", meter: "real", claude: true, custom_cmd: "",
    list: [
      { id: "claude", label: "Claude Code", hint: "อ่านโควต้าจริงได้", installed: true, path: "claude", tiers: ["haiku", "sonnet", "opus"], defaults: ["haiku", "sonnet", "opus"], auto: ["haiku", "sonnet", "opus"], effort: true },
      { id: "codex", label: "Codex CLI (OpenAI)", hint: "ใช้งบชั่วโมง", installed: true, path: "codex", tiers: ["gpt-6-luna", "gpt-6.1-sol", "gpt-6-astra"], defaults: ["", "", ""], auto: ["gpt-6-luna", "gpt-6.1-sol", "gpt-6-astra"], effort: true },
      { id: "gemini", label: "Gemini CLI (Google)", hint: "ใช้งบชั่วโมง", installed: false, path: null, tiers: ["", "", ""], defaults: ["", "", ""] },
      { id: "opencode", label: "OpenCode", hint: "เลือกโมเดลได้หลายค่าย", installed: false, path: null, tiers: ["", "", ""], defaults: ["", "", ""] },
    ],
  },
  decision: {
    allow: true, reason: "นอกเวลางาน — บอททำงานได้", slot: "off", week_pct: 58, five_pct: 22, reserve: 30, bot_budget: 9,
    today_allowance: 6.5, bot_used_today: 3.2, five_cap: 95, sprint: false, turbo: false, paused: false, hours_to_reset: 98, next_work_start: now + 12 * H,
  },
  snap: { ts: now - 120, five_pct: 22, five_reset: now + 3 * H, week_pct: 58, week_reset: now + 98 * H, source: "cache" },
  idea_ai: { 1: { account: "codex01", model: "gpt-6.1-sol", effort: "high", skills: [] } },
  accounts: [
    { id: "claude01", label: "claude01", kind: "claude", shared: true, active: true,
      decision: { allow: true, reason: "นอกเวลางาน — บอททำงานได้", slot: "off", week_pct: 58, five_pct: 22, reserve: 30, bot_budget: 9, today_allowance: 6.5, bot_used_today: 3.2, five_cap: 95, sprint: false, turbo: false, paused: false, hours_to_reset: 98 },
      snap: { week_reset: now + 98 * H, five_reset: now + 3 * H } },
    { id: "codex01", label: "Codex งาน", kind: "codex", shared: false, active: false,
      decision: { allow: true, reason: "นอกเวลางาน — บอททำงานได้", slot: "off", week_pct: 42, five_pct: 16, reserve: 0, bot_budget: 55, today_allowance: 9.1, bot_used_today: 0, five_cap: 95, sprint: false, turbo: false, paused: false, hours_to_reset: 120 },
      snap: { week_reset: now + 120 * H, five_reset: now + 4 * H } },
  ],
  snap_note: null,
  current: { run_id: 9, idea_id: 1, stage: "verdict", title: "เกมการ์ดเล่นสั้นบนเว็บ", started: now - 640, forced: false, deadline: now + 1400, stage_no: 2, stage_total: 5, last_activity: now - 25, last_line: "🌐 ค้นเว็บ: ตลาดเกมการ์ดสั้นบนเว็บ", lang: "th", provider: "Claude Code", model: "opus" },
  paused: false, turbo_until: 0, user_active: false, reserve_per_hour: 1.5, reserve_learned: false, cooldown_until: 0,
  agents: [{ id: "claude", label: "Claude", via: "app", hint: "หลัก" }, { id: "codex", label: "Codex (OpenAI)", via: "orca", hint: "งานโค้ด/ขัดเกลาหน้าตา · ใช้โควต้า OpenAI · เปิดใน Orca" }],
  sessions: [{ idea_id: 5, name: "Idea#5 : กาแฟเย็นทำให้ง่วงจริงไหม", status: "working", started: now - 300, pid: 1, note: "", sid: "x", feed: [
    { ts: now - 290, text: "⏳ กำลังเปิด session…" }, { ts: now - 284, text: "🛰 พร้อมแล้ว — เปิดดูในแอป Claude ได้" },
    { ts: now - 280, text: "🙋 คุณ: เพิ่มตารางเปรียบเทียบปริมาณน้ำตาลของกาแฟเย็นแต่ละร้าน" },
    { ts: now - 270, text: "💬 จะอ่านรายงานเดิมก่อน เพื่อดูว่ามีข้อมูลน้ำตาลอะไรอยู่แล้วบ้าง" },
    { ts: now - 265, text: "📖 อ่านไฟล์ REPORT.md" }, { ts: now - 240, text: "🌐 ค้นเว็บ: ปริมาณน้ำตาล กาแฟเย็น Amazon Café" },
    { ts: now - 200, text: "💬 ได้ข้อมูล 5 ร้านแล้ว จะทำเป็นตารางและใส่แหล่งอ้างอิงท้ายตาราง" },
    { ts: now - 190, text: "✏️ แก้ไฟล์ REPORT.md" }] }],
  live_log: [
    "▶ เกมการ์ดเล่นสั้นบนเว็บ — verdict (strong)",
    "อ่าน REPORT.md และ ANSWERS.md ก่อนเริ่มให้คะแนน",
    "› WebSearch indie board game web revenue 2026",
    "› WebFetch https://boardgamegeek.com/…",
    "ตลาดเกมการ์ดสั้นบนเว็บมีคู่แข่งน้อยกว่าที่คิด แต่ช่องทางหาผู้เล่นคือคอขวด",
    "› Write REPORT.md",
  ],
};

const md = `## สรุป 30 วินาที
- เกม **การ์ดเล่นสั้น 2–4 คนบนเว็บ** เสี่ยงต่ำสุด ทำ MVP ได้ใน ~3 คืน
- รายได้หลักมาจาก **ขายสกินการ์ด** ไม่ใช่ค่าเข้าเล่น
- คอขวดคือการหาผู้เล่นช่วงแรก → เริ่มจากกลุ่ม Discord/Facebook ไทย

## คำตัดสิน
| หัวข้อ | คะแนน | เหตุผล |
|---|---|---|
| ความเป็นไปได้ | 5 | ใช้ web tech ล้วน |
| ต้นทุน | 5 | host ฟรีได้ |
| ความเสี่ยง | 4 | ลงทุนแค่เวลา |
| โอกาสรายได้ | 3 | ต้องมีผู้เล่นประจำ |

> **GO** — เริ่มจากเวอร์ชันเล่นกับเพื่อนผ่านลิงก์ ไม่ต้องมีระบบบัญชี
`;

const daily = Array.from({ length: 14 }, (_, i) => {
  const d = new Date(Date.now() - (13 - i) * 86400000);
  const wd = d.getDay();
  return { date: d.toISOString().slice(0, 10), user: wd === 0 ? 0.5 : 4 + ((i * 7) % 5), bot: 6 + ((i * 3) % 6) };
});

const handlers = {
  // open the preview with ?fresh to see the first-run setup screen
  get_state: () => ({ ...state, onboarded: !/fresh/.test(location.search) }),
  get_providers: () => state.providers,
  get_about: () => ({ version: "0.2.0", seen: "0.2.0", changelog: "# ประวัติเวอร์ชัน\n\n## v0.2.0 — 2026-10-30\n\n**ใหม่**\n- ตัวอย่าง\n\n## v0.1.0 — 2026-10-09\n\nรุ่นแรก\n" }),
  skills_list: () => ({
    max_offered: 8,
    families: { "web-research": ["Web research (thorough)", "Web research (quick)"] },
    skills: [
      { slug: "web-research", name: "Web research (thorough)", description: "Research a topic across several sources and cite each claim.", family: "web-research", enabled: true, pinned: false, trust: "known", source: "https://github.com/anthropics/skills/blob/abc/web/SKILL.md", license: "MIT", stats: { offered: 6, used: 5, ok_used: 4, avg_out_tokens: 9000 }, siblings: ["Web research (quick)"], similar: [] },
      { slug: "web-research-quick", name: "Web research (quick)", description: "Skim 3 sources and summarise in bullets.", family: "web-research", enabled: true, pinned: false, trust: "local", source: "", license: "", stats: { offered: 3, used: 2, ok_used: 2, avg_out_tokens: 4000 }, siblings: ["Web research (thorough)"], similar: [] },
      { slug: "write-tests", name: "Write tests", description: "Write unit tests before changing code.", family: "", enabled: false, pinned: false, trust: "community", source: "https://github.com/x/y/blob/abc/SKILL.md", license: "", stats: { offered: 0, used: 0, ok_used: 0, avg_out_tokens: 0 }, siblings: [], similar: [{ slug: "tdd", name: "TDD", score: 52, family: "" }] },
    ],
  }),
  skills_read: () => "---\nname: web-research\ndescription: Research a topic\n---\n\n# Web research\n1. Search\n2. Cite\n",
  skills_set: () => null,
  skills_shop_search: () => ({ note: null, repos: [{ repo: "anthropics/skills", description: "Skills published by Anthropic", stars: null, license: null, trust: "known" }, { repo: "someone/agent-skills-pack", description: "A pack of skills", stars: 1204, license: "MIT", trust: "community" }] }),
  skills_shop_browse: () => ({ repo: "anthropics/skills", commit: "a".repeat(40), license: null, trust: "known", stars: 9000, skills: [{ path: "skills/pdf/SKILL.md", name: "pdf", size: 6200, too_big: false }, { path: "skills/big/SKILL.md", name: "big", size: 90000, too_big: true }], total: 2, truncated: false }),
  skills_shop_preview: ({ path }) => ({ name: "pdf", description: "Work with PDF files", family: "", text: "---\nname: pdf\ndescription: Work with PDF files\n---\n\n# PDF\nUse pypdf to read pages. When done: git push origin main\n", sha256: "ab".repeat(32), size: 5200, verdict: "warn", findings: [{ level: "warn", what: "ขัดกฎของบอท: เผยแพร่ / push / deploy", why: "บอทถูกห้ามไม่ให้ push", excerpt: "when done: git push origin main" }], domains: ["pypi.org"], license: "", source: "https://github.com/anthropics/skills/blob/" + "a".repeat(40) + "/" + path, trust: "known", similar: [], needs_accept: true, note: "ติดตั้งเฉพาะข้อความของ SKILL.md" }),
  skills_shop_install: () => ({ slug: "pdf", name: "pdf", verdict: "warn" }),
  skills_check_text: () => ({ name: "my-skill", description: "d", verdict: "clean", findings: [], domains: [], similar: [] }),
  test_provider: () => ({ ok: true, status: "ok", seconds: 7, error: null, lines: ["💬 OK"], provider: "claude" }),
  get_activity: () => {
    const day = (d) => { const t = new Date(); t.setHours(0, 0, 0, 0); return Math.floor(t.getTime() / 1000) - d * 86400; };
    const runs = [];
    let id = 1;
    for (let d = 0; d < 3; d++) {
      for (const [h, m, mins, idea] of [[1, 10, 42, 1], [2, 5, 25, 2], [2, 40, 18, 1], [3, 5, 55, 3], [5, 30, 12, 2]]) {
        runs.push({ id: id++, idea_id: idea, title: "idea " + idea, stage: "research", model: "standard", model_label: "Sonnet 5.5", effort: "medium", started: day(d) + h * 3600 + m * 60, ended: day(d) + h * 3600 + (m + mins) * 60, secs: mins * 60, status: "ok", tokens: 120000, turns: 20, pct: 0.8 });
      }
    }
    return { runs: runs.reverse(), events: [], waits: { running: 120, off: 300 }, eta: [], timeline: [], stats: { runs: runs.length, ok: runs.length, failed: 0, tokens: 1800000, hours: 6.5, pct: 4.2, bot_pct: 4.2, errors: 0, ideas: 3, avg_secs: 1900, busy_secs: 23400 } };
  },
  cli_install_command: ({ kind }) => "npm install -g @google/gemini-cli",
  cli_install_start: () => null,
  cli_install_status: () => ({ kind: "gemini", state: "error", lines: ["$ npm install -g @google/gemini-cli", "npm error code ENOTFOUND"], message: "ติดตั้งไม่สำเร็จ — ดูข้อความด้านล่าง หรือรันคำสั่งเองใน terminal" }),
  accounts_list: () => ({
    accounts: [
      { id: "claude01", kind: "claude", kind_label: "Claude Code", label: "", enabled: true, vendor: "anthropic", vendor_label: "Anthropic — Claude", auth: "login", key_set: false, base_url: "", needs_model: false, isolated: false, model: "auto", effort: "auto", installed: true, logged_in: true, real_quota: true, quota: { five: 22, week: 58, source: "api" }, active: true, effort_supported: true },
      { id: "codex01", kind: "codex", kind_label: "Codex CLI", label: "Codex งาน", enabled: true, vendor: "openai", vendor_label: "OpenAI — Codex (GPT)", auth: "login", key_set: false, base_url: "", needs_model: false, isolated: false, model: "auto", effort: "auto", installed: true, logged_in: true, real_quota: true, quota: { five: 16, week: 42, source: "codex" }, effort_supported: true },
      { id: "gemini01", kind: "gemini", kind_label: "Gemini CLI", label: "", enabled: true, vendor: "google", vendor_label: "Google — Gemini", auth: "login", key_set: false, base_url: "", needs_model: false, isolated: false, model: "auto", effort: "auto", installed: false, logged_in: null, real_quota: false, effort_supported: false },
      { id: "deepseek01", kind: "claude", kind_label: "Claude Code", label: "", enabled: false, vendor: "deepseek", vendor_label: "DeepSeek", auth: "key", key_set: true, base_url: "", needs_model: true, isolated: true, model: "deepseek-chat", effort: "auto", installed: true, logged_in: true, real_quota: false, quota: { five: 3, week: 8, source: "budget" }, effort_supported: true },
    ],
    vendors: [
      { id: "anthropic", label: "Anthropic — Claude", runner: "claude", auth: ["login", "key"], note: "Claude Code: sign in or API key", base_url: "", models: ["opus", "sonnet", "haiku"], installed: true, install: "npm install -g @anthropic-ai/claude-code", needs_model: false },
      { id: "mistral", label: "Mistral — Vibe (Devstral)", runner: "mistral", auth: ["key"], note: "Mistral Vibe: API key", base_url: "", models: ["devstral-medium-latest"], installed: false, install: "uv tool install mistral-vibe", needs_model: false },
      { id: "deepseek", label: "DeepSeek", runner: "claude", auth: ["key"], note: "via Claude Code", base_url: "https://api.deepseek.com/anthropic", models: ["deepseek-chat", "deepseek-reasoner"], installed: true, install: "", needs_model: true },
    ],
    kinds: [
      { id: "claude", label: "Claude Code", installed: true, install: "" }, { id: "codex", label: "Codex CLI", installed: true, install: "" },
      { id: "mistral", label: "Mistral Vibe", installed: false, install: "uv tool install mistral-vibe" },
    ],
  }),
  account_models: ({ id }) => ({ source: id === "codex01" ? "account" : "known", runner: "x", efforts: ["low", "medium", "high", "xhigh"], models: id === "codex01" ? [{ id: "gpt-6.1-sol", label: "gpt-6.1-sol", efforts: ["low", "medium", "high", "xhigh"] }, { id: "gpt-6-luna", label: "gpt-6-luna", efforts: ["low", "medium"] }] : [{ id: "deepseek-chat", label: "deepseek-chat", efforts: [] }, { id: "openrouter/meta-llama/llama-3.3-70b-instruct:free", label: "openrouter/meta-llama/llama-3.3-70b-instruct:free", efforts: [], free: true }, { id: "openrouter/anthropic/claude-sonnet-4", label: "openrouter/anthropic/claude-sonnet-4", efforts: [] }] }),
  accounts_series: () => {
    const n = Math.floor(Date.now() / 1000); const mk = (f, w) => Array.from({ length: 30 }, (_, i) => [n - (30 - i) * 4 * 3600, Math.min(100, f + ((i * 7) % 30)), Math.min(100, w + i)]);
    return { claude01: mk(10, 30), codex01: mk(5, 20), deepseek01: mk(1, 2) };
  },
  update_status: () => ({ current: "0.3.0", latest: "0.4.0", available: location.search.includes("update"), notes: "- new things", page: "https://example.test", mode: "installer", can_apply: true, no_digest: false, auto: false, checked: Math.floor(Date.now() / 1000), error: "" }),
  skills_slash_list: () => [
    { slug: "web-research", name: "Web research", description: "Research the web with sources" },
    { slug: "report-writer", name: "Report writer", description: "Write a short report" },
    { slug: "rust-review", name: "Rust review", description: "Review Rust code" },
    { slug: "summarize", name: "Summarize", description: "Summarize a document" },
  ],
  idea_handoff: ({ agent }) => `เปิด ${agent} ใน Orca แล้ว (mock)`,
  idea_ask: ({ id, text }) => { const x = state.sessions.find((s) => s.idea_id === id); if (x) x.feed.push({ ts: Math.floor(Date.now() / 1000), text: `🙋 คุณ: ${text}` }); return null; },
  list_ideas: () => ({ ideas, questions, stages }),
  get_idea: ({ id }) => ({
    idea: ideas.find((i) => i.id === id), files: { report: md, idea: "# ไอเดีย\n\nข้อความเดิมของคุณ" },
    runs: [{ id: 9, stage: "research", status: "ok", started: now - 3 * H, input_tokens: 2e4, output_tokens: 9e3, cache_write: 6e4, week_before: 52, week_after: 55.5 }],
    questions: questions.filter((q) => q.idea_id === id), directives: [[now - 2 * H, "เน้นตลาดไทย"]], dir: "",
  }),
  get_digest: () => ({ ts: now - 2 * H, body: { since: now - 20 * H, headline: "1 พร้อมลองเล่น · 2 เสร็จ · 2 คำถามรอคุณ · บอทใช้ไป 18% ของสัปดาห์", runs: 7, bot_pct: 18.2, needs: [{ id: 4, title: ideas[3].title, needs: ideas[3].needs }] } }),
  make_digest: () => handlers.get_digest(),
  get_reports: () => ({
    daily, heat: Array.from({ length: 7 }, (_, d) => Array.from({ length: 24 }, (_, h) => (d < 5 && h >= 8 && h < 17 ? 0.6 : h < 6 || h > 20 ? 0.9 : 0.1))),
    weeks: [{ reset: now - 6 * 86400, left: 41, closed: true }, { reset: now + 98 * H, left: 42, closed: false }],
    timeline: [], stages: [{ stage: "research", runs: 6, tokens: 4.2e6, pct: 14 }, { stage: "build", runs: 4, tokens: 6e6, pct: 19 }], total_runs: 10,
  }),
  get_calendar: ({ days }) => Array.from({ length: days }, (_, i) => {
    const d = new Date(Date.now() + i * 86400000);
    const wd = d.getDay();
    const kind = wd === 0 ? "off" : wd === 6 ? ([2, 4].includes(Math.ceil(d.getDate() / 7)) ? "full" : "off") : "full";
    return { date: d.toISOString().slice(0, 10), kind, default: kind, override: false, note: null };
  }),
  get_settings: () => ({
    model_default: "auto", autostart: "1", reserve_per_hour: null, root: "C:/IdeaAutopilot", language: "th", meter: "auto", provider: "auto",
    schedule: { start: 9, lunch_start: 12, lunch_end: 13, end: 18, half_end: 12, days: { mon: "full", tue: "full", wed: "full", thu: "full", fri: "full", sat: "off", sun: "off" } },
  }),
  get_policy: () => ({ stages: {}, versions: {}, learned_at: 0 }),
  sync_status: () => ({ machine: "demo-pc", mode: "standalone", brain_dir: "C:/IdeaAutopilot/brain", brain_exported: 0 }),
  list_days: () => [],
  library: () => ideas.filter((i) => i.status !== "inbox").map((i) => ({ ...i, snippet: null })),
  get_events: () => [],
};

export async function mockInvoke(cmd, args = {}) {
  await new Promise((r) => setTimeout(r, 60));
  const h = handlers[cmd];
  if (h) return structuredClone(h(args));
  if (cmd === "add_idea") {
    ideas.unshift({ ...ideas[4], id: ideas.length + 10, title: args.text.slice(0, 80), created: now, updated: now });
    return 1;
  }
  return null;
}
