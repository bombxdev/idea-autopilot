// Which accounts the quota page shows, and whether each gets its own plan and chart ("split") or they are drawn as one
// ("combined"). Shared by the plan, the charts and the picker; remembered in the setting `quota_view`.
import { api } from "./api.js";

export const qv = $state({ show: null, mode: "split", loaded: false }); // show: null = every account

export async function loadQv() {
  if (qv.loaded) return;
  try {
    const s = await api.settings();
    const v = JSON.parse(s.quota_view || "{}");
    qv.show = Array.isArray(v.show) ? v.show : null;
    qv.mode = v.mode === "combined" ? "combined" : "split";
  } catch {}
  qv.loaded = true;
}

export function saveQv() {
  api.setSetting("quota_view", JSON.stringify({ show: qv.show, mode: qv.mode })).catch(() => {});
}

export const isShown = (id) => !qv.show || qv.show.includes(id);

export function toggleAccount(id, all) {
  const cur = qv.show ?? all;
  const next = cur.includes(id) ? cur.filter((x) => x !== id) : [...cur, id];
  qv.show = next.length === all.length ? null : next;
  saveQv();
}

export function onlyAccount(id) {
  qv.show = [id];
  saveQv();
}

export function setMode(m) {
  qv.mode = m;
  saveQv();
}
