<script>
  // The video & sound add-on (video.rs): optional, downloaded only when turned on here. Shows what is installed,
  // suggests the best speech model for the owner's language, and installs/removes with the size shown first.
  import Icon from "./Icon.svelte";
  import { onMount, onDestroy } from "svelte";
  import { api } from "../lib/api.js";
  import { toast } from "../lib/store.svelte.js";
  import { ask } from "../lib/dialog.svelte.js";

  let st = $state(null);
  let more = $state(false);
  let timer;

  async function load() {
    try { st = await api.videoAddonStatus(); } catch (e) { toast(String(e), "err"); }
    clearTimeout(timer);
    if (st?.job?.state === "running") timer = setTimeout(load, 1000);
  }
  onMount(load);
  onDestroy(() => clearTimeout(timer));

  const running = $derived(st?.job?.state === "running");
  const models = $derived(st?.models ?? []);
  const rec = $derived(models.find((m) => m.recommended));
  const others = $derived(models.filter((m) => !m.recommended));
  const anySpeech = $derived(models.some((m) => m.installed && m.engine_installed));

  // what a model install downloads in all: the model, its engine and ffmpeg when missing
  function total(m) {
    return m.size_mb + (m.engine_installed ? 0 : m.engine_size_mb) + (st?.ffmpeg?.found ? 0 : st?.ffmpeg?.size_mb ?? 0);
  }

  async function install(part, label, mb) {
    if (!(await ask(`ดาวน์โหลด ${label} ราว ${mb} MB จากแหล่งทางการ (ฟรี) แล้วติดตั้งในโฟลเดอร์ของแอป — ลบออกได้ทุกเมื่อ`, { ok: "ติดตั้ง" }))) return;
    try { await api.videoAddonInstall(part); } catch (e) { toast(String(e), "err"); }
    load();
  }

  async function remove(part, label) {
    if (!(await ask(`ลบ ${label} ออกจากเครื่อง? (ติดตั้งใหม่ได้ภายหลัง)`, { ok: "ลบ" }))) return;
    try { await api.videoAddonRemove(part); } catch (e) { toast(String(e), "err"); }
    load();
  }
</script>

<div class="card">
  <h2><Icon name="film" /> ส่วนเสริม: ให้ AI เข้าใจวิดีโอและเสียง</h2>
  <p class="muted small">
    แนบวิดีโอหรือไฟล์เสียงแล้ว แอปจะสรุปให้ AI อ่าน — ช็อต จังหวะตัดต่อ โทนสี ภาพรวมในภาพเดียว และคำพูดพร้อมเวลา
    ใช้ token น้อยกว่าส่งวิดีโอทั้งไฟล์มาก (คลิป 1 นาทีเหลือราว 2,000 token) ใช้ได้กับ AI ทุกตัว ทำงานในเครื่องทั้งหมด ฟรี
    ไม่ใช้ก็ไม่ต้องติดตั้ง — ไม่กินพื้นที่หรือเครื่อง
  </p>

  {#if !st}
    <p class="muted small">กำลังตรวจ…</p>
  {:else}
    <div class="row field">
      <span class="grow"><b>ภาพและจังหวะ</b> <span class="muted small">(ffmpeg)</span><br />
        <span class="muted small">
          {#if st.ffmpeg.found}พร้อมใช้ {st.ffmpeg.own ? "· ติดตั้งโดยแอป" : "· ใช้ตัวที่มีในเครื่องอยู่แล้ว"}{:else}ยังไม่มีในเครื่อง · {st.ffmpeg.size_mb} MB{/if}
        </span>
      </span>
      {#if st.ffmpeg.found}
        <span class="ok"><Icon name="checkCircle" /> พร้อม</span>
        {#if st.ffmpeg.own}<button class="ghost small" disabled={running} onclick={() => remove("ffmpeg", "ffmpeg")}><Icon name="trash" /></button>{/if}
      {:else}
        <button class="primary small" disabled={running} onclick={() => install("ffmpeg", "ffmpeg", st.ffmpeg.size_mb)}><Icon name="download" /> ติดตั้ง</button>
      {/if}
    </div>

    {#snippet modelRow(m)}
      <div class="row field">
        <span class="grow"><b>ถอดคำพูด: {m.label}</b>{#if m.recommended} <span class="tag">แนะนำสำหรับภาษาของคุณ</span>{/if}<br />
          <span class="muted small">{m.note} · {m.license}{#if !m.installed}{" · "}{total(m)} MB{/if}</span>
        </span>
        {#if m.installed && m.engine_installed}
          <span class="ok"><Icon name="checkCircle" /> พร้อม</span>
          <button class="ghost small" disabled={running} onclick={() => remove(`model:${m.id}`, m.label)}><Icon name="trash" /></button>
        {:else}
          <button class={m.recommended ? "primary small" : "small"} disabled={running} onclick={() => install(`model:${m.id}`, m.label, total(m))}><Icon name="download" /> ติดตั้ง</button>
        {/if}
      </div>
    {/snippet}

    {#if rec}{@render modelRow(rec)}{/if}
    {#if others.length}
      <button class="ghost small" onclick={() => (more = !more)}><Icon name={more ? "chevronUp" : "chevronDown"} /> {more ? "ซ่อน" : "ภาษาอื่น / ตัวเลือกอื่น"} ({others.length})</button>
      {#if more}{#each others as m (m.id)}{@render modelRow(m)}{/each}{/if}
    {/if}

    {#if st.job.state === "running"}
      <div class="small job">
        {st.job.message || "กำลังติดตั้ง…"}
        {#if st.job.total_mb}<progress max={st.job.total_mb} value={st.job.done_mb}></progress> {Math.round(st.job.done_mb)} / {st.job.total_mb} MB{/if}
      </div>
    {:else if st.job.state === "error"}
      <div class="warnbox small">ติดตั้งไม่สำเร็จ: {st.job.message}</div>
    {/if}

    <p class="faint small">
      {#if anySpeech}ถ้าคลิปเป็นภาษาอื่น แอปเลือกโมเดลที่เหมาะเองเมื่อมีหลายตัว{:else}ไม่ติดตั้งตัวถอดคำพูดก็ได้ — AI ยังเห็นภาพ จังหวะ และสี แค่ไม่รู้ว่าพูดอะไร{/if}
      · รายการเครื่องมือเปลี่ยนได้ที่ <code>data/tools/recipes.json</code>{st.custom_recipes ? " (กำลังใช้ของเครื่องนี้)" : ""}
    </p>
  {/if}
</div>

<style>
  .ok { color: var(--teal); white-space: nowrap; font-size: 13px; }
  .tag { font-size: 11px; color: var(--teal); border: 1px solid currentColor; border-radius: 999px; padding: 0 6px; margin-left: 4px; }
  .job { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; margin: 6px 0; }
  .job progress { flex: 1; min-width: 120px; }
  .field { gap: 10px; align-items: center; }
</style>
