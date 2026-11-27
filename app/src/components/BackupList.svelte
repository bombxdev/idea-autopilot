<script>
  // Backup accounts, in order (ISSUE-176): when the idea's account hits its limit the work carries on with the first
  // backup that can work, then the next. At most as many as the other AIs this machine has.
  import { onMount } from "svelte";
  import Icon from "./Icon.svelte";
  import { api } from "../lib/api.js";

  let { value = [], exclude = "", onchange = null, label = "AI สำรองเมื่อตัวหลักชนลิมิต" } = $props();
  let all = $state([]);
  let adding = $state(false);
  onMount(() => api.accounts().then((r) => (all = (r?.accounts ?? []).filter((a) => a.enabled && a.installed))).catch(() => {}));

  const name = (id) => { const a = all.find((x) => x.id === id); return a ? `${a.label || a.id} · ${a.vendor_label || a.kind_label}` : id; };
  const list = $derived((value ?? []).filter((id) => id !== exclude));
  const free = $derived(all.filter((a) => a.id !== exclude && !list.includes(a.id)));
  function put(l) { onchange?.(l); }
  function add(id) { if (id) put([...list, id]); adding = false; }
  function move(i, d) { const l = [...list]; const j = i + d; if (j < 0 || j >= l.length) return; [l[i], l[j]] = [l[j], l[i]]; put(l); }
  function drop(i) { put(list.filter((_, k) => k !== i)); }
</script>

<div class="bl small">
  <span class="faint">{label}:</span>
  {#each list as id, i (id)}
    <span class="chip b"><b>{i + 1}</b> {name(id)}
      <button type="button" class="ghost mini" onclick={() => move(i, -1)} disabled={i === 0} title="เลื่อนขึ้น"><Icon name="chevronUp" /></button>
      <button type="button" class="ghost mini" onclick={() => move(i, 1)} disabled={i === list.length - 1} title="เลื่อนลง"><Icon name="chevronDown" /></button>
      <button type="button" class="ghost mini" onclick={() => drop(i)} title="เอาออก"><Icon name="close" /></button></span>
  {:else}<span class="faint">ยังไม่มี</span>{/each}
  {#if free.length}
    {#if adding}
      <select onchange={(e) => add(e.currentTarget.value)}><option value="">เลือก AI สำรอง…</option>{#each free as a (a.id)}<option value={a.id}>{a.label || a.id} · {a.vendor_label || a.kind_label}</option>{/each}</select>
      <button type="button" class="ghost mini" onclick={() => (adding = false)}><Icon name="close" /></button>
    {:else}
      <button type="button" class="ghost small add" onclick={() => (adding = true)}><Icon name="plus" /> เพิ่มโมเดลสำรอง</button>
    {/if}
  {/if}
</div>

<style>
  .bl { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
  .b { display: inline-flex; align-items: center; gap: 4px; padding: 2px 4px 2px 8px; }
  .b b { color: var(--teal); }
  .mini { padding: 0 3px; font-size: 12px; }
  .add { padding: 2px 8px; color: var(--teal); }
  select { padding: 3px 8px; font-size: 12.5px; }
  .faint { color: var(--text-3); }
</style>
