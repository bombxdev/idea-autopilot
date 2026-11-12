<script>
  // Updating the program: the latest version on GitHub Releases, an "update now" button and an automatic-update switch.
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";
  import { toast } from "../lib/store.svelte.js";
  import { ask } from "../lib/dialog.svelte.js";

  let { compact = false } = $props();
  let u = $state(null);
  let busy = $state(false);
  let working = $state(false);

  async function load(refresh = false) {
    busy = refresh;
    try {
      u = await api.updateStatus(refresh);
      if (refresh && u && !u.error && !u.available) toast("เป็นรุ่นล่าสุดแล้ว");
    } catch {}
    busy = false;
  }
  onMount(() => load(false));

  async function apply() {
    if (!(await ask("โปรแกรมจะปิดแล้วเปิดใหม่เอง (ไอเดียและข้อมูลไม่หาย)", { title: `อัปเดตเป็น v${u.latest}?`, ok: "อัปเดต" }))) return;
    working = true;
    try {
      await api.updateApply();
      toast("กำลังอัปเดต — โปรแกรมจะปิดแล้วเปิดใหม่เอง", "ok", 8000);
    } catch (e) {
      working = false;
      toast(String(e), "err", 9000);
    }
  }
  async function setAuto(on) {
    await api.setSetting("auto_update", on ? "1" : "0");
    u = { ...u, auto: on };
  }
  const MODE = { installer: "ติดตั้งด้วยตัวติดตั้ง", scoop: "ติดตั้งด้วย Scoop", portable: "แบบพกพา", dev: "รุ่นสำหรับพัฒนา" };
  const when = (ts) => (ts ? new Date(ts * 1000).toLocaleString() : "—");
</script>

{#if u}
  <div class="upd" class:compact>
    <div class="row wrap">
      <span>เวอร์ชันที่ใช้อยู่ <b>v{u.current}</b></span>
      {#if u.available}<span class="chip amber">มีรุ่นใหม่ v{u.latest}</span>
      {:else if u.latest}<span class="chip teal">เป็นรุ่นล่าสุด</span>{/if}
      <span class="grow"></span>
      <button class="small" disabled={busy} onclick={() => load(true)}>{busy ? "กำลังตรวจ…" : "ตรวจหาเวอร์ชันใหม่"}</button>
      {#if u.available}
        {#if u.can_apply}<button class="primary small" disabled={working} onclick={apply}>{working ? "กำลังอัปเดต…" : `อัปเดตเป็น v${u.latest}`}</button>
        {:else if u.page}<a class="btn small" href={u.page} target="_blank" rel="noreferrer">เปิดหน้าดาวน์โหลด</a>{/if}
      {/if}
    </div>
    {#if u.available && !compact}
      {#if u.notes}<pre class="notes">{u.notes}</pre>{/if}
      {#if u.mode === "scoop"}<div class="small faint">เครื่องนี้ติดตั้งด้วย Scoop — อัปเดตด้วยคำสั่ง <code>scoop update idea-autopilot</code></div>
      {:else if u.mode === "dev"}<div class="small faint">เครื่องนี้เป็น{MODE[u.mode]} — อัปเดตด้วยการ build ใหม่ (deploy) ไม่ใช่จาก Releases</div>
      {:else if u.mode === "portable" && u.available && !u.can_apply && !u.no_digest}<div class="small faint">รุ่นนี้ไม่มีไฟล์แบบพกพา (.zip) ให้อัปเดตอัตโนมัติ — โหลดจากหน้า Releases แล้วแทนที่โฟลเดอร์ bin (ข้อมูลของคุณไม่หาย)</div>
      {:else if u.no_digest}<div class="small faint">GitHub ไม่ได้ให้ค่า SHA-256 ของไฟล์ติดตั้งรุ่นนี้ จึงไม่อัปเดตให้อัตโนมัติ (ตรวจความถูกต้องไม่ได้) — โหลดจากหน้า Releases เอง</div>{/if}
    {/if}
    {#if !compact}
      <label class="row small auto">
        <input type="checkbox" checked={u.auto} onchange={(e) => setAuto(e.currentTarget.checked)} disabled={u.mode !== "installer" && u.mode !== "portable"} />
        อัปเดตอัตโนมัติ: ตรวจทุก 6 ชั่วโมง และติดตั้งให้เองเมื่อบอทว่างและไม่มี session คุยกับ AI เปิดอยู่
        {#if u.mode !== "installer" && u.mode !== "portable"}<span class="faint">(ใช้ได้กับเครื่องที่ติดตั้งด้วยตัวติดตั้ง หรือแบบพกพา)</span>{/if}
      </label>
      <div class="small faint">ตรวจล่าสุด {when(u.checked)} · แหล่งอัปเดต: GitHub Releases ของโปรเจกต์ · ไฟล์ติดตั้งต้องมีค่า SHA-256 ตรงกับที่ GitHub รายงาน ไม่งั้นไม่ติดตั้ง{#if u.error} · <span class="err">{u.error}</span>{/if}</div>
    {/if}
  </div>
{/if}

<style>
  .upd { display: flex; flex-direction: column; gap: 8px; }
  .notes { background: var(--bg-2); padding: 8px 10px; border-radius: 8px; max-height: 160px; overflow: auto; white-space: pre-wrap; font-size: 12px; margin: 0; }
  .auto { gap: 8px; }
  .chip.amber { color: var(--amber); }
  .chip.teal { color: var(--teal); }
  .btn { display: inline-block; padding: 4px 12px; border: 1px solid var(--line); border-radius: 8px; color: var(--text); text-decoration: none; }
  .err { color: var(--red); }
  .faint { color: var(--text-3); }
</style>
