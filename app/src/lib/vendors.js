// Each AI program's colour and a simple glyph drawn by the app (ISSUE-173) — used by the dashboard and its 3D studio.
// Plain shapes of our own, not the companies' logos: a colour + a mark that is easy to tell apart.
export const VENDORS = {
  claude: { name: "Claude", color: "#d97757", glyph: "burst" },
  codex: { name: "Codex (GPT)", color: "#10a37f", glyph: "hex" },
  gemini: { name: "Gemini", color: "#4285f4", glyph: "star" },
  antigravity: { name: "Antigravity", color: "#7b61ff", glyph: "arc" },
  qwen: { name: "Qwen", color: "#615ced", glyph: "ring" },
  opencode: { name: "OpenCode", color: "#9aa4b2", glyph: "brackets" },
  mistral: { name: "Mistral", color: "#fa520f", glyph: "bars" },
};
const ALIAS = { anthropic: "claude", "claude code": "claude", openai: "codex", gpt: "codex", google: "gemini", agy: "antigravity", alibaba: "qwen" };
export const vendorOf = (kind) => {
  const k = (kind || "").toLowerCase();
  return VENDORS[k] ?? VENDORS[ALIAS[k]] ?? { name: kind || "AI", color: "#5dcaa5", glyph: "ring" };
};

/** The glyph as SVG inner markup on a 20×20 grid (white on the vendor colour). */
export function glyphSvg(g) {
  switch (g) {
    case "burst": return `<path d="M10 2.5v15M2.5 10h15M4.7 4.7l10.6 10.6M15.3 4.7L4.7 15.3" stroke="#fff" stroke-width="2" stroke-linecap="round"/>`;
    case "hex": return `<path d="M10 3l6 3.5v7L10 17l-6-3.5v-7z" fill="none" stroke="#fff" stroke-width="2" stroke-linejoin="round"/>`;
    case "star": return `<path d="M10 2.5c.8 4 3.5 6.7 7.5 7.5-4 .8-6.7 3.5-7.5 7.5-.8-4-3.5-6.7-7.5-7.5 4-.8 6.7-3.5 7.5-7.5z" fill="#fff"/>`;
    case "arc": return `<path d="M4 15a6 6 0 0 1 12 0" fill="none" stroke="#fff" stroke-width="2.2" stroke-linecap="round"/><circle cx="10" cy="6" r="2.2" fill="#fff"/>`;
    case "brackets": return `<path d="M7.5 5L4 10l3.5 5M12.5 5L16 10l-3.5 5" fill="none" stroke="#fff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>`;
    case "bars": return `<path d="M5 15V6M10 15V4M15 15V8" stroke="#fff" stroke-width="2.4" stroke-linecap="round"/>`;
    default: return `<circle cx="10" cy="10" r="5" fill="none" stroke="#fff" stroke-width="2.2"/>`;
  }
}
