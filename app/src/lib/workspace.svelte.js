// The Workspace page (ISSUE-087): ideas the owner is focusing on, each a "window" on a taskbar.
// One window is shown full size, or two side by side; the rest are minimized on the taskbar.
// Taking an idea out of the Workspace only closes its window — the idea itself is untouched.
// Kept per machine in the setting `workspace_windows`.
import { api } from "./api.js";

export const ws = $state({ ids: [], active: null, side: null, loaded: false });

const save = () => api.setSetting("workspace_windows", JSON.stringify({ ids: ws.ids, active: ws.active, side: ws.side })).catch(() => {});

export async function wsLoad() {
  if (ws.loaded) return;
  try {
    const s = await api.settings();
    const v = JSON.parse(s.workspace_windows || "{}");
    ws.ids = Array.isArray(v.ids) ? v.ids.filter(Number.isInteger) : [];
    ws.active = ws.ids.includes(v.active) ? v.active : null;
    ws.side = ws.ids.includes(v.side) && v.side !== ws.active ? v.side : null;
  } catch {}
  ws.loaded = true;
}

/** windows on screen, in order */
export const shown = () => [ws.active, ws.side].filter((x) => x != null);

/** add an idea (or bring it back) and show it */
export function wsOpen(id) {
  if (!ws.ids.includes(id)) ws.ids = [...ws.ids, id];
  if (ws.side === id) ws.side = null;
  ws.active = id;
  save();
}

/** a click on its taskbar button: show it, or minimize it when it is already the window in front */
export function wsToggle(id) {
  if (ws.active === id) wsMinimize(id);
  else wsOpen(id);
}

export function wsMinimize(id) {
  if (ws.active === id) {
    ws.active = ws.side;
    ws.side = null;
  } else if (ws.side === id) ws.side = null;
  save();
}

/** show next to the window in front (two side by side) */
export function wsSide(id) {
  if (ws.active == null || ws.active === id) return wsOpen(id);
  if (!ws.ids.includes(id)) ws.ids = [...ws.ids, id];
  ws.side = id;
  save();
}

/** take it off the Workspace (not a delete) */
export function wsRemove(id) {
  ws.ids = ws.ids.filter((x) => x !== id);
  if (ws.active === id || ws.side === id) wsMinimize(id);
  else save();
}

/** forget ideas that no longer exist (deleted / archived elsewhere) */
export function wsPrune(alive) {
  const keep = ws.ids.filter((id) => alive.has(id));
  if (keep.length === ws.ids.length) return;
  ws.ids = keep;
  if (!alive.has(ws.active)) ws.active = null;
  if (!alive.has(ws.side)) ws.side = null;
  if (ws.active == null && ws.side != null) [ws.active, ws.side] = [ws.side, null];
  save();
}
