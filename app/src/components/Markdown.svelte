<script>
  import { marked } from "marked";
  import DOMPurify from "dompurify";
  import { openUrl } from "@tauri-apps/plugin-opener";

  let { text = "" } = $props();
  // reports contain web content written by the bot: always sanitize before rendering
  const html = $derived(DOMPurify.sanitize(marked.parse(text ?? "", { gfm: true }), { FORBID_TAGS: ["style", "form", "iframe"] }));

  function click(e) {
    const a = e.target.closest("a");
    if (a?.href?.startsWith("http")) {
      e.preventDefault();
      openUrl(a.href);
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="md" onclick={click}>{@html html}</div>
