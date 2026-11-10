import { api, listen } from "./api.js";

export const store = $state({
  lang: "en", // interface language (setting `language`)
  state: null,
  ideas: [],
  questions: [],
  stages: [],
  openIdea: null, // id of the idea shown in the drawer
  settingsTab: null, // a tab of the Settings page to open next (e.g. "about" from the version button)
  page: (typeof location !== "undefined" && new URLSearchParams(location.search).get("page")) || "morning",
  toast: null,
  now: Math.floor(Date.now() / 1000), // ticks every few seconds so "x minutes ago" stays true
});
setInterval(() => (store.now = Math.floor(Date.now() / 1000)), 5000);

/** 45 sec / 3 min / 1 hr 5 min */
export function dur(sec) {
  sec = Math.max(0, Math.round(sec));
  if (sec < 60) return `${sec} วินาที`;
  if (sec < 3600) return `${Math.floor(sec / 60)} นาที`;
  return `${Math.floor(sec / 3600)} ชม. ${Math.floor((sec % 3600) / 60)} นาที`;
}

let busy = false;
export async function refresh() {
  if (busy) return;
  busy = true;
  try {
    const [s, l] = await Promise.all([api.state(), api.ideas()]);
    store.state = s;
    store.ideas = l.ideas;
    store.questions = l.questions;
    store.stages = l.stages;
  } catch (e) {
    console.error(e);
  } finally {
    busy = false;
  }
}

export async function refreshState() {
  try {
    store.state = await api.state();
  } catch (e) {
    console.error(e);
  }
}

export function toast(text, kind = "ok", ms = 2600) {
  store.toast = { text, kind, id: Math.random() };
  setTimeout(() => {
    if (store.toast?.text === text) store.toast = null;
  }, ms);
}

export function stageLabel(key) {
  return store.stages.find((s) => s.key === key)?.label ?? key;
}

export function questionsFor(id) {
  return store.questions.filter((q) => q.idea_id === id);
}

/** The steps of a recipe with repeats folded into one: [research, verify, verify] shows "verify ×2", not two tags. */
export function stepGroups(recipe, stageIdx, active) {
  const out = [];
  recipe.forEach((key, i) => {
    const last = out[out.length - 1];
    if (last && last.key === key) {
      last.count += 1;
      last.to = i;
    } else out.push({ key, count: 1, from: i, to: i });
  });
  return out.map((g) => ({ ...g, done: g.to < stageIdx, now: active && stageIdx >= g.from && stageIdx <= g.to }));
}

/** Progress in % — finished stages over all stages; the running stage counts half. */
export function progress(idea) {
  if (["ready", "done"].includes(idea.status)) return 100;
  const total = idea.recipe.length + 1; // +1 for the clarify step
  const done = idea.status === "inbox" ? 0 : 1 + idea.stage_idx;
  // the running stage counts by how much of its time budget has passed, so the bar keeps creeping
  // while the AI works instead of sitting at a fixed half
  const c = store.state?.current;
  const running = c?.idea_id === idea.id ? Math.min(0.9, Math.max(0.05, (store.now - c.started) / Math.max(1, c.deadline - c.started))) : 0;
  return Math.min(99, Math.round(((done + running) / total) * 100));
}

/** Board column for an idea. */
/** A chat (Remote Control / Codex / other AI) is open on this idea. */
export function chatOpen(idea) {
  return (store.state?.sessions ?? []).some((s) => s.idea_id === idea.id);
}

/** Being changed after it was ready/done: the owner asked for fixes (flag) or a chat is open on it. */
export function fixing(idea) {
  return ["ready", "done"].includes(idea.status) && (idea.fixing || chatOpen(idea));
}

export function column(idea) {
  if (idea.status === "inbox") return "inbox";
  if (fixing(idea)) return "building";
  if (idea.status === "ready") return "ready";
  if (["done", "archived", "failed"].includes(idea.status)) return "done";
  const stage = idea.recipe[idea.stage_idx];
  if (["plan", "build", "verify", "compare"].includes(stage)) return "building";
  // a program sent back for changes ("replan" after it was built) is being fixed, not thought about
  if (idea.run_cmd || idea.fixing) return "building";
  return "thinking";
}

export function start() {
  refresh();
  listen("state-changed", refresh);
  listen("live-log", refreshState);
  listen("open-idea", (e) => (store.openIdea = e.payload));
  listen("navigate", (e) => {
    if (e.payload?.page) store.page = e.payload.page;
    store.openIdea = e.payload?.idea ?? null;
  });
  setInterval(refresh, 20000);
}
