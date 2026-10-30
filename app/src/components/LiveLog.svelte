<script>
  import { store, stageLabel, dur } from "../lib/store.svelte.js";
  import { api, isStep, clock } from "../lib/api.js";

  let box;
  const cur = $derived(store.state?.current);
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
      {#if cur.stage_total}<span class="chip">ขั้น {cur.stage_no}/{cur.stage_total}</span>{/if}
      <span class="grow"></span>
      <button class="small" onclick={() => api.stopCurrent()}>■ หยุด</button>
    </div>
    <div class="row wrap small">
      <span class="mood {mood}">
        {#if mood === "ok"}● ทำงานอยู่{:else if mood === "think"}◐ กำลังคิด (เงียบมา {dur(idle)}){:else}⚠ เงียบมานาน {dur(idle)} — อาจค้าง{/if}
      </span>
      <span class="faint">ทำมาแล้ว {dur(store.now - cur.started)}</span>
      {#if cur.provider}<span class="faint">· {cur.provider}{cur.model ? ` · ${cur.model}` : ""}</span>{/if}
    </div>
    <div class="row small"><div class="pb grow"><i style="width:{used}%"></i></div>
      <span class="faint">ใช้เวลาของขั้นนี้ {used}% · หยุดเองเมื่อครบ {clock(cur.deadline)}</span></div>
    {#if cur.last_line}<div class="last small"><span class="faint">ล่าสุด ({dur(idle)}ที่แล้ว):</span> {cur.last_line}</div>{/if}
    {#if mood === "warn"}
      <div class="hint small">เงียบเกิน 7 นาที: ถ้าเป็นงานคิดนานก็ปล่อยได้ ระบบจะหยุดเองเมื่อครบเวลา แล้วทำต่อรอบหน้า — หรือกด ■ หยุด เพื่อให้ลองใหม่เลย</div>
    {/if}
  {/if}
  <div class="log" bind:this={box}>
    {#each lines as l}<div class:tool={isStep(l)}>{l}</div>{:else}<div class="faint">ยังไม่มี log</div>{/each}
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
  .last { background: var(--bg-2); border-radius: 8px; padding: 6px 10px; }
  .hint { color: var(--amber); }
  .t { border: none; padding: 0; }
  .log { max-height: 220px; overflow: auto; font-size: 13px; line-height: 1.55; background: var(--bg-2); border-radius: 8px; padding: 10px 12px; white-space: pre-wrap; }
  .log .tool { color: var(--text-3); font-family: ui-monospace, Consolas, monospace; font-size: 12px; }
</style>
