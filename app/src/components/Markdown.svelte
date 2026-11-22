<script>
  import { marked } from "marked";
  import DOMPurify from "dompurify";
  import { openUrl } from "@tauri-apps/plugin-opener";

  // onlink(href): a link to a file (relative like `experiment/x.json`, or an absolute path) — the drawer opens the
  // real file from the idea's folder. Without it such links do nothing rather than send the app's own window to
  // http://tauri.localhost/… (ISSUE-148).
  let { text = "", onlink = null } = $props();
  // reports contain web content written by the bot: always sanitize before rendering
  const html = $derived(DOMPurify.sanitize(marked.parse(text ?? "", { gfm: true }), { FORBID_TAGS: ["style", "form", "iframe"] }));
  let box;

  function click(e) {
    const a = e.target.closest("a");
    if (!a) return;
    const raw = (a.getAttribute("href") ?? "").trim();
    if (!raw) return;
    e.preventDefault();
    if (raw.startsWith("#")) {
      const id = decodeURIComponent(raw.slice(1));
      box?.querySelector(`[id="${CSS.escape(id)}"]`)?.scrollIntoView({ behavior: "smooth", block: "start" });
      return;
    }
    if (/^(https?:|mailto:)/i.test(raw) && !/^https?:\/\/tauri\.localhost/i.test(raw)) {
      openUrl(raw);
      return;
    }
    onlink?.(raw);
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="md" bind:this={box} onclick={click}>{@html html}</div>
