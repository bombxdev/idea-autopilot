<script>
  // 16 languages, each with its flag and a line in that language. Choosing one saves it at once and switches the whole
  // interface. Thai and English are built in; any other language is translated once by your own AI (it asks first).
  import { onMount } from "svelte";
  import { api, listen } from "../lib/api.js";
  import { store, toast } from "../lib/store.svelte.js";
  import { LANGS, t, isRtl } from "../lib/i18n.js";
  import { setUiLang, refreshDict, status } from "../lib/uitrans.svelte.js";
  import Flag from "./Flag.svelte";

  let { value = "en", follow = "1" } = $props();
  let current = $state(value);
  let followInput = $state(follow);
  $effect(() => {
    current = value;
    followInput = follow;
  });

  const builtin = (c) => c === "th" || c === "en";
  const needsTranslation = $derived(!builtin(current) && ["missing", "partial"].includes(status.state));
  const translating = $derived(!builtin(current) && status.state === "translating");

  onMount(() => {
    let un = () => {};
    listen("ui-translate", () => refreshDict()).then((f) => (un = f));
    const timer = setInterval(() => status.state === "translating" && refreshDict(), 4000);
    return () => {
      un();
      clearInterval(timer);
    };
  });

  async function pick(code) {
    if (code === current) return;
    current = code;
    try {
      await api.setSetting("language", code);
      store.lang = code;
      if (typeof document !== "undefined") document.documentElement.dir = isRtl(code) ? "rtl" : "ltr";
      await setUiLang(code);
      // the confirmation is in the language just chosen
      toast(`${LANGS.find((l) => l.code === code)?.sample ?? ""} ✓`, "ok");
    } catch (e) {
      toast(String(e), "err");
    }
  }
  async function setFollow(v) {
    followInput = v;
    try {
      await api.setSetting("lang_follow", v);
      toast(t("lang_saved", current), "ok", 1500);
    } catch (e) {
      toast(String(e), "err");
    }
  }
  async function translate() {
    try {
      await api.uiTranslateStart(current);
      status.state = "translating";
      refreshDict();
    } catch (e) {
      toast(String(e), "err");
    }
  }
  const pctDone = $derived(status.total ? Math.min(100, Math.round((status.done / status.total) * 100)) : 0);
</script>

<p class="muted small">{t("lang_desc", current)}</p>
<div class="grid" role="radiogroup" aria-label={t("lang_title", current)}>
  {#each LANGS as l (l.code)}
    <button type="button" class="lang" class:on={current === l.code} role="radio" aria-checked={current === l.code} onclick={() => pick(l.code)} dir={l.rtl ? "rtl" : "ltr"}>
      <Flag code={l.code} size={20} />
      <span class="txt"><b>{l.name}</b><span class="sample">{l.sample}</span></span>
      {#if current === l.code}<span class="tick">✓</span>{/if}
    </button>
  {/each}
</div>

{#if needsTranslation}
  <div class="tr card small">
    <b>{LANGS.find((l) => l.code === current)?.name}</b>
    <div>This language has not been translated yet, so the screens show English for now. Your own AI can translate the whole interface once (about 20 short runs on its fastest model, a few minutes) and the result is kept on this machine. Only the interface text is sent — nothing you wrote.</div>
    {#if status.error}<div class="err">{status.error}</div>{/if}
    <div class="row"><button class="primary small" onclick={translate}>Translate with my AI</button>{#if status.done}<span class="faint">{status.done}/{status.total}</span>{/if}</div>
  </div>
{:else if translating}
  <div class="tr card small">
    <b>Translating the interface…</b>
    <div class="pb"><i style="width:{pctDone}%"></i></div>
    <div class="faint">{status.done}/{status.total} · the screens show English until it is done</div>
  </div>
{/if}

<label class="row small follow">
  <input type="checkbox" checked={followInput === "1"} onchange={(e) => setFollow(e.currentTarget.checked ? "1" : "0")} />
  {t("lang_follow", current)}
</label>

<style>
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(230px, 1fr)); gap: 8px; margin: 8px 0; }
  .lang { display: flex; align-items: center; gap: 10px; text-align: start; padding: 8px 12px; border-radius: 10px; border: 1px solid var(--line); background: var(--bg-2); }
  .lang.on { border-color: var(--teal); background: var(--teal-bg); }
  .txt { display: flex; flex-direction: column; min-width: 0; flex: 1; }
  .sample { font-size: 12px; color: var(--text-3); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tick { color: var(--teal); font-weight: 700; }
  .follow { gap: 8px; }
  .tr { display: flex; flex-direction: column; gap: 8px; border-left: 3px solid var(--amber); margin: 8px 0; }
  .pb { height: 6px; background: var(--bg-2); border-radius: 6px; overflow: hidden; }
  .pb i { display: block; height: 100%; background: var(--teal); transition: width 0.4s; }
  .err { color: var(--red); }
  .faint { color: var(--text-3); }
</style>
