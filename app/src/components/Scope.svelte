<script>
  import Icon from "./Icon.svelte";
  // Which set an idea belongs to: personal or work (company). Work ideas are exported and stored apart (portable brain).
  let { value = $bindable("personal"), onchange = null, compact = false } = $props();
  function pick(v) {
    if (v === value) return;
    value = v;
    onchange?.(v);
  }
</script>

<div class="scope" class:compact role="group" aria-label="ชุดของไอเดีย">
  <button type="button" class:on={value !== "work"} onclick={() => pick("personal")} title="เรื่องส่วนตัว — เก็บในชุดส่วนตัว"><Icon name="home" />{#if !compact} ส่วนตัว{/if}</button>
  <button type="button" class:on={value === "work"} onclick={() => pick("work")} title="เรื่องงานบริษัท — เก็บแยกในชุดงาน"><Icon name="briefcase" />{#if !compact} งาน{/if}</button>
</div>

<style>
  .scope { display: inline-flex; border: 1px solid var(--line); border-radius: 8px; overflow: hidden; flex: none; }
  .scope button { border: none; border-radius: 0; background: transparent; padding: 4px 9px; font-size: 13px; color: var(--muted, inherit); cursor: pointer; }
  .scope button + button { border-left: 1px solid var(--line); }
  .scope button.on { background: var(--teal); color: #fff; }
  .compact button { padding: 4px 7px; }
</style>
