<script>
  // ตั้งค่าครั้งแรก: ตรวจว่าเครื่องนี้มี AI ตัวไหน → เลือก → ตั้งเวลางาน → งบโควต้า → เริ่มใช้
  // ทุกช่องมีค่าเริ่มต้นที่ใช้ได้เลย: กด "เริ่มใช้งาน" ได้ทันที แล้วปรับทีหลังในหน้าตั้งค่า
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";
  import { refresh, toast } from "../lib/store.svelte.js";

  let prov = $state(null);
  let choice = $state("auto");
  let custom = $state("");
  let testing = $state(false);
  let test = $state(null);
  let preset = $state("office9");
  let hours = $state(10);
  let lang = $state("th");
  let autostart = $state(true);
  let saving = $state(false);

  const PRESETS = {
    office9: { label: "ทำงานออฟฟิศ จ–ศ 09:00–18:00", start: 9, end: 18, work: ["mon", "tue", "wed", "thu", "fri"] },
    office8: { label: "ทำงานออฟฟิศ จ–ศ 08:00–17:00", start: 8, end: 17, work: ["mon", "tue", "wed", "thu", "fri"] },
    anytime: { label: "ไม่มีเวลางานตายตัว — ให้บอททำได้ตลอด", start: 9, end: 18, work: [] },
  };
  function scheduleFor(key) {
    const p = PRESETS[key];
    const days = {};
    for (const d of ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]) days[d] = p.work.includes(d) ? "full" : "off";
    return { start: p.start, lunch_start: 12, lunch_end: 13, end: p.end, half_end: 12, days };
  }

  async function load() {
    prov = await api.providers();
    if (choice === "auto" && prov.active === "none") choice = "custom";
  }
  onMount(load);

  const meter = $derived(choice === "claude" || (choice === "auto" && prov?.active === "claude") ? prov?.meter : "budget");
  const usable = $derived(choice === "custom" ? !!custom.trim() : choice === "auto" ? prov?.active !== "none" : prov?.list.find((p) => p.id === choice)?.installed);

  async function runTest() {
    testing = true;
    test = null;
    try {
      if (choice === "custom") await api.setSetting("custom_cmd", custom.trim());
      test = await api.testProvider(choice === "auto" ? null : choice);
    } catch (e) {
      test = { ok: false, error: String(e), lines: [] };
    } finally {
      testing = false;
    }
  }

  async function finish() {
    saving = true;
    try {
      await api.setSetting("provider", choice);
      if (choice === "custom") await api.setSetting("custom_cmd", custom.trim());
      await api.setSetting("schedule", JSON.stringify(scheduleFor(preset)));
      if (meter === "budget") await api.setSetting("budget_hours_week", String(hours));
      await api.setSetting("language", lang);
      await api.setSetting("autostart", autostart ? "1" : "0");
      await api.setSetting("onboarded", "1");
      toast("พร้อมแล้ว — จดไอเดียแรกได้เลย");
      refresh();
    } catch (e) {
      toast(String(e), "err");
    } finally {
      saving = false;
    }
  }
</script>

