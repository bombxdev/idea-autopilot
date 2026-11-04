<script>
  // Choose which accounts the quota page shows (one, several, all) and whether each has its own plan and chart or they
  // are shown together.
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";
  import { qv, loadQv, isShown, toggleAccount, onlyAccount, setMode } from "../lib/quotaview.svelte.js";

  let { accounts = [] } = $props(); // [{ id, label }]
  onMount(loadQv);
  const ids = $derived(accounts.map((a) => a.id));
</script>

{#if accounts.length > 1}
  <div class="ctl row wrap small">
    <span class="faint">แสดงโควต้าของ</span>
    {#each accounts as a (a.id)}
      <label class="pick" class:off={!isShown(a.id)}>
        <input type="checkbox" checked={isShown(a.id)} onchange={() => toggleAccount(a.id, ids)} />
        {a.label || a.id}
        <button type="button" class="only" onclick={() => onlyAccount(a.id)} title="ดูเฉพาะบัญชีนี้">เฉพาะ</button>
      </label>
    {/each}
    <span class="grow"></span>
    <div class="seg" role="group" aria-label="รูปแบบ">
      <button type="button" class:on={qv.mode === "split"} onclick={() => setMode("split")}>แยกกราฟ</button>
      <button type="button" class:on={qv.mode === "combined"} onclick={() => setMode("combined")}>รวมกัน</button>
    </div>
  </div>
{/if}

<style>
  .ctl { gap: 10px; align-items: center; margin-bottom: 8px; }
  .pick { display: inline-flex; align-items: center; gap: 6px; border: 1px solid var(--line); border-radius: 999px; padding: 3px 10px; background: var(--bg-2); }
  .pick.off { opacity: 0.5; }
  .only { border: none; background: transparent; color: var(--text-3); font-size: 11px; padding: 0 2px; }
  .only:hover { color: var(--teal); background: transparent; }
  .seg { display: inline-flex; border: 1px solid var(--line); border-radius: 8px; overflow: hidden; }
  .seg button { border: none; border-radius: 0; padding: 4px 12px; background: transparent; }
  .seg button.on { background: var(--teal-bg); color: var(--teal); }
  .faint { color: var(--text-3); }
</style>
