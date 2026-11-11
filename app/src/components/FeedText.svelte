<script>
  // A feed line from the AI: ``` code blocks as a box with a copy button, `inline code` and **bold**; the rest as before
  // (emoji drawn as the app's icons). A block cut off at the end (a long message) still shows as code (ISSUE-073).
  import IconText from "./IconText.svelte";
  import { toast } from "../lib/store.svelte.js";

  let { text = "" } = $props();

  const blocks = $derived.by(() => {
    const out = [];
    const re = /\x60{3}([\w+-]*)[ \t]*\n?([\s\S]*?)(?:\x60{3}|$)/g;
    let last = 0;
    let m;
    while ((m = re.exec(text)) && m[0].length) {
      if (m.index > last) out.push({ t: text.slice(last, m.index) });
      out.push({ code: m[2].replace(/\s+$/, ""), lang: m[1] });
      last = re.lastIndex;
    }
    if (last < text.length) out.push({ t: text.slice(last) });
    return out;
  });
  // (written as \x60 / char 96: a literal backtick here confuses the i18n extractor)
  const bits = (t) => t.split(/(\x60[^\x60\n]+\x60|\*\*[^*\n]+\*\*)/).filter((p) => p !== "");
  const TICK = String.fromCharCode(96);

  async function copy(c) {
    try {
      await navigator.clipboard.writeText(c);
      toast("คัดลอกแล้ว");
    } catch {
      toast("คัดลอกไม่ได้", "err");
    }
  }
</script>

{#each blocks as b}{#if b.code !== undefined}<span class="cb"><span class="bar"><span class="lang">{b.lang}</span><button type="button" class="cp" onclick={() => copy(b.code)}>คัดลอก</button></span><pre>{b.code}</pre></span>{:else}{#each bits(b.t) as p}{#if p.length > 2 && p.startsWith(TICK) && p.endsWith(TICK)}<code>{p.slice(1, -1)}</code>{:else if p.length > 4 && p.startsWith("**") && p.endsWith("**")}<b>{p.slice(2, -2)}</b>{:else}<IconText text={p} />{/if}{/each}{/if}{/each}

<style>
  .cb { display: block; margin: 4px 0; border: 1px solid var(--line); border-radius: 8px; background: var(--card-2); overflow: hidden; }
  .bar { display: flex; align-items: center; justify-content: space-between; padding: 2px 6px 2px 10px; border-bottom: 1px solid var(--line); font-size: 11px; color: var(--text-3); }
  .cp { font-size: 11px; padding: 1px 8px; border: none; background: transparent; color: var(--teal); }
  pre { margin: 0; padding: 6px 10px; white-space: pre-wrap; word-break: break-all; font: 12px/1.45 ui-monospace, Consolas, monospace; color: var(--text); }
  code { font: 12px ui-monospace, Consolas, monospace; background: var(--card-2); border-radius: 4px; padding: 0 4px; }
</style>
