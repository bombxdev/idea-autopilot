<script>
  // Usage report: which models did the work, how many tokens / runs / hours each took, over a day, week, month, year or
  // any range, with filters (agent, model, idea, stage, set) and PNG / PDF / CSV export. Aggregation happens here, on
  // the runs the backend returns for the period, so every filter answers instantly.
  import { onMount } from "svelte";
  import Icon from "../components/Icon.svelte";
  import { api } from "../lib/api.js";
  import { toast, stageLabel } from "../lib/store.svelte.js";

  // ---------- period ----------
  const PERIODS = [["day", "รายวัน"], ["week", "รายสัปดาห์"], ["month", "รายเดือน"], ["year", "รายปี"], ["range", "ช่วงเวลา"]];
  let period = $state("week");
  let anchor = $state(new Date());
  const iso = (d) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  let rangeFrom = $state(iso(new Date(Date.now() - 29 * 864e5)));
  let rangeTo = $state(iso(new Date()));

  const startOf = (d, p) => {
    const x = new Date(d.getFullYear(), d.getMonth(), d.getDate());
    if (p === "week") x.setDate(x.getDate() - ((x.getDay() + 6) % 7)); // Monday
    if (p === "month") x.setDate(1);
    if (p === "year") { x.setMonth(0); x.setDate(1); }
    return x;
  };
  const add = (d, p, n) => {
    const x = new Date(d);
    if (p === "day") x.setDate(x.getDate() + n);
    else if (p === "week") x.setDate(x.getDate() + 7 * n);
    else if (p === "month") x.setMonth(x.getMonth() + n);
    else if (p === "year") x.setFullYear(x.getFullYear() + n);
    return x;
  };
  const span = $derived.by(() => {
    if (period === "range") {
      const a = new Date(rangeFrom + "T00:00"), b = new Date(rangeTo + "T00:00");
      const [f, t] = a <= b ? [a, b] : [b, a];
      const to = new Date(t); to.setDate(to.getDate() + 1);
      return { from: f, to };
    }
    const from = startOf(anchor, period);
    return { from, to: add(from, period, 1) };
  });
  const prevSpan = $derived({ from: new Date(span.from.getTime() - (span.to - span.from)), to: span.from });
  const MONTHS = ["ม.ค.", "ก.พ.", "มี.ค.", "เม.ย.", "พ.ค.", "มิ.ย.", "ก.ค.", "ส.ค.", "ก.ย.", "ต.ค.", "พ.ย.", "ธ.ค."];
  const dshort = (d) => `${d.getDate()} ${MONTHS[d.getMonth()]}`;
  const spanLabel = $derived.by(() => {
    const { from, to } = span, last = new Date(to.getTime() - 1);
    if (period === "day") return `${dshort(from)} ${from.getFullYear()}`;
    if (period === "month") return `${MONTHS[from.getMonth()]} ${from.getFullYear()}`;
    if (period === "year") return `ปี ${from.getFullYear()}`;
    return `${dshort(from)} – ${dshort(last)} ${last.getFullYear()}`;
  });
  function shift(n) { anchor = add(startOf(anchor, period), period, n); }

  // ---------- data ----------
  let runs = $state([]);
  let first = $state(null);
  let loading = $state(false);
  async function load() {
    loading = true;
    try {
      const r = await api.usageRuns(Math.floor(prevSpan.from / 1000), Math.floor(span.to / 1000));
      runs = r?.runs ?? [];
      first = r?.first ?? null;
    } catch (e) {
      toast(String(e), "err");
    } finally {
      loading = false;
    }
  }
  $effect(() => { span.from; span.to; load(); });

  // ---------- filters ----------
  const GROUPS = [["model", "โมเดล"], ["agent", "AI / บัญชี"], ["idea", "ไอเดีย"], ["stage", "ขั้นงาน"], ["source", "บอท / แชต"]];
  const METRICS = [
    ["tokens", "token ทั้งหมด"], ["io", "token ไม่รวม cache"], ["runs", "จำนวนรอบ"], ["hours", "เวลา (ชม.)"], ["cost", "มูลค่าเทียบราคา API"],
  ];
  let groupBy = $state("model");
  let metric = $state("tokens");
  let off = $state({ agent: [], model: [], stage: [], scope: [], source: [] }); // values switched off
  let ideaPick = $state([]); // idea ids chosen in the idea filter
  let ideaMode = $state("exclude"); // exclude = hide the chosen ideas · only = show only them
  let ideaOpen = $state(false);
  let okOnly = $state(false);
  let sortKey = $state("val");
  let sortDesc = $state(true);

  const agentOf = (r) => (r.account ? `${r.provider} · ${r.account}` : r.provider);
  const total = (r) => r.input + r.output + r.cache_read + r.cache_write;
  const isChat = (r) => r.source === "chat";
  // a chat has no runs of its own: "runs" counts the bot's runs, chats are counted in messages
  const val = (r, m = metric) => (m === "tokens" ? total(r) : m === "io" ? r.input + r.output : m === "runs" ? (isChat(r) ? 0 : 1) : m === "hours" ? r.secs / 3600 : r.cost);
  const stageName = (st) => (st === "chat" ? "แชต" : stageLabel(st) || st);
  const keyOf = (r, g = groupBy) =>
    g === "model" ? r.model : g === "agent" ? agentOf(r) : g === "idea" ? `#${r.idea_id} ${r.idea}` : g === "source" ? (isChat(r) ? "แชตกับ AI" : "งานของบอท") : stageName(r.stage);

  const inSpan = (r, s) => r.started * 1000 >= s.from.getTime() && r.started * 1000 < s.to.getTime();
  const pass = (r) =>
    !off.agent.includes(agentOf(r)) && !off.model.includes(r.model) && !off.stage.includes(r.stage) && !off.scope.includes(r.scope || "personal") && !off.source.includes(r.source || "bot") &&
    (!okOnly || r.status === "ok") &&
    (!ideaPick.length || (ideaMode === "only" ? ideaPick.includes(r.idea_id) : !ideaPick.includes(r.idea_id)));
  const cur = $derived(runs.filter((r) => inSpan(r, span) && pass(r)));
  const prev = $derived(runs.filter((r) => inSpan(r, prevSpan) && pass(r)));
  const periodRuns = $derived(runs.filter((r) => inSpan(r, span)));

  const uniq = (xs) => [...new Set(xs)].sort((a, b) => String(a).localeCompare(String(b)));
  const optAgents = $derived(uniq(periodRuns.map(agentOf)));
  const optModels = $derived(uniq(periodRuns.map((r) => r.model)));
  const optStages = $derived(uniq(periodRuns.map((r) => r.stage)));
  const optIdeas = $derived.by(() => {
    const m = new Map();
    for (const r of periodRuns) m.set(r.idea_id, { id: r.idea_id, title: r.idea || `#${r.idea_id}`, n: (m.get(r.idea_id)?.n ?? 0) + total(r) });
    return [...m.values()].sort((a, b) => b.n - a.n);
  });
  function toggle(kind, v) {
    const s = off[kind];
    off = { ...off, [kind]: s.includes(v) ? s.filter((x) => x !== v) : [...s, v] };
  }
  function pickIdea(id) { ideaPick = ideaPick.includes(id) ? ideaPick.filter((x) => x !== id) : [...ideaPick, id]; }
  function clearFilters() { off = { agent: [], model: [], stage: [], scope: [], source: [] }; ideaPick = []; okOnly = false; }
  const filtered = $derived(off.agent.length + off.model.length + off.stage.length + off.scope.length + off.source.length + ideaPick.length + (okOnly ? 1 : 0));
  const hasChats = $derived(periodRuns.some(isChat));

  // ---------- aggregation ----------
  function sum(list) {
    const s = { runs: 0, ok: 0, msgs: 0, botTokens: 0, input: 0, output: 0, cache_read: 0, cache_write: 0, secs: 0, cost: 0, reported: 0 };
    for (const r of list) {
      if (isChat(r)) s.msgs += r.msgs || 0;
      else { s.runs += 1; s.ok += r.status === "ok" ? 1 : 0; s.botTokens += total(r); }
      s.input += r.input; s.output += r.output; s.cache_read += r.cache_read; s.cache_write += r.cache_write;
      s.secs += r.secs; s.cost += r.cost; s.reported += total(r) > 0 ? 1 : 0;
    }
    s.tokens = s.input + s.output + s.cache_read + s.cache_write;
    s.io = s.input + s.output;
    s.hours = s.secs / 3600;
    return s;
  }
  const K = $derived(sum(cur));
  const P = $derived(sum(prev));
  const groups = $derived.by(() => {
    const m = new Map();
    for (const r of cur) { const k = keyOf(r); if (!m.has(k)) m.set(k, []); m.get(k).push(r); }
    const all = cur.reduce((a, r) => a + val(r), 0);
    return [...m.entries()].map(([k, list]) => {
      const s = sum(list);
      const v = list.reduce((a, r) => a + val(r), 0);
      return { key: k, ...s, val: v, share: all > 0 ? v / all : 0, provider: list[0].provider, noTokens: s.reported === 0 };
    });
  });
  const sorted = $derived([...groups].sort((a, b) => {
    const x = a[sortKey], y = b[sortKey];
    const c = typeof x === "string" ? x.localeCompare(y) : x - y;
    return sortDesc ? -c : c;
  }));
  const byVal = $derived([...groups].sort((a, b) => b.val - a.val));
  function sortBy(k) { if (sortKey === k) sortDesc = !sortDesc; else { sortKey = k; sortDesc = true; } }

  // colour follows the entity: slots are handed out by name over the whole period, so a filter never repaints a series
  const SLOTS = 8;
  const colorKeys = $derived(uniq(periodRuns.map((r) => keyOf(r))));
  const colorOf = (k) => (k === "อื่นๆ" ? "var(--u-other)" : `var(--u${(colorKeys.indexOf(k) % SLOTS) + 1})`);
  const TOP = 7;
  const shown = $derived(byVal.slice(0, TOP).map((g) => g.key));

  // ---------- time buckets ----------
  const buckets = $derived.by(() => {
    const { from, to } = span;
    const days = (to - from) / 864e5;
    const unit = period === "day" ? "hour" : period === "year" || days > 186 ? "month" : days > 62 ? "week" : "day";
    const out = [];
    let x = new Date(from);
    if (unit === "week") x = startOf(x, "week");
    while (x < to) {
      const n = new Date(x);
      if (unit === "hour") n.setHours(n.getHours() + 1);
      else if (unit === "day") n.setDate(n.getDate() + 1);
      else if (unit === "week") n.setDate(n.getDate() + 7);
      else n.setMonth(n.getMonth() + 1);
      const label = unit === "hour" ? `${x.getHours()}` : unit === "month" ? MONTHS[x.getMonth()] : `${x.getDate()}${x.getDate() === 1 || unit === "week" ? " " + MONTHS[x.getMonth()] : ""}`;
      out.push({ from: x.getTime(), to: n.getTime(), label, full: unit === "hour" ? `${dshort(x)} ${x.getHours()}:00` : unit === "month" ? `${MONTHS[x.getMonth()]} ${x.getFullYear()}` : unit === "week" ? `สัปดาห์ของ ${dshort(x)}` : dshort(x) });
      x = n;
    }
    for (const b of out) {
      b.parts = {};
      b.total = 0;
      for (const r of cur) {
        const t = r.started * 1000;
        if (t < b.from || t >= b.to) continue;
        const k = shown.includes(keyOf(r)) ? keyOf(r) : "อื่นๆ";
        const v = val(r);
        b.parts[k] = (b.parts[k] ?? 0) + v;
        b.total += v;
      }
    }
    return { unit, list: out };
  });
  const stackKeys = $derived([...shown, ...(byVal.length > TOP ? ["อื่นๆ"] : [])]);
  const maxBucket = $derived(Math.max(0, ...buckets.list.map((b) => b.total)));
  const niceMax = $derived.by(() => {
    if (maxBucket <= 0) return 1;
    const p = Math.pow(10, Math.floor(Math.log10(maxBucket)));
    for (const m of [1, 2, 2.5, 5, 10]) if (m * p >= maxBucket) return m * p;
    return 10 * p;
  });

  // ---------- formatting ----------
  const fmt = (n, m = metric) => {
    if (m === "runs") return Math.round(n).toLocaleString();
    if (m === "hours") return n < 10 ? n.toFixed(1) : Math.round(n).toLocaleString();
    if (m === "cost") return "$" + (n < 100 ? n.toFixed(2) : Math.round(n).toLocaleString());
    return tok(n);
  };
  const tok = (n) => (n >= 1e9 ? (n / 1e9).toFixed(2) + "B" : n >= 1e6 ? (n / 1e6).toFixed(n >= 1e8 ? 0 : 1) + "M" : n >= 1e3 ? (n / 1e3).toFixed(n >= 1e5 ? 0 : 1) + "K" : String(Math.round(n)));
  const pctf = (x) => (x >= 0.995 ? "100%" : x >= 0.1 ? Math.round(x * 100) + "%" : (x * 100).toFixed(1) + "%");
  const delta = (a, b) => (b > 0 ? (a - b) / b : null);
  const metricLabel = $derived(METRICS.find((m) => m[0] === metric)[1]);
  const cacheShare = $derived(K.input + K.cache_read + K.cache_write > 0 ? K.cache_read / (K.input + K.cache_read + K.cache_write) : 0);
  const perOk = $derived(K.ok > 0 ? K.botTokens / K.ok : 0);
  const silent = $derived(groups.filter((g) => g.noTokens && g.runs > 0));

  // short, rule-based notes for managing tokens (no AI involved)
  const notes = $derived.by(() => {
    const out = [];
    const top = [...groups].filter((g) => !g.noTokens).sort((a, b) => b.tokens - a.tokens)[0];
    if (top && K.tokens > 0) out.push(`${top.key} ใช้ token มากที่สุด ${pctf(top.tokens / K.tokens)} ของทั้งหมด${top.runs ? ` จาก ${top.runs} รอบ (สำเร็จ ${pctf(top.ok / top.runs)})` : ""}${top.msgs ? ` และแชต ${top.msgs} ข้อความ` : ""}`);
    const chatTok = cur.filter(isChat).reduce((a, r) => a + total(r), 0);
    if (chatTok > 0 && K.tokens > 0) out.push(`แชตกับ AI ใช้ ${pctf(chatTok / K.tokens)} ของ token ทั้งหมด (${tok(chatTok)}) ที่เหลือเป็นงานของบอท`);
    const weak = groups.filter((g) => g.runs >= 3 && g.ok / g.runs < 0.6 && !g.noTokens && g.tokens / Math.max(1, K.tokens) > 0.1);
    for (const g of weak.slice(0, 2)) out.push(`${g.key} สำเร็จแค่ ${pctf(g.ok / g.runs)} แต่กิน token ${pctf(g.tokens / K.tokens)} — ลองดู log ว่าล้มเพราะอะไร หรือลดให้ใช้กับงานที่ง่ายกว่า`);
    if (K.tokens > 0) out.push(`token ขาเข้า ${pctf(cacheShare)} มาจาก cache (ยิ่งสูงยิ่งประหยัด) · เฉลี่ย ${tok(perOk)} token ต่อรอบที่สำเร็จ`);
    const d = delta(K.tokens, P.tokens);
    if (d !== null && Math.abs(d) >= 0.2) out.push(`ใช้ token ${d > 0 ? "มากขึ้น" : "น้อยลง"} ${pctf(Math.abs(d))} เทียบกับช่วงก่อนหน้า`);
    if (silent.length) out.push(`${silent.map((g) => g.key).join(", ")} ไม่มีตัวเลข token ให้อ่าน แม้ใน log ของ AI เอง — นับเป็นจำนวนรอบและเวลาแทน`);
    const fromLog = cur.filter((r) => !isChat(r) && r.from_log).length;
    if (fromLog) out.push(`${fromLog} รอบของบอทไม่ได้ส่ง token ตอนจบ (ถูกหยุดกลางคัน หรือเป็น Mistral Vibe ซึ่งไม่แสดงตัวเลข) — อ่านจาก log ของ AI นั้นมาแทน`);
    return out;
  });

  // ---------- tooltip ----------
  let tip = $state(null);
  function showTip(e, b) {
    const rows = stackKeys.filter((k) => b.parts[k]).map((k) => ({ k, v: b.parts[k] }));
    tip = { x: e.clientX, y: e.clientY, title: b.full, rows, total: b.total };
  }

  // ---------- export ----------
  let report;
  let busy = $state("");
  const fileBase = $derived(`usage-${period}-${iso(span.from)}`);
  async function snapshot() {
    const { toPng } = await import("html-to-image");
    const bg = getComputedStyle(document.body).backgroundColor;
    report.classList.add("exporting");
    try {
      return await toPng(report, { backgroundColor: bg, pixelRatio: 2, cacheBust: true, filter: (n) => !n.classList?.contains("no-export") });
    } finally {
      report.classList.remove("exporting");
    }
  }
  async function save(name, dataUrl) {
    const path = await api.saveExport(name, dataUrl.split(",")[1]);
    toast(`บันทึกแล้ว: ${path}`, "ok", 5000);
  }
  async function exportPng() {
    busy = "png";
    try { await save(`${fileBase}.png`, await snapshot()); } catch (e) { toast(String(e), "err"); } finally { busy = ""; }
  }
  async function exportPdf() {
    busy = "pdf";
    try {
      const png = await snapshot();
      const { jsPDF } = await import("jspdf");
      const img = new Image();
      await new Promise((ok, bad) => { img.onload = ok; img.onerror = bad; img.src = png; });
      const pdf = new jsPDF({ orientation: "portrait", unit: "mm", format: "a4" });
      const pw = pdf.internal.pageSize.getWidth(), ph = pdf.internal.pageSize.getHeight(), m = 8;
      const w = pw - 2 * m, h = (img.height * w) / img.width, pageH = ph - 2 * m;
      // a long report continues on the next page: the same image, shifted up one page height each time
      for (let y = 0, i = 0; y < h; y += pageH, i++) {
        if (i) pdf.addPage();
        pdf.addImage(png, "PNG", m, m - y, w, h);
      }
      await save(`${fileBase}.pdf`, pdf.output("datauristring"));
    } catch (e) { toast(String(e), "err"); } finally { busy = ""; }
  }
  async function exportCsv() {
    busy = "csv";
    try {
      const head = ["ที่มา", "เริ่ม", "ไอเดีย", "ขั้น", "AI", "บัญชี", "โมเดล", "effort", "สถานะ", "วินาที", "input", "output", "cache_read", "cache_write", "token รวม", "มูลค่า USD"];
      // CSV field: wrapped in double quotes, inner quotes doubled
      const DQ = String.fromCharCode(34);
      const q = (s) => DQ + String(s ?? "").split(DQ).join(DQ + DQ) + DQ;
      const SRC = { chat: "แชต", bot: "บอท" };
      const lines = cur.map((r) => [SRC[isChat(r) ? "chat" : "bot"], new Date(r.started * 1000).toLocaleString(), `#${r.idea_id} ${r.idea}`, r.stage, r.provider, r.account, r.model, r.effort, r.status, r.secs, r.input, r.output, r.cache_read, r.cache_write, total(r), r.cost.toFixed(4)].map(q).join(","));
      const text = "﻿" + [head.map(q).join(","), ...lines].join("\r\n");
      const b64 = btoa(String.fromCharCode(...new TextEncoder().encode(text)));
      await save(`${fileBase}.csv`, "data:text/csv;base64," + b64);
    } catch (e) { toast(String(e), "err"); } finally { busy = ""; }
  }

  // chart geometry
  let chartW = $state(800);
  const CH = 200, PADL = 52, PADB = 24;
  const plotW = $derived(Math.max(100, chartW - PADL - 8));
  const bw = $derived(plotW / Math.max(1, buckets.list.length));
  onMount(() => {});
