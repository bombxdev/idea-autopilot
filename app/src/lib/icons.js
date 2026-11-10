// The app's own icon set: drawn for Idea Autopilot on a 20 x 20 grid, 1.5 stroke, round ends, no fill unless a
// shape needs it, `currentColor` everywhere so every icon follows the text colour and the light/dark theme.
// Each entry is the inside of an <svg viewBox="0 0 20 20">. Render with <Icon name="..." /> (components/Icon.svelte).
//
// Text that comes from the backend or from older data (log lines, chat feed, toasts) may still start with an emoji
// (the same lines also go to Telegram and Windows notifications, where emoji are right). EMOJI maps each of those
// to an icon here and <IconText> swaps them while showing the text, so no emoji is ever drawn as an icon in the app.

const P = (d) => `<path d="${d}"/>`;
const C = (cx, cy, r) => `<circle cx="${cx}" cy="${cy}" r="${r}"/>`;
const DOT = (cx, cy) => `<circle cx="${cx}" cy="${cy}" r="1" fill="currentColor" stroke="none"/>`;

export const ICONS = {
  // navigation
  sun: C(10, 10, 3.4) + P("M10 2.2v1.9M10 15.9v1.9M2.2 10h1.9M15.9 10h1.9M4.5 4.5l1.3 1.3M14.2 14.2l1.3 1.3M4.5 15.5l1.3-1.3M14.2 5.8l1.3-1.3"),
  sunrise: P("M2.5 14.5h15M5.5 17h9M6 14.5a4 4 0 0 1 8 0M10 3.5V6M4.2 7.4l1.5 1.5M15.8 7.4l-1.5 1.5"),
  board: `<rect x="2.8" y="3.2" width="4.2" height="9.6" rx="1.2"/><rect x="7.9" y="3.2" width="4.2" height="6.2" rx="1.2"/><rect x="13" y="3.2" width="4.2" height="13.6" rx="1.2"/>`,
  gauge: P("M3.2 14.6a7 7 0 1 1 13.6 0M10 13.2l3.2-4.4M6.6 17h6.8") + DOT(10, 13.4),
  book: P("M10 5.6C8.4 4.4 6.2 3.9 3.2 4.1v11.2c3-.2 5.2.3 6.8 1.5 1.6-1.2 3.8-1.7 6.8-1.5V4.1c-3-.2-5.2.3-6.8 1.5zM10 5.6v11.2"),
  toolbox: P("M2.8 8h14.4v7.6a1.4 1.4 0 0 1-1.4 1.4H4.2a1.4 1.4 0 0 1-1.4-1.4zM7 8V5.4A1.4 1.4 0 0 1 8.4 4h3.2A1.4 1.4 0 0 1 13 5.4V8M2.8 11.6h14.4M7.6 10.6v2M12.4 10.6v2"),
  link: P("M8.2 11.8l3.6-3.6M9.2 6.2l1.1-1.1a3.1 3.1 0 0 1 4.4 4.4l-1.1 1.1M10.8 13.8l-1.1 1.1a3.1 3.1 0 0 1-4.4-4.4l1.1-1.1"),
  list: P("M7.6 5h9M7.6 10h9M7.6 15h6") + C(3.9, 5, 1.1) + C(3.9, 10, 1.1) + C(3.9, 15, 1.1),
  gear: P("M15.73 8.6L17.51 8.86L17.51 11.14L15.73 11.4L15.04 13.06L16.12 14.51L14.51 16.12L13.06 15.04L11.4 15.73L11.14 17.51L8.86 17.51L8.6 15.73L6.94 15.04L5.49 16.12L3.88 14.51L4.96 13.06L4.27 11.4L2.49 11.14L2.49 8.86L4.27 8.6L4.96 6.94L3.88 5.49L5.49 3.88L6.94 4.96L8.6 4.27L8.86 2.49L11.14 2.49L11.4 4.27L13.06 4.96L14.51 3.88L16.12 5.49L15.04 6.94Z") + C(10, 10, 2.4),

  // actions
  check: P("M4.4 10.6l3.6 3.6 7.6-8.4"),
  checkCircle: C(10, 10, 7.4) + P("M6.8 10.3l2.2 2.2 4.3-4.7"),
  close: P("M5.4 5.4l9.2 9.2M14.6 5.4l-9.2 9.2"),
  closeCircle: C(10, 10, 7.4) + P("M7.4 7.4l5.2 5.2M12.6 7.4l-5.2 5.2"),
  refresh: P("M15.8 10a5.8 5.8 0 1 1-1.9-4.3M16 3.4v3.8h-3.8"),
  download: P("M10 3v9.4M6.2 8.8l3.8 3.8 3.8-3.8M3.6 16.6h12.8"),
  arrowDown: P("M10 3.4v13M5.2 11.6l4.8 4.8 4.8-4.8"),
  arrowRight: P("M3.6 10h12.6M11.6 5.4l4.6 4.6-4.6 4.6"),
  arrowLeft: P("M16.4 10H3.8M8.4 5.4L3.8 10l4.6 4.6"),
  search: C(8.6, 8.6, 5.2) + P("M12.4 12.4l4.4 4.4"),
  fork: P("M3.2 6.4h3c3.2 0 4.4 7.2 7.6 7.2h3M3.2 13.6h3c1.3 0 2.2-1.2 3-2.7M16.8 6.4h-3c-1.3 0-2.2 1.2-3 2.7M14.6 4.2l2.2 2.2-2.2 2.2M14.6 11.4l2.2 2.2-2.2 2.2"),
  scissors: C(5.6, 14.4, 2.2) + C(14.4, 14.4, 2.2) + P("M7 12.7l7.8-9.5M13 12.7L5.2 3.2"),
  bolt: P("M11.2 2.4L4.4 11.4H10l-1.2 6.2 6.8-9.2H10z"),
  chat: P("M4.2 3.8h11.6a1.6 1.6 0 0 1 1.6 1.6v7.2a1.6 1.6 0 0 1-1.6 1.6H9.4l-3.8 3v-3H4.2a1.6 1.6 0 0 1-1.6-1.6V5.4a1.6 1.6 0 0 1 1.6-1.6z"),
  wrench: P("M12.6 2.9a4.1 4.1 0 0 0-4.8 5.5L3.4 12.8a1.9 1.9 0 0 0 2.7 2.7l4.4-4.4a4.1 4.1 0 0 0 5.5-4.8l-2.4 2.4-2.4-.6-.6-2.4z"),
  terminal: `<rect x="2.4" y="3.4" width="15.2" height="13.2" rx="1.8"/>` + P("M5.8 8l2.6 2.2-2.6 2.2M10.6 12.6h3.6"),
  expand: P("M11.4 3.4h5.2v5.2M16.6 3.4l-5.4 5.4M8.6 16.6H3.4v-5.2M3.4 16.6l5.4-5.4"),
  collapse: P("M16.4 3.6l-4.8 4.8M11.4 4.6v4h4M3.6 16.4l4.8-4.8M8.6 15.4v-4h-4"),
  paperclip: P("M15.4 9.4l-5.6 5.6a3.4 3.4 0 0 1-4.8-4.8l6-6a2.3 2.3 0 0 1 3.2 3.2l-5.9 5.9a1.1 1.1 0 0 1-1.6-1.6l5.4-5.4"),
  pencil: P("M13.2 3.2l3.6 3.6-9.4 9.4H3.8v-3.6zM11.2 5.2l3.6 3.6"),
  trash: P("M3.4 5.6h13.2M8 5.6V3.6h4v2M5 5.6l.8 11h8.4l.8-11M8.5 8.6v5M11.5 8.6v5"),
  pause: P("M7 4.4v11.2M13 4.4v11.2"),
  play: P("M6.2 4l9.6 6-9.6 6z"),
  stop: `<rect x="5" y="5" width="10" height="10" rx="1.6"/>`,
  pin: P("M11.6 2.8l5.6 5.6M13 4.2l-3.4 3.4-3.6.4 6 6 .4-3.6 3.4-3.4M8.6 11.4l-5.2 5.2"),
  star: P("M10 2.6l2.2 4.6 5 .7-3.6 3.5.9 5L10 14l-4.5 2.4.9-5-3.6-3.5 5-.7z"),
  plus: P("M10 4v12M4 10h12"),
  copy: `<rect x="6.6" y="6.6" width="10" height="10" rx="1.6"/>` + P("M13.4 6.6V4.8a1.4 1.4 0 0 0-1.4-1.4H4.8a1.4 1.4 0 0 0-1.4 1.4V12a1.4 1.4 0 0 0 1.4 1.4h1.8"),
  chevronUp: P("M5.4 12.4L10 7.8l4.6 4.6"),
  chevronDown: P("M5.4 7.8l4.6 4.6 4.6-4.6"),
  chevronRight: P("M7.8 5.4l4.6 4.6-4.6 4.6"),
  // the app's mark: a day half lit — the bot works the half you are not using
  logo: C(10, 10, 7.4) + `<path d="M10 2.6a7.4 7.4 0 0 1 0 14.8z" fill="currentColor"/>`,

  // status
  warning: P("M10 3.2L2.6 16.4h14.8z") + P("M10 8.2v3.6") + DOT(10, 14),
  stopSign: C(10, 10, 7.4) + P("M5.8 10h8.4"),
  hourglass: P("M5.4 2.8h9.2M5.4 17.2h9.2M6.4 2.8v2.6c0 2.1 3.6 3 3.6 4.6s-3.6 2.5-3.6 4.6v2.6M13.6 2.8v2.6c0 2.1-3.6 3-3.6 4.6s3.6 2.5 3.6 4.6v2.6"),
  timer: C(10, 11.2, 6) + P("M10 8.2v3l2 1.6M8.2 2.8h3.6"),
  dot: `<circle cx="10" cy="10" r="3.6" fill="currentColor"/>`,
  approx: P("M4.4 8.2c1.8-1.5 3.6-1.5 5.6 0s3.8 1.5 5.6 0M4.4 12.6c1.8-1.5 3.6-1.5 5.6 0s3.8 1.5 5.6 0"),
  question: C(10, 10, 7.4) + P("M7.8 7.9a2.3 2.3 0 1 1 3.2 2.1c-.7.3-1 .8-1 1.5v.3") + DOT(10, 14.1),
  shield: P("M10 2.4l6.2 2.5v4.6c0 4-2.7 6.9-6.2 8.1-3.5-1.2-6.2-4.1-6.2-8.1V4.9z"),
  shieldCheck: P("M10 2.4l6.2 2.5v4.6c0 4-2.7 6.9-6.2 8.1-3.5-1.2-6.2-4.1-6.2-8.1V4.9zM7.4 10l1.9 1.9 3.4-3.7"),
  lock: `<rect x="4" y="8.8" width="12" height="8.6" rx="1.6"/>` + P("M7 8.8V6.4a3 3 0 0 1 6 0v2.4M10 12.2v2"),
  key: C(6.6, 10, 3.2) + P("M9.8 10h7.6M14.6 10v2.6M17.2 10v2"),
  signal: P("M6.6 6.6a4.8 4.8 0 0 0 0 6.8M13.4 6.6a4.8 4.8 0 0 1 0 6.8M4.2 4.2a8.2 8.2 0 0 0 0 11.6M15.8 4.2a8.2 8.2 0 0 1 0 11.6") + DOT(10, 10),
  flame: P("M10 17.6c-3 0-5.2-2.1-5.2-5 0-3.2 2.6-4.6 3.2-8 2.1 1 3.2 2.7 3.4 4.3.9-.5 1.4-1.5 1.5-2.6 1.6 1.4 2.4 3.5 2.4 5.9 0 3.2-2.1 5.4-5.3 5.4z"),

  // people & things
  user: C(10, 6.8, 3.2) + P("M3.8 17.2c.8-3.3 3.1-5.2 6.2-5.2s5.4 1.9 6.2 5.2"),
  bot: `<rect x="4" y="6.6" width="12" height="9.4" rx="2.6"/>` + P("M10 6.6V4.2M2.4 10.2v2.6M17.6 10.2v2.6M8 13.4h4") + C(10, 3.4, 0.9) + DOT(7.8, 10.6) + DOT(12.2, 10.6),
  thought: P("M6.4 13.6a3.9 3.9 0 0 1-1.6-7.5 5.1 5.1 0 0 1 9.8 1.2 3.2 3.2 0 0 1-.6 6.3z") + C(5, 16.2, 1) + C(2.9, 18, 0.5),
  home: P("M3.2 9.2L10 3.4l6.8 5.8M5 7.8v8.8h3.6v-4.6h2.8v4.6H15V7.8"),
  briefcase: `<rect x="2.6" y="6" width="14.8" height="10.6" rx="1.6"/>` + P("M7 6V4.6a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1V6M2.6 10.6h14.8M8.6 10.6v1.6h2.8v-1.6"),
  folder: P("M2.6 5.6A1.4 1.4 0 0 1 4 4.2h3.6l1.6 2H16a1.4 1.4 0 0 1 1.4 1.4v7.8a1.4 1.4 0 0 1-1.4 1.4H4a1.4 1.4 0 0 1-1.4-1.4z"),
  folders: P("M5 7.4A1.3 1.3 0 0 1 6.3 6.1h3l1.4 1.7h5.6a1.3 1.3 0 0 1 1.3 1.3v6a1.3 1.3 0 0 1-1.3 1.3H6.3A1.3 1.3 0 0 1 5 15.1zM2.4 12.8V5a1.3 1.3 0 0 1 1.3-1.3h3l1.4 1.7"),
  file: P("M5 2.6h6.4l4 4v9.8a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V3.6a1 1 0 0 1 1-1zM11.4 2.6v4h4"),
  doc: P("M5 2.6h6.4l4 4v9.8a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V3.6a1 1 0 0 1 1-1zM11.4 2.6v4h4M7 10.2h6M7 13.2h4.4"),
  clipboard: P("M7 4H5.4A1.4 1.4 0 0 0 4 5.4v10.2A1.4 1.4 0 0 0 5.4 17h9.2a1.4 1.4 0 0 0 1.4-1.4V5.4A1.4 1.4 0 0 0 14.6 4H13M7.4 2.8h5.2v2.6H7.4zM7 9.6h6M7 12.8h4"),
  checklist: P("M9 5.4h7.6M9 10h7.6M9 14.6h7.6M3.4 5.4l1.1 1.1 2-2.1M3.4 10l1.1 1.1 2-2.1M3.4 14.6l1.1 1.1 2-2.1"),
  image: `<rect x="2.6" y="3.8" width="14.8" height="12.4" rx="1.6"/>` + C(7, 8, 1.4) + P("M2.6 14.2l4.6-4 3.4 3 2.6-2.2 4.2 3.6"),
  music: P("M7.8 14.8V4.6l8-1.6v10.2") + C(5.8, 14.8, 2) + C(13.8, 13.2, 2),
  film: `<rect x="3" y="3" width="14" height="14" rx="1.6"/>` + P("M6.8 3v14M13.2 3v14M3 7h3.8M3 10h3.8M3 13h3.8M13.2 7H17M13.2 10H17M13.2 13H17"),
  chart: P("M3.2 16.8h13.6M6 16.8V10.4M10 16.8V4.6M14 16.8v-4.4"),
  globe: C(10, 10, 7.4) + P("M2.6 10h14.8M10 2.6c2 2 3 4.5 3 7.4s-1 5.4-3 7.4c-2-2-3-4.5-3-7.4s1-5.4 3-7.4z"),
  plug: P("M7 2.6v3.8M13 2.6v3.8M4.8 6.4h10.4v2.8a5.2 5.2 0 0 1-10.4 0zM10 14.4v3"),
  mail: `<rect x="2.6" y="4.4" width="14.8" height="11.2" rx="1.6"/>` + P("M2.8 5.6l7.2 5.2 7.2-5.2"),
  rocket: P("M11.6 3.4c2.4-.9 4.4-.8 5.1-.7.1.7.2 2.7-.7 5.1l-5.2 5.2-3.8-3.8zM7.4 8.8l-3.2.4-1.6 2.2 3.6.5M11.2 12.6l-.4 3.2-2.2 1.6-.5-3.6M5.2 14.8c-.9.9-1.1 2.1-1.1 2.1s1.2-.2 2.1-1.1") + C(12.9, 7.1, 1.2),
  sparkle: P("M10 2.4l1.7 4.6 4.6 1.7-4.6 1.7L10 15l-1.7-4.6-4.6-1.7 4.6-1.7zM15.6 13.4l.7 1.8 1.8.7-1.8.7-.7 1.8-.7-1.8-1.8-.7 1.8-.7z"),
  bulb: P("M7.6 14.4h4.8M8.4 17h3.2M10 2.6a5 5 0 0 0-3 9c.6.5.6 1.4.6 2.8h4.8c0-1.4 0-2.3.6-2.8a5 5 0 0 0-3-9z"),
  compass: C(10, 10, 7.4) + P("M13 7l-1.7 4.3L7 13l1.7-4.3z"),
  coin: C(10, 10, 7.4) + P("M12.2 7.6c-.4-.7-1.2-1.1-2.2-1.1-1.2 0-2.1.7-2.1 1.7 0 2.3 4.4 1.3 4.4 3.7 0 1.1-.9 1.8-2.3 1.8-1 0-1.8-.4-2.2-1.2M10 4.9v1.6M10 13.5v1.6"),
  flask: P("M7.8 2.6h4.4M8.4 2.6v5L3.9 15.4A1.4 1.4 0 0 0 5.1 17.4h9.8a1.4 1.4 0 0 0 1.2-2L11.6 7.6v-5M5.8 12.6h8.4"),
  cart: P("M2.4 3.4h2.1l2 9.6h8.9l1.6-7H5.2") + C(8, 16, 1.2) + C(14.4, 16, 1.2),
  layers: P("M10 3L2.6 7 10 11l7.4-4zM2.6 10.4L10 14.4l7.4-4M2.6 13.8L10 17.8l7.4-4"),
};

