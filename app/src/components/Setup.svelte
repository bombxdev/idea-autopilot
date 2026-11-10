<script>
  import Icon from "./Icon.svelte";
  // ตั้งค่าครั้งแรก: ตรวจว่าเครื่องนี้มี AI ตัวไหน → เลือก → ตั้งเวลางาน → งบโควต้า → เริ่มใช้
  // ทุกช่องมีค่าเริ่มต้นที่ใช้ได้เลย: กด "เริ่มใช้งาน" ได้ทันที แล้วปรับทีหลังในหน้าตั้งค่า
  // หน้าตา: ซ้าย = ขั้นตอน + ค่าที่เลือกไว้ (กดข้ามไปขั้นไหนก็ได้) · ขวา = ทีละคำถาม แตะเลือกได้เลย
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";
  import { LANGS } from "../lib/i18n.js";
  import { refresh, toast } from "../lib/store.svelte.js";

  let prov = $state(null);
  let checking = $state(false);
  let choice = $state("auto");
  let custom = $state("");
  let testing = $state(false);
  let test = $state(null);
  let preset = $state("office9");
  let hours = $state(10);
  let lang = $state("en");
  let autostart = $state(true);
  let saving = $state(false);
  let step = $state(0);

  const PRESETS = {
    office9: { title: "จันทร์–ศุกร์ 09:00–18:00", sub: "เวลาออฟฟิศทั่วไป", icon: "briefcase", start: 9, end: 18, work: ["mon", "tue", "wed", "thu", "fri"] },
    office8: { title: "จันทร์–ศุกร์ 08:00–17:00", sub: "เข้างานเช้า เลิกเร็ว", icon: "sunrise", start: 8, end: 17, work: ["mon", "tue", "wed", "thu", "fri"] },
    anytime: { title: "ไม่มีเวลางานตายตัว", sub: "บอททำงานได้ตลอด ไม่ต้องหลบ", icon: "bolt", start: 9, end: 18, work: [] },
  };
  const HOUR_CHOICES = [5, 10, 20, 40];
  function scheduleFor(key) {
    const p = PRESETS[key];
    const days = {};
    for (const d of ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]) days[d] = p.work.includes(d) ? "full" : "off";
    return { start: p.start, lunch_start: 12, lunch_end: 13, end: p.end, half_end: 12, days };
  }

  async function load() {
    checking = true;
    try {
      prov = await api.providers();
      if (choice === "auto" && prov.active === "none") choice = "custom";
    } finally {
      checking = false;
    }
  }
  onMount(load);

  const meter = $derived(choice === "claude" || (choice === "auto" && prov?.active === "claude") ? prov?.meter : "budget");
  const usable = $derived(choice === "custom" ? !!custom.trim() : choice === "auto" ? prov?.active !== "none" : prov?.list.find((p) => p.id === choice)?.installed);
  const installed = $derived(prov?.list.filter((p) => p.installed) ?? []);
  const missing = $derived(prov?.list.filter((p) => !p.installed) ?? []);
  const aiSummary = $derived(
    !prov ? "กำลังตรวจเครื่อง…"
    : choice === "auto" ? (prov.active === "none" ? "ยังไม่พบ AI" : prov.active_label)
    : choice === "custom" ? "คำสั่งของฉันเอง"
    : prov.list.find((p) => p.id === choice)?.label ?? choice
  );
  const STEPS = $derived([
    { title: "AI ที่ใช้ทำงาน", value: aiSummary },
    { title: "เวลางานของคุณ", value: PRESETS[preset].title },
    { title: "งบและภาษา", value: meter === "real" ? "อ่านโควต้าจริงอัตโนมัติ" : `${hours} ชม./สัปดาห์` },
  ]);
  // test result belongs to the AI it was run on
  $effect(() => { choice; test = null; });

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

  const last = 2;
  function next() { if (step < last) step += 1; else finish(); }
</script>

