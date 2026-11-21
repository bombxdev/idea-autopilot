<script>
  // The document add-on (docs.rs): PDF / Word / Excel / PowerPoint become Markdown before the AI reads them.
  // One switch; the converters come from recipes and are installed only when pressed here.
  import Icon from "./Icon.svelte";
  import { onMount, onDestroy } from "svelte";
  import { api } from "../lib/api.js";
  import { toast } from "../lib/store.svelte.js";
  import { ask } from "../lib/dialog.svelte.js";

  let st = $state(null);
  let job = $state(null);
  let timer;

  async function load() {
    try {
      st = await api.docsAddonStatus();
      job = (await api.videoAddonStatus()).job;
    } catch (e) { toast(String(e), "err"); }
    clearTimeout(timer);
    if (job?.state === "running") timer = setTimeout(load, 1000);
  }
  onMount(load);
  onDestroy(() => clearTimeout(timer));

  const running = $derived(job?.state === "running");

  async function toggle(on) {
    try { await api.docsAddonSet(on); } catch (e) { toast(String(e), "err"); }
    load();
  }
  async function install(c) {
    const mb = (c.size_mb || 0) + (st.uv.found ? 0 : st.uv.size_mb);
    const extra = st.uv.found ? "" : ` (รวม ${st.uv.label} ที่ใช้รันเครื่องมือ)`;
    if (!(await ask(`ติดตั้ง ${c.label}${extra} — ดาวน์โหลดราว ${mb || "ไม่ถึง 1"} MB จากแหล่งทางการ (ฟรี) ลบออกได้ทุกเมื่อ`, { ok: "ติดตั้ง" }))) return;
    try { await api.videoAddonInstall(`conv:${c.id}`); } catch (e) { toast(String(e), "err"); }
    load();
  }
  async function remove(c) {
    if (!(await ask(`ลบ ${c.label} ออกจากเครื่อง?`, { ok: "ลบ" }))) return;
    try { await api.videoAddonRemove(`conv:${c.id}`); } catch (e) { toast(String(e), "err"); }
    load();
  }
</script>

<div class="card">
  <h2><Icon name="doc" /> อ่านเอกสารแบบประหยัด token</h2>
  <p class="muted small">
    แนบ PDF, Word, Excel, PowerPoint แล้วแอปแปลงเป็นข้อความ (Markdown) ให้ AI อ่านก่อน — ตารางยังเป็นตาราง
    ใช้ token น้อยกว่าให้ AI เปิดไฟล์เองหลายเท่า (สไลด์ 2 MB เหลือข้อความราว 27 KB) และ AI ที่เปิด Word/Excel ไม่ได้ก็อ่านได้
    ไฟล์ต้นฉบับยังอยู่ AI เปิดดูได้เมื่อต้องดูภาพหรือกราฟ
  </p>

  {#if !st}
    <p class="muted small">กำลังตรวจ…</p>
  {:else}
    <label class="row field">
      <span class="grow"><b>แปลงเอกสารเป็นข้อความก่อนให้ AI อ่าน</b><br />
        <span class="muted small">{st.enabled ? "เปิดอยู่" : "ปิดอยู่ — ส่งไฟล์ให้ AI ตามปกติ"}</span></span>
      <input type="checkbox" checked={st.enabled} onchange={(e) => toggle(e.target.checked)} />
    </label>

    {#each st.converters as c (c.id)}
      <div class="row field">
        <span class="grow"><b>{c.label}</b><br /><span class="muted small">{c.note} · {c.license}</span></span>
        {#if c.ready}
          <span class="ok"><Icon name="checkCircle" /> พร้อม</span>
          {#if c.kind === "uv-script"}<button class="ghost small" disabled={running} onclick={() => remove(c)}><Icon name="trash" /></button>{/if}
        {:else}
          <button class={c.kind === "uvx" ? "primary small" : "small"} disabled={running || !st.enabled} onclick={() => install(c)}><Icon name="download" /> ติดตั้ง</button>
        {/if}
      </div>
    {/each}

    {#if running && (job.part === "uv" || job.part.startsWith("conv:"))}
      <div class="small job">{job.message || "กำลังติดตั้ง…"}{#if job.total_mb}<progress max={job.total_mb} value={job.done_mb}></progress>{/if}</div>
    {:else if job?.state === "error" && job.part.startsWith("conv:")}
      <div class="warnbox small">ติดตั้งไม่สำเร็จ: {job.message}</div>
    {/if}

    <p class="faint small">
      {st.uv.found ? (st.uv.own ? "ใช้ uv ที่แอปติดตั้งให้" : "ใช้ uv ที่มีในเครื่องอยู่แล้ว") : `ต้องมี uv (${st.uv.size_mb} MB) — แอปติดตั้งให้พร้อมกับตัวแปลง`}
      · เปลี่ยนเครื่องมือได้ที่ <code>data/tools/recipes.json</code>
    </p>
  {/if}
</div>

<style>
  .ok { color: var(--teal); white-space: nowrap; font-size: 13px; }
  .job { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; margin: 6px 0; }
  .job progress { flex: 1; min-width: 120px; }
  .field { gap: 10px; align-items: center; }
</style>