<div class="veil">
  <div class="card box">
    <h1>ยินดีต้อนรับสู่ Idea Autopilot</h1>
    <p class="muted">จดไอเดียทิ้งไว้ แล้วให้ AI ไปค้นคว้า ตัดสิน หรือสร้างให้ตอนที่คุณไม่ว่าง · ตอนนี้เริ่มใหม่ทั้งหมด ยังไม่มีไอเดียหรือข้อมูลของใครอยู่ในเครื่องนี้ ตั้งค่าได้ 3 อย่างด้านล่าง แล้วปรับทีหลังได้ทุกอย่าง</p>

    <section>
      <h2>1 · ให้ AI ตัวไหนทำงาน</h2>
      {#if !prov}
        <div class="faint small">กำลังตรวจเครื่อง…</div>
      {:else}
        <label class="opt"><input type="radio" bind:group={choice} value="auto" />
          <span class="grow"><b>เลือกให้อัตโนมัติ</b> <span class="faint small">ใช้ตัวแรกที่พบ: {prov.active === "none" ? "ยังไม่พบ AI ในเครื่อง" : prov.active_label}</span></span></label>
        {#each prov.list as p}
          <label class="opt" class:off={!p.installed}><input type="radio" bind:group={choice} value={p.id} disabled={!p.installed} />
            <span class="grow"><b>{p.label}</b> <span class="faint small">{p.installed ? "✓ พบในเครื่อง · " + p.hint : "ยังไม่ได้ติดตั้ง"}</span></span></label>
        {/each}
        <label class="opt"><input type="radio" bind:group={choice} value="custom" />
          <span class="grow"><b>คำสั่งของฉันเอง</b> <span class="faint small">ใช้ AI/โปรแกรมอื่นที่รันจาก command line ได้</span></span></label>
        {#if choice === "custom"}
          <input class="wide" bind:value={custom} placeholder="เช่น myagent --yes --message-file {'{prompt_file}'}" />
          <p class="faint small">ใส่ <code>{"{prompt_file}"}</code> = ไฟล์คำสั่งงาน · <code>{"{model}"}</code> = ชื่อโมเดล · โปรแกรมต้องทำงานในโฟลเดอร์ปัจจุบันและเขียนไฟล์ผลลัพธ์เอง</p>
        {/if}
        {#if prov.active === "none" && choice !== "custom"}
          <p class="small warn">ยังไม่พบ AI ในเครื่อง — ติดตั้งอย่างใดอย่างหนึ่ง (Claude Code, Codex CLI, Gemini CLI, OpenCode) แล้วกด “ตรวจใหม่” หรือใช้ “คำสั่งของฉันเอง”</p>
        {/if}
        <div class="row">
          <button onclick={load}>ตรวจใหม่</button>
          <button disabled={testing || !usable} onclick={runTest}>{testing ? "กำลังทดสอบ… (ไม่เกิน 2 นาที)" : "ทดสอบสั่งงานจริง"}</button>
          <span class="faint small">ส่งคำสั่งสั้นๆ ให้ AI ตอบว่า OK — ใช้โควต้าน้อยมาก ช่วยเช็กว่า login แล้วจริง</span>
        </div>
        {#if test}
          <div class="small result" class:bad={!test.ok}>
            {test.ok ? `✓ ใช้ได้ (${test.seconds} วินาที)` : `✗ ไม่สำเร็จ: ${test.error ?? test.status}`}
            {#each test.lines ?? [] as l}<div class="faint">{l}</div>{/each}
          </div>
        {/if}
      {/if}
    </section>

    <section>
      <h2>2 · เวลาทำงานของคุณ</h2>
      <p class="muted small">ช่วงนี้บอทจะหลบ ไม่รบกวน และกันโควต้าไว้ให้คุณก่อน</p>
      {#each Object.entries(PRESETS) as [k, p]}
        <label class="opt"><input type="radio" bind:group={preset} value={k} /><span>{p.label}</span></label>
      {/each}
      <p class="faint small">ตารางละเอียด (วันหยุด วันเสาร์ ครึ่งวัน) ปรับได้ที่ ตั้งค่า → ตารางงานของคุณ</p>
    </section>

    <section>
      <h2>3 · งบการใช้ AI ของบอท</h2>
      {#if meter === "real"}
        <p class="muted small">✓ อ่านโควต้าจริงของ Claude ได้อัตโนมัติ — บอทใช้ส่วนที่เหลือโดยกันไว้ให้งานคุณเอง ไม่ต้องตั้งอะไร</p>
      {:else}
        <p class="muted small">AI ตัวนี้ไม่มีตัวอ่านโควต้า จึงให้บอททำงานได้ไม่เกินกี่ชั่วโมงต่อสัปดาห์ (นับเวลาที่บอทรันจริง)</p>
        <div class="row"><input type="number" min="1" max="100" step="1" style="width:90px" bind:value={hours} /><span>ชั่วโมงต่อสัปดาห์ <span class="faint small">(แนะนำเริ่มที่ 10 — ถ้าใช้ AI แบบจ่ายตามการใช้จริง ให้ตั้งต่ำไว้ก่อน)</span></span></div>
      {/if}
    </section>

    <section class="row wrap">
      <label class="row small">ภาษารายงานของ AI
        <select bind:value={lang}><option value="th">ไทย</option><option value="en">English</option></select></label>
      <label class="row small"><input type="checkbox" bind:checked={autostart} /> เปิดพร้อม Windows (ซ่อนอยู่ที่ tray)</label>
    </section>

    <div class="row end">
      <button class="primary big" disabled={saving || !usable} onclick={finish}>{saving ? "กำลังบันทึก…" : "เริ่มใช้งาน"}</button>
      {#if !usable && prov}<span class="faint small">เลือก AI ที่ใช้ได้ก่อน</span>{/if}
    </div>
  </div>
</div>

<style>
  .veil { position: fixed; inset: 0; z-index: 50; background: var(--bg); overflow: auto; display: flex; justify-content: center; align-items: flex-start; padding: 32px 16px; }
  .box { width: 100%; max-width: 720px; box-sizing: border-box; display: flex; flex-direction: column; gap: 18px; }
  h1 { margin: 0; font-size: 22px; }
  h2 { margin: 0 0 6px; font-size: 15px; }
  p { margin: 0; }
  section { display: flex; flex-direction: column; gap: 8px; padding-top: 14px; border-top: 1px solid var(--line); }
  .opt { display: flex; align-items: center; gap: 10px; padding: 6px 8px; border-radius: 8px; cursor: pointer; }
  .opt:hover { background: var(--bg-2); }
  .opt.off { opacity: 0.55; }
  .wide { width: 100%; }
  .warn { color: var(--amber); }
  .result { background: var(--bg-2); border-radius: 8px; padding: 8px 10px; color: var(--teal); }
  .result.bad { color: var(--red); }
  .end { justify-content: flex-end; }
  .big { padding: 10px 28px; font-size: 15px; }
</style>
