// Side-by-side translation of English content (ISSUE-174): whether it is on, into which language, and which AI
// would translate right now ("" = none has quota left — then no side panel is shown).
import { api } from "./api.js";

export const ct = $state({ on: false, lang: "th", lang_name: "Thai", account: "", fallback: true, accounts: [], now: "", now_label: "" });
let loaded = 0;

export async function loadCt(force = false) {
  if (!force && Date.now() - loaded < 30_000) return;
  loaded = Date.now();
  try { Object.assign(ct, await api.ctStatus()); } catch {}
}

/** Mostly Latin letters = English (or another Latin-script language) worth offering a translation for. */
export function isForeign(text) {
  const letters = [...(text || "")].filter((c) => /\p{L}/u.test(c));
  if (letters.length < 12) return false;
  return letters.filter((c) => /[a-z]/i.test(c)).length * 10 >= letters.length * 7;
}
