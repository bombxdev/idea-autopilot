<script>
  // One model chooser used everywhere (account settings, AI of an idea, AI of a chat): a box you can type in to search
  // the list, or to enter a name that is not listed. When some of the models are free (OpenRouter's ":free" ones and
  // the like) a tick shows only those.
  let { value = "", models = [], onchange = () => {}, auto = "", placeholder = "ชื่อโมเดล", disabled = false, width = 260 } = $props();
  let open = $state(false);
  let q = $state("");
  let typing = $state(false);
  let freeOnly = $state(false);
  let active = $state(0);
  let root;

  const hasFree = $derived(models.some((m) => m.free));
  const shown = $derived.by(() => {
    const needle = typing ? q.trim().toLowerCase() : "";
    return models.filter((m) => (!freeOnly || m.free) && (!needle || `${m.id} ${m.label ?? ""}`.toLowerCase().includes(needle)));
  });
  const text = $derived(typing ? q : value === "auto" ? "" : value);
  const known = (v) => models.some((m) => m.id === v);

  function pick(v) {
    open = false;
    typing = false;
    if (v !== value) onchange(v);
  }
  function commit() {
    if (!typing) return;
    const v = q.trim();
    typing = false;
    open = false;
    if (v !== (value === "auto" ? "" : value)) onchange(v);
  }
  function key(e) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      open = true;
      active = Math.min(active + 1, shown.length - 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      active = Math.max(active - 1, 0);
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (typing && shown[active] && q.trim() && !known(q.trim())) pick(shown[active].id);
      else commit();
    } else if (e.key === "Escape") {
      typing = false;
      open = false;
    }
  }
  function outside(e) {
    if (open && root && !root.contains(e.target)) commit(), (open = false);
  }
</script>

<svelte:window onmousedown={outside} />

<div class="mp" bind:this={root} style="width:{width}px">
  <input
    value={text}
    {disabled}
    {placeholder}
    autocomplete="off"
    spellcheck="false"
    onfocus={(e) => { open = true; active = 0; e.currentTarget.select(); }}
    oninput={(e) => { q = e.currentTarget.value; typing = true; open = true; active = 0; }}
    onkeydown={key}
  />
  <button type="button" class="arrow" tabindex="-1" {disabled} aria-label="รายการ" onclick={() => (open = !open)}>▾</button>
  {#if open}
    <div class="list" role="listbox">
      {#if hasFree}
        <label class="free"><input type="checkbox" bind:checked={freeOnly} /> แสดงเฉพาะโมเดลฟรี</label>
      {/if}
      {#if auto}
        <button type="button" class="opt" class:on={!value || value === "auto"} onclick={() => pick("")}>{auto}</button>
      {/if}
      {#each shown as m, i (m.id)}
        <button type="button" class="opt" class:on={m.id === value} class:act={i === active} onclick={() => pick(m.id)}>
          <span class="nm">{m.label ?? m.id}</span>{#if m.free}<span class="badge">ฟรี</span>{/if}
        </button>
      {/each}
      {#if typing && q.trim() && !known(q.trim())}
        <button type="button" class="opt use" onclick={() => pick(q.trim())}>ใช้ชื่อ “{q.trim()}”</button>
      {:else if !shown.length}
        <div class="none">{freeOnly ? "ไม่พบโมเดลฟรีในรายการ" : "ไม่พบรายชื่อ — พิมพ์ชื่อโมเดลได้เลย"}</div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .mp { position: relative; display: inline-flex; max-width: 100%; }
  .mp > input { width: 100%; padding-right: 26px; }
  .arrow { position: absolute; right: 2px; top: 50%; transform: translateY(-50%); border: none; background: transparent; padding: 2px 6px; color: var(--text-3); }
  .list { position: absolute; z-index: 60; top: calc(100% + 4px); left: 0; min-width: 100%; max-height: 260px; overflow: auto; background: var(--card-2, var(--bg-2)); border: 1px solid var(--line-2, var(--line)); border-radius: 10px; padding: 4px; box-shadow: 0 10px 30px rgba(0, 0, 0, 0.4); display: flex; flex-direction: column; }
  .opt { display: flex; justify-content: space-between; align-items: center; gap: 8px; text-align: left; border: none; background: transparent; padding: 6px 10px; border-radius: 6px; color: var(--text); font-size: 13px; }
  .opt:hover, .opt.act { background: var(--bg); }
  .opt.on { color: var(--teal); }
  .opt.use { color: var(--teal); }
  .nm { overflow-wrap: anywhere; }
  .badge { font-size: 11px; padding: 0 6px; border-radius: 999px; background: var(--teal-bg); color: var(--teal); flex: none; }
  .free { display: flex; gap: 6px; align-items: center; padding: 6px 10px; font-size: 12px; color: var(--text-2); border-bottom: 1px solid var(--line); margin-bottom: 4px; }
  .none { padding: 8px 10px; color: var(--text-3); font-size: 12px; }
</style>
