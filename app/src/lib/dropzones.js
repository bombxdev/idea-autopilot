// Files dragged in from Explorer. With Tauri's native drag & drop on, the page gets no HTML drop events;
// the window reports the dropped paths and where the pointer was. One listener serves every attach box:
// the box under the pointer wins (the highest priority one), and a box marked `fallback` takes drops that
// land anywhere else.
const zones = new Set();
let started = false;

async function start() {
  if (started || typeof window === "undefined" || !window.__TAURI_INTERNALS__) return;
  started = true;
  try {
    const { getCurrentWebview } = await import("@tauri-apps/api/webview");
    await getCurrentWebview().onDragDropEvent((e) => {
      const p = e.payload;
      if (p.type === "leave") return hover(null);
      const target = pick(p.position);
      if (p.type === "drop") {
        hover(null);
        if (target && p.paths?.length) target.onDrop(p.paths);
      } else {
        hover(target);
      }
    });
  } catch {
    started = false;
  }
}

function pick(pos) {
  const k = window.devicePixelRatio || 1;
  const x = pos ? pos.x / k : -1;
  const y = pos ? pos.y / k : -1;
  const live = [...zones].filter((z) => z.el()?.isConnected);
  const inside = live.filter((z) => {
    const r = z.el().getBoundingClientRect();
    return r.width > 0 && x >= r.left && x <= r.right && y >= r.top && y <= r.bottom;
  });
  const best = (list) => list.sort((a, b) => b.priority - a.priority)[0];
  return best(inside) ?? best(live.filter((z) => z.fallback));
}

function hover(target) {
  for (const z of zones) z.onHover?.(z === target);
}

/** Register a drop target. Returns the function that removes it. */
export function dropzone(zone) {
  zones.add(zone);
  start();
  return () => zones.delete(zone);
}
