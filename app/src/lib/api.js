import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen } from "@tauri-apps/api/event";
import { mockInvoke } from "./mock.js";
import { ui } from "./uilang.svelte.js";
import { locale } from "./i18n.js";

// outside Tauri (plain browser preview) fall back to sample data
const inTauri = typeof window !== "undefined" && !!window.__TAURI_INTERNALS__;
const invoke = inTauri ? tauriInvoke : mockInvoke;
const listen = inTauri ? tauriListen : async () => () => {};

/** a narrated step (not the AI talking): old "›" lines or emoji-led lines except 💬 */
export const isStep = (l) => l.startsWith("›") || (/^\p{Extended_Pictographic}/u.test(l) && !l.startsWith("💬"));

export const api = {
  state: () => invoke("get_state"),
  ideas: () => invoke("list_ideas"),
  addIdea: (text, attachments = [], ai = null, scope = null) => invoke("add_idea", { text, scope, attachments: attachments.length ? attachments : null, ai: ai && (ai.account || ai.model || ai.effort) ? { account: ai.account || "", model: ai.model || "", effort: ai.effort || "", skills: [] } : null }),
  ideaScopeSet: (id, scope) => invoke("idea_scope_set", { id, scope }),
  ideaAiSet: (id, account, model, effort) => invoke("idea_ai_set", { id, account: account || null, model: model || null, effort: effort || null }),
  idea: (id) => invoke("get_idea", { id }),
  answer: (id, answer) => invoke("answer_question", { id, answer }),
  action: (id, action, text, attachments = []) => invoke("idea_action", { id, action, text: text ?? null, attachments: attachments.length ? attachments : null }),
  reorder: (ids) => invoke("reorder", { ids }),
  setPaused: (paused) => invoke("set_paused", { paused }),
  pauseAll: () => invoke("pause_all"),
  resumeAll: () => invoke("resume_all"),
  shelfDismiss: () => invoke("shelf_dismiss"),
  setTurbo: (hours) => invoke("set_turbo", { hours }),
  stopCurrent: () => invoke("stop_current"),
  refreshUsage: () => invoke("refresh_usage"),
  reports: (days) => invoke("get_reports", { days }),
  exportCsv: () => invoke("export_csv"),
  digest: () => invoke("get_digest"),
  makeDigest: () => invoke("make_digest"),
  events: (hours) => invoke("get_events", { hours }),
  calendar: (days) => invoke("get_calendar", { days }),
  listDays: () => invoke("list_days"),
  setDay: (date, kind, note) => invoke("set_day", { date, kind, note: note ?? null }),
  settings: () => invoke("get_settings"),
  setSetting: (key, value) => invoke("set_setting", { key, value }),
  library: (q) => invoke("library", { q: q ?? null }),
  openFolder: (id) => invoke("open_folder", { id }),
  runPrototype: (id) => invoke("run_prototype", { id }),
  hideCapture: () => invoke("hide_capture"),
  exportBrain: () => invoke("export_brain"),
  activity: (days) => invoke("get_activity", { days }),
  policy: () => invoke("get_policy"),
  remote: (id, on) => invoke("idea_remote", { id, on }),
  sessionRevive: (id) => invoke("session_revive", { id }),
  sessionCutIn: (id) => invoke("session_cut_in", { id }),
  codexThread: (id) => invoke("codex_thread", { id }),
  ask: (id, text, attachments = [], ai = null) => invoke("idea_ask", { id, text, attachments: attachments.length ? attachments : null, ai }),
  handoff: (id, agent, text, attachments = [], ai = null) => invoke("idea_handoff", { id, agent, text, attachments: attachments.length ? attachments : null, ai }),
  sessionsOverview: () => invoke("sessions_overview"),
  sessionClose: (id, summarize) => invoke("session_close", { id, summarize }),
  sessionCloseAll: (summarize) => invoke("session_close_all", { summarize }),
  skillsSlash: () => invoke("skills_slash_list"),
  hold: (id, on) => invoke("idea_hold", { id, on }),
  runLog: (runId) => invoke("get_run_log", { runId }),
  providers: () => invoke("get_providers"),
  about: () => invoke("get_about"),
  editIdea: (id, text) => invoke("edit_idea", { id, text }),
  switchCopy: () => invoke("switch_copy"),
  testProvider: (provider) => invoke("test_provider", { provider: provider ?? null }),
  syncStatus: () => invoke("sync_status"),
  takeover: () => invoke("takeover"),
  showMain: (idea) => invoke("show_main", { idea: idea ?? null }),
  skills: () => invoke("skills_list"),
  skillRead: (slug) => invoke("skills_read", { slug }),
  skillSet: (slug, field, value) => invoke("skills_set", { slug, field, value }),
  skillDelete: (slug) => invoke("skills_delete", { slug }),
  skillAddLocal: (text, acceptWarnings) => invoke("skills_add_local", { text, acceptWarnings }),
  // attachments: a pasted/picked file goes up as raw bytes (name in a header); a dragged file goes by path
  attachStage: async (file) => invoke("attach_stage", new Uint8Array(await file.arrayBuffer()), { headers: { "x-name": encodeURIComponent(file.name || "file") } }),
  attachStagePath: (path) => invoke("attach_stage_path", { path }),
  attachUnstage: (token) => invoke("attach_unstage", { token }),
  attachInfo: () => invoke("attach_info"),
  attachList: (id) => invoke("attach_list", { id }),
  attachPreview: (id, name) => invoke("attach_preview", { id, name }),
  attachDelete: (id, name) => invoke("attach_delete", { id, name }),
  accounts: () => invoke("accounts_list"),
  accountAdd: (o) => invoke("account_add", { vendor: o.vendor, id: o.id ?? null, isolated: !!o.isolated, auth: o.auth ?? null, key: o.key ?? null, baseUrl: o.base_url ?? null, model: o.model ?? null }),
  accountSetKey: (id, key, baseUrl = null) => invoke("account_set_key", { id, key, baseUrl }),
  accountClearKey: (id) => invoke("account_clear_key", { id }),
  accountModels: (id) => invoke("account_models", { id }),
  modelCheckNow: (account, models = null) => invoke("model_check_now", { account, models }),
  accountTest: (id) => invoke("account_test", { id }),
  cliInstallCommand: (kind) => invoke("cli_install_command", { kind }),
  cliInstallStart: (kind, registry = null) => invoke("cli_install_start", { kind, registry }),
  cliInstallStatus: () => invoke("cli_install_status"),
  accountOmniDupOk: (id, ok) => invoke("account_omni_dup_ok", { id, ok }),
  accountUpdate: (id, patch) => invoke("account_update", { id, label: patch.label ?? null, enabled: patch.enabled ?? null, model: patch.model ?? null, effort: patch.effort ?? null }),
  accountRemove: (id) => invoke("account_remove", { id }),
  accountLogin: (id) => invoke("account_login", { id }),
  accountsSeries: (days = 7) => invoke("accounts_series", { days }),
  uiDict: (lang) => invoke("ui_dict", { lang }),
  uiTranslateStart: (lang) => invoke("ui_translate_start", { lang }),
  updateStatus: (refresh = false) => invoke("update_status", { refresh }),
  updateApply: () => invoke("update_apply"),
  modelEconomics: () => invoke("model_economics"),
  powerSetIdea: (id, on) => invoke("power_set_idea", { id, on }),
  skillCheck: (text) => invoke("skills_check_text", { text }),
  skillDeepCheck: (text) => invoke("skills_deep_check", { text }),
  skillspectorStatus: (refresh = false) => invoke("skillspector_status", { refresh }),
  skillspectorInstall: () => invoke("skillspector_install"),
  skillspectorRescan: () => invoke("skillspector_rescan"),
  shopSearch: (query) => invoke("skills_shop_search", { query }),
  shopBrowse: (repo) => invoke("skills_shop_browse", { repo }),
  shopResolve: (url) => invoke("skills_shop_resolve", { url }),
  shopPreview: (repo, commit, path) => invoke("skills_shop_preview", { repo, commit, path }),
  shopInstall: (repo, commit, path, sha256, acceptWarnings) => invoke("skills_shop_install", { repo, commit, path, sha256, acceptWarnings }),
};