</script>

<div class="usage">
  <div class="head row wrap">
    <div class="grow">
      <h1>รายงานการใช้โมเดล</h1>
      <p class="muted small">token, จำนวนรอบ และเวลา แยกตามโมเดล AI ไอเดีย และขั้นงาน — นับทั้งงานของบอทและแชตกับ AI ตามที่แต่ละ AI รายงานได้</p>
    </div>
    <div class="row wrap">
      <button onclick={exportPng} disabled={!!busy}><Icon name="image" /> {busy === "png" ? "กำลังสร้าง…" : "PNG"}</button>
      <button onclick={exportPdf} disabled={!!busy}><Icon name="doc" /> {busy === "pdf" ? "กำลังสร้าง…" : "PDF"}</button>
      <button onclick={exportCsv} disabled={!!busy || !cur.length}><Icon name="download" /> CSV</button>
    </div>
  </div>

  <!-- period + filters -->
  <div class="card ctl">
    <div class="row wrap">
      <div class="seg" role="group" aria-label="ช่วงเวลา">
        {#each PERIODS as [k, l]}<button class:on={period === k} onclick={() => { period = k; anchor = new Date(); }}>{l}</button>{/each}
      </div>
      {#if period === "range"}
        <label class="row small">ตั้งแต่ <input type="date" id="u-from" bind:value={rangeFrom} /></label>
        <label class="row small">ถึง <input type="date" id="u-to" bind:value={rangeTo} /></label>
      {:else}
        <div class="row nav">
          <button class="ghost" onclick={() => shift(-1)} aria-label="ช่วงก่อนหน้า"><Icon name="arrowLeft" /></button>
          <b class="spanlabel">{spanLabel}</b>
          <button class="ghost" onclick={() => shift(1)} aria-label="ช่วงถัดไป" disabled={span.to > new Date()}><Icon name="arrowRight" /></button>
          <button class="ghost small" onclick={() => (anchor = new Date())}>ปัจจุบัน</button>
        </div>
      {/if}
      <span class="grow"></span>
      <label class="row small">วัดด้วย
        <select id="u-metric" bind:value={metric}>{#each METRICS as [k, l]}<option value={k}>{l}</option>{/each}</select></label>
      <label class="row small">แยกตาม
        <select id="u-group" bind:value={groupBy}>{#each GROUPS as [k, l]}<option value={k}>{l}</option>{/each}</select></label>
    </div>

    <div class="filters">
      {#if optAgents.length > 1}
        <div class="frow"><span class="flab">AI / บัญชี</span>
          {#each optAgents as a}<button class="pill" class:offp={off.agent.includes(a)} aria-pressed={!off.agent.includes(a)} onclick={() => toggle("agent", a)}>{a}</button>{/each}</div>
      {/if}
      {#if optModels.length > 1}
        <div class="frow"><span class="flab">โมเดล</span>
          {#each optModels as m}<button class="pill" class:offp={off.model.includes(m)} aria-pressed={!off.model.includes(m)} onclick={() => toggle("model", m)}>{m}</button>{/each}</div>
      {/if}
      {#if optStages.length > 1}
        <div class="frow"><span class="flab">ขั้นงาน</span>
          {#each optStages as s}<button class="pill" class:offp={off.stage.includes(s)} aria-pressed={!off.stage.includes(s)} onclick={() => toggle("stage", s)}>{stageName(s)}</button>{/each}</div>
      {/if}
      {#if hasChats}
        <div class="frow"><span class="flab">ที่มา</span>
          <button class="pill" class:offp={off.source.includes("bot")} aria-pressed={!off.source.includes("bot")} onclick={() => toggle("source", "bot")}><Icon name="bot" /> งานของบอท</button>
          <button class="pill" class:offp={off.source.includes("chat")} aria-pressed={!off.source.includes("chat")} onclick={() => toggle("source", "chat")}><Icon name="chat" /> แชตกับ AI</button>
        </div>
      {/if}
      <div class="frow"><span class="flab">ไอเดีย</span>
        <button class="pill" class:offp={off.scope.includes("personal")} aria-pressed={!off.scope.includes("personal")} onclick={() => toggle("scope", "personal")}><Icon name="home" /> ส่วนตัว</button>
        <button class="pill" class:offp={off.scope.includes("work")} aria-pressed={!off.scope.includes("work")} onclick={() => toggle("scope", "work")}><Icon name="briefcase" /> งาน</button>
        <button class="pill" onclick={() => (ideaOpen = !ideaOpen)} aria-expanded={ideaOpen}>
          {ideaPick.length ? `${ideaMode === "only" ? "เฉพาะ" : "ไม่รวม"} ${ideaPick.length} ไอเดีย` : "เลือกไอเดีย…"} <Icon name={ideaOpen ? "chevronUp" : "chevronDown"} /></button>
        <label class="row small"><input type="checkbox" id="u-ok" bind:checked={okOnly} /> เฉพาะรอบที่สำเร็จ</label>
        {#if filtered}<button class="ghost small" onclick={clearFilters}><Icon name="close" /> ล้างตัวกรอง ({filtered})</button>{/if}
      </div>
      {#if ideaOpen}
        <div class="ideabox">
          <div class="row small">
            <label class="row"><input type="radio" name="u-im" value="exclude" bind:group={ideaMode} /> ไม่รวมไอเดียที่เลือก</label>
            <label class="row"><input type="radio" name="u-im" value="only" bind:group={ideaMode} /> แสดงเฉพาะไอเดียที่เลือก</label>
            <span class="grow"></span>
            {#if ideaPick.length}<button class="ghost small" onclick={() => (ideaPick = [])}>ยกเลิกที่เลือก</button>{/if}
          </div>
          <div class="ideas">
            {#each optIdeas as i}
              <label class="ideaopt"><input type="checkbox" checked={ideaPick.includes(i.id)} onchange={() => pickIdea(i.id)} />
                <span class="ellipsis grow">#{i.id} {i.title}</span><span class="faint mono">{tok(i.n)}</span></label>
            {:else}<span class="faint small">ช่วงนี้ยังไม่มีรอบงาน</span>{/each}
          </div>
        </div>
      {/if}
    </div>
  </div>

  <!-- everything from here down is what PNG / PDF export -->
  <div class="report" bind:this={report}>
    <div class="exphead">
      <b>รายงานการใช้โมเดล · Idea Autopilot</b>
      <span class="muted small">{spanLabel} · วัดด้วย {metricLabel} · แยกตาม {GROUPS.find((g) => g[0] === groupBy)[1]}{filtered ? ` · ใช้ตัวกรอง ${filtered} รายการ` : ""}</span>
    </div>

    {#if loading && !runs.length}
      <div class="card faint">กำลังโหลด…</div>
    {:else if !cur.length}
      <div class="card empty">
        <b>ไม่มีรอบงานในช่วงนี้{filtered ? "ที่ตรงกับตัวกรอง" : ""}</b>
        <span class="muted small">{first ? `มีข้อมูลตั้งแต่ ${dshort(new Date(first * 1000))} ${new Date(first * 1000).getFullYear()} ลองเลือกช่วงอื่น` : "รายงานจะเริ่มมีข้อมูลเมื่อบอททำงานรอบแรก"}{filtered ? " หรือล้างตัวกรอง" : ""}</span>
      </div>
    {:else}
      <div class="kpis">
        {#each [
          ["token ทั้งหมด", tok(K.tokens), delta(K.tokens, P.tokens), `เข้า ${tok(K.input + K.cache_read + K.cache_write)} · ออก ${tok(K.output)}`],
          ["รอบของบอท", K.runs.toLocaleString(), delta(K.runs, P.runs), `สำเร็จ ${pctf(K.runs ? K.ok / K.runs : 0)}${K.msgs ? ` · แชต ${K.msgs.toLocaleString()} ข้อความ` : ""}`],
          ["เวลาที่ AI ทำงาน", `${K.hours.toFixed(1)} ชม.`, delta(K.hours, P.hours), K.msgs ? "รวมช่วงที่คุยในแชต" : `เฉลี่ย ${K.runs ? Math.round(K.secs / K.runs / 60) : 0} นาที/รอบ`],
          ["มูลค่าเทียบราคา API", `$${K.cost.toFixed(2)}`, delta(K.cost, P.cost), "ถ้าจ่ายเป็น API แทนแพ็กเกจ"],
          ["ได้จาก cache", pctf(cacheShare), null, "ของ token ขาเข้า"],
          ["token ต่อรอบที่สำเร็จ", tok(perOk), delta(perOk, P.ok ? P.botTokens / P.ok : 0), "งานของบอท · ยิ่งต่ำยิ่งคุ้ม"],
        ] as [label, v, d, sub]}
          <div class="kpi">
            <span class="klab">{label}</span>
            <b class="kval">{v}</b>
            <span class="ksub">{#if d !== null && isFinite(d)}<span class={label.startsWith("token ต่อ") ? (d > 0 ? "up-bad" : "dn-good") : "neutral"}>{d >= 0 ? "▲" : "▼"} {pctf(Math.abs(d))}</span> เทียบช่วงก่อน · {/if}{sub}</span>
          </div>
        {/each}
      </div>

      <div class="card">
        <div class="row wrap"><h3 class="grow">สัดส่วน {metricLabel} แยกตาม{GROUPS.find((g) => g[0] === groupBy)[1]}</h3><span class="faint small">รวม {fmt(groups.reduce((a, g) => a + g.val, 0))}</span></div>
        <div class="sharebar" role="img" aria-label="สัดส่วน">
          {#each byVal.filter((g) => g.val > 0) as g (g.key)}
            <i style="flex-grow:{g.val};background:{colorOf(shown.includes(g.key) ? g.key : 'อื่นๆ')}" title="{g.key}: {fmt(g.val)} ({pctf(g.share)})">{#if g.share >= 0.08}<span>{pctf(g.share)}</span>{/if}</i>
          {/each}
        </div>
        <div class="legend">
          {#each byVal.slice(0, 12) as g (g.key)}
            <span class="lg"><i style="background:{colorOf(shown.includes(g.key) ? g.key : 'อื่นๆ')}"></i><span class="ellipsis">{g.key}</span><b class="mono">{g.noTokens && (metric === "tokens" || metric === "io") ? "ไม่รายงาน" : pctf(g.share)}</b></span>
          {/each}
          {#if byVal.length > 12}<span class="faint small">และอีก {byVal.length - 12} รายการ (ดูในตาราง)</span>{/if}
        </div>
      </div>

      <div class="card">
        <div class="row wrap"><h3 class="grow">{metricLabel} ตามเวลา</h3>
          <span class="faint small">แต่ละแท่ง = 1 {buckets.unit === "hour" ? "ชั่วโมง" : buckets.unit === "day" ? "วัน" : buckets.unit === "week" ? "สัปดาห์" : "เดือน"}</span></div>
        <div class="chart" bind:clientWidth={chartW}>
          <svg width={chartW} height={CH + PADB} role="img" aria-label="{metricLabel}ตามเวลา">
            {#each [0, 0.25, 0.5, 0.75, 1] as f}
              {@const y = CH - f * (CH - 10)}
              <line x1={PADL} x2={chartW - 4} y1={y} y2={y} class="gridl" />
              <text x={PADL - 6} y={y + 4} text-anchor="end" class="tick">{fmt(niceMax * f)}</text>
            {/each}
            {#each buckets.list as b, i}
              {@const x = PADL + i * bw}
              {@const w = Math.max(2, Math.min(28, bw - Math.max(2, bw * 0.25)))}
              {@const every = Math.ceil(buckets.list.length / Math.max(1, Math.floor(plotW / 44)))}
              {#if b.total > 0}
                {@const ks = stackKeys.filter((k) => b.parts[k])}
                {#each ks as k, j}
                  {@const below = ks.slice(0, j).reduce((a, kk) => a + b.parts[kk], 0)}
                  {@const h = (b.parts[k] / niceMax) * (CH - 10)}
                  {@const y0 = CH - (below / niceMax) * (CH - 10) - h}
                  <rect x={x + (bw - w) / 2} y={y0 + (j ? 1 : 0)} width={w} height={Math.max(0, h - (j ? 1 : 0))} fill={colorOf(k)}
                    rx={j === ks.length - 1 ? 3 : 0} />
                {/each}
              {/if}
              <rect x={x} y={0} width={bw} height={CH} fill="transparent" role="presentation"
                onpointermove={(e) => showTip(e, b)} onpointerleave={() => (tip = null)} />
              {#if i % every === 0}<text x={x + bw / 2} y={CH + 16} text-anchor="middle" class="tick">{b.label}</text>{/if}
            {/each}
            <line x1={PADL} x2={chartW - 4} y1={CH} y2={CH} class="axis" />
          </svg>
        </div>
        <div class="legend">
          {#each stackKeys as k}<span class="lg"><i style="background:{colorOf(k)}"></i><span class="ellipsis">{k}</span></span>{/each}
        </div>
      </div>

      <div class="card">
        <h3>ตารางสรุป</h3>
        <div class="tablewrap">
          <table>
            <thead><tr>
              {#each [["key", GROUPS.find((g) => g[0] === groupBy)[1]], ["runs", "รอบ"], ["msgs", "ข้อความแชต"], ["ok", "สำเร็จ"], ["input", "input"], ["output", "output"], ["cache_read", "cache อ่าน"], ["cache_write", "cache เขียน"], ["tokens", "token รวม"], ["val", `% ${metricLabel}`], ["hours", "ชม."], ["cost", "มูลค่า $"]] as [k, l]}
                <th class:num={k !== "key"}><button class="th" onclick={() => sortBy(k)}>{l}{#if sortKey === k} <Icon name={sortDesc ? "arrowDown" : "arrowUp"} size="0.9em" />{/if}</button></th>
              {/each}
            </tr></thead>
            <tbody>
              {#each sorted as g (g.key)}
                <tr>
                  <td><span class="row"><i class="sw" style="background:{colorOf(shown.includes(g.key) ? g.key : 'อื่นๆ')}"></i><span class="ellipsis" title={g.key}>{g.key}</span>{#if g.noTokens}<span class="chip">ไม่รายงาน token</span>{/if}</span></td>
                  <td class="num">{g.runs || "–"}</td>
                  <td class="num">{g.msgs || "–"}</td>
                  <td class="num">{g.runs ? pctf(g.ok / g.runs) : "–"}</td>
                  <td class="num">{tok(g.input)}</td>
                  <td class="num">{tok(g.output)}</td>
                  <td class="num">{tok(g.cache_read)}</td>
                  <td class="num">{tok(g.cache_write)}</td>
                  <td class="num"><b>{tok(g.tokens)}</b></td>
                  <td class="num"><span class="minibar"><i style="width:{g.share * 100}%"></i></span>{pctf(g.share)}</td>
                  <td class="num">{g.hours.toFixed(1)}</td>
                  <td class="num">{g.cost ? g.cost.toFixed(2) : "–"}</td>
                </tr>
              {/each}
            </tbody>
            <tfoot><tr>
              <td>รวม</td><td class="num">{K.runs}</td><td class="num">{K.msgs || "–"}</td><td class="num">{pctf(K.runs ? K.ok / K.runs : 0)}</td><td class="num">{tok(K.input)}</td><td class="num">{tok(K.output)}</td>
              <td class="num">{tok(K.cache_read)}</td><td class="num">{tok(K.cache_write)}</td><td class="num"><b>{tok(K.tokens)}</b></td><td class="num">100%</td>
              <td class="num">{K.hours.toFixed(1)}</td><td class="num">{K.cost.toFixed(2)}</td>
            </tr></tfoot>
          </table>
        </div>
      </div>

      {#if notes.length}
        <div class="card">
          <h3>ข้อสังเกตสำหรับบริหาร token</h3>
          <ul class="notes">{#each notes as n}<li>{n}</li>{/each}</ul>
          <p class="faint small">มูลค่าเทียบราคา API เป็นตัวเลขที่ Claude รายงานเอง ไม่ใช่เงินที่จ่ายจริงในแพ็กเกจรายเดือน · AI ที่ไม่รายงาน token จะนับเฉพาะรอบและเวลา · แชตนับเฉพาะ Claude และ Codex (Gemini, OpenCode, Mistral ไม่บอกจำนวน token)</p>
        </div>
      {/if}
    {/if}
  </div>
</div>

{#if tip}
  <div class="tip" style="left:{Math.min(tip.x + 14, window.innerWidth - 270)}px;top:{tip.y + 14}px">
    <b>{tip.title}</b>
    {#each tip.rows as r}<div class="row"><i class="sw" style="background:{colorOf(r.k)}"></i><span class="grow ellipsis">{r.k}</span><span class="mono">{fmt(r.v)}</span></div>{/each}
    <div class="row tt"><span class="grow">รวม</span><b class="mono">{fmt(tip.total)}</b></div>
  </div>
{/if}

<style>
  .usage {
    max-width: 1180px; display: flex; flex-direction: column; gap: 16px;
    /* categorical series: validated palette (dark app first, light steps below); "อื่นๆ" stays neutral */
    --u1: #3987e5; --u2: #d95926; --u3: #199e70; --u4: #c98500; --u5: #d55181; --u6: #008300; --u7: #9085e9; --u8: #e66767; --u-other: #6f7a8c;
  }
  @media (prefers-color-scheme: light) {
    :global(:root:not([data-theme="dark"])) .usage { --u1: #2a78d6; --u2: #eb6834; --u3: #1baf7a; --u4: #eda100; --u5: #e87ba4; --u6: #008300; --u7: #6250d6; --u8: #e34948; --u-other: #a3abb8; }
  }
  .head h1 { margin-bottom: 2px; }
  .head p { margin: 0; }
  .ctl { display: flex; flex-direction: column; gap: 12px; }
  .seg { display: inline-flex; border: 1px solid var(--line-2); border-radius: 8px; overflow: hidden; }
  .seg button { border: none; border-radius: 0; padding: 5px 12px; }
  .seg button + button { border-left: 1px solid var(--line-2); }
  .seg button.on { background: var(--teal); color: #08251c; font-weight: 500; }
  .nav { gap: 2px; }
  .spanlabel { min-width: 150px; text-align: center; font-variant-numeric: tabular-nums; }
  select { padding: 5px 10px; }
  input[type="date"] { padding: 5px 8px; }
  .filters { display: flex; flex-direction: column; gap: 8px; }
  .frow { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
  .flab { font-size: 12px; color: var(--text-3); min-width: 76px; }
  .pill { font-size: 12.5px; padding: 2px 10px; border-radius: 999px; border-color: var(--teal); background: var(--teal-bg); max-width: 280px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pill.offp { background: transparent; border-color: var(--line-2); color: var(--text-3); text-decoration: line-through; }
  .ideabox { border: 1px solid var(--line); border-radius: 10px; padding: 10px 12px; display: flex; flex-direction: column; gap: 8px; background: var(--bg-2); }
  .ideas { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 2px 14px; max-height: 220px; overflow: auto; }
  .ideaopt { display: flex; align-items: center; gap: 8px; font-size: 13px; padding: 3px 0; min-width: 0; cursor: pointer; }
  .mono { font-variant-numeric: tabular-nums; }
  .report { display: flex; flex-direction: column; gap: 14px; }
  .exphead { display: none; }
  .report:global(.exporting) { padding: 18px; background: var(--bg); }
  .report:global(.exporting) .exphead { display: flex; flex-direction: column; gap: 2px; }
  .empty { display: flex; flex-direction: column; gap: 4px; }
  .kpis { display: grid; grid-template-columns: repeat(auto-fit, minmax(170px, 1fr)); gap: 10px; }
  .kpi { background: var(--card); border: 1px solid var(--line); border-radius: 12px; padding: 12px 14px; display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .klab { font-size: 12px; color: var(--text-2); }
  .kval { font-size: 24px; font-weight: 600; font-variant-numeric: tabular-nums; line-height: 1.25; }
  .ksub { font-size: 12px; color: var(--text-3); }
  .neutral { color: var(--text-2); }
  .up-bad { color: var(--red); }
  .dn-good { color: var(--teal); }
  .card h3 { margin-bottom: 10px; }
  .sharebar { display: flex; height: 30px; border-radius: 6px; overflow: hidden; gap: 2px; background: var(--card); }
  .sharebar i { display: flex; align-items: center; justify-content: center; min-width: 2px; }
  .sharebar i:first-child { border-radius: 6px 0 0 6px; }
  .sharebar i:last-child { border-radius: 0 6px 6px 0; }
  .sharebar span { font-size: 11.5px; font-weight: 600; color: #fff; text-shadow: 0 0 3px rgb(0 0 0 / .45); font-variant-numeric: tabular-nums; }
  .legend { display: flex; flex-wrap: wrap; gap: 6px 16px; margin-top: 10px; font-size: 12.5px; }
  .lg { display: inline-flex; align-items: center; gap: 6px; max-width: 320px; min-width: 0; }
  .lg i, .sw { width: 10px; height: 10px; border-radius: 3px; display: inline-block; flex: none; }
  .lg b { font-weight: 500; color: var(--text-2); }
  .chart { width: 100%; overflow: hidden; }
  .chart svg { display: block; }
  .gridl { stroke: var(--line); stroke-width: 1; }
  .axis { stroke: var(--line-2); stroke-width: 1; }
  .tick { font-size: 10.5px; fill: var(--text-3); font-variant-numeric: tabular-nums; }
  .tablewrap { overflow-x: auto; }
  table { width: 100%; border-collapse: collapse; font-size: 13px; }
  th, td { padding: 6px 8px; border-bottom: 1px solid var(--line); text-align: left; white-space: nowrap; }
  td:first-child { max-width: 320px; }
  th.num, td.num { text-align: right; font-variant-numeric: tabular-nums; }
  tfoot td { border-top: 2px solid var(--line-2); border-bottom: none; font-weight: 500; }
  .th { border: none; padding: 0; background: none; font: inherit; font-weight: 600; color: var(--text-2); display: inline-flex; align-items: center; gap: 2px; }
  .th:hover { background: none; color: var(--text); }
  .minibar { display: inline-block; width: 44px; height: 6px; background: var(--card-2); border-radius: 3px; margin-right: 6px; vertical-align: middle; overflow: hidden; }
  .minibar i { display: block; height: 100%; background: var(--teal); border-radius: 3px; }
  .notes { margin: 0 0 8px; padding-left: 20px; display: flex; flex-direction: column; gap: 4px; }
  .tip { position: fixed; z-index: 50; pointer-events: none; background: var(--card); border: 1px solid var(--line-2); border-radius: 8px; padding: 8px 10px; font-size: 12.5px; min-width: 200px; max-width: 260px; box-shadow: 0 6px 20px rgb(0 0 0 / .25); display: flex; flex-direction: column; gap: 3px; }
  .tip .tt { border-top: 1px solid var(--line); padding-top: 3px; margin-top: 2px; }
</style>
