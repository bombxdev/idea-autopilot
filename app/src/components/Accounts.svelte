<script>
  import IconText from "./IconText.svelte";
  import Icon from "./Icon.svelte";
  // AI accounts: any provider of the catalog, signed in with the agent's own login or with a key / token, each with its
  // own name, on/off switch, model + effort (auto, or chosen from what that account can use) and quota. The bot uses
  // the first one switched on that its own quota allows.
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";
  import { toast, store } from "../lib/store.svelte.js";
  import { ask } from "../lib/dialog.svelte.js";
  import ModelPicker from "./ModelPicker.svelte";
  import { clock } from "../lib/api.js";
  import { openUrl } from "@tauri-apps/plugin-opener";
  // OmniRoute's own dashboard (its key, connected providers, free-tier quota)
  const omniDash = (a) => { try { openUrl((a?.base_url || "http://localhost:20128").replace(/\/v1\/?$/, "")); } catch (e) { toast(String(e), "err"); } };

  let data = $state({ accounts: [], kinds: [], vendors: [] });
  let vendorId = $state("anthropic");
  let newId = $state("");
  let authSel = $state("login");
  let keyIn = $state("");
  let baseIn = $state("");
  let modelIn = $state("");
  let isolated = $state(true);
  let busy = $state(false);
  let openCfg = $state({}); // account id -> model list panel open
  let models = $state({}); // account id -> { models, efforts, source }
  let loadingModels = $state({});
  let keyEdit = $state({}); // account id -> new key typed
  let testing = $state({});
  let tested = $state({});
  const EFFORTS = ["low", "medium", "high", "xhigh"];

  // a "check now" that just ended: read the model list again so the newly passed models show
  let checkEnded = {};
  async function load() {
    try {
      data = await api.accounts();
      for (const a of data.accounts ?? []) {
        const f = a.check?.finished ?? 0;
        if (f && checkEnded[a.id] === 0 && openCfg[a.id]) loadModels(a);
        checkEnded[a.id] = f;
      }
    } catch {}
  }
  const checking = () => (data.accounts ?? []).some((a) => a.check && !a.check.finished);
  onMount(() => {
    load();
    // an install started earlier keeps running in the app: show where it is (or how it ended) when coming back
    api.cliInstallStatus().then((s) => { if (s && s.state !== "idle") { inst = s; if (s.state === "running") pollInstall(); } }).catch(() => {});
    // every 8 s; every 3 s while a model check is running (so its progress moves on screen)
    let n = 0;
    const t = setInterval(() => { n++; if (checking() || n % 3 === 0) load(); }, 3000);
    return () => clearInterval(t);
  });
  const VERDICT = { bot: "✓ ทำงานบอทได้", chat: "✗ แชตได้อย่างเดียว", unknown: "? ตรวจไม่ได้ตอนนี้" };
  const mins = (s) => (s < 60 ? `${s} วินาที` : `${Math.floor(s / 60)} นาที ${s % 60} วินาที`);

  const vendor = $derived(data.vendors.find((v) => v.id === vendorId));
  $effect(() => {
    // keep the sign-in method valid for the chosen provider
    if (vendor && !vendor.auth.includes(authSel)) authSel = vendor.auth[0];
    if (vendor) modelIn = vendor.models[0] ?? "";
  });

  // OmniRoute: the owner says the subscription connected in it is another account than the app's (or takes it back)
  async function omniDup(a, ok) {
    try { await api.accountOmniDupOk(a.id, ok); await load(); } catch (e) { toast(String(e), "err"); }
  }

  async function upd(a, patch) {
    try {
      await api.accountUpdate(a.id, patch);
      await load();
    } catch (e) {
      toast(String(e), "err");
    }
  }
  async function add() {
    busy = true;
    try {
      const id = await api.accountAdd({ vendor: vendorId, id: newId.trim() || null, isolated, auth: authSel, key: authSel === "key" ? keyIn : null, base_url: baseIn.trim() || null, model: vendor?.needs_model ? modelIn.trim() || null : null });
      toast(authSel === "key" ? `เพิ่ม “${id}” แล้ว — ลองกด “ทดสอบ” ว่า key ใช้ได้` : isolated ? `เพิ่ม “${id}” แล้ว — กด “ล็อกอิน” เพื่อเชื่อมบัญชี` : `เพิ่ม “${id}” แล้ว`, "ok");
      newId = "";
      keyIn = "";
      baseIn = "";
      await load();
    } catch (e) {
      toast(String(e), "err");
    }
    busy = false;
  }
  async function login(a) {
    try {
      await api.accountLogin(a.id);
      toast("เปิดหน้าต่างล็อกอินแล้ว — ทำให้เสร็จในเบราว์เซอร์ แล้วกลับมาที่นี่");
    } catch (e) {
      toast(String(e), "err");
    }
  }
  async function remove(a) {
    const msg = a.isolated
      ? `ยกเลิกการเชื่อมต่อ “${a.label || a.id}” และลบ login / key ของบัญชีนี้ออกจากเครื่อง? (ถ้าต้องการใช้อีกต้องตั้งใหม่)`
      : `เอา “${a.label || a.id}” ออกจากโปรแกรม? (login ปกติของ ${a.kind_label} ในเครื่องจะไม่ถูกแตะต้อง)`;
    if (!(await ask(msg, { title: a.isolated ? "ยกเลิกการเชื่อมต่อบัญชี" : "เอาบัญชีออก", ok: a.isolated ? "ยกเลิกการเชื่อมต่อ" : "เอาออก", danger: true }))) return;
    try {
      await api.accountRemove(a.id);
      await load();
    } catch (e) {
      toast(String(e), "err");
    }
  }
  async function saveKey(a) {
    try {
      await api.accountSetKey(a.id, keyEdit[a.id] ?? "", a.vendor === "custom" ? a.base_url : null);
      keyEdit = { ...keyEdit, [a.id]: undefined };
      toast("บันทึก key แล้ว (เก็บในโฟลเดอร์ของบัญชีนี้ ไม่แสดงกลับ)", "ok");
      await load();
    } catch (e) {
      toast(String(e), "err");
    }
  }
  async function clearKey(a) {
    if (!(await ask("บัญชีจะถูกปิดไว้จนกว่าจะใส่ key ใหม่", { title: "ลบ key ของบัญชีนี้?", ok: "ลบ key", danger: true }))) return;
    await api.accountClearKey(a.id);
    await load();
  }
  async function test(a) {
    testing = { ...testing, [a.id]: true };
    try {
      tested = { ...tested, [a.id]: await api.accountTest(a.id) };
    } catch (e) {
      tested = { ...tested, [a.id]: { ok: false, error: String(e) } };
    }
    testing = { ...testing, [a.id]: false };
    await load();
  }
  async function checkNow(a) {
    try {
      const n = await api.modelCheckNow(a.id);
      toast(n ? `จะตรวจ ${n} โมเดลทีละตัวในพื้นหลัง (ตัวละไม่เกิน 4 นาที) — ดูความคืบหน้าใต้ชื่อบัญชีนี้` : "ไม่มีโมเดลที่รอตรวจ", "ok");
      await load();
    } catch (e) {
      toast(String(e), "err");
    }
  }
  let testingAll = $state(false);
  async function testAll() {
    testingAll = true;
    for (const a of data.accounts.filter((x) => x.installed)) await test(a);
    testingAll = false;
    const bad = data.accounts.filter((x) => x.installed && x.last_test && !x.last_test.ok).length;
    toast(bad ? `ทดสอบครบแล้ว — ไม่ผ่าน ${bad} บัญชี` : "ทดสอบครบแล้ว — ทุกบัญชีใช้ได้", bad ? "err" : "ok");
  }
  const ago = (ts) => (ts ? new Date(ts * 1000).toLocaleString([], { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" }) : "");
  async function loadModels(a) {
    loadingModels = { ...loadingModels, [a.id]: true };
    try {
      models = { ...models, [a.id]: await api.accountModels(a.id) };
    } catch (e) {
      toast(String(e), "err");
    }
    loadingModels = { ...loadingModels, [a.id]: false };
  }
  function toggleCfg(a) {
    openCfg = { ...openCfg, [a.id]: !openCfg[a.id] };
    if (openCfg[a.id] && !models[a.id]) loadModels(a);
  }
  const SRC = { account: "รายชื่อจากบัญชีนี้โดยตรง", known: "รายชื่อมาตรฐานของผู้ให้บริการ (ยังไม่ได้อ่านจากบัญชี)", history: "รายชื่อมาตรฐาน + โมเดลที่เคยรันบนบัญชีนี้" };
  const pct = (n) => `${Math.round(n)}%`;
  // percent of an hour budget back to hours ("1.2 ชม.")
  const hrs = (p, budget) => `${(Math.round(((p ?? 0) / 100) * budget * 10) / 10).toFixed(1)} ชม.`;
  const manual = (a) => a.model && a.model !== "auto";
  // the effort levels the chosen model accepts, when the account's own list says
  const effortsFor = (a) => {
    const m = models[a.id]?.models?.find((x) => x.id === a.model);
    const list = m?.efforts?.length ? m.efforts : models[a.id]?.efforts?.length ? models[a.id].efforts : EFFORTS;
    return list;
  };
  const byKind = $derived(
    data.kinds.map((k) => {
      const mine = data.accounts.filter((a) => a.kind === k.id);
      return { ...k, count: mine.length, on: mine.filter((a) => a.enabled).length, mine };
    }),
  );
  async function setKind(k, enabled) {
    try {
      for (const a of k.mine) if (a.enabled !== enabled) await api.accountUpdate(a.id, { enabled });
      await load();
    } catch (e) {
      toast(String(e), "err");
    }
  }
  // ---- install a missing CLI (the exact command is shown first; pressing OK is the consent)
  let inst = $state({ kind: "", state: "idle", lines: [], message: "" });
  let registry = $state("");
  async function pollInstall() {
    try {
      inst = await api.cliInstallStatus();
    } catch {}
    if (inst.state === "running") setTimeout(pollInstall, 1000);
    else if (inst.state === "ok" || inst.state === "error") {
      toast(inst.message, inst.state === "ok" ? "ok" : "err");
      await load();
    }
  }
  async function installCli(kind) {
    const cmd = await api.cliInstallCommand(kind);
    const extra = registry.trim() ? ` (ถ้าเครือข่ายเข้าไม่ถึง จะลองอีกครั้งผ่าน ${registry.trim()})` : "";
    if (!(await ask(`จะรันคำสั่งนี้บนเครื่องนี้เพื่อติดตั้งโปรแกรม${extra}`, { title: "ติดตั้งโปรแกรมของค่ายนี้?", ok: "ติดตั้งเลย", code: cmd }))) return;
    try {
      inst = { kind, state: "running", lines: [], message: "" };
      await api.cliInstallStart(kind, registry.trim() || null);
      pollInstall();
    } catch (e) {
      inst = { kind, state: "idle", lines: [], message: "" };
      toast(String(e), "err");
    }
  }
  const installing = $derived(inst.state === "running");
  const AUTH = { login: "ล็อกอินด้วยบัญชีของค่าย", key: "API key / token" };
</script>

<div class="accs">
  <div class="kinds">
    {#each byKind as k (k.id)}
      <div class="kind" class:off={k.count > 0 && k.on === 0} class:missing={!k.installed}>
        <b>{k.label}</b>
        <span class="faint small">
          {#if !k.installed}ยังไม่ได้ติดตั้ง
          {:else if k.count === 0}ยังไม่มีบัญชี
          {:else}เปิดอยู่ {k.on}/{k.count} บัญชี{/if}
        </span>
        {#if k.count > 0}
          {#if k.on > 0}<button class="small" onclick={() => setKind(k, false)} title="ปิดทุกบัญชีของค่ายนี้ชั่วคราว (ไม่ลบ login)">ปิดค่ายนี้</button>
          {:else}<button class="small" onclick={() => setKind(k, true)}>เปิดค่ายนี้</button>{/if}
        {/if}
      </div>
    {/each}
  </div>
  <div class="row wrap small">
    <button class="small primary" disabled={testingAll} onclick={testAll} title="สั่งงานจริงสั้นๆ ทีละบัญชี (ใช้โควต้านิดเดียว) เพื่อยืนยันว่าล็อกอิน/key และโมเดลใช้ได้">{testingAll ? "กำลังทดสอบทีละบัญชี…" : "ทดสอบทุกบัญชี"}</button>
    <span class="faint">ผลทดสอบล่าสุดของแต่ละบัญชีจะแสดงค้างไว้ที่การ์ดของบัญชีนั้น</span>
  </div>
  <p class="faint small">รายการ AI ที่เลือกใช้ได้ในโปรแกรม (ช่องคุยกับ AI, ปุ่มส่งงาน, AI หลัก) จะแสดงเฉพาะค่ายที่ <b>เปิดอยู่</b> ค่ายที่ปิดหรือยังไม่มีบัญชีจะไม่ถูกเสนอ</p>

  {#if data.accounts.some((a) => !a.installed) || inst.state !== "idle"}
    <div class="inst">
      {#if inst.state !== "idle"}
        <div class="small" style="color:{inst.state === 'error' ? 'var(--red)' : inst.state === 'ok' ? 'var(--teal)' : 'inherit'}">
          {inst.state === "running" ? "กำลังติดตั้ง…" : inst.message}
        </div>
        <pre class="log small faint">{inst.lines.slice(-12).join("\n")}</pre>
      {/if}
      <label class="small faint">registry สำรอง (ถ้าเครือข่ายเข้า npm ปกติไม่ได้ — ไม่บังคับ):
        <input bind:value={registry} placeholder="https://registry.npmmirror.com" style="width:260px" /></label>
    </div>
  {/if}

  {#each data.accounts as a (a.id)}
    <div class="acc" class:off={!a.enabled} class:active={a.active}>
      <div class="row wrap">
        <label class="sw" title="เปิด/ปิดบัญชีนี้ชั่วคราว"><input type="checkbox" checked={a.enabled} onchange={(e) => upd(a, { enabled: e.currentTarget.checked })} /></label>
        <b>{a.label || a.id}</b>
        <span class="faint small">({a.id})</span>
        <span class="chip">{a.vendor_label || a.kind_label}</span>
        <span class="chip" title={AUTH[a.auth]}>{#if a.auth === "key"}<Icon name="key" /> key{:else}ล็อกอิน{/if}</span>
        {#if a.isolated && a.auth !== "key"}<span class="chip" title="login แยกในโฟลเดอร์ของบัญชีนี้">แยก login</span>{/if}
        {#if a.active}<span class="chip teal">กำลังใช้งาน</span>{/if}
        {#if store.state?.account_limits?.[a.id]}<span class="chip amber" title={store.state.account_limits[a.id].why}><Icon name="hourglass" /> ติดลิมิต · รอถึง {clock(store.state.account_limits[a.id].until)}</span>{/if}
        {#if !a.installed}<span class="chip amber">ยังไม่ได้ติดตั้ง CLI</span>
          <button class="small primary" disabled={installing} onclick={() => installCli(a.kind)} title="ติดตั้งโปรแกรมของค่ายนี้ให้ (จะแสดงคำสั่งให้ยืนยันก่อน)">{#if !(installing && inst.kind === a.kind)}<Icon name="download" /> {/if}{installing && inst.kind === a.kind ? "กำลังติดตั้ง…" : "ติดตั้ง CLI"}</button>
        {:else if a.auth === "key"}
          {#if a.key_set}<span class="chip teal">มี key แล้ว</span>{:else}<span class="chip amber">ยังไม่มี key</span>{/if}
        {:else if a.logged_in === false}<span class="chip amber">ยังไม่ได้ล็อกอิน</span>
        {:else if a.logged_in}<span class="chip teal">ล็อกอินแล้ว</span>{/if}
        {#if a.last_test}<span class="chip {a.last_test.ok ? 'teal' : 'red'}" title={a.last_test.ok ? `โมเดล: ${a.last_test.model ?? ""}` : (a.last_test.error ?? "")}>{a.last_test.ok ? "✓ ทดสอบผ่าน" : "✗ ทดสอบไม่ผ่าน"} · {ago(a.last_test.ts)}</span>
        {:else if a.installed}<span class="chip">ยังไม่เคยทดสอบ</span>{/if}
        {#if !a.enabled}<span class="chip">ปิดชั่วคราว</span>{/if}
        <span class="grow"></span>
        {#if a.installed && a.auth !== "key"}<button class="small" onclick={() => login(a)} title="เปิดหน้าต่างล็อกอินของ {a.kind_label} (ทำต่อในเบราว์เซอร์) แล้วกด “ทดสอบ” เพื่อยืนยัน">{a.logged_in ? "ล็อกอินใหม่" : "ล็อกอิน"}</button>{/if}
        <button class="small" disabled={testing[a.id] || !a.installed} onclick={() => test(a)} title="ลองสั่งงานจริงสั้นๆ เพื่อตรวจว่าล็อกอิน/key และโมเดลใช้ได้">{testing[a.id] ? "กำลังทดสอบ…" : "ทดสอบ"}</button>
        <button class="small danger" onclick={() => remove(a)} title={a.isolated ? "ยกเลิกการเชื่อมต่อ + ลบ login / key ของบัญชีนี้" : "เอาออกจากโปรแกรม"}>{a.isolated ? "ยกเลิกการเชื่อมต่อ" : "เอาออก"}</button>
      </div>
      {#if tested[a.id]}
        <div class="small" style="color:{tested[a.id].ok ? 'var(--teal)' : 'var(--red)'}">
          <Icon name={tested[a.id].ok ? "check" : "closeCircle"} /> {tested[a.id].ok ? `ใช้ได้ (${tested[a.id].seconds} วินาที · โมเดล ${tested[a.id].model ?? "?"})` : `ไม่สำเร็จ: ${tested[a.id].error ?? tested[a.id].status}`}
          {#each tested[a.id].lines ?? [] as l}<div class="faint"><IconText text={l} /></div>{/each}
        </div>
      {/if}
      {#if a.limit && a.limit.kind !== "window"}
        <!-- only Claude / ChatGPT sign-ins have 5-hour + weekly windows; the others are shown the way they really limit -->
        <div class="small lim">
          <span class="chip">{a.limit.kind === "daily" ? "ลิมิตรายวัน" : "จ่ายตามใช้"}</span>
          <span class="faint">ลิมิตจริงของค่าย:</span> {a.limit.text}
        </div>
        {#if a.limit.balance}<div class="small"><b>{a.limit.balance}</b></div>{/if}
        {#if a.limit.reset}<div class="small faint">เริ่มนับรายวันใหม่: {clock(a.limit.reset)}</div>{/if}
        {#if a.limit.kind === "paid" && a.quota && a.budget}
          <div class="row small q">
            <span class="faint" title="ค่ายนี้ไม่มีรอบสัปดาห์ — นี่คือเพดานของแอปเองกันใช้เงินเกิน (ปรับได้ที่ ตั้งค่า → เวลา/โควต้า)">เพดานของแอป (กันใช้เงินเกิน)</span>
            <span>สัปดาห์นี้ <b>{hrs(a.quota.week, a.budget.week_h)}</b> / {a.budget.week_h} ชม.</span><div class="bar"><i class="w" style="width:{Math.min(100, a.quota.week)}%"></i></div>
          </div>
        {:else if a.limit.kind === "daily"}
          <div class="small faint">แอปไม่ตั้งเพดานชั่วโมงเอง — บอทใช้ได้จนค่ายแจ้งว่าครบลิมิต แล้วรอถึงเวลาเริ่มนับใหม่ก่อนใช้ต่อ</div>
        {/if}
      {:else if a.quota}
        <div class="row small q">
          <span class="faint">{a.real_quota ? "โควต้าจริง" : "งบชั่วโมงของโปรแกรม"}</span>
          <span>5 ชม. <b>{pct(a.quota.five)}</b></span><div class="bar"><i style="width:{Math.min(100, a.quota.five)}%"></i></div>
          <span>สัปดาห์ <b>{pct(a.quota.week)}</b></span><div class="bar"><i class="w" style="width:{Math.min(100, a.quota.week)}%"></i></div>
        </div>
        {#if !a.real_quota && a.budget}
          <div class="small faint">ยังอ่านโควต้าจริงไม่ได้ (Codex ต้องรันก่อนหนึ่งครั้ง) — ระหว่างนี้นับเวลาที่บอทรันด้วยบัญชีนี้: {hrs(a.quota.five, a.budget.five_h)} จากงบ {a.budget.five_h} ชม. ใน 5 ชม.ล่าสุด · {hrs(a.quota.week, a.budget.week_h)} จากงบ {a.budget.week_h} ชม./สัปดาห์</div>
        {/if}
      {/if}
      {#if a.omni}
        <!-- what OmniRoute itself knows: the quota left at each provider behind it, connections that need the owner -->
        {#if a.omni.error}
          <div class="small faint">OmniRoute: {a.omni.error}</div>
        {:else}
          {#if a.omni.quota?.length}
            <div class="small omni">
              <span class="faint">โควต้าที่เหลือในแต่ละค่าย (จาก OmniRoute):</span>
              {#each a.omni.quota as q}
                <span class="chip" title={q.name}>{q.provider} <b>{q.pct == null ? "?" : `${Math.round(q.pct)}%`}</b>{#if q.total} ({q.used ?? 0}/{q.total}){/if}{#if q.reset} · ใหม่ {clock(Date.parse(q.reset) / 1000)}{/if}</span>
              {/each}
            </div>
          {/if}
          {#each a.omni.warnings ?? [] as w}<div class="small warn">⚠️ {w}</div>{/each}
          {#if a.omni.block}
            <div class="small block">
              ⛔ {a.omni.block}
              <div class="row"><button class="small" onclick={() => omniDash(a)}>เปิด dashboard ของ OmniRoute</button><button class="small" onclick={() => omniDup(a, true)}>เป็นคนละบัญชี — ใช้ต่อ</button></div>
            </div>
          {:else if a.omni.dup_ok}
            <div class="small faint"><button class="small" onclick={() => omniDup(a, false)}>ยกเลิก “เป็นคนละบัญชี”</button></div>
          {/if}
        {/if}
      {/if}
      {#if a.check}
        <!-- "check now": how far it got, what is being tried, or why it waits -->
        {@const c = a.check}
        <div class="small chk">
          <div class="row">
            <b>{c.finished ? `ตรวจครบ ${c.total} โมเดลแล้ว` : `ตรวจโมเดล ${c.done}/${c.total}`}</b>
            <div class="bar"><i style="width:{c.total ? (c.done / c.total) * 100 : 0}%"></i></div>
            <span class="faint">ผ่าน {c.bot} · แชตอย่างเดียว {c.chat}{c.unknown ? ` · ตรวจไม่ได้ ${c.unknown}` : ""}</span>
          </div>
          {#if c.current}
            <div>🔎 กำลังลอง <b>{c.current}</b> · {mins(Math.max(0, Math.round(Date.now() / 1000 - c.current_since)))} (ไม่เกิน 4 นาทีต่อตัว)</div>
          {:else if c.waiting}
            <div class="faint">⏸ {c.waiting}</div>
          {/if}
          {#if c.last}<div class="faint">ล่าสุด: {c.last.model} — {VERDICT[c.last.v] ?? c.last.v}{c.last.note ? ` (${c.last.note})` : ""}</div>{/if}
          {#if !c.finished && c.total > c.done}<div class="faint">เหลือ {c.total - c.done} ตัว · ใช้เวลาอีกไม่เกิน ~{(c.total - c.done) * 4} นาที</div>{/if}
        </div>
      {/if}
      {#if a.enabled && a.allowed === false && a.reason}<div class="small faint">ตอนนี้บอทรอ: {a.reason}</div>{/if}
      {#if a.note}<div class="small faint">{a.note}</div>{/if}

      <div class="row wrap small cfg">
        <span class="faint">โมเดล</span>
        <b>{manual(a) ? a.model : "auto"}</b>
        <span class="faint">· effort</span>
        <b>{a.effort === "auto" ? "auto" : a.effort}</b>
        <button class="small" onclick={() => toggleCfg(a)}>{openCfg[a.id] ? "ซ่อนการตั้งค่า" : "เลือกโมเดล / effort"}</button>
        <label>ชื่อเรียก <input style="width:150px" value={a.label} placeholder={a.id} onchange={(e) => upd(a, { label: e.currentTarget.value })} /></label>
      </div>

      {#if openCfg[a.id]}
        <div class="panel small">
          <div class="row wrap">
            <label>โหมด
              <select value={manual(a) ? "manual" : "auto"} onchange={(e) => upd(a, { model: e.currentTarget.value === "auto" ? "auto" : (a.model !== "auto" ? a.model : (models[a.id]?.models?.[0]?.id ?? "strong")) })} disabled={a.needs_model}>
                <option value="auto">auto (โปรแกรมเลือกให้)</option><option value="manual">manual</option>
              </select></label>
            {#if a.needs_model}<span class="faint">ผู้ให้บริการนี้ต้องระบุชื่อโมเดลเอง</span>{/if}
            <button class="small" disabled={loadingModels[a.id]} onclick={() => loadModels(a)}>{loadingModels[a.id] ? "กำลังอ่าน…" : "อ่านรายชื่อโมเดลใหม่"}</button>
          </div>
          {#if manual(a) || a.needs_model}
            <div class="row wrap">
              <span>โมเดล</span>
              <ModelPicker value={a.model === "auto" ? "" : a.model} models={models[a.id]?.models ?? []} hidden={models[a.id]?.hidden ?? []} width={340} placeholder="ค้นหา หรือพิมพ์ชื่อโมเดล เช่น fast / strong หรือชื่อจริง" onchange={(v) => upd(a, { model: v || "auto" })} />
            </div>
          {/if}
          {#if models[a.id]}<div class="faint">{SRC[models[a.id].source] ?? ""}{models[a.id].models.length ? ` · ${models[a.id].models.length} โมเดลที่ทำงานบอทได้` : " · ยังไม่มีโมเดลที่ผ่านการตรวจ"}</div>
            {#if models[a.id].hidden?.length}
              {@const hc = models[a.id].hidden_counts ?? {}}
              <div class="faint">ซ่อนไว้ {models[a.id].hidden.length} ตัว:{hc.chat ? ` ${hc.chat} แชตได้อย่างเดียว` : ""}{hc.pending ? ` · ${hc.pending} รอตรวจ` : ""}{hc.unknown ? ` · ${hc.unknown} ตรวจไม่ได้ตอนนี้ (เช่น เครดิตไม่พอ)` : ""}
                {#if models[a.id].checking && !a.check}<span> · กำลังตรวจในพื้นหลัง…</span>{/if}
                {#if hc.pending || hc.unknown}<button class="small" onclick={() => checkNow(a)}>ตรวจที่เหลือตอนนี้</button>{/if}</div>
            {/if}
            {#if manual(a) && models[a.id].source === "account" && models[a.id].models.length && !models[a.id].models.some((m) => m.id === a.model)}<div style="color:var(--amber)">⚠ โมเดลที่ตั้งไว้ “{a.model}” ไม่อยู่ในรายชื่อของบัญชีนี้ — ตอนรันจะใช้ตัวที่บัญชีรันได้แทน เลือกใหม่จากรายการได้</div>{/if}
            {#if models[a.id].problem}<div style="color:var(--amber)">⚠ อ่านรายชื่อจากบัญชีไม่สำเร็จ: {models[a.id].problem}</div>{/if}{/if}
          <div class="row wrap">
            <label>Effort
              <select value={a.effort} disabled={!a.effort_supported} onchange={(e) => upd(a, { effort: e.currentTarget.value })}>
                <option value="auto">auto (โปรแกรมเลือกให้)</option>
                {#each effortsFor(a) as x}<option value={x}>{x}</option>{/each}
              </select></label>
            {#if !a.effort_supported}<span class="faint">AI ตัวนี้ปรับ effort ไม่ได้</span>
            {:else if manual(a) && models[a.id]?.models?.find((m) => m.id === a.model)?.efforts?.length}<span class="faint">ระดับที่โมเดลนี้รองรับ: {models[a.id].models.find((m) => m.id === a.model).efforts.join(", ")}</span>{/if}
          </div>
        </div>
      {/if}

      {#if a.auth === "key"}
        <div class="row wrap small keyrow">
          <input type="password" autocomplete="off" style="width:280px" placeholder={a.key_set ? "ใส่ key ใหม่เพื่อเปลี่ยน (key เดิมไม่แสดง)" : "วาง API key / token"} bind:value={keyEdit[a.id]} />
          <button class="small" disabled={!keyEdit[a.id]} onclick={() => saveKey(a)}>บันทึก key</button>
          {#if a.key_set}<button class="small danger" onclick={() => clearKey(a)}>ลบ key</button>{/if}
          {#if a.vendor === "custom"}<span class="faint">API: {a.base_url || "—"}</span>{/if}
          {#if a.vendor === "omniroute"}<span class="faint">{a.key_set ? "" : "ไม่มี key ก็ใช้ได้ (แอปอ่านรายชื่อโมเดลจาก OmniRoute ในเครื่องเอง) — ใส่ key เพื่อคัดโมเดลที่เรียกเครื่องมือไม่ได้ออกก่อน · "}</span><button class="small" onclick={() => omniDash(a)} title="สร้าง key ที่ API Manager · เชื่อมค่ายต่างๆ · ดูโควต้าฟรีที่เหลือ">เปิด dashboard ของ OmniRoute</button>{/if}
        </div>
      {/if}
    </div>
  {:else}
    <div class="faint small">ยังไม่มีบัญชี — ติดตั้ง AI อย่างน้อยหนึ่งตัว หรือเพิ่มด้านล่าง</div>
  {/each}

  <div class="addbox">
    <b>＋ เพิ่มบัญชี</b>
    <div class="row wrap">
      <label>ผู้ให้บริการ
        <select bind:value={vendorId}>
          {#each data.vendors as v}<option value={v.id}>{v.label}{v.installed ? "" : " (ยังไม่ได้ติดตั้งโปรแกรม)"}</option>{/each}
        </select></label>
      <input bind:value={newId} placeholder="ชื่อเรียก เช่น {vendorId}01 (เว้นว่าง = ตั้งให้)" style="width:240px" />
    </div>
    {#if vendor}
      <div class="small faint">{vendor.note}</div>
      <div class="row wrap">
        <span class="faint small">เข้าสู่ระบบด้วย</span>
        {#each vendor.auth as m}
          <label class="small"><input type="radio" name="auth" value={m} bind:group={authSel} /> {AUTH[m]}</label>
        {/each}
      </div>
      {#if authSel === "key"}
        <div class="row wrap">
          <input type="password" autocomplete="off" bind:value={keyIn} placeholder={vendorId === "omniroute" ? "key ของ OmniRoute (ไม่บังคับ — ใส่เพื่อคัดโมเดลที่เรียกเครื่องมือไม่ได้ออกก่อน)" : "วาง API key / token (เก็บในโฟลเดอร์ของบัญชีนี้ ไม่ส่งไปที่อื่น)"} style="width:360px" />
          {#if vendor.runner === "claude" && vendor.id !== "anthropic"}
            <input bind:value={baseIn} placeholder={vendor.base_url || "ที่อยู่ API (base URL) เช่น https://…/anthropic"} style="width:300px" />
          {/if}
          {#if vendor.needs_model}
            <ModelPicker bind:value={modelIn} models={vendor.models.map((m) => ({ id: m, label: m, free: /:free$/.test(m) }))} onchange={(v) => (modelIn = v)} width={260} />
          {/if}
        </div>
      {:else}
        <label class="small"><input type="checkbox" bind:checked={isolated} /> แยก login (สำหรับบัญชีที่ต่างจากที่ล็อกอินไว้ในเครื่อง)</label>
      {/if}
      {#if !vendor.installed}
        <div class="small faint">ยังไม่ได้ติดตั้งโปรแกรมที่ใช้กับผู้ให้บริการนี้ในเครื่อง — ติดตั้งตามเอกสารของผู้ให้บริการ เช่น <code>{vendor.install}</code> แล้วเปิดโปรแกรมใหม่
          <button class="small primary" disabled={installing} onclick={() => installCli(vendor.runner)}>{#if !(installing && inst.kind === vendor.runner)}<Icon name="download" /> {/if}{installing && inst.kind === vendor.runner ? "กำลังติดตั้ง…" : "ติดตั้ง CLI"}</button></div>
      {/if}
    {/if}
    <div><button class="primary small" onclick={add} disabled={busy}>＋ เพิ่มบัญชี</button></div>
  </div>

  <p class="faint small">
    บอทใช้บัญชีแรกในรายการที่เปิดอยู่และโควต้าของมันยังพอ ถ้าเต็มจะสลับไปบัญชีถัดไปเอง · บัญชีที่ใช้ key หรือแยก login เป็นบัญชีสำหรับบอทโดยเฉพาะ ไม่แบ่งโควต้ากับการทำงานของคุณ จึงไม่ต้องกันโควต้าไว้ให้คุณและทำงานในเวลางานได้ ·
    “ปิดชั่วคราว” คือพักไว้ แล้วเปิดใหม่ได้ · “ยกเลิกการเชื่อมต่อ” ลบ login / key ของบัญชีแยกออกจากเครื่องนี้ (login ปกติของ AI ในเครื่องไม่ถูกแตะ) ·
    key เก็บเป็นไฟล์ในโฟลเดอร์ของบัญชีนี้ภายใต้โฟลเดอร์ข้อมูลของโปรแกรม ไม่ถูกแสดงกลับและไม่ถูกส่งไปที่ใดนอกจากผู้ให้บริการที่คุณเลือก
  </p>
</div>

<style>
  .accs { display: flex; flex-direction: column; gap: 10px; }
  .kinds { display: flex; flex-wrap: wrap; gap: 8px; }
  .kind { display: flex; align-items: center; gap: 8px; border: 1px solid var(--line); border-radius: 999px; padding: 4px 12px; background: var(--bg-2); }
  .kind.off { opacity: 0.55; }
  .kind.missing { border-style: dashed; }
  .acc { border: 1px solid var(--line); border-radius: 10px; padding: 10px 12px; display: flex; flex-direction: column; gap: 6px; background: var(--bg-2); }
  .acc.active { border-color: var(--teal); }
  .acc.off { opacity: 0.6; }
  .sw input { width: 18px; height: 18px; }
  .q { align-items: center; gap: 8px; }
  .bar { width: 90px; height: 6px; background: var(--card-2); border-radius: 6px; overflow: hidden; }
  .bar i { display: block; height: 100%; background: var(--teal); }
  .bar i.w { background: var(--blue, var(--teal)); }
  .cfg { gap: 10px; align-items: center; }
  .cfg label, .panel label { display: inline-flex; gap: 6px; align-items: center; }
  .panel { display: flex; flex-direction: column; gap: 8px; background: var(--card); border-radius: 8px; padding: 10px; }
  .keyrow { gap: 8px; align-items: center; }
  .addbox { display: flex; flex-direction: column; gap: 8px; border: 1px dashed var(--line); border-radius: 10px; padding: 12px; }
  .chip.teal { color: var(--teal); }
  .chip.amber { color: var(--amber); }
  .chip.red { color: var(--red); }
  .danger { color: var(--red); }
  .faint { color: var(--text-3); }
  .inst { display: flex; flex-direction: column; gap: 6px; margin: 8px 0; }
  .log { margin: 0; max-height: 140px; overflow: auto; white-space: pre-wrap; background: var(--bg-2); border-radius: 6px; padding: 6px 8px; }
  .omni { display: flex; flex-wrap: wrap; gap: 4px; align-items: center; margin-top: 4px; }
  .warn { color: var(--amber, #b7791f); margin-top: 4px; }
  .block { color: var(--red); margin-top: 4px; border: 1px solid var(--red); border-radius: 6px; padding: 6px 8px; }
  .block .row { margin-top: 6px; gap: 6px; }
  .chk { margin-top: 6px; padding: 6px 8px; border: 1px solid var(--line); border-radius: 6px; display: flex; flex-direction: column; gap: 3px; }
  .chk .row { gap: 8px; align-items: center; flex-wrap: wrap; }
  .chk .bar { width: 140px; }
</style>
