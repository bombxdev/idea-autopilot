<script>
  import { store, stageLabel } from "../lib/store.svelte.js";
  import { api, isStep, clock } from "../lib/api.js";
  import { t, durIn } from "../lib/i18n.js";

  let box;
  const cur = $derived(store.state?.current);
  // the panel speaks the language of the idea being worked on (what the owner typed, or the main setting)
  const L = $derived(store.lang);
  const d = (sec) => durIn(sec, L);
  const lines = $derived(store.state?.live_log ?? []);
  $effect(() => {
    lines.length;
    if (box) box.scrollTop = box.scrollHeight;
  });
  // "alive" = the agent produced something lately; a long silence is normal while it thinks, but
  // past ~7 minutes it is worth a look
  const idle = $derived(cur ? Math.max(0, store.now - (cur.last_activity || cur.started)) : 0);
  const mood = $derived(idle < 120 ? "ok" : idle < 420 ? "think" : "warn");
  const used = $derived(cur ? Math.min(100, Math.round(((store.now - cur.started) / Math.max(1, cur.deadline - cur.started)) * 100)) : 0);
</script>

<div class="card live">
  {#if cur}
    <div class="row wrap">
      <span class="dot {mood}"></span>
      <button class="ghost t" onclick={() => (store.openIdea = cur.idea_id)}><b>{cur.title}</b></button>
      <span class="chip">{stageLabel(cur.stage)}</span>
      {#if cur.power}<span class="chip" title="โหมดเต็มที่: โมเดลแรงสุด effort สูงสุด">🚀 เต็มที่</span>{/if}
      {#if cur.stage_total}<span class="chip">{t("step", L, { a: cur.stage_no, b: cur.stage_total })}</span>{/if}
      <span class="grow"></span>
      <button class="small" onclick={() => api.stopCurrent()}>■ {t("stop", L)}</button>
    </div>
    <div class="now {mood}" aria-live="polite">
      <span class="faint small">{t("now", L)}</span>
      <div class="nowtext">{cur.last_line || "…"}</div>
    </div>
    <div class="row wrap small">
      <span class="mood {mood}">
        {#if mood === "ok"}● {t("working", L)}{:else if mood === "think"}◐ {t("thinking", L, { t: d(idle) })}{:else}⚠ {t("silent", L, { t: d(idle) })}{/if}
      </span>
      <span class="faint">{t("elapsed", L, { t: d(store.now - cur.started) })}</span>
      {#if cur.provider}<span class="faint">· {cur.provider}{cur.model ? ` · ${cur.model}` : ""}</span>{/if}
    </div>
    <div class="row small"><div class="pb grow"><i style="width:{used}%"></i></div>
      <span class="faint">{t("used", L, { p: used, c: clock(cur.deadline) })}</span></div>
    {#if mood === "warn"}
      <div class="hint small">{t("hint", L)}</div>
    {/if}
  {/if}
  <div class="log" bind:this={box}>
    {#each lines as l}<div class:tool={isStep(l)}>{l}</div>{:else}<div class="faint">{t("nolog", L)}</div>{/each}
  </div>
</div>

<style>
  .live { display: flex; flex-direction: column; gap: 10px; }
  .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--teal); animation: pulse 1.4s infinite; }
  .dot.think { background: var(--amber); }
  .dot.warn { background: var(--red); animation: none; }
  .mood.ok { color: var(--teal); }
  .mood.think { color: var(--amber); }
  .mood.warn { color: var(--red); }
  .pb { height: 6px; background: var(--bg-2); border-radius: 6px; overflow: hidden; }
  .pb i { display: block; height: 100%; background: var(--teal); transition: width 1s; }
  .now { background: var(--teal-bg); border-left: 3px solid var(--teal); border-radius: 8px; padding: 8px 12px; }
  .now.think { background: var(--amber-bg); border-color: var(--amber); }
  .now.warn { background: var(--red-bg); border-color: var(--red); }
  .nowtext { font-size: 15px; font-weight: 500; line-height: 1.5; overflow-wrap: anywhere; }
  .hint { color: var(--amber); }
  .t { border: none; padding: 0; }
  .log { max-height: 220px; overflow: auto; font-size: 13px; line-height: 1.55; background: var(--bg-2); border-radius: 8px; padding: 10px 12px; white-space: pre-wrap; }
  .log .tool { color: var(--text-3); font-family: ui-monospace, Consolas, monospace; font-size: 12px; }
</style>
