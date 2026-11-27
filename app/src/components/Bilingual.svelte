<script>
  // English content with its translation beside it (ISSUE-174): the original on the left, the owner's language on
  // the right. Shown only when the mode is on, the text is English and some AI still has quota; a translation is made
  // when asked (a click) and kept, so opening a page never spends quota. Otherwise just the original.
  import { onMount } from "svelte";
  import Markdown from "./Markdown.svelte";
  import Icon from "./Icon.svelte";
  import { api } from "../lib/api.js";
  import { ct, loadCt, isForeign } from "../lib/ct.svelte.js";
  import { toast } from "../lib/store.svelte.js";
  import { LANGS } from "../lib/i18n.js";

  let { text = "", md = false, small = true } = $props();
  let tr = $state("");
  let busy = $state(false);
  let by = $state("");
  // the language by its own name (ไทย, 日本語…)
  const langName = $derived(LANGS.find((l) => l.code === ct.lang)?.name ?? ct.lang_name);

  onMount(() => loadCt());
  const show = $derived(ct.on && !!ct.now && isForeign(text));
  // the translation belongs to one text: a new text clears it (a status refresh must not)
  let trFor = $state("");
  $effect(() => {
    const t = text;
    if (!show || !t || t === trFor) return;
    tr = "";
    trFor = t;
    api.ctCached(t).then((x) => { if (x && t === text) tr = x; }).catch(() => {});
  });
  async function go() {
    busy = true;
    try {
      const r = await api.ctTranslate(text);
      tr = r.text;
      by = r.account;
      loadCt(true);
    } catch (e) {
      toast(String(e), "err");
      loadCt(true);
    }
    busy = false;
  }
</script>

{#if show}
  <div class="bi" class:small>
    <div class="col">
      <div class="lbl">ต้นฉบับ</div>
      {#if md}<Markdown {text} />{:else}<div class="pl">{text}</div>{/if}
    </div>
    <div class="col tr">
      <div class="lbl">คำแปล · {langName}{#if by} <span class="faint">({by})</span>{/if}</div>
      {#if tr}
        {#if md}<Markdown text={tr} />{:else}<div class="pl">{tr}</div>{/if}
      {:else}
        <button type="button" class="ghost small go" onclick={go} disabled={busy} title={ct.fallback === false ? "ใช้โควต้าของ " + ct.now_label + " เท่านั้น (ไม่สลับบัญชี)" : "ใช้โควต้าของ " + ct.now_label + " — ถ้าหมด แอปสลับไปใช้ AI ตัวอื่นที่ยังเหลือให้เอง"}>
          <Icon name="globe" /> {busy ? "กำลังแปล…" : "แปลเป็น " + langName + " (ใช้ " + ct.now_label + ")"}</button>
      {/if}
    </div>
  </div>
{:else if md}<Markdown {text} />{:else}<div class="pl" class:small>{text}</div>{/if}

<style>
  .bi { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; }
  .col { min-width: 0; }
  .tr { border-left: 2px solid var(--teal); padding-left: 12px; }
  .lbl { font-size: 11px; color: var(--text-3); margin-bottom: 2px; text-transform: none; }
  .pl { white-space: pre-wrap; overflow-wrap: anywhere; color: var(--text-2); }
  .small { font-size: 13px; }
  .go { padding: 2px 8px; }
  .faint { color: var(--text-3); }
  @media (max-width: 760px) { .bi { grid-template-columns: 1fr; } .tr { border-left: none; border-top: 2px solid var(--teal); padding: 6px 0 0; } }
</style>