<div class="veil">
  <div class="shell">
    <aside class="rail">
      <div class="brand"><span class="mark"><Icon name="logo" size="20px" /></span>Idea Autopilot</div>
      <h1>จดไอเดียไว้ ให้ AI ทำต่อตอนคุณไม่ว่าง</h1>
      <p class="lede">AI จะค้นคว้า ตัดสิน หรือสร้างต้นแบบให้ ในเวลาที่คุณไม่ได้ใช้โควต้า · ตอบ 3 ข้อ ไม่ถึงนาที ทุกข้อมีค่าแนะนำไว้แล้ว</p>
      <ol class="steps">
        {#each STEPS as s, i}
          <li>
            <button class="stepbtn" class:on={step === i} class:done={i < step} aria-current={step === i ? "step" : undefined} onclick={() => (step = i)}>
              <span class="num">{#if i < step}<Icon name="check" size="14px" />{:else}{i + 1}{/if}</span>
              <span class="grow">
                <span class="st">{s.title}</span>
                <span class="sv ellipsis">{s.value}</span>
              </span>
            </button>
          </li>
        {/each}
      </ol>
      <p class="trust"><Icon name="lock" /> เครื่องนี้เริ่มใหม่ทั้งหมด ยังไม่มีไอเดียหรือข้อมูลของใคร</p>
    </aside>

    <main class="panel">
      <div class="progress" aria-hidden="true">
        <span>ขั้นที่ {step + 1} จาก 3</span>
        <span class="bar"><span style="width:{((step + 1) / 3) * 100}%"></span></span>
      </div>

      {#key step}
        <div class="body">
          {#if step === 0}
            <header>
              <h2>ให้ AI ตัวไหนทำงาน</h2>
              <p class="muted">เลือกจากที่ติดตั้งในเครื่องนี้ — ถ้าไม่แน่ใจ ใช้ “เลือกให้อัตโนมัติ”</p>
            </header>
            {#if !prov}
              <div class="faint small">กำลังตรวจเครื่อง…</div>
            {:else}
              <div class="opts" role="radiogroup" aria-label="AI ที่ใช้ทำงาน">
                <label class="opt" class:sel={choice === "auto"}>
                  <input type="radio" bind:group={choice} value="auto" disabled={prov.active === "none"} />
                  <span class="dot"></span>
                  <span class="grow"><span class="ot">เลือกให้อัตโนมัติ <span class="tag">แนะนำ</span></span>
                    <span class="od">{prov.active === "none" ? "ยังไม่พบ AI ในเครื่อง" : `ใช้ตัวแรกที่พบ: ${prov.active_label}`}</span></span>
                </label>
                {#each installed as p}
                  <label class="opt" class:sel={choice === p.id}>
                    <input type="radio" bind:group={choice} value={p.id} />
                    <span class="dot"></span>
                    <span class="grow"><span class="ot">{p.label}</span>
                      <span class="od"><Icon name="check" /> พบในเครื่อง · {p.hint}</span></span>
                  </label>
                {/each}
                <label class="opt" class:sel={choice === "custom"}>
                  <input type="radio" bind:group={choice} value="custom" />
                  <span class="dot"></span>
                  <span class="grow"><span class="ot">คำสั่งของฉันเอง</span>
                    <span class="od">ใช้ AI/โปรแกรมอื่นที่รันจาก command line ได้</span></span>
                </label>
              </div>
              {#if choice === "custom"}
                <div class="customcmd">
                  <input class="wide" bind:value={custom} placeholder="เช่น myagent --yes --message-file {'{prompt_file}'}" />
                  <p class="faint small">ใส่ <code>{"{prompt_file}"}</code> = ไฟล์คำสั่งงาน · <code>{"{model}"}</code> = ชื่อโมเดล · โปรแกรมต้องทำงานในโฟลเดอร์ปัจจุบันและเขียนไฟล์ผลลัพธ์เอง</p>
                </div>
              {/if}
              {#if missing.length}
                <p class="faint small">ยังไม่ได้ติดตั้ง: {missing.map((p) => p.label).join(", ")}</p>
              {/if}
              {#if prov.active === "none" && choice !== "custom"}
                <p class="note warn"><Icon name="warning" /> ยังไม่พบ AI ในเครื่อง — ติดตั้งอย่างใดอย่างหนึ่ง (Claude Code, Codex CLI, Gemini CLI, OpenCode) แล้วกด “ตรวจใหม่” หรือใช้ “คำสั่งของฉันเอง”</p>
              {/if}
              <div class="check">
                <div class="row wrap">
                  <button disabled={testing || !usable} onclick={runTest}><Icon name="play" /> {testing ? "กำลังทดสอบ… (ไม่เกิน 2 นาที)" : "ทดสอบสั่งงานจริง"}</button>
                  <button class="ghost" disabled={checking} onclick={load}><Icon name="refresh" /> ตรวจใหม่</button>
                </div>
                <p class="faint small">ส่งคำสั่งสั้นๆ ให้ AI ตอบว่า OK — ใช้โควต้าน้อยมาก ช่วยเช็กว่า login แล้วจริง</p>
                {#if test}
                  <div class="note result" class:bad={!test.ok} role="status">
                    <Icon name={test.ok ? "checkCircle" : "closeCircle"} /> {test.ok ? `ใช้ได้ (${test.seconds} วินาที)` : `ไม่สำเร็จ: ${test.error ?? test.status}`}
                    {#each test.lines ?? [] as l}<div class="faint">{l}</div>{/each}
                  </div>
                {/if}
              </div>
            {/if}
          {:else if step === 1}
            <header>
              <h2>คุณทำงานช่วงไหน</h2>
              <p class="muted">ช่วงนี้บอทจะหลบ ไม่รบกวน และกันโควต้าไว้ให้คุณก่อน</p>
            </header>
            <div class="opts" role="radiogroup" aria-label="เวลางานของคุณ">
              {#each Object.entries(PRESETS) as [k, p]}
                <label class="opt" class:sel={preset === k}>
                  <input type="radio" bind:group={preset} value={k} />
                  <span class="ic"><Icon name={p.icon} size="18px" /></span>
                  <span class="grow"><span class="ot">{p.title}</span><span class="od">{p.sub}</span></span>
                  <span class="dot"></span>
                </label>
              {/each}
            </div>
            <p class="faint small">ตารางละเอียด (วันหยุด วันเสาร์ ครึ่งวัน) ปรับได้ที่ ตั้งค่า → ตารางงานของคุณ</p>
          {:else}
            <header>
              <h2>งบการใช้ AI ของบอท</h2>
              {#if meter === "real"}
                <p class="muted">อ่านโควต้าจริงของ Claude ได้อัตโนมัติ — บอทใช้ส่วนที่เหลือโดยกันไว้ให้งานคุณเอง ไม่ต้องตั้งอะไร</p>
              {:else}
                <p class="muted">AI ตัวนี้ไม่มีตัวอ่านโควต้า จึงให้บอททำงานได้ไม่เกินกี่ชั่วโมงต่อสัปดาห์ (นับเวลาที่บอทรันจริง)</p>
              {/if}
            </header>
            {#if meter === "real"}
              <div class="note result"><Icon name="gauge" /> ไม่ต้องตั้งงบ — ดูโควต้าได้ที่หน้า โควต้า</div>
            {:else}
              <div class="seg" role="radiogroup" aria-label="ชั่วโมงต่อสัปดาห์">
                {#each HOUR_CHOICES as h}
                  <button role="radio" aria-checked={hours === h} class:sel={hours === h} onclick={() => (hours = h)}>{h}</button>
                {/each}
                <label class="other"><input type="number" min="1" max="100" step="1" bind:value={hours} aria-label="ชั่วโมงต่อสัปดาห์" /> ชม./สัปดาห์</label>
              </div>
              <p class="faint small">แนะนำเริ่มที่ 10 — ถ้าใช้ AI แบบจ่ายตามการใช้จริง ให้ตั้งต่ำไว้ก่อน</p>
            {/if}

            <div class="prefs">
              <label class="pref">
                <span class="grow"><span class="ot">ภาษารายงานของ AI</span><span class="od">ภาษาที่ AI ใช้เขียนรายงานและตอบคุณ</span></span>
                <select bind:value={lang}>{#each LANGS as l}<option value={l.code}>{l.name}</option>{/each}</select>
              </label>
              <label class="pref">
                <span class="grow"><span class="ot">เปิดพร้อม Windows</span><span class="od">ซ่อนอยู่ที่ tray ทำงานเงียบๆ ตอนคุณไม่ว่าง</span></span>
                <input class="switch" type="checkbox" role="switch" bind:checked={autostart} />
              </label>
            </div>
          {/if}
        </div>
      {/key}

      <footer>
        {#if step > 0}
          <button class="ghost" onclick={() => (step -= 1)}><Icon name="arrowLeft" /> ย้อนกลับ</button>
        {/if}
        <span class="grow"></span>
        {#if step < last && usable}
          <button class="ghost skip" disabled={saving} onclick={finish}>ใช้ค่าแนะนำแล้วเริ่มเลย</button>
        {/if}
        <button class="primary big" disabled={saving || (!usable && (step === 0 || step === last))} onclick={next}>
          {#if step < last}ถัดไป <Icon name="arrowRight" />{:else}{saving ? "กำลังบันทึก…" : "เริ่มใช้งาน"}{/if}
        </button>
      </footer>
      {#if !usable && prov && step === last}<p class="faint small end">เลือก AI ที่ใช้ได้ก่อน (ขั้นที่ 1)</p>{/if}
    </main>
  </div>
</div>

<style>
  .veil { position: fixed; inset: 0; z-index: 50; background: var(--bg); overflow: auto; display: flex; justify-content: center; align-items: flex-start; padding: clamp(16px, 6vh, 64px) 16px 32px; }
  .shell { width: 100%; max-width: 940px; display: grid; grid-template-columns: 300px 1fr; background: var(--card); border: 1px solid var(--line); border-radius: 16px; overflow: hidden; box-shadow: 0 12px 40px -12px rgb(0 0 0 / 0.35); }
  p { margin: 0; }

  /* left: what this is + where you are */
  .rail { background: var(--bg-2); border-right: 1px solid var(--line); padding: 28px 24px; display: flex; flex-direction: column; gap: 14px; }
  .brand { display: flex; align-items: center; gap: 10px; font-weight: 600; font-size: 14px; color: var(--text-2); }
  .mark { display: grid; place-items: center; width: 32px; height: 32px; border-radius: 9px; background: var(--teal-bg); color: var(--teal); }
  h1 { font-size: 22px; line-height: 1.35; margin-top: 10px; text-wrap: balance; }
  .lede { color: var(--text-2); font-size: 14px; }
  .steps { list-style: none; margin: 10px 0 0; padding: 0; display: flex; flex-direction: column; gap: 4px; }
  .stepbtn { width: 100%; display: flex; align-items: center; gap: 12px; text-align: left; border: 1px solid transparent; border-radius: 10px; padding: 9px 10px; }
  .stepbtn:hover { background: var(--card); border-color: var(--line); }
  .stepbtn:active { transform: none; }
  .stepbtn.on { background: var(--card); border-color: var(--line-2); }
  .num { flex: none; display: grid; place-items: center; width: 26px; height: 26px; border-radius: 50%; border: 1px solid var(--line-2); font-size: 13px; font-weight: 600; color: var(--text-2); font-variant-numeric: tabular-nums; }
  .stepbtn.on .num { background: var(--teal); border-color: var(--teal); color: var(--card); }
  /* text on teal: the card colour reads in both themes (the global dark-green text is faint on light teal) */
  footer .primary { color: var(--card); }
  .stepbtn.done .num { background: var(--teal-bg); border-color: transparent; color: var(--teal); }
  .st { display: block; font-size: 14px; font-weight: 500; line-height: 1.35; }
  .sv { display: block; font-size: 12.5px; color: var(--text-3); line-height: 1.4; }
  .stepbtn.on .sv, .stepbtn.done .sv { color: var(--text-2); }
  .trust { margin-top: auto; padding-top: 16px; font-size: 12.5px; color: var(--text-3); display: flex; gap: 6px; align-items: baseline; }

  /* right: one question at a time */
  .panel { padding: 28px 32px 24px; display: flex; flex-direction: column; gap: 18px; min-width: 0; min-height: 620px; }
  .progress { display: none; }
  /* step change: slides in from an already-visible start, so a paused animation never hides the question */
  .body { display: flex; flex-direction: column; gap: 14px; flex: 1; animation: enter .28s cubic-bezier(.16, 1, .3, 1); }
  @keyframes enter { from { transform: translateY(8px); } to { transform: none; } }
  header { display: flex; flex-direction: column; gap: 4px; margin-bottom: 4px; }
  h2 { font-size: 19px; }
  header .muted { font-size: 14px; max-width: 60ch; }

  .opts { display: flex; flex-direction: column; gap: 8px; }
  .opt { position: relative; display: flex; align-items: center; gap: 14px; padding: 12px 14px; border: 1px solid var(--line); border-radius: 12px; cursor: pointer; transition: border-color .12s, background .12s; }
  .opt:hover { border-color: var(--line-2); background: var(--card-2); }
  .opt.sel { border-color: var(--teal); background: var(--teal-bg); }
  .opt input { position: absolute; opacity: 0; pointer-events: none; }
  .opt:has(input:focus-visible) { outline: 2px solid var(--teal); outline-offset: 2px; }
  .opt:has(input:disabled) { opacity: .5; cursor: not-allowed; }
  .dot { flex: none; width: 18px; height: 18px; border-radius: 50%; border: 1.5px solid var(--line-2); display: grid; place-items: center; }
  .opt.sel .dot { border-color: var(--teal); }
  .opt.sel .dot::after { content: ""; width: 8px; height: 8px; border-radius: 50%; background: var(--teal); }
  .ic { flex: none; display: grid; place-items: center; width: 36px; height: 36px; border-radius: 10px; background: var(--card-2); color: var(--text-2); }
  .opt.sel .ic { background: var(--card); color: var(--teal); }
  .ot { display: flex; align-items: center; gap: 8px; font-weight: 500; line-height: 1.4; }
  .od { display: block; font-size: 13px; color: var(--text-2); line-height: 1.45; }
  .tag { font-size: 11.5px; font-weight: 500; padding: 0 8px; border-radius: 999px; background: var(--teal-bg); color: var(--teal); }
  .opt.sel .tag { background: var(--card); }

  .customcmd { display: flex; flex-direction: column; gap: 6px; }
  .wide { width: 100%; }
  code { background: var(--card-2); padding: 1px 5px; border-radius: 5px; font-size: 12.5px; }
  .check { display: flex; flex-direction: column; gap: 6px; padding-top: 14px; border-top: 1px solid var(--line); }
  .check button { display: inline-flex; align-items: center; gap: 6px; }
  .note { font-size: 13px; border-radius: 10px; padding: 9px 12px; }
  .warn { color: var(--amber); background: var(--amber-bg); }
  .result { background: var(--teal-bg); color: var(--teal); }
  .result.bad { background: var(--red-bg); color: var(--red); }

  .seg { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
  .seg button { min-width: 56px; padding: 8px 14px; border-radius: 10px; font-weight: 500; font-variant-numeric: tabular-nums; }
  .seg button.sel { background: var(--teal-bg); border-color: var(--teal); color: var(--teal); }
  .other { display: inline-flex; align-items: center; gap: 8px; font-size: 13px; color: var(--text-2); margin-left: 4px; }
  .other input { width: 76px; padding: 7px 10px; font-variant-numeric: tabular-nums; }

  .prefs { margin-top: 8px; border: 1px solid var(--line); border-radius: 12px; }
  .pref { display: flex; align-items: center; gap: 16px; padding: 12px 14px; }
  .pref + .pref { border-top: 1px solid var(--line); }
  .pref select { max-width: 180px; }
  .switch { appearance: none; flex: none; width: 40px; height: 22px; padding: 0; border-radius: 999px; background: var(--line-2); border: none; position: relative; cursor: pointer; transition: background .15s; }
  .switch::after { content: ""; position: absolute; top: 3px; left: 3px; width: 16px; height: 16px; border-radius: 50%; background: #fff; transition: transform .15s ease-out; box-shadow: 0 1px 2px rgb(0 0 0 / .3); }
  .switch:checked { background: var(--teal); }
  .switch:checked::after { transform: translateX(18px); }
  .switch:focus-visible, .stepbtn:focus-visible, footer button:focus-visible, .seg button:focus-visible { outline: 2px solid var(--teal); outline-offset: 2px; }

  footer { display: flex; align-items: center; gap: 8px; padding-top: 16px; border-top: 1px solid var(--line); }
  footer button { display: inline-flex; align-items: center; gap: 6px; }
  .skip { color: var(--text-2); font-size: 14px; }
  .big { padding: 10px 24px; font-size: 15px; }
  button:disabled { opacity: .5; cursor: not-allowed; }
  .end { text-align: right; margin-top: -8px; }

  @media (prefers-reduced-motion: reduce) { .body { animation: none; } .switch, .switch::after, .opt { transition: none; } }

  /* narrow: rail shrinks to a header, progress bar shows where you are */
  @media (max-width: 760px) {
    /* width: the app underneath can be wider than a phone screen; the setup screen still fits the screen */
    .veil { padding: 0; right: auto; bottom: auto; width: 100vw; height: 100dvh; }
    .shell { grid-template-columns: minmax(0, 1fr); grid-template-rows: auto 1fr; border: none; border-radius: 0; box-shadow: none; min-height: 100%; }
    .rail { border-right: none; border-bottom: 1px solid var(--line); padding: 20px 16px 16px; gap: 8px; }
    h1 { font-size: 19px; margin-top: 6px; }
    .lede { font-size: 13px; }
    .steps, .trust { display: none; }
    .panel { padding: 18px 16px 20px; min-height: 0; }
    .progress { display: flex; align-items: center; gap: 12px; font-size: 12.5px; color: var(--text-2); white-space: nowrap; }
    .bar { flex: 1; height: 4px; border-radius: 4px; background: var(--line); overflow: hidden; }
    .bar span { display: block; height: 100%; background: var(--teal); border-radius: 4px; transition: width .25s ease-out; }
    footer { position: sticky; bottom: 0; background: var(--card); margin: 0 -16px -20px; padding: 12px 16px; flex-wrap: wrap; }
    .skip { order: 3; width: 100%; justify-content: center; }
    .big { flex: 1; justify-content: center; }
  }
</style>
