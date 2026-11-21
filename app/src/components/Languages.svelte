<script>
  // the languages the idea's code is written in, by size — like the "Languages" bar of a GitHub repository (ISSUE-108)
  import { api } from "../lib/api.js";

  let { id, stamp = 0 } = $props();
  let list = $state([]);
  $effect(() => {
    stamp; // read again after the AI changed something
    api.ideaLanguages(id).then((r) => (list = r ?? [])).catch(() => (list = []));
  });
  const SHOW = 6;
  const rows = $derived.by(() => {
    if (list.length <= SHOW + 1) return list;
    const rest = list.slice(SHOW).reduce((a, x) => a + x.pct, 0);
    return [...list.slice(0, SHOW), { name: "Other", color: "", pct: Math.round(rest * 10) / 10 }];
  });
</script>

{#if rows.length}
  <div class="langs">
    <div class="bar" role="img" aria-label={rows.map((r) => `${r.name} ${r.pct}%`).join(", ")}>
      {#each rows as r (r.name)}<i style="width:{Math.max(r.pct, 0.4)}%;{r.color ? `background:${r.color}` : ''}" class:other={!r.color} title="{r.name} {r.pct}%"></i>{/each}
    </div>
    <div class="keys">
      {#each rows as r (r.name)}
        <span class="key"><i style={r.color ? `background:${r.color}` : ""} class:other={!r.color}></i><b>{r.name}</b> <span class="pct">{r.pct}%</span></span>
      {/each}
    </div>
  </div>
{/if}

<style>
  .langs { display: flex; flex-direction: column; gap: 6px; }
  .bar { display: flex; height: 8px; border-radius: 999px; overflow: hidden; gap: 2px; background: var(--card-2); }
  .bar i { display: block; height: 100%; flex: none; }
  .keys { display: flex; flex-wrap: wrap; gap: 4px 14px; font-size: 12px; }
  .key { display: inline-flex; align-items: center; gap: 6px; white-space: nowrap; }
  .key i { width: 9px; height: 9px; border-radius: 50%; flex: none; }
  .key b { font-weight: 600; color: var(--text); }
  .pct { color: var(--text-2); font-variant-numeric: tabular-nums; }
  .other { background: var(--text-3); }
</style>