/** emoji (written without the U+FE0F variation selector) -> icon name */
export const EMOJI = {
  "☀": "sun", "🌅": "sunrise", "🗂": "folders", "⛽": "gauge", "📚": "book", "📖": "book", "🧰": "toolbox",
  "🔗": "link", "📜": "list", "📋": "clipboard", "⚙": "gear", "📝": "doc", "✅": "checkCircle", "✓": "check",
  "✔": "check", "✕": "close", "✗": "closeCircle", "❌": "closeCircle", "↻": "refresh", "⬇": "download",
  "👍": "arrowRight", "🔍": "search", "🔎": "search", "🔀": "fork", "✂": "scissors", "⚡": "bolt", "💬": "chat",
  "🛠": "wrench", "💻": "terminal", "⤢": "expand", "📎": "paperclip", "✏": "pencil", "✍": "pencil",
  "🗑": "trash", "⏸": "pause", "▶": "play", "■": "stop", "📌": "pin", "📍": "pin", "★": "star", "⭐": "star",
  "⚠": "warning", "⛔": "stopSign", "⏳": "hourglass", "⏱": "timer", "●": "dot", "❔": "question", "❓": "question",
  "🛡": "shieldCheck", "🔐": "lock", "🔑": "key", "🛰": "signal", "🔥": "flame", "🙋": "user", "🧑‍💻": "user",
  "🧑": "user", "🤖": "bot", "🤔": "thought", "🧠": "thought", "🏠": "home", "💼": "briefcase", "📁": "folder",
  "📄": "file", "📃": "doc", "🧾": "checklist", "🖼": "image", "🎵": "music", "🎞": "film", "📊": "chart",
  "🌐": "globe", "🔌": "plug", "🧩": "plug", "📨": "mail", "🚀": "rocket", "🎉": "sparkle", "✨": "sparkle",
  "💡": "bulb", "🧭": "compass", "💰": "coin", "🧪": "flask", "🛒": "cart", "🍝": "layers", "👋": "sunrise",
};

const esc = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
const KEYS = Object.keys(EMOJI).sort((a, b) => b.length - a.length);
const RE = new RegExp(`(${KEYS.map(esc).join("|")})\\uFE0F?`, "gu");

/** Split a text into pieces: {t: "text"} and {i: "iconName"}. */
export function splitIcons(text) {
  const s = String(text ?? "");
  const out = [];
  let last = 0;
  for (const m of s.matchAll(RE)) {
    if (m.index > last) out.push({ t: s.slice(last, m.index) });
    out.push({ i: EMOJI[m[1]] });
    last = m.index + m[0].length;
  }
  if (last < s.length) out.push({ t: s.slice(last) });
  return out;
}

/** The text without its icon emoji (for a title, a placeholder or a comparison). */
export const stripIcons = (text) => String(text ?? "").replace(RE, "").replace(/\s{2,}/g, " ").trim();
