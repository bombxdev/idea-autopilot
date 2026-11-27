// A persona's face as a self-contained SVG (ISSUE-172): inline colours only, so the same drawing works on a page
// and as a picture (the 3D studio turns it into a texture).
const SKIN = ["#f6d7b8", "#e8b98f", "#c98e5e", "#9a6440", "#6b4226"];
const esc = (c) => String(c || "").replace(/[^#a-zA-Z0-9(),. %-]/g, "");

export function avatarSvg(look = {}, { size = 64, bg = "#1b3a33", ring = "" } = {}) {
  const L = { skin: 1, hair: "short", hair_color: "#2b2118", glasses: "none", shirt: "#4f7cff", extra: "none", face: "smile", ...(look || {}) };
  const skin = SKIN[Math.max(0, Math.min(4, Number(L.skin) || 0))];
  const hc = esc(L.hair_color) || "#2b2118";
  const shirt = esc(L.shirt) || "#4f7cff";
  const ink = `fill="#2a1d16"`;
  const line = `fill="none" stroke="#3b2a20" stroke-width="1.6" stroke-linecap="round"`;
  const p = [];
  p.push(`<circle cx="32" cy="32" r="31" fill="${esc(bg)}"${ring ? ` stroke="${esc(ring)}" stroke-width="2"` : ""}/>`);
  p.push(`<path d="M10 64c1-12 10-18 22-18s21 6 22 18z" fill="${shirt}"/><rect x="27" y="40" width="10" height="8" rx="3" fill="${skin}"/>`);
  if (L.hair === "long") p.push(`<path d="M15 30c0-12 7-19 17-19s17 7 17 19v16c-4 2-8 2-10 0V30H25v16c-2 2-6 2-10 0z" fill="${hc}"/>`);
  if (L.hair === "bun") p.push(`<circle cx="32" cy="10" r="6" fill="${hc}"/>`);
  p.push(`<ellipse cx="32" cy="30" rx="13" ry="14" fill="${skin}"/><circle cx="19" cy="31" r="2.6" fill="${skin}"/><circle cx="45" cy="31" r="2.6" fill="${skin}"/>`);
  const top = {
    short: `<path d="M19 27c0-9 6-13 13-13s13 4 13 13c-3-4-7-6-13-6s-10 2-13 6z" fill="${hc}"/>`,
    fluffy: `<path d="M16 28c-3-6 1-12 5-12 0-5 6-7 9-4 3-4 10-3 11 2 5-1 8 4 6 8 3 3 1 8-2 8-2-5-7-9-13-9s-12 3-14 9c-3 0-4-1-2-2z" fill="${hc}"/>`,
    long: `<path d="M19 28c0-9 6-14 13-14s13 5 13 14c-4-5-8-7-13-7s-9 2-13 7z" fill="${hc}"/>`,
    bun: `<path d="M19 28c0-9 6-14 13-14s13 5 13 14c-4-5-8-7-13-7s-9 2-13 7z" fill="${hc}"/>`,
    spiky: `<path d="M18 27l2-9 4 4 3-8 4 6 4-7 3 8 4-4 2 9c-4-4-8-6-13-6s-9 2-13 7z" fill="${hc}"/>`,
    curly: `<g fill="${hc}"><circle cx="21" cy="22" r="4.5"/><circle cx="27" cy="17" r="5"/><circle cx="34" cy="16" r="5"/><circle cx="41" cy="19" r="4.6"/><circle cx="44" cy="25" r="3.6"/><circle cx="19" cy="28" r="3.4"/></g>`,
    cap: `<path d="M18 26c0-8 6-12 14-12s14 4 14 12z" fill="${hc}"/><path d="M30 25h22c0 2-2 3-4 3H30z" fill="${hc}"/>`,
  }[L.hair];
  if (top) p.push(top);
  if (L.face === "wink") p.push(`<circle cx="26.5" cy="30" r="1.7" ${ink}/><path d="M35 30h4" ${line}/>`);
  else if (L.face === "calm") p.push(`<path d="M24.5 30.5h4M35.5 30.5h4" ${line}/>`);
  else p.push(`<circle cx="26.5" cy="30" r="1.7" ${ink}/><circle cx="37.5" cy="30" r="1.7" ${ink}/>`);
  if (L.face === "grin") p.push(`<path d="M26 36c2 4 10 4 12 0z" fill="#fff" stroke="#3b2a20" stroke-width="1.2" stroke-linejoin="round"/>`);
  else if (L.face === "calm") p.push(`<path d="M28.5 37h7" ${line}/>`);
  else p.push(`<path d="M27 36c2 2.6 8 2.6 10 0" ${line}/>`);
  const gl = `fill="rgba(255,255,255,0.12)" stroke="#222" stroke-width="1.4"`;
  if (L.glasses === "round") p.push(`<g ${gl}><circle cx="26.5" cy="30" r="4.2"/><circle cx="37.5" cy="30" r="4.2"/><path d="M30.7 30h2.6"/></g>`);
  else if (L.glasses === "square") p.push(`<g ${gl}><rect x="21.8" y="26.4" width="9.4" height="7.4" rx="1.6"/><rect x="32.8" y="26.4" width="9.4" height="7.4" rx="1.6"/><path d="M31.2 30h1.6"/></g>`);
  else if (L.glasses === "sun") p.push(`<g><rect x="21.4" y="26.6" width="10" height="6.6" rx="2.4" fill="#1d1d1d"/><rect x="32.6" y="26.6" width="10" height="6.6" rx="2.4" fill="#1d1d1d"/><path d="M31.4 29.4h1.2" stroke="#1d1d1d" stroke-width="1.4"/></g>`);
  if (L.extra === "beard") p.push(`<path d="M20 33c1 8 6 12 12 12s11-4 12-12c-2 3-5 5-7 5-1-2-3-3-5-3s-4 1-5 3c-2 0-5-2-7-5z" fill="${hc}"/>`);
  else if (L.extra === "headphones") p.push(`<path d="M17 31c0-10 7-17 15-17s15 7 15 17" fill="none" stroke="#333" stroke-width="2.4"/><rect x="14" y="28" width="5" height="9" rx="2.4" fill="#333"/><rect x="45" y="28" width="5" height="9" rx="2.4" fill="#333"/>`);
  else if (L.extra === "earrings") p.push(`<circle cx="19" cy="35" r="1.4" fill="#f2b04a"/><circle cx="45" cy="35" r="1.4" fill="#f2b04a"/>`);
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="${size}" height="${size}">${p.join("")}</svg>`;
}
