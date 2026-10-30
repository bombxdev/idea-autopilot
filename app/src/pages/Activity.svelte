<script>
  import { onMount } from "svelte";
  import { store, stageLabel } from "../lib/store.svelte.js";
  import { api, pct, isStep } from "../lib/api.js";

  let days = $state(7);
  let data = $state(null);
  let tab = $state("runs");
  let open = $state({}); // run id -> log lines
  let ideaFilter = $state(0);

  async function load() {
    data = await api.activity(days);
  }
  onMount(() => {
    load();
    const t = setInterval(load, 60000);
    return () => clearInterval(t);
  });

  const WAIT = {
    running: ["ทำงาน", "var(--teal)"],
    off: ["พร้อม แต่คิวว่าง", "var(--blue)"],
    work: ["ช่วงงาน พร้อมรันเบาๆ", "var(--blue)"],
    sprint: ["โค้งสุดท้าย", "var(--teal)"],
    turbo: ["Turbo", "var(--teal)"],
    daily_cap: ["ครบโควต้ารายวัน", "var(--amber)"],
    no_budget: ["กันไว้ให้งานหมดแล้ว", "var(--amber)"],
    five_cap: ["ติดเพดาน 5 ชม.", "var(--amber)"],
    user_active: ["คุณใช้ Claude อยู่", "var(--purple)"],
    paused: ["พักบอท", "var(--red)"],
    no_data: ["อ่านโควต้าไม่ได้", "var(--red)"],
    stale: ["ข้อมูลโควต้าเก่า", "var(--red)"],
  };
  const STATUS = { ok: "สำเร็จ", error: "ผิดพลาด", timeout: "หมดเวลา", cancelled: "หยุด", rate_limited: "ชนลิมิต", interrupted: "ขัดจังหวะ", running: "กำลังทำ" };
  const PALETTE = ["#5dcaa5", "#6aa8ec", "#a49bf0", "#f2b04a", "#ec8fb0", "#7fd1e3", "#c9d46a"];
  const color = (id) => PALETTE[id % PALETTE.length];

  const runs = $derived((data?.runs ?? []).filter((r) => !ideaFilter || r.idea_id === ideaFilter));
  const byDay = $derived.by(() => {
    const m = new Map();
    for (const r of runs) {
      const k = new Date(r.started * 1000).toLocaleDateString("th-TH", { weekday: "short", day: "numeric", month: "short" });
      if (!m.has(k)) m.set(k, []);
      m.get(k).push(r);
    }
    return [...m.entries()];
  });
  const waitTotal = $derived(Object.values(data?.waits ?? {}).reduce((a, b) => a + b, 0) || 1);
  const ideasInLog = $derived([...new Map((data?.runs ?? []).map((r) => [r.idea_id, r.title])).entries()]);
  const events = $derived((data?.events ?? []).filter((e) => !ideaFilter || e.idea_id === ideaFilter));

  const hm = (ts) => new Date(ts * 1000).toLocaleTimeString("th-TH", { hour: "2-digit", minute: "2-digit" });
  const dur = (s) => (s >= 3600 ? `${Math.floor(s / 3600)} ชม. ${Math.round((s % 3600) / 60)} น.` : `${Math.max(1, Math.round(s / 60))} นาที`);
  const minOfDay = (ts) => {
    const d = new Date(ts * 1000);
    return d.getHours() * 60 + d.getMinutes();
  };
  async function toggle(r) {
    if (open[r.id]) {
      delete open[r.id];
      open = { ...open };
    } else {
      open = { ...open, [r.id]: await api.runLog(r.id) };
    }
  }
</script>

