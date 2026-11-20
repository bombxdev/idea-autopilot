// Review inbox (ISSUE-100): everything that waits for the owner in one list — questions to answer, prototypes to try,
// and work the bot finished — and it stays there until the owner has looked at it ("ตรวจแล้ว") or steered it further.
// What was reviewed is a setting, so it follows the owner to the other machines.
import { api } from "./api.js";
import { store, questionsFor } from "./store.svelte.js";

export const review = $state({ from: 0, ids: {}, ready: false });
const nowS = () => Math.floor(Date.now() / 1000);
const save = () => api.setSetting("reviewed", JSON.stringify({ from: review.from, ids: review.ids })).catch(() => {});

export async function loadReview() {
  let v = null;
  try {
    v = JSON.parse((await api.settings()).reviewed || "null");
  } catch {}
  const first = !v;
  // first time: only what finished in the last day counts as new, older work is taken as seen
  if (first) v = { from: nowS() - 86400, ids: {} };
  review.from = v.from ?? 0;
  review.ids = v.ids ?? {};
  review.ready = true;
  if (first) save();
}

/// Finished (or failed, or ready to try) and not looked at since it last changed.
export function needsReview(i) {
  if (!review.ready || !["done", "failed", "ready"].includes(i.status)) return false;
  const seen = review.ids[i.id] ?? (i.status === "ready" ? 0 : review.from);
  return i.updated > seen;
}

export function markReviewed(i) {
  review.ids = { ...review.ids, [i.id]: nowS() };
  save();
}

/// Ideas waiting for the owner: those with a question first, then the ones to review (newest first).
export function inbox() {
  const live = store.ideas.filter((i) => !["archived", "deleted"].includes(i.status));
  const asking = live.filter((i) => questionsFor(i.id).length > 0);
  const rest = live.filter((i) => !asking.includes(i) && needsReview(i)).sort((a, b) => b.updated - a.updated);
  return { asking, rest, count: asking.length + rest.length };
}
