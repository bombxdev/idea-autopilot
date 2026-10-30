<script>
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";
  import { toast, refresh } from "../lib/store.svelte.js";
  import Markdown from "../components/Markdown.svelte";

  let s = $state({});
  let cal = $state([]);
  let date = $state("");
  let kind = $state("off");
  let note = $state("");
  let err = $state("");
  let sync = $state({});
  let sched = $state(null);
  let busy = $state("");
  let policy = $state(null);
  let prov = $state(null);
  let tiers = $state({});
  let testing = $state(false);
  let test = $state(null);
  let customDraft = $state("");
  let about = $state(null);
  let showLog = $state(false);

  async function load() {
    [s, cal, sync, policy, days, prov] = await Promise.all([api.settings(), api.calendar(60), api.syncStatus(), api.policy(), api.listDays(), api.providers()]);
    sched = s.schedule ? JSON.parse(JSON.stringify(s.schedule)) : null;
    customDraft = s.custom_cmd ?? "";
    about = await api.about();
    const a = prov.list.find((p) => p.id === prov.active);
    tiers = a ? { fast: a.tiers[0], standard: a.tiers[1], strong: a.tiers[2] } : {};
  }

  // waiting for the first Telegram message → poll so "ผูกกับ" appears without reopening the page
  $effect(() => {
    if (!s.telegram_token || s.telegram_owner) return;
    const t = setInterval(async () => {
      const n = await api.settings();
      s.telegram_owner = n.telegram_owner;
      s.telegram_status = n.telegram_status;
    }, 3000);
    return () => clearInterval(t);
  });

  const DAYS = [["mon", "จ."], ["tue", "อ."], ["wed", "พ."], ["thu", "พฤ."], ["fri", "ศ."], ["sat", "ส."], ["sun", "อา."]];
  const toTime = (h) => `${String(Math.floor(h)).padStart(2, "0")}:${String(Math.round((h % 1) * 60)).padStart(2, "0")}`;
  const fromTime = (t) => {
    const [h, m] = t.split(":").map(Number);
    return h + (m || 0) / 60;
  };
  const ruleKind = (r) => (typeof r === "string" ? r : "byweek");
  function setRule(day, v) {
    sched.days[day] = v === "byweek" ? { "1": "off", "2": "full", "3": "off", "4": "full", "5": "half" } : v;
  }
  const TIER_LABEL = { fast: "เร็ว/ถูก (งานจัดคิว)", standard: "กลาง (ค้นเว็บ เขียนโค้ด)", strong: "แรง (ตัดสิน วางแผน)" };
  async function saveTiers() {
    let all = {};
    try { all = JSON.parse(s.tier_models || "{}"); } catch { all = {}; }
    all[prov.active] = { ...tiers };
    await save("tier_models", JSON.stringify(all));
  }
  async function runTest() {
    testing = true;
    test = null;
    try {
      test = await api.testProvider(null);
    } catch (e) {
      test = { ok: false, error: String(e), lines: [] };
    } finally {
      testing = false;
    }
  }
  async function saveSched() {
    await save("schedule", JSON.stringify(sched));
  }
  async function exportBrain() {
    busy = "brain";
    try {
      await api.exportBrain();
      toast("export สมองแล้ว");
      load();
    } catch (e) {
      toast(String(e), "err");
    } finally {
      busy = "";
    }
  }
  async function takeover() {
    if (!confirm(`ย้ายเครื่องหลักมาที่ ${sync.machine}?\nเครื่องเดิม (${sync.leader ?? "-"}) จะหยุดรันบอทและเป็นเครื่องรอง`)) return;
    busy = "takeover";
    try {
      await api.takeover();
      toast("เครื่องนี้เป็นเครื่องหลักแล้ว");
      load();
      refresh();
    } catch (e) {
      toast(String(e), "err");
    } finally {
      busy = "";
    }
  }
  const ago = (ts) => (ts ? `${Math.round((Date.now() / 1000 - ts) / 60)} นาทีที่แล้ว` : "—");
  onMount(() => {
    load();
    // the model policy is re-learned after every run; keep the table fresh while this page is open
    const t = setInterval(async () => (policy = await api.policy()), 30000);
    return () => clearInterval(t);
  });

  async function save(key, value) {
    try {
      await api.setSetting(key, String(value));
      toast("บันทึกแล้ว");
      load();
      refresh();
    } catch (e) {
      toast(String(e), "err");
    }
  }
  // dates are typed as dd/mm/yyyy (the native date picker follows the OS locale: mm/dd/yyyy)
  let days = $state([]);
  let showPast = $state(false);
  const pad = (n) => String(n).padStart(2, "0");
  const todayIso = (() => { const d = new Date(); return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`; })();
  const toDmy = (iso) => { const [y, m, d] = iso.split("-"); return `${d}/${m}/${y}`; };
  const WEEK = ["อา.", "จ.", "อ.", "พ.", "พฤ.", "ศ.", "ส."];
  const weekday = (iso) => WEEK[new Date(iso + "T00:00:00").getDay()];
  function parseDmy(t) {
    const m = t.trim().match(/^(\d{1,2})\/(\d{1,2})\/(\d{4})$/);
    if (!m) return null;
    const [d, mo, y] = [Number(m[1]), Number(m[2]), Number(m[3])];
    const dt = new Date(y, mo - 1, d);
    if (dt.getFullYear() !== y || dt.getMonth() !== mo - 1 || dt.getDate() !== d) return null;
    return `${y}-${pad(mo)}-${pad(d)}`;
  }
  function typeDate(e) {
    // auto-insert slashes while typing digits: 05122026 -> 05/12/2026
    const digits = e.target.value.replace(/\D/g, "").slice(0, 8);
    date = [digits.slice(0, 2), digits.slice(2, 4), digits.slice(4)].filter(Boolean).join("/");
    err = "";
  }
  const preview = $derived(parseDmy(date));
  const upcoming = $derived(days.filter((d) => d.date >= todayIso));
  const past = $derived(days.filter((d) => d.date < todayIso).reverse());

  async function addDay() {
    const iso = parseDmy(date);
    if (!iso) {
      err = "ใส่วันที่เป็น dd/mm/yyyy เช่น 05/12/2026";
      return;
    }
    await api.setDay(iso, kind, note);
    date = note = "";
    toast("ตั้งวันแล้ว");
    load();
    refresh();
  }
  async function clearDay(d) {
    await api.setDay(d, null);
    load();
    refresh();
  }
  const KIND = { off: "หยุด / ลา", work: "ทำงานเต็มวัน", half: "ครึ่งวันเช้า" };
  const MODELS = [
    ["strong", "บังคับระดับแรง (ฉลาดสุด กินโควต้าสุด)"],
    ["standard", "บังคับระดับกลาง (สมดุล)"],
    ["fast", "บังคับระดับเร็ว (ถูกสุด งานง่าย)"],
  ];
</script>

<div class="wrapx">
  <h1>ตั้งค่า</h1>

  {#if prov}
    <div class="card">
      <h2>AI ที่ใช้ทำงาน</h2>
      <p class="muted small">เครื่องนี้ไม่จำเป็นต้องมีครบทุกตัว — เลือกตัวที่คุณมีได้เลย ผลงานทั้งหมดเป็นไฟล์ในโฟลเดอร์ไอเดีย จึงเปลี่ยน AI ได้ทุกเมื่อโดยงานเดิมไม่หาย</p>
      <div class="row field"><span class="grow">ตอนนี้ใช้: <b>{prov.active_label}</b>
        <span class="faint small">· วัดโควต้า: {prov.meter === "real" ? "อ่านจากบริการจริง" : "งบชั่วโมงของแอป"}</span></span>
        <select value={prov.setting} onchange={(e) => save("provider", e.target.value)}>
          <option value="auto">เลือกให้อัตโนมัติ (ตัวแรกที่พบ)</option>
          {#each prov.list as p}<option value={p.id} disabled={!p.installed}>{p.label}{p.installed ? "" : " — ยังไม่ได้ติดตั้ง"}</option>{/each}
          <option value="custom">คำสั่งของฉันเอง</option>
        </select></div>
      {#if prov.setting === "custom" || prov.active === "custom"}
        <div class="row field"><span class="grow">คำสั่ง <span class="faint small">ใช้ {"{prompt_file}"} = ไฟล์คำสั่งงาน, {"{model}"} = ชื่อโมเดล</span></span>
          <input style="width:420px" bind:value={customDraft} placeholder="myagent --yes --message-file {'{prompt_file}'}" onchange={() => save("custom_cmd", customDraft.trim())} /></div>
      {/if}
      {#if prov.list.some((p) => p.id === prov.active)}
        <div class="faint small">ชื่อโมเดลของแต่ละระดับ — ปล่อยว่างไว้ = โปรแกรมเลือกให้เองจากรายชื่อโมเดลที่ AI นั้นมีอยู่ตอนนี้ (ตามที่เห็นเป็นตัวอักษรจาง) หรือใช้ค่าเริ่มต้นของโปรแกรม ถ้า AI นั้นไม่บอกรายชื่อ{prov.active === "opencode" ? " · OpenCode ใช้รูปแบบ provider/model" : ""}</div>
        {#each ["fast", "standard", "strong"] as t}
          <div class="row field"><span class="grow">{TIER_LABEL[t]}</span>
            <input style="width:260px" bind:value={tiers[t]} placeholder={(prov.list.find((p) => p.id === prov.active)?.auto[["fast", "standard", "strong"].indexOf(t)] || "ค่าเริ่มต้นของโปรแกรม")} onchange={saveTiers} /></div>
        {/each}
      {/if}
      <div class="row">
        <button disabled={testing || prov.active === "none"} onclick={runTest}>{testing ? "กำลังทดสอบ… (ไม่เกิน 2 นาที)" : "ทดสอบสั่งงานจริง"}</button>
        <span class="faint small">ให้ AI ตอบว่า OK หนึ่งคำ เพื่อเช็กว่า login และตั้งค่าถูก</span>
      </div>
      {#if test}
        <div class="small" style="color:{test.ok ? 'var(--teal)' : 'var(--red)'}">
          {test.ok ? `✓ ใช้ได้ (${test.seconds} วินาที)` : `✗ ไม่สำเร็จ: ${test.error ?? test.status}`}
          {#each test.lines ?? [] as l}<div class="faint">{l}</div>{/each}
        </div>
      {/if}
      <div class="row field"><span class="grow">ภาษารายงานของ AI</span>
        <select value={s.language ?? "th"} onchange={(e) => save("language", e.target.value)}><option value="th">ไทย</option><option value="en">English</option></select></div>
    </div>
  {/if}

  {#if sched}
    <div class="card">
      <h2>ตารางงานของคุณ</h2>
      <p class="muted small">ช่วงเวลางานบอทจะหลบและกันโควต้าไว้ให้ นอกนั้นบอทใช้เต็มที่ — เพื่อนที่เวลาไม่ตรงกันก็ตั้งของตัวเองได้</p>
      <div class="row wrap">
        <label class="row small">เริ่มงาน <input type="time" value={toTime(sched.start)} onchange={(e) => (sched.start = fromTime(e.target.value))} /></label>
        <label class="row small">พักเที่ยง <input type="time" value={toTime(sched.lunch_start)} onchange={(e) => (sched.lunch_start = fromTime(e.target.value))} />
          – <input type="time" value={toTime(sched.lunch_end)} onchange={(e) => (sched.lunch_end = fromTime(e.target.value))} /></label>
        <label class="row small">เลิกงาน <input type="time" value={toTime(sched.end)} onchange={(e) => (sched.end = fromTime(e.target.value))} /></label>
        <label class="row small">ครึ่งวันถึง <input type="time" value={toTime(sched.half_end)} onchange={(e) => (sched.half_end = fromTime(e.target.value))} /></label>
      </div>
      <div class="days">
        {#each DAYS as [k, label]}
          {@const r = sched.days[k] ?? "off"}
          <div class="day">
            <b>{label}</b>
            <select value={ruleKind(r)} onchange={(e) => setRule(k, e.target.value)}>
              <option value="full">เต็มวัน</option><option value="half">ครึ่งวัน</option><option value="off">หยุด</option><option value="byweek">ตามสัปดาห์ที่…</option>
            </select>
            {#if typeof r === "object"}
              {#each ["1", "2", "3", "4", "5"] as n}
                <label class="row small nth">ที่ {n}
                  <select value={r[n] ?? "off"} onchange={(e) => (sched.days[k][n] = e.target.value)}>
                    <option value="full">เต็ม</option><option value="half">ครึ่ง</option><option value="off">หยุด</option>
                  </select></label>
              {/each}
            {/if}
          </div>
        {/each}
      </div>
      <div class="row"><button class="primary" onclick={saveSched}>บันทึกตารางงาน</button></div>
    </div>
  {/if}

  <div class="card">
    <h2>วันหยุดพิเศษ / วันลา / ทำงานเพิ่ม</h2>
    <div class="row wrap">
      <input style="width:150px" inputmode="numeric" value={date} oninput={typeDate} placeholder="dd/mm/yyyy" maxlength="10" />
      {#if preview}<span class="faint small">{weekday(preview)}</span>{/if}
      <select bind:value={kind}><option value="off">หยุด / ลา</option><option value="work">ทำงานเต็มวัน</option><option value="half">ครึ่งวันเช้า</option></select>
      <input class="grow" bind:value={note} placeholder="หมายเหตุ เช่น วันปิยมหาราช" />
      <button class="primary" onclick={addDay}>เพิ่ม</button>
    </div>
    {#if err}<div class="small" style="color:var(--red)">{err}</div>{/if}
    {#each upcoming as o (o.date)}
      <div class="row small ov"><b class="dt">{weekday(o.date)} {toDmy(o.date)}</b><span>{KIND[o.kind] ?? o.kind}</span><span class="faint grow">{o.note ?? ""}</span><button class="ghost small" onclick={() => clearDay(o.date)}>ลบ</button></div>
    {:else}
      <div class="faint small">ยังไม่มีวันหยุดพิเศษที่จะมาถึง</div>
    {/each}
    {#if past.length}
      <button class="ghost small" style="align-self:flex-start" onclick={() => (showPast = !showPast)}>{showPast ? "▾" : "▸"} ผ่านไปแล้ว ({past.length})</button>
      {#if showPast}
        {#each past as o (o.date)}
          <div class="row small ov faint"><span class="dt">{weekday(o.date)} {toDmy(o.date)}</span><span>{KIND[o.kind] ?? o.kind}</span><span class="grow">{o.note ?? ""}</span><button class="ghost small" onclick={() => clearDay(o.date)}>ลบ</button></div>
        {/each}
      {/if}
    {/if}
  </div>

  <div class="card">
    <h2>โมเดลและ effort</h2>
    <p class="muted small">โหมดอัตโนมัติเลือกด้วยกฎ + สถิติจริง จึงไม่เสีย token ในการตัดสินใจ และ Claude เรียกรุ่นล่าสุดของแต่ละตระกูลเสมอ: งานปริมาณมาก (ค้นเว็บ เขียนโค้ด) ใช้ระดับกลาง, งานตัดสินใจ (ตัดสิน วางแผน) ใช้ระดับแรง, งานจัดคิวใช้ระดับเร็ว (แต่ละ AI จับคู่ระดับกับโมเดลของตัวเองตามที่ตั้งไว้ด้านบน · Claude = Haiku / Sonnet / Opus) · ถ้าเคยติดขัดจะขยับขึ้น · ถ้าโควต้าเหลือและกำลังจะหมดอายุจะอัปเกรดคุณภาพ · ถ้าโควต้าใกล้หมดจะลด effort · ทุก 5 รอบจะลองรุ่นที่ถูกกว่า 1 ครั้ง ถ้าดีพอๆ กัน 5 รอบขึ้นไปจะเปลี่ยนไปใช้ถาวร และเริ่มนับใหม่เมื่อมีโมเดลเวอร์ชันใหม่</p>
    {#if policy && Object.keys(policy.stages).length}
      <div class="pol">
        <div class="faint small">เรียนรู้จากผลงานจริงทุกชั่วโมง (ไม่ใช้ token) · เวอร์ชันล่าสุดที่เห็น: {Object.values(policy.versions).join(", ") || "—"}</div>
        <table>
          <thead><tr><th>ขั้น</th><th>ตาราง</th><th>ใช้จริง</th><th>สถิติ (ดี/ทั้งหมด)</th></tr></thead>
          <tbody>
            {#each Object.entries(policy.stages) as [stage, p]}
              <tr class:down={p.offset < 0}>
                <td>{stage}</td><td>{p.base}</td><td><b>{p.chosen}</b></td>
                <td class="faint">{Object.entries(p.stats).map(([t, x]) => `${t} ${x.good}/${x.runs}`).join(" · ")}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
    <p class="faint small">ช่องโมเดลเว้นว่างหรือ “auto” = ให้โปรแกรมเลือกตามกฎด้านบน · พิมพ์ระดับ (fast / standard / strong) หรือชื่อโมเดลจริงของ AI ที่ใช้ เพื่อบังคับ · effort ก็เหมือนกัน เลือก auto เพื่อให้โปรแกรมเลือก{prov && !prov.list.find((p) => p.id === prov.active)?.effort ? " (AI ตัวนี้ปรับ effort ไม่ได้ ค่านี้จะถูกข้าม)" : ""}</p>
    <datalist id="model-opts">
      <option value="auto"></option><option value="strong"></option><option value="standard"></option><option value="fast"></option>
      {#each (prov?.list.find((p) => p.id === prov.active)?.auto ?? []).filter(Boolean) as m}<option value={m}></option>{/each}
    </datalist>
    {#each [["model_default", "ค่าเริ่มต้น (ทุกขั้น)"], ["model_build", "ขั้นสร้าง"], ["model_verify", "ขั้นตรวจ"]] as [k, label]}
      {@const ek = k.replace("model_", "effort_")}
      <div class="row field"><span class="grow">{label}</span>
        <input list="model-opts" style="width:230px" value={s[k] || "auto"} onchange={(e) => save(k, e.target.value.trim() || "auto")} />
        <select value={s[ek] || "auto"} onchange={(e) => save(ek, e.target.value)}>
          <option value="auto">effort: auto</option><option value="low">low</option><option value="medium">medium</option><option value="high">high</option><option value="xhigh">xhigh</option>
        </select></div>
    {/each}
  </div>

  <div class="card">
    <h2>การกันโควต้า</h2>
    {#if prov?.meter === "real"}
      <div class="row field"><span class="grow">กันไว้ให้งานต่อชั่วโมงทำงาน (% ของสัปดาห์) — ใช้จนกว่าจะมีสถิติพอ (~3 วันทำงาน)</span>
        <input type="number" min="0.2" max="6" step="0.1" style="width:90px" value={s.reserve_per_hour ?? 1.5}
          onchange={(e) => save("reserve_per_hour", e.target.value)} /></div>
    {:else}
      <p class="muted small">AI ตัวนี้ไม่มีตัวอ่านโควต้า แอปจึงนับเวลาที่บอทรันจริงเทียบกับงบที่ตั้งไว้ (ไม่นับการใช้งานของคุณเอง)</p>
      <div class="row field"><span class="grow">งบต่อสัปดาห์ (ชั่วโมงที่บอทรันได้)</span>
        <input type="number" min="1" max="168" step="1" style="width:90px" value={s.budget_hours_week ?? 10} onchange={(e) => save("budget_hours_week", e.target.value)} /></div>
      <div class="row field"><span class="grow">งบต่อ 5 ชั่วโมง (กันบอทรันรวดเดียวนานเกิน)</span>
        <input type="number" min="0.5" max="5" step="0.5" style="width:90px" value={s.budget_hours_5h ?? 3} onchange={(e) => save("budget_hours_5h", e.target.value)} /></div>
      <div class="row field"><span class="grow">งบเริ่มนับใหม่ทุกวัน</span>
        <select value={s.budget_reset_day ?? "0"} onchange={(e) => save("budget_reset_day", e.target.value)}>
          {#each ["จันทร์", "อังคาร", "พุธ", "พฤหัสบดี", "ศุกร์", "เสาร์", "อาทิตย์"] as d, i}<option value={String(i)}>{d}</option>{/each}
        </select></div>
    {/if}
    {#if prov && prov.list.find((p) => p.id === "claude")?.installed}
      <div class="row field"><span class="grow">วิธีวัดโควต้า <span class="faint small">(Claude อ่านจริงได้ — เปลี่ยนเป็นงบชั่วโมงถ้าใช้ API key)</span></span>
        <select value={s.meter ?? "auto"} onchange={(e) => save("meter", e.target.value)}><option value="auto">อัตโนมัติ</option><option value="budget">ใช้งบชั่วโมงของแอป</option></select></div>
    {/if}
  </div>

  <div class="card">
    <h2>สั่งงานจากมือถือ (Telegram)</h2>
    <p class="muted small">1) ใน Telegram คุยกับ <b>@BotFather</b> พิมพ์ <code>/newbot</code> ตั้งชื่อ แล้ว copy token มาวางด้านล่าง 2) เปิดบอทของคุณแล้วส่งข้อความอะไรก็ได้ — คนแรกที่ทักจะถูกผูกเป็นเจ้าของ คนอื่นทักมาบอทจะไม่สนใจ<br/>จากนั้นพิมพ์ไอเดียได้เลย, ตอบคำถามด้วยปุ่ม, ได้แจ้งเตือนเมื่องานเสร็จ, พิมพ์ /help ดูคำสั่ง</p>
    <div class="row field"><span class="grow">Bot token</span>
      <input type="password" style="width:380px" value={s.telegram_token ?? ""} placeholder="123456:ABC-DEF..." onchange={(e) => save("telegram_token", e.target.value.trim())} /></div>
    {#if s.telegram_token}
      <div class="kv small">
        <span>การเชื่อมต่อ</span><b>{s.telegram_status === "ok" ? "✓ ต่อได้" : (s.telegram_status ?? "กำลังต่อ…")}</b>
        <span>ผูกกับ</span><b>{s.telegram_owner || "ยังไม่มี — ส่งข้อความหาบอทก่อน"}</b>
      </div>
      <div class="row field"><span class="grow">ส่งความคืบหน้าทุกขั้น + แจ้งเตือนเมื่อไอเดียพร้อมใช้งาน<br/><span class="faint small">เปิดไว้เป็นค่าเริ่มต้น</span></span>
        <input type="checkbox" checked={s.telegram_progress !== "0"} onchange={(e) => save("telegram_progress", e.target.checked ? "1" : "0")} /></div>
      <div class="row field"><span class="grow">ให้บอทถาม-ตอบใน Telegram ด้วย (ส่งคำถามมาพร้อมปุ่มให้แตะตอบ)<br/><span class="faint small">ปิดไว้เป็นค่าเริ่มต้น — Telegram จะได้แค่ความคืบหน้า แต่ยังพิมพ์จดไอเดียเข้ามาได้ (เหมาะกับ iPhone ที่ยังไม่มีแอปจดไอเดีย)</span></span>
        <input type="checkbox" checked={s.telegram_questions === "1"} onchange={(e) => save("telegram_questions", e.target.checked ? "1" : "0")} /></div>
      {#if s.telegram_owner}
        <div class="row"><button onclick={() => save("telegram_chat", "")}>ยกเลิกการผูก</button></div>
      {/if}
    {/if}
  </div>

  <div class="card">
    <h2>หลายเครื่อง (ที่ทำงาน · ที่บ้าน)</h2>
    <p class="muted small">ชี้ไปที่โฟลเดอร์ที่ sync ข้ามเครื่องได้ — Google Drive / OneDrive ส่วนตัว หรือโฟลเดอร์ที่ clone จาก GitHub private repo (ถ้าเป็น git ระบบ pull/push ให้เองทุก 5 นาที) เครื่องหลักรันบอท เครื่องอื่นจดไอเดีย ดูรายงาน ตอบคำถามได้</p>
    <div class="row field"><span class="grow">โฟลเดอร์ sync</span>
      <input style="width:380px" value={s.sync_dir ?? ""} placeholder="เช่น G:/My Drive/IdeaAutopilot" onchange={(e) => save("sync_dir", e.target.value)} /></div>
    {#if sync.sync_dir}
      <div class="kv small">
        <span>เครื่องนี้</span><b>{sync.machine} · {sync.mode === "leader" ? "เครื่องหลัก (รันบอท)" : "เครื่องรอง"}</b>
        <span>เครื่องหลัก</span><b>{sync.leader ?? "—"} · heartbeat {ago(sync.heartbeat)}</b>
        <span>วิธี sync</span><b>{sync.git ? "git (pull/push อัตโนมัติ)" : "โฟลเดอร์ cloud drive"}</b>
      </div>
      {#if sync.mode === "follower"}
        <div class="row"><button class="primary" disabled={busy === "takeover"} onclick={takeover}>ย้ายเครื่องหลักมาที่นี่</button>
          <span class="faint small">ใช้เมื่อจะเลิกใช้เครื่องเดิม — ข้อมูลล่าสุดจะถูกดึงมาที่เครื่องนี้</span></div>
      {/if}
    {/if}
  </div>

  <div class="card">
    <h2>สมองพกพา</h2>
    <p class="muted small">รวมความรู้ที่สะสมไว้ (memory และ skill ของ Claude Code ถ้ามี, คลังความรู้, ไอเดียทั้งหมด) เป็นไฟล์ Markdown ที่โมเดลไหนก็อ่านได้ แยกชุด <b>ส่วนตัว</b> กับ <b>งานบริษัท</b> — export อัตโนมัติทุกวัน</p>
    <div class="kv small">
      <span>เก็บไว้ที่</span><code>{sync.brain_dir}</code>
      <span>export ล่าสุด</span><b>{ago(sync.brain_exported)}</b>
    </div>
    <div class="row"><button class="primary" disabled={busy === "brain"} onclick={exportBrain}>export ตอนนี้ + เปิดโฟลเดอร์</button>
      <span class="faint small">นำไปใช้ที่อื่น: เปิด personal/IMPORT.md</span></div>
  </div>

  <div class="card">
    <h2>เกี่ยวกับ · ประวัติเวอร์ชัน</h2>
    <div class="row field"><span class="grow">เวอร์ชันที่ใช้อยู่ <b>v{about?.version ?? ""}</b>
      <span class="faint small">· อัปเดตด้วยไฟล์ติดตั้งใหม่ได้เลย ไอเดียและข้อมูลเดิมไม่หาย</span></span>
      <button onclick={() => (showLog = !showLog)}>{showLog ? "ซ่อน" : "ดู"}ประวัติทุกเวอร์ชัน</button></div>
    <div class="faint small">© 2026 bombxdev · เป็นโอเพนซอร์สภายใต้ GPL-3.0-or-later — ใช้ แก้ไข และแจกต่อได้ แต่งานที่ดัดแปลงต้องเปิดซอร์สภายใต้สัญญาเดียวกัน</div>
    {#if showLog && about}<div class="logbox"><Markdown text={about.changelog} /></div>{/if}
  </div>

  <div class="card">
    <h2>ระบบ</h2>
    <div class="row field"><span class="grow">เปิดพร้อม Windows (ซ่อนอยู่ที่ tray)</span>
      <input type="checkbox" checked={s.autostart === "1"} onchange={(e) => save("autostart", e.target.checked ? "1" : "0")} /></div>
    <div class="row field"><span class="grow">ส่งไอเดียด้วยปุ่ม <span class="faint small">(กันกด Enter พลาดตอนอยากขึ้นบรรทัดใหม่)</span></span>
      <select value={s.send_key ?? "enter"} onchange={(e) => save("send_key", e.target.value)}>
        <option value="enter">Enter ส่ง · Shift+Enter ขึ้นบรรทัดใหม่</option>
        <option value="ctrl_enter">Ctrl+Enter ส่ง · Enter ขึ้นบรรทัดใหม่ (กันพลาด)</option>
      </select></div>
    <div class="row field"><span class="grow">รอก่อนเริ่มทำไอเดียที่เพิ่งจด (วินาที) <span class="faint small">— เวลาให้แก้ข้อความถ้าพิมพ์พลาด</span></span>
      <input type="number" min="0" max="3600" step="10" style="width:90px" value={s.grace_secs ?? 90} onchange={(e) => save("grace_secs", e.target.value)} /></div>
    <div class="row field"><span class="grow">ปุ่มลัดจดไอเดีย</span><b>{s.hotkey ? s.hotkey.toUpperCase() : "ไม่ว่าง (ทุกปุ่มถูกโปรแกรมอื่นใช้อยู่)"}</b></div>
    <div class="row field"><span class="grow">โฟลเดอร์ข้อมูล</span><code class="small">{s.root}</code></div>
    <div class="row field"><span class="grow">โฟลเดอร์รับไอเดียเพิ่ม (คั่นด้วย ;) เช่น โฟลเดอร์ที่ sync กับมือถือ</span>
      <input style="width:340px" value={s.extra_inbox ?? ""} placeholder="เช่น D:/Sync/Ideas" onchange={(e) => save("extra_inbox", e.target.value)} /></div>
    <p class="faint small">จดไอเดียแบบไฟล์ได้ด้วย: วางไฟล์ .md / .txt ในโฟลเดอร์ <code>workspace/inbox</code> ระบบจะดึงเข้าคิวเอง</p>
  </div>
</div>

<style>
  .logbox { max-height: 360px; overflow: auto; background: var(--bg-2); border-radius: 8px; padding: 6px 14px; }
  .wrapx { max-width: 820px; display: flex; flex-direction: column; gap: 16px; }
  .card { display: flex; flex-direction: column; gap: 10px; }
  .card p { margin: 0; }
  .ov { padding: 4px 0; border-top: 1px solid var(--line); }
  .dt { width: 130px; flex: none; font-variant-numeric: tabular-nums; }
  .field { padding: 4px 0; }
  .pol table { width: 100%; border-collapse: collapse; font-size: 13px; margin-top: 6px; }
  .pol th, .pol td { text-align: left; padding: 4px 8px; border-bottom: 1px solid var(--line); }
  .pol th { color: var(--text-3); font-weight: 500; }
  .pol tr.down td b { color: var(--teal); }
  .days { display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); gap: 8px; }
  .day { display: flex; flex-direction: column; gap: 4px; background: var(--bg-2); border-radius: 8px; padding: 8px; }
  .day select { padding: 4px 6px; font-size: 13px; }
  .nth { justify-content: space-between; }
  .kv { display: grid; grid-template-columns: auto 1fr; gap: 3px 14px; }
  .kv span { color: var(--text-2); }
  input[type="time"] { padding: 4px 8px; }
</style>
