// Interface translation. Every piece of text the system writes is in Thai in the source; scripts/extract-i18n.py
// lists them as templates in i18n/source.json ({0}, {1} = a value filled in at run time). A language file maps each
// template to its text in that language. This module matches what is displayed against the templates and swaps
// the text — in text nodes and in title / placeholder / aria-label / alt — and keeps doing so as the page changes.
// Only a whole text that equals a template is replaced, so what the owner typed (idea titles, notes, reports)
// is never touched, and a text with no translation stays as it is.
//
// English ships with the app (i18n/en.json, same order as source.json). Any other language is translated once by the
// owner's own AI (the app asks first) and cached on disk: see ui_translate.rs.
import source from "./i18n/source.json";
import en from "./i18n/en.json";
import { api } from "./api.js";
import { ui } from "./uilang.svelte.js";

const ATTRS = ["title", "placeholder", "aria-label", "alt"];
const WEEKDAYS = ["พฤ.", "อา.", "จ.", "อ.", "พ.", "ศ.", "ส."];
const SKIP = new Set(["SCRIPT", "STYLE", "TEXTAREA", "CODE", "PRE"]);

let lang = "th";
let exact = new Map();
let patterns = [];
let started = false;
let observer = null;
const seen = new WeakMap(); // node or element -> { text: {src,out} | attr: {name: {src,out}} }

const norm = (s) => s.replace(/\s+/g, " ").trim();
const hasThai = (s) => /[฀-๿]/.test(s);
const esc = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/** Build the matchers from {thai template: translation}. */
function compile(map) {
  exact = new Map();
  patterns = [];
  for (const [k, v] of Object.entries(map)) {
    if (!v || typeof v !== "string") continue;
    if (!/\{\d+\}/.test(k)) {
      exact.set(k, v);
      continue;
    }
    const parts = k.split(/\{\d+\}/);
    const order = [...k.matchAll(/\{(\d+)\}/g)].map((m) => Number(m[1]));
    const re = new RegExp("^" + parts.map(esc).join("(.*?)") + "$", "s");
    patterns.push({ re, order, out: v, weight: parts.join("").length });
  }
  patterns.sort((a, b) => b.weight - a.weight);
}

