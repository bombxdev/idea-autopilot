<script>
  // Who works on an idea: the AI alone, or one of the owner's named teams (teams.rs). A team that fits the idea's
  // words is suggested as a chip — never picked by itself.
  import { onMount } from "svelte";
  import Icon from "./Icon.svelte";
  import { api } from "../lib/api.js";

  let { value = $bindable(""), text = "", onchange = null } = $props();
  let teams = $state([]);
  let suggested = $state("");

  onMount(async () => {
    try { teams = (await api.teams()).teams; } catch {}
  });
  // suggest from the idea's words while it is typed (rule-based, no tokens)
  let timer;
  $effect(() => {
    const t = text;
    clearTimeout(timer);
    if (!t || t.trim().length < 6 || !teams.length) { suggested = ""; return; }
    timer = setTimeout(async () => {
      try { suggested = (await api.teamSuggest(t)) ?? ""; } catch { suggested = ""; }
    }, 500);
  });
  function pick(v) {
    value = v;
    onchange?.(v);
  }
  const sug = $derived(teams.find((t) => t.slug === suggested && t.slug !== value));
  const cur = $derived(teams.find((t) => t.slug === value));
</script>

<span class="tp small">
  <span class="faint"><Icon name="user" /> ใครทำ:</span>
  <select value={value} onchange={(e) => pick(e.currentTarget.value)} title="AI ตัวเดียวทำทั้งหมด หรือให้ทีมแบ่งงานกันทำ (หัวหน้าทีม = AI ของไอเดียนี้)">
    <option value="">AI ตัวเดียว (อัตโนมัติ)</option>
    {#each teams as t (t.slug)}<option value={t.slug}>{t.emoji || "👥"} {t.name} ({t.member_roles.length} คน)</option>{/each}
  </select>
  {#if sug}<button type="button" class="chip sug" onclick={() => pick(sug.slug)} title="คำในไอเดียตรงกับงานที่ทีมนี้ถนัด">แนะนำ: {sug.emoji} {sug.name}</button>{/if}
  {#if cur}<span class="faint" title={cur.member_roles.map((m) => `${m.name}${m.title ? " · " + m.title : ""}`).join("\n")}>{cur.member_roles.map((m) => m.emoji || m.name.slice(0, 1)).join(" ")}</span>{/if}
  {#if !teams.length}<a href="?page=roles" class="faint" onclick={(e) => { e.preventDefault(); location.search = "?page=roles"; }}>สร้างทีม</a>{/if}
</span>

<style>
  .tp { display: inline-flex; align-items: center; gap: 6px; flex-wrap: wrap; }
  select { max-width: 240px; }
  .sug { border-color: var(--teal); color: var(--teal); cursor: pointer; }
</style>