export { listen };

export const INTENTS = {
  money: { label: "หารายได้", icon: "coin", color: "amber" },
  experiment: { label: "ทดลอง", icon: "flask", color: "blue" },
  learn: { label: "อยากเข้าใจ", icon: "book", color: "purple" },
  tool: { label: "ใช้เอง", icon: "wrench", color: "teal" },
  sell: { label: "ต่อยอดขาย", icon: "rocket", color: "pink" },
};

export function intentMeta(key) {
  return INTENTS[key] ?? { label: key, icon: "sparkle", color: "blue" };
}

export const STATUS = {
  inbox: { label: "รอตีความ", color: "blue" },
  active: { label: "กำลังทำ", color: "teal" },
  ready: { label: "พร้อมใช้", color: "teal" },
  done: { label: "เสร็จแล้ว", color: "purple" },
  failed: { label: "ติดปัญหา", color: "red" },
  archived: { label: "เก็บแล้ว", color: "blue" },
};

export function ago(ts) {
  if (!ts) return "";
  const s = Math.floor(Date.now() / 1000 - ts);
  if (s < 60) return "เมื่อกี้";
  if (s < 3600) return `${Math.floor(s / 60)} นาทีที่แล้ว`;
  if (s < 86400) return `${Math.floor(s / 3600)} ชม.ที่แล้ว`;
  return `${Math.floor(s / 86400)} วันที่แล้ว`;
}

export function clock(ts) {
  if (!ts) return "—";
  const d = new Date(ts * 1000);
  const day = d.toLocaleDateString(locale(ui.lang), { weekday: "short" });
  return `${day} ${d.getHours().toString().padStart(2, "0")}:${d.getMinutes().toString().padStart(2, "0")}`;
}

export function pct(v, d = 0) {
  return v == null ? "—" : `${Number(v).toFixed(d)}%`;
}