function translate(text) {
  const t = norm(text);
  if (!t || !hasThai(t)) return null;
  const hit = exact.get(t);
  if (hit) return hit;
  for (const p of patterns) {
    const m = p.re.exec(t);
    if (!m) continue;
    // the values may themselves be system text (a status word, a label): translate those too
    const vals = [];
    p.order.forEach((n, i) => (vals[n] = translate(m[i + 1]) ?? m[i + 1]));
    return p.out.replace(/\{(\d+)\}/g, (_, n) => vals[Number(n)] ?? "");
  }
  // "a · b · c": each part on its own (a line of facts joined with the dot)
  if (t.includes(" · ")) {
    const parts = t.split(" · ");
    const done = parts.map((p) => translate(p));
    if (done.some((x) => x != null)) return parts.map((p, i) => done[i] ?? p).join(" · ");
  }
  // a weekday abbreviation in front of a time or a date: "จ. 9:00", "พ. 4"
  const wd = WEEKDAYS.find((w) => t.startsWith(w + " "));
  if (wd && exact.has(wd)) return exact.get(wd) + t.slice(wd.length);
  // a symbol or emoji after a system word: "อรุณสวัสดิ์ 👋"
  const tail = t.match(/^(.+?)\s+([^\p{L}\p{N}]+)$/u);
  if (tail) {
    const head = translate(tail[1]);
    if (head != null) return head + " " + tail[2];
  }
  // a symbol or emoji in front of a system word: "💰 หารายได้"
  const lead = t.match(/^([^\p{L}\p{N}]+)\s+(.+)$/u);
  if (lead) {
    const rest = translate(lead[2]);
    if (rest != null) return lead[1] + " " + rest;
  }
  // "<value> — not installed": a value followed by a system phrase that starts with a separator
  for (const [k, v] of exact) {
    if (k.length < 6 || !/^[—·(]/.test(k) || !t.endsWith(k) || t.length === k.length) continue;
    const head = t.slice(0, t.length - k.length);
    if (/[\s)]$/.test(head) || /^[—·(]/.test(k)) return (translate(head) ?? head.trim()) + " " + v;
  }
  return null;
}

/** For code that builds a message by itself (confirm dialogs). */
export function tr(text) {
  return lang === "th" ? text : translate(text) ?? text;
}

function textNode(n) {
  const rec = seen.get(n)?.text;
  const now = n.nodeValue;
  if (rec && now === rec.out) return; // our own change
  if (!hasThai(now)) {
    if (rec) seen.delete(n);
    return;
  }
  const out = translate(now);
  if (out == null) return;
  const lead = now.match(/^\s*/)[0];
  const trail = now.match(/\s*$/)[0];
  const full = lead + out + trail;
  seen.set(n, { text: { src: now, out: full } });
  n.nodeValue = full;
}

function attrs(el) {
  for (const a of ATTRS) {
    const now = el.getAttribute?.(a);
    if (now == null) continue;
    const rec = seen.get(el)?.attr?.[a];
    if (rec && now === rec.out) continue;
    if (!hasThai(now)) continue;
    const out = translate(now);
    if (out == null) continue;
    const cur = seen.get(el) ?? {};
    cur.attr = { ...(cur.attr ?? {}), [a]: { src: now, out } };
    seen.set(el, cur);
    el.setAttribute(a, out);
  }
}

function walk(root) {
  if (!root) return;
  if (root.nodeType === 3) return textNode(root);
  if (root.nodeType !== 1) return;
  if (root.closest?.("[data-notrans]")) return;
  if (root.tagName !== "SCRIPT" && root.tagName !== "STYLE") attrs(root); // a textarea's placeholder is system text; what is typed in it is not
  if (SKIP.has(root.tagName)) return;
  for (let c = root.firstChild; c; c = c.nextSibling) walk(c);
}

/** Put every translated text back to its Thai source (switching to Thai, or to another language). */
function restore() {
  const visit = (n) => {
    const rec = seen.get(n);
    if (rec?.text && n.nodeValue === rec.text.out) n.nodeValue = rec.text.src;
    if (rec?.attr) for (const [a, r] of Object.entries(rec.attr)) if (n.getAttribute(a) === r.out) n.setAttribute(a, r.src);
    seen.delete(n);
    for (let c = n.firstChild; c; c = c.nextSibling) visit(c);
  };
  if (document.body) visit(document.body);
}

function observe() {
  if (observer || typeof MutationObserver === "undefined") return;
  observer = new MutationObserver((list) => {
    if (lang === "th") return;
    for (const m of list) {
      if (m.type === "characterData") textNode(m.target);
      else if (m.type === "attributes") attrs(m.target);
      else m.addedNodes.forEach((n) => walk(n));
    }
  });
  observer.observe(document.body, { childList: true, characterData: true, subtree: true, attributes: true, attributeFilter: ATTRS });
}

function wrapDialogs() {
  for (const k of ["confirm", "alert", "prompt"]) {
    const orig = window[k]?.bind(window);
    if (!orig) continue;
    window[k] = (msg, ...rest) => orig(typeof msg === "string" ? tr(msg) : msg, ...rest);
  }
}

const englishMap = () => en; // {thai template: english}

export const status = $state({ lang: "th", state: "ready", done: 0, total: 0, error: "" });

/** Switch the interface language. Returns "ready", or "missing" when this language still has to be translated. */
export async function setUiLang(code) {
  lang = code || "en";
  ui.lang = lang;
  status.lang = lang;
  status.error = "";
  if (!started && typeof document !== "undefined") {
    started = true;
    wrapDialogs();
  }
  restore();
  if (lang === "th") {
    status.state = "ready";
    return "ready";
  }
  let map = null;
  if (lang === "en") map = englishMap();
  else {
    try {
      const r = await api.uiDict(lang);
      status.state = r.status;
      status.done = r.done ?? 0;
      status.total = r.total ?? 0;
      if (r.status === "ready") map = r.map;
      else if (r.status === "translating") map = r.map ?? null;
    } catch {
      status.state = "missing";
    }
    // until the language is translated, the English text is better than Thai
    map = { ...englishMap(), ...(map ?? {}) };
  }
  if (lang === "en") status.state = "ready";
  compile(map);
  observe();
  walk(document.body);
  return status.state;
}

/** Re-read the cached translation (progress of a running translation, or a finished one). */
export async function refreshDict() {
  if (lang === "th" || lang === "en") return;
  const r = await api.uiDict(lang);
  status.state = r.status;
  status.done = r.done ?? 0;
  status.total = r.total ?? 0;
  if (r.map) {
    restore();
    compile({ ...englishMap(), ...r.map });
    walk(document.body);
  }
}

export const TEMPLATE_COUNT = source.length;
