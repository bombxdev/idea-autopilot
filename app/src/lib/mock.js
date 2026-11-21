// Sample data so the UI can be previewed in a normal browser (`npm run dev`) without Tauri.
// ?lang=en shows the interface and the sample ideas in English (for screenshots and demo videos)
const DEMO_LANG = typeof location !== "undefined" ? new URLSearchParams(location.search).get("lang") : null;
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
// more finished ideas, so a crowded "done" column (and its "show more") can be previewed
for (let n = 0; n < 6; n++) {
  ideas.push({ ...ideas[2], id: 20 + n, slug: `idea-00${20 + n}`, title: `บทสรุปเรื่องที่ ${n + 1}`, summary: "", sort: 20 + n, updated: now - (10 + n) * H });
}

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
  idea_ai: { 1: { account: "codex01", model: "gpt-6.1-sol", effort: "high", skills: [], team: "web-app" } },
  accounts: [
    { id: "claude01", label: "claude01", kind: "claude", shared: true, active: true,
      decision: { allow: true, reason: "นอกเวลางาน — บอททำงานได้", slot: "off", week_pct: 58, five_pct: 22, reserve: 30, bot_budget: 9, today_allowance: 6.5, bot_used_today: 3.2, five_cap: 95, sprint: false, turbo: false, paused: false, hours_to_reset: 98 },
      snap: { week_reset: now + 98 * H, five_reset: now + 3 * H } },
    { id: "codex01", label: "Codex งาน", kind: "codex", shared: false, active: false,
      decision: { allow: true, reason: "นอกเวลางาน — บอททำงานได้", slot: "off", week_pct: 42, five_pct: 16, reserve: 0, bot_budget: 55, today_allowance: 9.1, bot_used_today: 0, five_cap: 95, sprint: false, turbo: false, paused: false, hours_to_reset: 120 },
      snap: { week_reset: now + 120 * H, five_reset: now + 4 * H } },
  ],
  snap_note: null,
  current: { run_id: 9, idea_id: 1, stage: "verdict", title: "เกมการ์ดเล่นสั้นบนเว็บ", started: now - 640, forced: false, deadline: now + 1400, stage_no: 2, stage_total: 5, last_activity: now - 25, last_line: "🌐 ค้นเว็บ: ตลาดเกมการ์ดสั้นบนเว็บ", lang: "th", provider: "Claude Code", model: "Opus 5.5", account: "claude01" },
  paused: false, turbo_until: 0, user_active: false, reserve_per_hour: 1.5, reserve_learned: false, cooldown_until: 0,
  agents: [{ id: "claude", label: "Claude", via: "app", hint: "หลัก" }, { id: "codex", label: "Codex (OpenAI)", via: "orca", hint: "งานโค้ด/ขัดเกลาหน้าตา · ใช้โควต้า OpenAI · เปิดใน Orca" }],
  sessions: [{ idea_id: 5, name: "Idea#5 : กาแฟเย็นทำให้ง่วงจริงไหม", status: "working", started: now - 300, pid: 1, note: "", sid: "x", ai: "Claude Code · Opus 5.5", busy_since: now - 47 * 60, waiting: 1, quiet: 420, screen: "● Read(t2.png)\n  ⎿  Reading…", feed: [
    { ts: now - 26 * 3600, text: "🙋 คุณ: สรุปผลสำรวจเมื่อวานให้หน่อย" }, { ts: now - 26 * 3600 + 60, text: "✅ จบรอบนี้" }, { ts: now - 290, text: "⏳ กำลังเปิด session…" }, { ts: now - 284, text: "🛰 พร้อมแล้ว — เปิดดูในแอป Claude ได้" },
    { ts: now - 280, text: "🙋 คุณ: เพิ่มตารางเปรียบเทียบปริมาณน้ำตาลของกาแฟเย็นแต่ละร้าน" }, { ts: now - 278, text: "🙋 คุณ: At line:1 char:207\n+ ... ore } from './agent/keystore.mjs'; const p = wincredKeystore().get('g ...\n+ ~\nAn expression was expected after '('.\nAt line:1 char:244\n+ ... st p = wincredKeystore().get('goatcounter/password'); if (!p) throw n ...\n+ ~\nMissing statement block after if ( condition ).\n+ CategoryInfo : ParserError: (:) [], ParentContainsErrorRecordException\n+ FullyQualifiedErrorId : ExpectedExpression" },
    { ts: now - 270, text: "💬 จะอ่านรายงานเดิมก่อน เพื่อดูว่ามีข้อมูลน้ำตาลอะไรอยู่แล้วบ้าง" }, { ts: now - 265, text: "💬 ลองรันนี้ใน PowerShell:\n```powershell\ncd D:/x\nnode a.mjs | Set-Clipboard\n```\nแล้วกด **Ctrl+V** หรือล้างด้วย `Set-Clipboard -Value ''`" },
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

// sample runs for the usage report: a few agents and models over the last ~70 days (codex/claude report tokens, mistral does not)
const usageSample = (() => {
  const models = [
    ["claude", "", "claude-sonnet-5-5", 0.09], ["claude", "", "claude-opus-5-5", 0.25], ["claude", "", "claude-haiku-5-5", 0.02],
    ["codex", "Codex_main", "gpt-6.1-sol", 0], ["mistral", "Mistral_team", "devstral-medium-latest", 0],
  ];
  const ideaNames = [[3, "เกมการ์ดเล่นสั้นบนเว็บ", "personal"], [7, "แดชบอร์ด 5S", "work"], [9, "ร้านก๋วยเตี๋ยวเรือ", "personal"], [13, "Thai LLM self-hosting", "personal"]];
  const stages = ["research", "build", "verify", "clarify", "verdict"];
  let seed = 7;
  const rnd = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
  const out = [];
  const t0 = Math.floor(Date.now() / 1000) - 70 * 86400;
  for (let i = 0; i < 260; i++) {
    const [provider, account, model, price] = models[Math.floor(rnd() * rnd() * models.length * 1.6) % models.length];
    const [idea_id, idea, scope] = ideaNames[Math.floor(rnd() * ideaNames.length)];
    const quiet = provider === "mistral";
    const cr = quiet ? 0 : Math.floor(2e5 + rnd() * 2e6), out_ = quiet ? 0 : Math.floor(5e3 + rnd() * 3e4);
    out.push({ id: i + 1, idea_id, idea, scope, stage: stages[Math.floor(rnd() * stages.length)], status: rnd() < 0.82 ? "ok" : "error",
      started: t0 + Math.floor(i * (70 * 86400 / 260) + rnd() * 3600), secs: Math.floor(120 + rnd() * 1500),
      input: quiet ? 0 : Math.floor(rnd() * 900), output: out_, cache_read: cr, cache_write: quiet ? 0 : Math.floor(cr * 0.07),
      cost: +(price * (cr / 1e6) * 3).toFixed(3), turns: 8, model, tier: "standard", provider, account, effort: "medium", source: "bot", msgs: 0 });
  }
  for (let i = 0; i < 40; i++) {
    const [idea_id, idea, scope] = ideaNames[i % ideaNames.length];
    const codex = i % 4 === 0, cr = Math.floor(5e5 + rnd() * 4e6);
    out.push({ id: -i - 1, idea_id, idea, scope, stage: "chat", status: "ok", started: t0 + Math.floor((i + 0.5) * (70 * 86400 / 40)), secs: Math.floor(300 + rnd() * 2400),
      input: Math.floor(rnd() * 400), output: Math.floor(4e3 + rnd() * 2e4), cache_read: cr, cache_write: codex ? 0 : Math.floor(cr * 0.05), cost: 0, turns: 12,
      model: codex ? "gpt-6.1-sol" : "claude-opus-5-5", tier: "", provider: codex ? "codex" : "claude", account: codex ? "Codex_main" : "", effort: "", source: "chat", msgs: Math.floor(4 + rnd() * 30) });
  }
  out.sort((a, b) => a.started - b.started);
  return out;
})();

const handlers = {
  usage_runs: ({ from, to }) => ({ runs: usageSample.filter((r) => r.started >= from && r.started < to), first: usageSample[0].started }),
  save_export: ({ name }) => `C:/IdeaAutopilot/data/exports/${name}`,
  // open the preview with ?fresh to see the first-run setup screen
  get_state: () => ({ ...state, onboarded: !/fresh/.test(location.search), account_problems: location.search.includes("connfail") ? [{ id: "claude02", title: "Claude บ้าน", kind: "auth", cause: "การล็อกอินของบัญชีนี้หลุดหรือหมดอายุ", action: "login" }] : [] }),
  // ?noai: nothing installed (first-run setup without an AI, ISSUE-105)
  get_providers: () => /noai/.test(location.search)
    ? { ...state.providers, active: "none", active_label: "ยังไม่มี AI", claude: false, list: state.providers.list.map((p) => ({ ...p, installed: false, active: false })) }
    : state.providers,
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
  skills_shop_search: () => ({ note: null, repos: [{ repo: "anthropics/skills", description: "Skills published by Anthropic", stars: null, license: null, trust: "known", category: "docs" }, { repo: "someone/agent-skills-pack", description: "A pack of skills", stars: 1204, license: "MIT", trust: "community", category: "agents", created: "2026-10-02T00:00:00Z", stars_per_day: 25.6 }, { repo: "sec/ctf-skills", description: "Pentest and CTF helpers", stars: 310, license: "MIT", trust: "community", category: "security", created: "2026-11-01T00:00:00Z", stars_per_day: 16.3 }] }),
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
  video_addon_status: () => ({
    lang: "th",
    ffmpeg: { found: true, own: false, path: "C:\ffmpeg\bin\ffmpeg.exe", size_mb: 88, label: "ffmpeg 7.1.1" },
    models: [
      { id: "openthai2-asr", label: "OpenThai 2.0 ASR", note: "ภาษาไทยแม่นที่สุดในตัวฟรีที่ทดสอบ (FLEURS ผิด 8.5% ของตัวอักษร · คลิป YouTube 15.4%) · คำอังกฤษออกมาเป็นตัวไทย (AI เปลี่ยนศัพท์ที่ควรเป็นอังกฤษให้ตอนใช้งาน)", license: "OpenMDW-1.1", size_mb: 754, installed: false, engine: "transcribe.cpp", engine_installed: false, engine_size_mb: 17, score: 100, recommended: true },
      { id: "whisper-large-v3-turbo", label: "Whisper large-v3-turbo", note: "100 ภาษา รู้เองว่าคลิปเป็นภาษาอะไร", license: "MIT", size_mb: 620, installed: false, engine: "transcribe.cpp", engine_installed: false, engine_size_mb: 17, score: 55, recommended: false },
    ],
    job: { part: "", state: "idle", done_mb: 0, total_mb: 0, message: "" },
    custom_recipes: false,
  }),
  video_addon_install: () => null,
  mcp_list: () => [{ id: "rea", label: "rea", description: "", source: "claude-code", transport: "stdio", command: "cmd.exe /c rea mcp", env_names: [], enabled: true, always: false, skills: ["reverse-engineer-anything"], category: "security", version: "", registry_name: "" }],
  mcp_local: () => [{ id: "rea", command: "cmd.exe /c rea mcp", env_names: [], imported: true }, { id: "chrome-devtools", command: "npx chrome-devtools-mcp@latest", env_names: [], imported: false }],
  mcp_import: () => "chrome-devtools",
  mcp_popular: () => ({ servers: [
    { name: "io.github.upstash/context7", title: "Context7", description: "Up-to-date docs for any library", version: "1.0.0", repo: "https://github.com/upstash/context7", category: "docs", published: "2026-10-01", stars: 65196, popular: true, ways: [{ kind: "package", index: 0, command: "npx -y @upstash/context7-mcp", env: [] }] },
    { name: "io.github.ChromeDevTools/chrome-devtools-mcp", title: "Chrome DevTools", description: "Control and inspect a live Chrome browser", version: "0.9.0", repo: "https://github.com/ChromeDevTools/chrome-devtools-mcp", category: "test", published: "2026-10-01", stars: 58566, popular: true, ways: [{ kind: "package", index: 0, command: "npx -y chrome-devtools-mcp", env: [] }] },
  ], note: "" }),
  mcp_search: () => ({ servers: [{ name: "io.github.someone/postgres", title: "Postgres", description: "Query a Postgres database", version: "1.2.0", repo: "https://github.com/someone/pg-mcp", category: "data", published: "2026-11-19T00:00:00Z", ways: [{ kind: "package", index: 0, command: "cmd /c npx -y pg-mcp@1.2.0", env: [{ name: "DATABASE_URL", description: "Connection string", required: true, secret: true }] }] }, { name: "com.example/browser", title: "Browser", description: "Open and read web pages", version: "0.4.0", repo: null, category: "research", published: "2026-11-18T00:00:00Z", ways: [{ kind: "remote", index: 0, command: "https://mcp.example.com/mcp", env: [] }] }] }),
  mcp_install: () => "postgres",
  mcp_set: () => null,
  idea_team_set: () => null,
  mcp_remove: () => null,
  docs_addon_status: () => ({
    enabled: true,
    uv: { found: true, own: false, size_mb: 17, label: "uv 0.12.10 (Astral)" },
    converters: [
      { id: "markitdown", label: "MarkItDown 0.1.9 (Microsoft)", note: "PDF, Word, Excel, PowerPoint, Outlook, HTML, EPUB → Markdown · ครั้งแรกดาวน์โหลดส่วนประกอบราว 60 MB ผ่าน uv", license: "MIT", kind: "uvx", formats: ["pdf", "docx"], size_mb: 0, ready: true },
      { id: "openthai-ocr-text", label: "OpenThai 2.1 OCR Text", note: "อ่านตัวหนังสือไทย/อังกฤษจาก PDF ที่เป็นภาพสแกน · 7 MB · CPU ล้วน ~0.4 วิ/หน้า", license: "Apache-2.0", kind: "uv-script", formats: ["scan"], size_mb: 7, ready: false },
    ],
  }),
  docs_addon_set: () => null,
  video_addon_remove: () => null,
  model_check_now: () => 1,
  sessions_overview: () => ({ idle_min: 45, has_claude: true, resumable: [{ idea_id: 3, title: "ระบบจองคิวร้านตัดผม", last: Math.floor(Date.now() / 1000) - 7200 }], open: [{ idea_id: 1, name: "Idea#1 : เกมการ์ด", status: "idle", agent: "claude", started: Math.floor(Date.now() / 1000) - 900, idle_secs: 300, feed: [{ ts: Math.floor(Date.now() / 1000) - 300, text: "💬 แก้ปุ่มเสร็จแล้ว" }] }, { idea_id: 2, name: "Idea#2 · Codex", status: "working", agent: "codex", started: Math.floor(Date.now() / 1000) - 400, idle_secs: 20, feed: [] }, { idea_id: 5, name: "Idea#5 · Mistral Vibe", status: "idle", agent: "mistral", started: Math.floor(Date.now() / 1000) - 200, idle_secs: 100, feed: [] }] }),
  cli_install_status: () => ({ kind: "gemini", state: "error", lines: ["$ npm install -g @google/gemini-cli", "npm error code ENOTFOUND"], message: "ติดตั้งไม่สำเร็จ — ดูข้อความด้านล่าง หรือรันคำสั่งเองใน terminal" }),
  accounts_list: () => ({
    accounts: [
      ...(location.search.includes("connfail") ? [{ id: "claude02", kind: "claude", kind_label: "Claude Code", label: "Claude บ้าน", enabled: true, vendor: "anthropic", vendor_label: "Anthropic — Claude", auth: "login", key_set: false, base_url: "", needs_model: false, isolated: true, model: "auto", effort: "auto", installed: true, logged_in: true, real_quota: true, effort_supported: true, allowed: false, reason: "เชื่อมต่อไม่ได้", last_test: { ok: false, ts: Math.floor(Date.now() / 1000) - 600, error: "exit Some(1): Please run /login" }, problem: { kind: "auth", cause: "การล็อกอินของบัญชีนี้หลุดหรือหมดอายุ", fix: "กด “ล็อกอินใหม่” แล้วทำตามในหน้าต่างที่เปิดขึ้น", action: "login", until: Math.floor(Date.now() / 1000) + 3600 } }] : []),
      { id: "claude01", kind: "claude", kind_label: "Claude Code", label: "", enabled: true, vendor: "anthropic", vendor_label: "Anthropic — Claude", auth: "login", key_set: false, base_url: "", needs_model: false, isolated: false, model: "auto", effort: "auto", installed: true, logged_in: true, real_quota: true, quota: { five: 22, week: 58, source: "api" }, active: true, effort_supported: true, last_test: { ok: true, ts: 1793860000, model: "haiku" } },
      { id: "codex01", kind: "codex", kind_label: "Codex CLI", label: "Codex งาน", enabled: true, vendor: "openai", vendor_label: "OpenAI — Codex (GPT)", auth: "login", key_set: false, base_url: "", needs_model: false, isolated: false, model: "auto", effort: "auto", installed: true, logged_in: true, real_quota: true, quota: { five: 16, week: 42, source: "codex" }, effort_supported: true, last_test: { ok: false, ts: 1793860300, error: "Free usage limit reached" } },
      { id: "gemini01", kind: "gemini", kind_label: "Gemini CLI", label: "", enabled: true, vendor: "google", vendor_label: "Google — Gemini", auth: "login", key_set: false, base_url: "", needs_model: false, isolated: false, model: "auto", effort: "auto", installed: false, logged_in: null, real_quota: false, effort_supported: false , limit: { kind: "daily", text: "ล็อกอิน Google: ฟรีราว 1,000 คำขอ/วัน (60 คำขอ/นาที) ไม่มีรอบ 5 ชม./สัปดาห์ — เริ่มนับใหม่เที่ยงคืนเวลาแปซิฟิก", reset: Math.floor(Date.now() / 1000) + 9000 } },
      { id: "omni01", kind: "claude", kind_label: "Claude Code", label: "", enabled: true, vendor: "omniroute", vendor_label: "OmniRoute (เกตเวย์ในเครื่อง รวมหลายค่าย)", auth: "key", key_set: false, base_url: "http://localhost:20128", needs_model: true, isolated: true, model: "auto/coding", effort: "auto", installed: true, logged_in: false, real_quota: false, quota: { five: 0, week: 0, source: "provider" }, effort_supported: true, budget: { five_h: 3, week_h: 10 }, allowed: false, reason: "พักไว้: OmniRoute เชื่อมแพ็กเกจ Claude (me@example.com) ไว้ ซึ่งแอปก็ใช้ Claude อยู่โดยตรง", limit: { kind: "daily", text: "เกตเวย์ในเครื่อง: ลิมิตจริงเป็นของแต่ละค่ายที่เชื่อมไว้ใน OmniRoute" }, omni: { quota: [{ provider: "groq", name: "groq omniroute", pct: 99.9, used: 1, total: 1000, reset: null }, { provider: "opencode-zen", name: "OpenCode Zen", pct: 100, used: 0, total: null, reset: null }], warnings: ["การเชื่อมต่อใน OmniRoute ใช้ไม่ได้แล้ว: kiro (kiro) — OmniRoute จะข้ามไป · วิธีแก้: dashboard → Providers → ล็อกอินใหม่ หรือลบทิ้ง"], block: "OmniRoute เชื่อมแพ็กเกจ Claude (me@example.com) ไว้ ซึ่งแอปก็ใช้แพ็กเกจนี้อยู่โดยตรง — ถ้าเป็นบัญชีเดียวกัน โควต้าจะถูกใช้สองทางและแอปคุมเพดานไม่ได้ · จึงพักบัญชี OmniRoute นี้ไว้ · วิธีแก้: ปิดการเชื่อมต่อ claude ใน dashboard ของ OmniRoute (Providers) หรือปิดบัญชีนี้ในแอป · ถ้าเป็นคนละบัญชีกัน กด “เป็นคนละบัญชี — ใช้ต่อ”", dup_ok: false } },
      { id: "openrouter01", kind: "opencode", kind_label: "OpenCode", label: "", enabled: true, vendor: "openrouter", vendor_label: "OpenRouter", auth: "key", key_set: true, base_url: "", needs_model: true, isolated: true, model: "auto", effort: "auto", installed: true, logged_in: true, real_quota: false, quota: { five: 0, week: 2, source: "budget" }, effort_supported: false, budget: { five_h: 3, week_h: 10 }, limit: { kind: "paid", text: "จ่ายตามใช้" }, check: { total: 12, done: 3, bot: 1, chat: 2, unknown: 0, started: Math.floor(Date.now() / 1000) - 400, finished: 0, current: "openrouter/qwen/qwen3-coder:free", current_since: Math.floor(Date.now() / 1000) - 75, last: { model: "openrouter/nvidia/nemotron-3-nano:free", v: "chat", note: "ไม่ได้เขียน STATUS.json" }, waiting: null } },
      { id: "deepseek01", kind: "claude", kind_label: "Claude Code", label: "", enabled: false, vendor: "deepseek", vendor_label: "DeepSeek", auth: "key", key_set: true, base_url: "", needs_model: true, isolated: true, model: "deepseek-chat", effort: "auto", installed: true, logged_in: true, real_quota: false, quota: { five: 3, week: 8, source: "budget" }, effort_supported: true , budget: { five_h: 3, week_h: 10 }, limit: { kind: "paid", text: "จ่ายตามโทเคนจากยอดเงินที่เติมไว้ ไม่มีรอบ 5 ชม./สัปดาห์ — เงินหมดแล้วจึงหยุด", balance: "ยอดเงินคงเหลือ 4.12 USD" } },
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
  account_models: ({ id }) => id === "claude01" ? ({ source: "known", runner: "claude", efforts: ["low", "medium", "high", "xhigh", "max"], hidden: [], hidden_counts: {}, checking: false,
    models: [["claude-opus-5-5", "Opus 5.5", "main", ""], ["claude-sonnet-5-5", "Sonnet 5.5", "main", "ค่าเริ่มต้นของ Claude Code"], ["claude-fable-5-2", "Fable 5.2", "main", ""], ["claude-haiku-5-5", "Haiku 5.5", "main", ""], ["claude-fable-5-1", "Fable 5.1", "more", ""], ["claude-opus-4-8", "Opus 4.8", "more", ""]]
      .map(([id, label, group, note]) => ({ id, label, group, note, efforts: ["low", "medium", "high", "xhigh", "max"] })) }) : ({ source: id === "codex01" ? "account" : "known", runner: "x", efforts: ["low", "medium", "high", "xhigh"], models: id === "codex01" ? [{ id: "gpt-6.1-sol", label: "gpt-6.1-sol", efforts: ["low", "medium", "high", "xhigh"] }, { id: "gpt-6-luna", label: "gpt-6-luna", efforts: ["low", "medium"] }] : [{ id: "deepseek-chat", label: "deepseek-chat", efforts: [] }, { id: "openrouter/meta-llama/llama-3.3-70b-instruct:free", label: "openrouter/meta-llama/llama-3.3-70b-instruct:free", efforts: [], free: true }, { id: "openrouter/anthropic/claude-sonnet-4", label: "openrouter/anthropic/claude-sonnet-4", efforts: [] }], hidden: id === "codex01" ? [] : [{ id: "openrouter/nvidia/nemotron-3-nano:free", state: "chat", note: "ไม่ได้เขียน STATUS.json", free: true }, { id: "openrouter/new/model-x", state: "pending", note: "" }], hidden_counts: { chat: 1, pending: 1, unknown: 0 }, checking: false }),
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
  idea_languages: ({ id }) => (id === 1 ? [{ name: "Rust", color: "#dea584", bytes: 629, pct: 62.9 }, { name: "Svelte", color: "#ff3e00", bytes: 263, pct: 26.3 }, { name: "HTML", color: "#e34c26", bytes: 52, pct: 5.2 }, { name: "JavaScript", color: "#f1e05a", bytes: 34, pct: 3.4 }, { name: "PowerShell", color: "#012456", bytes: 8, pct: 0.8 }, { name: "Java", color: "#b07219", bytes: 6, pct: 0.6 }, { name: "CSS", color: "#663399", bytes: 5, pct: 0.5 }, { name: "Shell", color: "#89e051", bytes: 3, pct: 0.3 }] : []),
  codex_thread: ({ id }) => (id === 1 ? { thread: "019a-demo-thread", account: "codex01", command: "codex resume 019a-demo-thread" } : null),
  idea_ask: ({ id, text, replace }) => { const x = state.sessions.find((s) => s.idea_id === id); if (x) { x.feed.push({ ts: Math.floor(Date.now() / 1000), text: `🙋 คุณ: ${replace ? "✏️ " : ""}${text}` }); x.status = "working"; } return null; },
  list_ideas: () => ({ ideas, questions, stages }),
  get_idea: ({ id }) => ({
    idea: ideas.find((i) => i.id === id), files: { report: md, idea: "# ไอเดีย\n\nข้อความเดิมของคุณ" },
    runs: [{ id: 9, stage: "research", status: "ok", started: now - 3 * H, input_tokens: 2e4, output_tokens: 9e3, cache_write: 6e4, week_before: 52, week_after: 55.5 }, { id: 8, stage: "clarify", status: "ok", started: now - 27 * H, input_tokens: 1e4, output_tokens: 4e3, cache_write: 2e4, week_before: 50, week_after: 51 }],
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
    model_default: "auto", autostart: "1", reserve_per_hour: null, root: "C:/IdeaAutopilot", language: DEMO_LANG || "th", meter: "auto", provider: "auto",
    schedule: { start: 9, lunch_start: 12, lunch_end: 13, end: 18, half_end: 12, days: { mon: "full", tue: "full", wed: "full", thu: "full", fri: "full", sat: "off", sun: "off" } },
  }),
  get_policy: () => ({ agents: {}, learned_at: 0 }),
  sync_status: () => ({ machine: "demo-pc", mode: "standalone", brain_dir: "C:/IdeaAutopilot/brain", brain_personal_dir: "C:/IdeaAutopilot/brain/personal", brain_work_dir: "C:/IdeaAutopilot/brain/work", brain_work_keywords: [], brain_exported: 0 }),
  list_days: () => [],
  library: () => ideas.filter((i) => i.status !== "inbox").map((i) => ({ ...i, snippet: null })),
  get_events: () => [],
};

// longest first, so a sentence is swapped before the words inside it
const EN = Object.entries({
  "เกมการ์ดเล่นสั้นบนเว็บ": "Quick card game for the web",
  "อยากทำเกมการ์ดเล่นสั้นบนเว็บ…": "I want to make a quick card game for the web…",
  "ตลาดเกมการ์ดเล่นสั้นบนเว็บยังโต ต้นทุนต่ำ — กำลังให้คะแนนความคุ้ม": "The web card game market is still growing and cheap to enter — scoring whether it is worth it",
  "โปรแกรมสรุปรายจ่ายจากสลิป": "Expense summary from payment slips",
  "ลากรูปสลิปใส่หน้าเว็บ แล้วได้ตารางรายจ่ายรายเดือน + กราฟ (OCR ในเครื่อง ไม่ต้องใช้เน็ต)": "Drop slip photos on a web page and get a monthly expense table + chart (on-device OCR, works offline)",
  "Rust async ทำงานยังไงจริงๆ": "How Rust async really works",
  "สรุป 5 นาที + cheat sheet + ตัวอย่างรันได้ 3 ไฟล์": "5-minute summary + cheat sheet + 3 runnable examples",
  "แอปจองคิวร้านตัดผม ขายได้ไหม": "Can a barber booking app sell?",
  "ตลาดมีเจ้าใหญ่ฟรีอยู่แล้ว แข่งยาก — แนะนำเปลี่ยนเป็น LINE OA template ขายร้านเล็ก": "Big free players already own this market — pivot to a chat-bot template for small shops",
  "สมัคร LINE Developers (ฟรี) ถ้าจะลองแนวทางใหม่": "Sign up for a free developer account to try the new direction",
  "กาแฟเย็นทำให้ง่วงจริงไหม": "Does iced coffee really make you sleepy?",
  "บทสรุปเรื่องที่ ": "Summary #",
  "ร้านก๋วยเตี๋ยวเรือ": "Noodle shop",
  "แดชบอร์ด 5S": "5S dashboard",
  "ค้นเว็บ: ตลาดเกมการ์ดสั้นบนเว็บ": "Web search: web card game market",
  "ตลาดเกมการ์ดสั้นบนเว็บมีคู่แข่งน้อยกว่าที่คิด แต่ช่องทางหาผู้เล่นคือคอขวด": "Fewer competitors than expected, but finding players is the bottleneck",
  "อ่าน REPORT.md และ ANSWERS.md ก่อนเริ่มให้คะแนน": "Reading REPORT.md and ANSWERS.md before scoring",
  "อยากให้เกมเล่นกับเพื่อนแบบไหน?": "How should friends play together?",
  "ออนไลน์ real-time": "Online, real-time",
  "ผลัดกันเล่น (async)": "Take turns (async)",
  "เล่นคนเดียวกับบอท": "Solo against a bot",
  "สลิปส่วนใหญ่มาจากไหน?": "Where do most slips come from?",
  "แคปหน้าจอแอปธนาคาร": "Banking app screenshots",
  "อีเมล": "Email",
  "พร้อมลองเล่น · 2 เสร็จ · 2 คำถามรอคุณ · บอทใช้ไป 18% ของสัปดาห์": "ready to try · 2 done · 2 questions for you · bot used 18% of the week",
  "นอกเวลางาน — บอททำงานได้": "Outside work hours — the bot can work",
  "ค้นเว็บ: ปริมาณน้ำตาล กาแฟเย็น Amazon Café": "Web search: sugar in iced coffee at popular cafés",
  "จะอ่านรายงานเดิมก่อน เพื่อดูว่ามีข้อมูลน้ำตาลอะไรอยู่แล้วบ้าง": "Reading the earlier report first to see what sugar data is already there",
  "ได้ข้อมูล 5 ร้านแล้ว จะทำเป็นตารางและใส่แหล่งอ้างอิงท้ายตาราง": "Got data for 5 cafés — turning it into a table with sources",
  "คุณ: เพิ่มตารางเปรียบเทียบปริมาณน้ำตาลของกาแฟเย็นแต่ละร้าน": "You: add a table comparing sugar in each café's iced coffee",
  "คุณ: สรุปผลสำรวจเมื่อวานให้หน่อย": "You: summarise yesterday's survey",
  "อ่านไฟล์ REPORT.md": "Read REPORT.md",
  "แก้ไฟล์ REPORT.md": "Edit REPORT.md",
  "แก้ปุ่มเสร็จแล้ว": "Button fixed",
  "จบรอบนี้": "Round finished",
  "กำลังเปิด session…": "Opening session…",
  "ระบบจองคิวร้านตัดผม": "Barber booking system",
}).sort((a, b) => b[0].length - a[0].length);
function toEnglish(v) {
  let j = JSON.stringify(v);
  for (const [th, en] of EN) j = j.split(th).join(en);
  return JSON.parse(j);
}

// team roles (roles.rs) for the preview
const demoRoles = [
  { slug: "bom", name: "บอม", title: "Front-end developer", emoji: "🧑‍💻", description: "ทำหน้าเว็บ Svelte/React ให้สวย ใช้ง่าย รองรับมือถือ", instructions: "You build clean, accessible UIs.", persona: "พูดตรง สั้น เป็นกันเอง ชอบลองของใหม่แต่เลือกทางที่ดูแลง่าย", tier: "balanced", account: "", model_override: "", tools: "", source: "", license: "" },
  { slug: "nam", name: "น้ำ", title: "Tester", emoji: "🧪", description: "หาบั๊ก เขียนเทสต์ ตรวจว่าใช้งานได้จริง", instructions: "You find bugs and write tests.", persona: "", tier: "fast", account: "codex01", model_override: "", tools: "", source: "https://github.com/VoltAgent/awesome-claude-code-subagents/blob/x/categories/04-quality-security/qa-expert.md", license: "MIT" },
];
const roleRender = (r, withPersona) => `---\nname: "${r.name}"\ndescription: "${r.description}"\ntitle: "${r.title}"\ntools: Read, Write, Edit, Bash, Glob, Grep, WebSearch, WebFetch\ntier: ${r.tier}\n---\n\n${r.instructions}\n` + (withPersona && r.persona ? `\n<!-- persona -->\n## Persona\n\n${r.persona}\n<!-- /persona -->\n` : "");
function rolesMock(cmd, args) {
  if (cmd === "roles_list") return { roles: structuredClone(demoRoles), tiers: ["auto", "fast", "balanced", "strong"], curated: [] };
  if (cmd === "role_save") {
    const r = { ...args.role, slug: args.role.slug || args.role.name.toLowerCase().replace(/[^a-z0-9]+/g, "-") || `role-${demoRoles.length + 1}` };
    const i = demoRoles.findIndex((x) => x.slug === r.slug);
    if (i >= 0) demoRoles[i] = r; else demoRoles.push(r);
    return r;
  }
  if (cmd === "role_delete") { demoRoles.splice(demoRoles.findIndex((x) => x.slug === args.slug), 1); return null; }
  if (cmd === "role_export") { const r = demoRoles.find((x) => x.slug === args.slug); return { text: roleRender(r, args.withPersona), warnings: [], path: args.save ? `C:/IdeaAutopilot/data/exports/${r.slug}.agent.md` : null }; }
  if (cmd === "roles_shop_search") return { repos: [
    { repo: "msitarzewski/agency-agents", description: "The Agency: agents with a full personality", stars: 169578, listed: "curated" },
    { repo: "wshobson/agents", description: "Production-ready subagents", stars: 42744, listed: "curated" },
    { repo: "VoltAgent/awesome-claude-code-subagents", description: "100+ subagents in categories", stars: 26706, listed: "curated" },
  ], note: null };
  if (cmd === "roles_shop_browse") return { repo: args.repo, commit: "0".repeat(40), description: "demo", stars: 169578, license: "MIT", url: `https://github.com/${args.repo}`, total: 4, roles: [
    { path: "engineering/engineering-frontend-developer.md", name: "engineering frontend developer", category: "engineering", size: 9000, too_big: false },
    { path: "engineering/engineering-backend-architect.md", name: "engineering backend architect", category: "engineering", size: 9000, too_big: false },
    { path: "design/design-ui-designer.md", name: "design ui designer", category: "design", size: 8000, too_big: false },
    { path: "marketing/marketing-growth-hacker.md", name: "marketing growth hacker", category: "marketing", size: 8000, too_big: false },
  ] };
  if (cmd === "roles_shop_preview" || cmd === "role_check_text") {
    const role = { slug: "frontend-developer", name: "Frontend Developer", title: "", emoji: "🖥️", description: "Expert frontend developer", tier: "auto", account: "", model_override: "", instructions: "You are Frontend Developer…", persona: "", tools: "", source: "", license: "MIT" };
    return { role, text: roleRender(role, false), sha256: "x", size: 100, verdict: "clean", findings: [], domains: [], source: "https://github.com/x", license: "MIT", needs_accept: false };
  }
  if (cmd === "roles_shop_install" || cmd === "role_import_text") { const r = { ...demoRoles[0], slug: "frontend-developer", name: "Frontend Developer", emoji: "🖥️", persona: "", team: false }; demoRoles.push(r); return r; }
  return undefined;
}

// named teams (teams.rs) for the preview
const demoTeams = [{ slug: "web-app", name: "ทีมเว็บแอป", emoji: "🌐", description: "สร้างเว็บตั้งแต่หน้าตาถึงหลังบ้าน แล้วทดสอบ", use_for: "เว็บ, แอป, web, app, dashboard", members: ["bom", "nam"], helpers: 3, lead: "" }];
const teamJson = (t) => ({ ...t, member_roles: t.members.map((m) => demoRoles.find((r) => r.slug === m)).filter(Boolean).map((r) => ({ slug: r.slug, name: r.name, emoji: r.emoji, title: r.title, tier: r.tier, account: r.account })) });
const demoParts = [
  { role: "bom", account: "", model: "", folder: "parts/ui", task: "ทำหน้าเลือกการ์ดและหน้าห้องเล่น (Svelte)", status: "ok", started: now - 1500, ended: now - 900 },
  { role: "nam", account: "codex01", model: "gpt-6.1-sol", folder: "parts/test", task: "เขียนเทสต์กติกาการ์ดและลองเล่นครบ 1 เกม", status: "running", started: now - 400, ended: null },
];
function teamsMock(cmd, args) {
  if (cmd === "teams_list") return { teams: demoTeams.map(teamJson), presets: [
    { key: "web-app", name: "ทีมเว็บแอป", emoji: "🌐", description: "สร้างเว็บ/แอป", use_for: "" },
    { key: "research", name: "ทีมค้นคว้า/ตัดสินใจ", emoji: "🔎", description: "หาข้อมูล ให้คะแนน", use_for: "" },
    { key: "marketing", name: "ทีมการตลาด/คอนเทนต์", emoji: "📣", description: "คอนเทนต์ โพสต์", use_for: "" },
  ] };
  if (cmd === "team_save") { const t = { ...args.team, slug: args.team.slug || `team-${demoTeams.length + 1}` }; const i = demoTeams.findIndex((x) => x.slug === t.slug); if (i >= 0) demoTeams[i] = t; else demoTeams.push(t); return teamJson(t); }
  if (cmd === "team_delete") { demoTeams.splice(demoTeams.findIndex((x) => x.slug === args.slug), 1); return null; }
  if (cmd === "team_preset") { const t = { slug: `${args.key}-2`, name: args.key === "marketing" ? "ทีมการตลาด/คอนเทนต์" : "ทีมค้นคว้า/ตัดสินใจ", emoji: "🔎", description: "", use_for: "", members: [], helpers: 3, lead: "" }; demoTeams.push(t); return teamJson(t); }
  if (cmd === "team_suggest") return /เว็บ|web|แอป|app|dashboard/i.test(args.text) ? "web-app" : null;
  if (cmd === "team_export") { const t = demoTeams.find((x) => x.slug === args.slug); return { text: `---\nname: "${t.name}"\nmembers: ${t.members.join(", ")}\nhelpers: ${t.helpers}\n---\n` + t.members.map((m) => `\n<!-- role: ${m} -->\n` + roleRender(demoRoles.find((r) => r.slug === m), args.withPersona)).join(""), warnings: [], path: args.save ? `C:/IdeaAutopilot/data/exports/${t.slug}.team.md` : null }; }
  if (cmd === "team_check_text") return { team: { name: "ทีมเพื่อน", emoji: "🤝", description: "จากไฟล์", use_for: "", helpers: 2 }, roles: demoRoles.slice(0, 2), verdict: "clean", findings: [], needs_accept: false };
  if (cmd === "team_import_text") { const t = { slug: "friends", name: "ทีมเพื่อน", emoji: "🤝", description: "", use_for: "", members: ["bom"], helpers: 2, lead: "" }; demoTeams.push(t); return teamJson(t); }
  if (cmd === "idea_team_parts") return args.id === 1 ? structuredClone(demoParts) : [];
  if (cmd === "idea_team_set") { state.idea_ai[args.id] = { ...(state.idea_ai[args.id] ?? {}), team: args.team }; return null; }
  return undefined;
}

// the account list as the app keeps it: switched-on first, each group in the order the owner set (ISSUE-121)
const accState = { on: {}, moves: [] };
function accountsInOrder(r) {
  let v = r.accounts.map((a) => (a.id in accState.on ? { ...a, enabled: accState.on[a.id] } : a));
  v = [...v.filter((a) => a.enabled), ...v.filter((a) => !a.enabled)];
  for (const [id, dir] of accState.moves) {
    v = [...v.filter((a) => a.enabled), ...v.filter((a) => !a.enabled)];
    const i = v.findIndex((a) => a.id === id), j = i + (dir < 0 ? -1 : 1);
    if (i >= 0 && j >= 0 && j < v.length && v[j].enabled === v[i].enabled) [v[i], v[j]] = [v[j], v[i]];
  }
  return { ...r, accounts: v };
}

export async function mockInvoke(cmd, args = {}) {
  await new Promise((r) => setTimeout(r, 60));
  if (cmd.startsWith("role")) { const r = rolesMock(cmd, args); if (r !== undefined) return r; }
  if (cmd.startsWith("team") || cmd.startsWith("idea_team")) { const r = teamsMock(cmd, args); if (r !== undefined) return r; }
  if (cmd === "account_update" && args.enabled != null) { accState.on[args.id] = args.enabled; return null; }
  if (cmd === "account_move") { accState.moves.push([args.id, args.dir]); return null; }
  if (cmd === "accounts_list") return accountsInOrder(DEMO_LANG === "en" ? toEnglish(handlers.accounts_list()) : structuredClone(handlers.accounts_list()));
  const h = handlers[cmd];
  if (h) return DEMO_LANG === "en" ? toEnglish(h(args)) : structuredClone(h(args));
  if (cmd === "session_cut_in") { const x = state.sessions.find((s) => s.idea_id === args.id); if (x) { x.busy_since = 0; x.waiting = 0; x.status = "idle"; x.feed.push({ ts: Math.floor(Date.now() / 1000), text: "✋ กำลังหยุดรอบที่ทำอยู่ตามที่คุณสั่ง…" }); } return null; }
  if (cmd === "pause_all") {
    state.shelf = { ts: now, by_owner: true, was_paused: false, run: state.current ? [state.current.idea_id, state.current.title, state.current.stage] : null,
      sessions: state.sessions.map((x) => ({ id: x.idea_id, title: x.name.split(": ")[1] ?? x.name, working: x.status === "working", ai: {} })) };
    state.paused = true; state.current = null; state.sessions = [];
    return state.shelf;
  }
  if (cmd === "resume_all") { const n = state.shelf?.sessions.length ?? 0; state.shelf = null; state.paused = false; return n; }
  if (cmd === "shelf_dismiss") { state.shelf = null; state.paused = false; return null; }
  if (cmd === "idea_action") {
    const i = ideas.find((x) => x.id === args.id);
    if (i && ["pin", "unpin"].includes(args.action)) i.pinned = args.action === "pin";
    if (i && ["fixing", "works"].includes(args.action)) i.fixing = args.action === "fixing";
    return null;
  }
  if (cmd === "add_idea") {
    ideas.unshift({ ...ideas[4], id: ideas.length + 10, title: args.text.slice(0, 80), scope: args.scope || "personal", created: now, updated: now });
    return 1;
  }
  return null;
}
