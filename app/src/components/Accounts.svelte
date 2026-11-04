<script>
  // AI accounts: any provider of the catalog, signed in with the agent's own login or with a key / token, each with its
  // own name, on/off switch, model + effort (auto, or chosen from what that account can use) and quota. The bot uses
  // the first one switched on that its own quota allows.
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";
  import { toast } from "../lib/store.svelte.js";
  import { ask } from "../lib/dialog.svelte.js";
  import ModelPicker from "./ModelPicker.svelte";

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

  async function load() {
    try {
      data = await api.accounts();
    } catch {}
  }
  onMount(() => {
    load();
    const t = setInterval(load, 8000);
    return () => clearInterval(t);
  });

  const vendor = $derived(data.vendors.find((v) => v.id === vendorId));
  $effect(() => {
    // keep the sign-in method valid for the chosen provider
    if (vendor && !vendor.auth.includes(authSel)) authSel = vendor.auth[0];
    if (vendor) modelIn = vendor.models[0] ?? "";
  });

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
  }
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
        <span class="chip" title={AUTH[a.auth]}>{a.auth === "key" ? "🔑 key" : "ล็อกอิน"}</span>
        {#if a.isolated && a.auth !== "key"}<span class="chip" title="login แยกในโฟลเดอร์ของบัญชีนี้">แยก login</span>{/if}
        {#if a.active}<span class="chip teal">กำลังใช้งาน</span>{/if}
        {#if !a.installed}<span class="chip amber">ยังไม่ได้ติดตั้ง CLI</span>
          <button class="small primary" disabled={installing} onclick={() => installCli(a.kind)} title="ติดตั้งโปรแกรมของค่ายนี้ให้ (จะแสดงคำสั่งให้ยืนยันก่อน)">{installing && inst.kind === a.kind ? "กำลังติดตั้ง…" : "⬇ ติดตั้ง CLI"}</button>
        {:else if a.auth === "key"}
          {#if a.key_set}<span class="chip teal">มี key แล้ว</span>{:else}<span class="chip amber">ยังไม่มี key</span>{/if}
        {:else if a.logged_in === false}<span class="chip amber">ยังไม่ได้ล็อกอิน</span>
        {:else if a.logged_in}<span class="chip teal">ล็อกอินแล้ว</span>{/if}
        {#if !a.enabled}<span class="chip">ปิดชั่วคราว</span>{/if}
        <span class="grow"></span>
        {#if a.installed && a.auth !== "key" && (a.isolated || a.logged_in === false)}<button class="small" onclick={() => login(a)}>ล็อกอิน</button>{/if}
        <button class="small" disabled={testing[a.id] || !a.installed} onclick={() => test(a)} title="ลองสั่งงานจริงสั้นๆ เพื่อตรวจว่าล็อกอิน/key และโมเดลใช้ได้">{testing[a.id] ? "กำลังทดสอบ…" : "ทดสอบ"}</button>
        <button class="small danger" onclick={() => remove(a)} title={a.isolated ? "ยกเลิกการเชื่อมต่อ + ลบ login / key ของบัญชีนี้" : "เอาออกจากโปรแกรม"}>{a.isolated ? "ยกเลิกการเชื่อมต่อ" : "เอาออก"}</button>
      </div>
      {#if tested[a.id]}
        <div class="small" style="color:{tested[a.id].ok ? 'var(--teal)' : 'var(--red)'}">
          {tested[a.id].ok ? `✓ ใช้ได้ (${tested[a.id].seconds} วินาที)` : `✗ ไม่สำเร็จ: ${tested[a.id].error ?? tested[a.id].status}`}
          {#each tested[a.id].lines ?? [] as l}<div class="faint">{l}</div>{/each}
        </div>
      {/if}
      {#if a.quota}
        <div class="row small q">
          <span class="faint">{a.real_quota ? "โควต้าจริง" : "งบชั่วโมงของโปรแกรม"}</span>
          <span>5 ชม. <b>{pct(a.quota.five)}</b></span><div class="bar"><i style="width:{Math.min(100, a.quota.five)}%"></i></div>
          <span>สัปดาห์ <b>{pct(a.quota.week)}</b></span><div class="bar"><i class="w" style="width:{Math.min(100, a.quota.week)}%"></i></div>
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
              <ModelPicker value={a.model === "auto" ? "" : a.model} models={models[a.id]?.models ?? []} width={340} placeholder="ค้นหา หรือพิมพ์ชื่อโมเดล เช่น fast / strong หรือชื่อจริง" onchange={(v) => upd(a, { model: v || "auto" })} />
            </div>
          {/if}
          {#if models[a.id]}<div class="faint">{SRC[models[a.id].source] ?? ""}{models[a.id].models.length ? ` · ${models[a.id].models.length} โมเดล` : " · ไม่พบรายชื่อ ใช้พิมพ์ชื่อเองได้"}</div>{/if}
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
          <input type="password" autocomplete="off" bind:value={keyIn} placeholder="วาง API key / token (เก็บในโฟลเดอร์ของบัญชีนี้ ไม่ส่งไปที่อื่น)" style="width:360px" />
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
          <button class="small primary" disabled={installing} onclick={() => installCli(vendor.runner)}>{installing && inst.kind === vendor.runner ? "กำลังติดตั้ง…" : "⬇ ติดตั้ง CLI"}</button></div>
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
  .danger { color: var(--red); }
  .faint { color: var(--text-3); }
  .inst { display: flex; flex-direction: column; gap: 6px; margin: 8px 0; }
  .log { margin: 0; max-height: 140px; overflow: auto; white-space: pre-wrap; background: var(--bg-2); border-radius: 6px; padding: 6px 8px; }
</style>