<div class="wrapx">
  <div class="row head">
    <div class="grow">
      <h1>บันทึก</h1>
      <p class="muted small">บอททำอะไร ไอเดียไหน กี่โมง ผลเป็นยังไง และช่วงที่ว่างเพราะอะไร</p>
    </div>
    <select bind:value={ideaFilter}>
      <option value={0}>ทุกไอเดีย</option>
      {#each ideasInLog as [id, t]}<option value={id}>#{id} {t}</option>{/each}
    </select>
    <select bind:value={days} onchange={load}>
      <option value={1}>วันนี้</option><option value={7}>7 วัน</option><option value={30}>30 วัน</option>
    </select>
  </div>

  {#if data}
    {@const s = data.stats}
    <div class="tiles">
      <div class="tile"><span class="lbl">รอบงาน</span><span class="num">{s.runs}</span><span class="faint small">สำเร็จ {s.ok} · ไม่สำเร็จ {s.failed}</span></div>
      <div class="tile"><span class="lbl">อัตราสำเร็จ</span><span class="num teal">{s.runs ? Math.round((s.ok / Math.max(1, s.ok + s.failed)) * 100) : 0}%</span><span class="faint small">เฉลี่ยรอบละ {dur(s.avg_secs)}</span></div>
      <div class="tile"><span class="lbl">บอททำงานรวม</span><span class="num">{dur(s.busy_secs)}</span><span class="faint small">ในช่วงที่เลือก</span></div>
      <div class="tile"><span class="lbl">โควต้าที่บอทใช้</span><span class="num">{pct(s.pct, 1)}</span><span class="faint small">{(s.tokens / 1e6).toFixed(1)}M token</span></div>
    </div>

    <div class="two">
      <div class="card">
        <h3>บอทใช้เวลาไปกับอะไร</h3>
        <p class="faint small">บันทึกทุก 5 นาที — ถ้า "ครบโควต้ารายวัน" หรือ "กันไว้ให้งานหมด" เยอะ แปลว่ากันโควต้าไว้มากไป ลดได้ที่ตั้งค่า</p>
        {#each Object.entries(data.waits).sort((a, b) => b[1] - a[1]) as [code, mins]}
          {@const w = WAIT[code] ?? [code, "var(--text-3)"]}
          <div class="wrow">
            <span class="wl">{w[0]}</span>
            <div class="wbar"><i style="width:{(mins / waitTotal) * 100}%;background:{w[1]}"></i></div>
            <span class="faint small wv">{dur(mins * 60)}</span>
          </div>
        {:else}<div class="faint small">ยังไม่มีข้อมูล — เริ่มเก็บตั้งแต่อัปเดตนี้</div>{/each}
      </div>
      <div class="card">
        <h3>ความคืบหน้าในคิว</h3>
        {#each data.eta as e}
          {@const idea = store.ideas.find((i) => i.id === e.id)}
          {@const total = (idea?.recipe.length ?? 0) + (idea?.status === "inbox" ? 1 : 0)}
          {@const p = total ? Math.round(((total - e.stages_left) / total) * 100) : 0}
          <button class="ghost eta" onclick={() => (store.openIdea = e.id)}>
            <div class="row"><b class="grow ellipsis">#{e.id} {e.title}</b><span class="small">{p}%</span></div>
            <div class="pbar"><i style="width:{p}%"></i></div>
            <div class="faint small">เหลือ {e.stages_left} ขั้น · ประมาณ {dur(e.secs)} ของเวลาบอท</div>
          </button>
        {:else}<div class="faint small">ไม่มีไอเดียค้างในคิว</div>{/each}
      </div>
    </div>

    <div class="card">
      <h3>ไทม์ไลน์รายวัน</h3>
      <div class="axis"><span></span>{#each [0, 3, 6, 9, 12, 15, 18, 21] as h}<span style="left:{(h / 24) * 100}%">{h}:00</span>{/each}</div>
      {#each byDay as [day, list]}
        <div class="tl">
          <span class="small faint dl">{day}</span>
          <div class="track">
            {#each list as r}
              {@const a = minOfDay(r.started)}
              {@const b = Math.max(a + 4, Math.min(1440, a + Math.round(r.secs / 60)))}
              <i title="{hm(r.started)} #{r.idea_id} {r.title} · {stageLabel(r.stage)} · {STATUS[r.status] ?? r.status}"
                class:bad={r.status !== "ok" && r.status !== "running"}
                style="left:{(a / 1440) * 100}%;width:{((b - a) / 1440) * 100}%;background:{color(r.idea_id)}"></i>
            {/each}
          </div>
        </div>
      {:else}<div class="faint small">ยังไม่มีรอบงานในช่วงนี้</div>{/each}
    </div>

    <div class="tabs">
      <button class="tab" class:on={tab === "runs"} onclick={() => (tab = "runs")}>รอบงาน ({runs.length})</button>
      <button class="tab" class:on={tab === "events"} onclick={() => (tab = "events")}>กิจกรรมทั้งหมด ({events.length})</button>
    </div>

    {#if tab === "runs"}
      {#each byDay as [day, list]}
        <h3 class="dayh">{day}</h3>
        {#each list as r (r.id)}
          <div class="run" class:bad={r.status !== "ok" && r.status !== "running"}>
            <button class="ghost rhead" onclick={() => toggle(r)}>
              <span class="time">{hm(r.started)}<span class="faint">–{r.ended ? hm(r.ended) : "…"}</span></span>
              <span class="dot" style="background:{color(r.idea_id)}"></span>
              <span class="grow ellipsis"><b>#{r.idea_id} {r.title}</b> · {stageLabel(r.stage)}</span>
              <span class="chip">{r.model}{r.effort ? ` · ${r.effort}` : ""}</span>
              <span class="chip st-{r.status}">{STATUS[r.status] ?? r.status}</span>
              <span class="faint small w80">{dur(r.secs)}</span>
              <span class="faint small w80">{(r.tokens / 1000).toFixed(0)}k tok</span>
              <span class="faint small w60">{r.pct != null ? `+${r.pct.toFixed(1)}%` : ""}</span>
            </button>
            {#if r.summary}<div class="sum small">{r.summary}</div>{/if}
            {#if r.error}<div class="err small">{r.error}</div>{/if}
            {#if open[r.id]}
              <div class="log">
                {#if r.reason}<div class="faint">เลือกโมเดลเพราะ: {r.reason}</div>{/if}
                {#each open[r.id] as l}<div class:tool={isStep(l)}>{l}</div>{:else}<div class="faint">ไม่มี log</div>{/each}
              </div>
            {/if}
          </div>
        {/each}
      {/each}
    {:else}
      {#each events as ev}
        <div class="ev row small">
          <span class="time">{new Date(ev.ts * 1000).toLocaleString("th-TH", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" })}</span>
          <span class="chip">{ev.kind}</span>
          <span class="grow">{ev.text}</span>
          {#if ev.idea_id}<button class="ghost small" onclick={() => (store.openIdea = ev.idea_id)}>เปิด</button>{/if}
        </div>
      {:else}<div class="faint">ไม่มีกิจกรรม</div>{/each}
    {/if}
  {:else}
    <div class="faint">กำลังโหลด…</div>
  {/if}
</div>

<style>
  .wrapx { max-width: 1100px; display: flex; flex-direction: column; gap: 18px; }
  .head p { margin: 4px 0 0; }
  .tiles { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 12px; }
  .tile { background: var(--card); border: 1px solid var(--line); border-radius: 14px; padding: 14px 16px; display: flex; flex-direction: column; gap: 2px; }
  .lbl { font-size: 13px; color: var(--text-2); }
  .num { font-size: 26px; font-weight: 600; }
  .teal { color: var(--teal); }
  .two { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; }
  .card h3 { margin-bottom: 6px; }
  .card p { margin: 0 0 10px; }
  .wrow { display: grid; grid-template-columns: 170px 1fr 90px; gap: 10px; align-items: center; margin: 5px 0; font-size: 13px; }
  .wbar { height: 8px; background: var(--bg-2); border-radius: 6px; overflow: hidden; }
  .wbar i { display: block; height: 100%; border-radius: 6px; }
  .wv { text-align: right; }
  .eta { display: flex; flex-direction: column; gap: 4px; width: 100%; text-align: left; padding: 8px 6px; border: none; border-radius: 8px; }
  .pbar { height: 6px; background: var(--bg-2); border-radius: 6px; overflow: hidden; }
  .pbar i { display: block; height: 100%; background: var(--teal); }
  .axis { position: relative; height: 16px; margin-left: 110px; font-size: 10px; color: var(--text-3); }
  .axis span { position: absolute; transform: translateX(-50%); }
  .tl { display: flex; align-items: center; gap: 10px; margin: 6px 0; }
  .dl { width: 100px; flex: none; }
  .track { position: relative; flex: 1; height: 18px; background: var(--bg-2); border-radius: 5px; overflow: hidden;
    background-image: repeating-linear-gradient(90deg, transparent 0, transparent calc(12.5% - 1px), var(--line) calc(12.5% - 1px), var(--line) 12.5%); }
  .track i { position: absolute; top: 2px; bottom: 2px; border-radius: 3px; opacity: .9; }
  .track i.bad { outline: 2px solid var(--red); }
  .tabs { display: flex; gap: 2px; border-bottom: 1px solid var(--line); }
  .tab { border: none; border-bottom: 2px solid transparent; border-radius: 0; padding: 8px 12px; color: var(--text-2); }
  .tab.on { color: var(--text); border-bottom-color: var(--teal); }
  .tab:hover { background: none; }
  .dayh { margin: 8px 0 0; color: var(--text-2); font-weight: 500; }
  .run { background: var(--card); border: 1px solid var(--line); border-radius: 10px; padding: 6px 10px; }
  .run.bad { border-color: var(--red-bg); }
  .rhead { display: flex; align-items: center; gap: 10px; width: 100%; border: none; padding: 4px 2px; text-align: left; }
  .rhead:hover { background: none; }
  .time { font-variant-numeric: tabular-nums; width: 100px; flex: none; font-size: 13px; }
  .dot { width: 10px; height: 10px; border-radius: 50%; flex: none; }
  .w80 { width: 70px; text-align: right; flex: none; }
  .w60 { width: 52px; text-align: right; flex: none; }
  .st-ok { background: var(--teal-bg); color: var(--teal); }
  .st-error, .st-interrupted { background: var(--red-bg); color: var(--red); }
  .st-timeout, .st-rate_limited, .st-cancelled { background: var(--amber-bg); color: var(--amber); }
  .st-running { background: var(--blue-bg); color: var(--blue); }
  .sum { color: var(--text-2); padding: 0 4px 4px 122px; }
  .err { color: var(--red); padding: 0 4px 4px 122px; }
  .log { max-height: 300px; overflow: auto; background: var(--bg-2); border-radius: 8px; padding: 10px 12px; font-size: 13px; white-space: pre-wrap; margin: 4px 0 6px; }
  .log .tool { color: var(--text-3); font-family: ui-monospace, Consolas, monospace; font-size: 12px; }
  .ev { padding: 6px 4px; border-bottom: 1px solid var(--line); }
</style>
