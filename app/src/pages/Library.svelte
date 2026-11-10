<script>
  import Icon from "../components/Icon.svelte";
  import { onMount } from "svelte";
  import { store } from "../lib/store.svelte.js";
  import { api, ago, INTENTS } from "../lib/api.js";
  import Intents from "../components/Intents.svelte";

  let q = $state("");
  let items = $state([]);
  let filter = $state("all");
  let timer;

  async function load() {
    items = await api.library(q.trim() || null);
  }
  onMount(load);
  function onInput() {
    clearTimeout(timer);
    timer = setTimeout(load, 250);
  }
  const shown = $derived(
    items.filter((i) => filter === "all" || (filter === "knowledge" ? i.knowledge : i.intents.includes(filter))),
  );
</script>

<div class="wrapx">
  <h1>คลังความรู้</h1>
  <p class="muted small">ทุกรายงานและบทความที่ AI เคยทำให้ ค้นหาได้ และไอเดียใหม่จะหยิบไปใช้ต่อเอง</p>
  <input class="search" bind:value={q} oninput={onInput} placeholder="ค้นหา เช่น บอร์ดเกม, rust, ราคา…" />
  <div class="row wrap">
    <button class="f" class:on={filter === "all"} onclick={() => (filter = "all")}>ทั้งหมด</button>
    <button class="f" class:on={filter === "knowledge"} onclick={() => (filter = "knowledge")}><Icon name="book" /> บทความความรู้</button>
    {#each Object.entries(INTENTS) as [k, m]}
      <button class="f" class:on={filter === k} onclick={() => (filter = k)}><Icon name={m.icon} /> {m.label}</button>
    {/each}
  </div>
  <div class="list">
    {#each shown as i (i.id)}
      <button class="item card" onclick={() => (store.openIdea = i.id)}>
        <div class="row"><b class="grow"><span class="idn">#{i.id}</span>{i.title}</b>{#if i.verdict}<span class="chip">{i.verdict}</span>{/if}<span class="faint small">{ago(i.updated)}</span></div>
        <div class="row wrap"><Intents intents={i.intents} />{#if i.knowledge}<span class="chip"><Icon name="book" /> ความรู้</span>{/if}</div>
        {#if i.snippet}<div class="snip small">…{i.snippet}…</div>{:else if i.summary}<div class="muted small">{i.summary}</div>{/if}
      </button>
    {:else}
      <div class="faint">{q ? "ไม่พบ" : "ยังไม่มีรายงาน — เมื่อ AI ทำงานเสร็จ รายงานจะมาอยู่ตรงนี้"}</div>
    {/each}
  </div>
</div>

<style>
  .wrapx { max-width: 900px; display: flex; flex-direction: column; gap: 12px; }
  .wrapx p { margin: 0; }
  .search { font-size: 16px; padding: 12px 16px; border-radius: 12px; }
  .f { border-radius: 999px; font-size: 13px; padding: 3px 12px; }
  .f.on { background: var(--card-2); border-color: var(--teal); }
  .list { display: flex; flex-direction: column; gap: 10px; margin-top: 6px; }
  .item { text-align: left; display: flex; flex-direction: column; gap: 6px; }
  .item:hover { border-color: var(--line-2); background: var(--card); }
  .snip { background: var(--bg-2); padding: 6px 10px; border-radius: 6px; color: var(--text-2); }
</style>
