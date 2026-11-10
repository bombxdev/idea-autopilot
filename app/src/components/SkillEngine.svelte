<script>
  import Icon from "./Icon.svelte";
  // Status of the SkillSpector tool: installed / version / updates / how many skills it has checked.
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api } from "../lib/api.js";
  import { toast } from "../lib/store.svelte.js";

  let { onchange = () => {} } = $props();
  let s = $state(null);
  let working = $state(false);

  async function load(refresh = false) {
    try {
      s = await api.skillspectorStatus(refresh);
    } catch {}
  }
  onMount(() => {
    load();
    const t = setInterval(() => load(), 5000);
    let un = () => {};
    listen("skills-changed", () => {
      load();
      onchange();
    }).then((f) => (un = f));
    return () => {
      clearInterval(t);
      un();
    };
  });

  async function install() {
    working = true;
    try {
      const v = await api.skillspectorInstall();
      toast(`พร้อมใช้ SkillSpector v${v}`, "ok");
    } catch (e) {
      toast(String(e) === "NEED_UV" ? "ต้องมีโปรแกรม uv ก่อน (ดูคำแนะนำในกล่อง)" : String(e), "err");
    }
    working = false;
    load(true);
    onchange();
  }
  async function recheck() {
    try {
      await api.skillspectorRescan();
      toast("เริ่มตรวจสกิลทั้งหมดใหม่ในเบื้องหลัง");
    } catch (e) {
      toast(String(e), "err");
    }
    load();
  }
</script>

{#if s}
  <div class="card eng small">
    <div class="row wrap">
      <b><Icon name="shieldCheck" /> ตัวตรวจสกิล: SkillSpector (NVIDIA)</b>
      {#if s.installed}
        <span class="chip teal">ติดตั้งแล้ว v{s.version}</span>
        <span class="faint">ตรวจแล้ว {s.checked}/{s.total} สกิล{s.busy ? " · กำลังตรวจ…" : ""}</span>
        <span class="grow"></span>
        {#if s.update}
          <span class="chip amber">มีรุ่นใหม่ v{s.latest}</span>
          <button class="primary small" disabled={working || s.busy} onclick={install}>{working ? "กำลังอัปเดต… (1-2 นาที)" : "อัปเดต"}</button>
        {:else if s.latest}
          <span class="faint">เป็นรุ่นล่าสุด</span>
        {/if}
        <button class="small" disabled={s.busy} onclick={() => load(true)} title="ดูว่าต้นทางบน GitHub มีรุ่นใหม่ไหม">เช็กรุ่นใหม่</button>
        <button class="small" disabled={s.busy} onclick={recheck}>ตรวจสกิลทั้งหมดใหม่</button>
      {:else}
        <span class="chip amber">ยังไม่ได้ติดตั้ง</span>
        <span class="grow"></span>
        {#if s.uv}
          <button class="primary small" disabled={working || s.busy} onclick={install}>{working || s.busy ? "กำลังติดตั้ง… (1-2 นาที)" : "ติดตั้งเลย (ฟรี)"}</button>
        {/if}
      {/if}
    </div>
    {#if !s.installed}
      <div class="faint">
        ตอนนี้สกิลถูกตรวจด้วยตัวสแกนในโปรแกรมเท่านั้น SkillSpector เป็นตัวตรวจจากภายนอกที่ลึกกว่า (prompt injection, การขโมยข้อมูล, supply chain ฯลฯ)
        ทำงานในเครื่องคุณ ไม่รันสกิล ไม่ส่งเนื้อหาสกิลออกไปไหน
        {#if s.uv}โปรแกรมจะติดตั้งให้เองเมื่อเปิดครั้งแรก หรือกดปุ่มด้านบน{:else}ต้องติดตั้ง <code>uv</code> ก่อน (ฟรี): <code>winget install --id astral-sh.uv</code> แล้วเปิดโปรแกรมใหม่{/if}
      </div>
    {:else}
      <div class="faint">โปรแกรมเช็กต้นทางบน GitHub ทุก 12 ชม. ถ้ามีรุ่นใหม่จะขึ้นปุ่มอัปเดตให้ — ทุกเครื่องอัปเดตเองได้ สกิลที่ไม่ผ่านการตรวจจะไม่ถูกเสนอให้บอท</div>
    {/if}
    {#if s.check_error}<div class="faint">เช็กรุ่นใหม่ไม่ได้ตอนนี้: {s.check_error}</div>{/if}
  </div>
{/if}

<style>
  .eng { display: flex; flex-direction: column; gap: 6px; border-left: 3px solid var(--teal); }
  .chip.teal { color: var(--teal); }
  .chip.amber { color: var(--amber); }
  .faint { color: var(--text-3); }
</style>
