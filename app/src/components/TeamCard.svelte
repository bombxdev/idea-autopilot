<script>
  // The idea's team at work: who got which part, on which AI/model, and how it went (team_parts in db.rs).
  import { onMount } from "svelte";
  import Icon from "./Icon.svelte";
  import RoleIcon from "./RoleIcon.svelte";
  import { api } from "../lib/api.js";
  import { store } from "../lib/store.svelte.js";

  let { id, team } = $props();
  let teams = $state([]);
  let parts = $state([]);

  async function load() {
    try {
      teams = (await api.teams()).teams;
      parts = await api.ideaTeamParts(id);
    } catch {}
  }
  onMount(() => {
    load();
    const t = setInterval(load, 8000);
    return () => clearInterval(t);
  });
  const t = $derived(teams.find((x) => x.slug === team));
  const member = (slug) => t?.member_roles.find((m) => m.slug === slug);
  const BUILTIN = { helper: "ผู้ช่วยทั่วไป", "helper-fast": "ผู้ช่วยงานเร็ว", reviewer: "ผู้ตรวจงาน" };
  const who = (p) => { const m = member(p.role); return m ? m.name : BUILTIN[p.role] ?? (p.role || "ผู้ช่วย"); };
  const STATUS = { running: ["กำลังทำ", "teal"], ok: ["เสร็จ", "teal"], error: ["ไม่สำเร็จ", "red"], timeout: ["หมดเวลา", "amber"], killed: ["ถูกหยุด", "amber"] };
  const clock = (s) => new Date(s * 1000).toLocaleString(store.lang === "th" ? "th-TH" : undefined, { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
</script>

{#if t}
  <div class="tc">
    <div class="row wrap small">
      <b><RoleIcon emoji={t.emoji || "i:users"} words={t.name} /> {t.name}</b>
      <span class="faint">หัวหน้า = AI ของไอเดียนี้ · แบ่งพร้อมกันได้ {t.helpers} งาน</span>
    </div>
    <div class="row wrap small">
      {#each t.member_roles as m (m.slug)}<span class="chip" title={m.title}><RoleIcon emoji={m.emoji} words={m.title + " " + m.name} /> {m.name}{m.account ? ` · ${m.account}` : ""}</span>{/each}
      {#if !t.member_roles.length}<span class="faint">ยังไม่มีสมาชิก — หัวหน้าใช้ผู้ช่วยทั่วไป</span>{/if}
    </div>
    {#if parts.length}
      <div class="parts">
        {#each parts as p, i (i)}
          <div class="part small">
            <span class="who">{who(p)}</span>
            <span class="task grow">{p.task}</span>
            {#if p.account}<span class="faint"><Icon name="bot" /> {p.account}{p.model ? " · " + p.model : ""}</span>{/if}
            <span class="chip {STATUS[p.status]?.[1] ?? ''}">{STATUS[p.status]?.[0] ?? p.status}</span>
            <span class="faint">{clock(p.started)}</span>
          </div>
        {/each}
      </div>
    {:else}
      <div class="faint small">ยังไม่มีการแบ่งงาน — หัวหน้าจะแบ่งเมื่อมีส่วนที่ทำแยกกันได้ (งานเล็กหัวหน้าทำเอง)</div>
    {/if}
  </div>
{/if}

<style>
  .tc { border: 1px solid var(--line); border-radius: 10px; padding: 8px 10px; display: flex; flex-direction: column; gap: 6px; }
  .parts { display: flex; flex-direction: column; gap: 3px; max-height: 220px; overflow: auto; }
  .part { display: flex; gap: 8px; align-items: center; }
  .who { white-space: nowrap; font-weight: 500; }
  .task { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
