<script>
  import Icon from "./Icon.svelte";
  import { store, refresh, toast } from "../lib/store.svelte.js";
  import { api } from "../lib/api.js";

  let { close } = $props();
  let q = $state("");
  let sel = $state(0);
  let el;
  $effect(() => el?.focus());

  const commands = [
    { icon: "pause", label: "พักบอท", run: () => api.setPaused(true) },
    { icon: "play", label: "ให้บอททำต่อ", run: () => api.setPaused(false) },
    { icon: "bolt", label: "Turbo 2 ชั่วโมง", run: () => api.setTurbo(2) },
    { icon: "bolt", label: "Turbo ทั้งคืน (8 ชม.)", run: () => api.setTurbo(8) },
    { icon: "refresh", label: "อ่านโควต้าใหม่", run: () => api.refreshUsage() },
    { icon: "sun", label: "ไปหน้าเช้านี้", run: () => (store.page = "morning") },
    { icon: "board", label: "ไปบอร์ด", run: () => (store.page = "board") },
    { icon: "gauge", label: "ไปหน้าโควต้า", run: () => (store.page = "quota") },
    { icon: "book", label: "ไปคลังความรู้", run: () => (store.page = "library") },
  ];

  const results = $derived.by(() => {
    const t = q.trim().toLowerCase();
    const ideas = store.ideas
      .filter((i) => i.status !== "deleted" && (!t || t.replace("#", "") === String(i.id) || (i.title + " " + i.summary).toLowerCase().includes(t)))
      .slice(0, 8)
      .map((i) => ({ label: `#${i.id} · ${i.title}`, sub: i.summary, run: () => (store.openIdea = i.id) }));
    const cmds = commands.filter((c) => !t || c.label.toLowerCase().includes(t));
    const add = t ? [{ icon: "plus", label: `จดเป็นไอเดียใหม่: “${q.trim()}”`, run: async () => { await api.addIdea(q.trim()); toast("จดแล้ว"); } }] : [];
    return [...ideas, ...cmds, ...add];
  });

  async function exec(r) {
    close();
    await r.run();
    refresh();
  }
  function key(e) {
    if (e.key === "ArrowDown") { sel = Math.min(sel + 1, results.length - 1); e.preventDefault(); }
    else if (e.key === "ArrowUp") { sel = Math.max(sel - 1, 0); e.preventDefault(); }
    else if (e.key === "Enter" && !e.isComposing && results[sel]) exec(results[sel]);
  }
</script>

<div class="scrim" onclick={close} role="presentation"></div>
<div class="pal">
  <input bind:this={el} bind:value={q} oninput={() => (sel = 0)} onkeydown={key} placeholder="ค้นหาไอเดีย หรือพิมพ์คำสั่ง…" />
  <div class="list">
    {#each results as r, i}
      <button class="it" class:on={i === sel} onmouseenter={() => (sel = i)} onclick={() => exec(r)}>
        <div class="ellipsis">{#if r.icon}<Icon name={r.icon} /> {/if}{r.label}</div>
        {#if r.sub}<div class="faint small ellipsis">{r.sub}</div>{/if}
      </button>
    {/each}
  </div>
</div>

<style>
  .scrim { position: fixed; inset: 0; background: rgba(0, 0, 0, .4); z-index: 30; }
  .pal { position: fixed; top: 14vh; left: 50%; transform: translateX(-50%); width: min(620px, 90vw); background: var(--card); border: 1px solid var(--line-2); border-radius: 14px; z-index: 31; overflow: hidden; animation: rise .15s; }
  input { width: 100%; border: none; border-bottom: 1px solid var(--line); border-radius: 0; padding: 16px 18px; font-size: 16px; background: transparent; }
  .list { max-height: 50vh; overflow: auto; padding: 6px; }
  .it { display: block; width: 100%; text-align: left; border: none; padding: 8px 12px; border-radius: 8px; }
  .it.on { background: var(--card-2); }
</style>
