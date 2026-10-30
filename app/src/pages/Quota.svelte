<script>
  import { onMount } from "svelte";
  import { store, refresh, toast, stageLabel } from "../lib/store.svelte.js";
  import { api, pct, clock } from "../lib/api.js";

  let rep = $state(null);
  let days = $state(28);
  let cal = $state([]);

  async function load() {
    [rep, cal] = await Promise.all([api.reports(days), api.calendar(8)]);
  }
  onMount(load);

  const s = $derived(store.state);
  const d = $derived(s?.decision);
  const weekStart = $derived(s?.snap?.week_reset ? s.snap.week_reset - 7 * 86400 : 0);
  const split = $derived.by(() => {
    let user = 0, bot = 0;
    for (const r of rep?.daily ?? []) {
      if (new Date(r.date).getTime() / 1000 + 86400 > weekStart) { user += r.user; bot += r.bot; }
    }
    const used = d?.week_pct ?? 0;
    // scale to the real weekly % (history may not cover the whole week yet)
    const t = user + bot;
    return t > 0 ? { user: (used * user) / t, bot: (used * bot) / t } : { user: used, bot: 0 };
  });

  // ring segments: user, bot, reserve, available
  const R = 70, C = 2 * Math.PI * R;
  const segs = $derived.by(() => {
    if (!d) return [];
    const reserve = Math.max(0, Math.min(100 - d.week_pct, d.reserve));
    const avail = Math.max(0, 100 - d.week_pct - reserve);
    const list = [
      { v: split.user, c: "var(--blue)", label: "คุณใช้" },
      { v: split.bot, c: "var(--teal)", label: "บอทใช้" },
      { v: reserve, c: "var(--amber)", label: "กันไว้ให้งาน", dash: true },
      { v: avail, c: "var(--line-2)", label: "บอทใช้ได้อีก" },
    ];
    let off = 0;
    return list.map((x) => { const r = { ...x, off }; off += x.v; return r; });
  });

  const maxDaily = $derived(Math.max(1, ...(rep?.daily ?? []).map((r) => r.user + r.bot)));
  const maxHeat = $derived(Math.max(0.01, ...(rep?.heat ?? []).flat()));
  const DAYS = ["จ.", "อ.", "พ.", "พฤ.", "ศ.", "ส.", "อา."];
  const KIND = { full: "ทำงาน", half: "ครึ่งวัน", off: "หยุด" };

  async function turbo(h) {
    await api.setTurbo(h);
    toast(h ? `Turbo ${h} ชม.` : "ปิด Turbo");
    refresh();
  }
  function hours(n) {
    return n >= 24 ? `${Math.floor(n / 24)} วัน ${Math.round(n % 24)} ชม.` : `${n.toFixed(1)} ชม.`;
  }
</script>

<div class="wrapx">
  <div class="row head">
    <div class="grow"><h1>โควต้า</h1><p class="muted small">แผนการใช้ให้ครบ 100% ก่อนรีเซ็ต โดยกันส่วนของงานคุณไว้ก่อนเสมอ</p></div>
    <button class="small" onclick={() => { api.refreshUsage(); toast("กำลังอ่านโควต้าใหม่"); }}>↻ อ่านใหม่</button>
  </div>

  {#if s?.snap_note}<div class="warn small">⚠ {s.snap_note}</div>{/if}

  <div class="top">
    <div class="card ringcard">
      <svg viewBox="0 0 180 180" width="190" height="190">
        <circle cx="90" cy="90" r={R} fill="none" stroke="var(--bg-2)" stroke-width="20" />
        {#each segs as g}
          {#if g.v > 0}
            <circle cx="90" cy="90" r={R} fill="none" stroke={g.c} stroke-width="20"
              stroke-dasharray="{(g.v / 100) * C} {C}" stroke-dashoffset={-(g.off / 100) * C}
              transform="rotate(-90 90 90)" opacity={g.dash ? 0.55 : 1} />
          {/if}
        {/each}
        <text x="90" y="86" text-anchor="middle" class="big">{pct(d?.week_pct)}</text>
        <text x="90" y="108" text-anchor="middle" class="sub">ของสัปดาห์</text>
      </svg>
      <div class="legend">
        {#each segs as g}<div class="row small"><i style="background:{g.c};opacity:{g.dash ? 0.55 : 1}"></i><span class="grow">{g.label}</span><b>{pct(g.v, 1)}</b></div>{/each}
        <div class="faint small">รีเซ็ต {clock(s?.snap?.week_reset)} (อีก {hours(d?.hours_to_reset ?? 0)})</div>
      </div>
    </div>

    <div class="card decide">
      <div class="row"><span class="dot" class:ok={d?.allow}></span><b class="grow">{d?.allow ? "บอททำงานได้" : "บอทรออยู่"}</b>
        {#if d?.sprint}<span class="chip sprint">🔥 โค้งสุดท้าย</span>{/if}
        {#if d?.turbo}<span class="chip sprint">⚡ Turbo</span>{/if}</div>
      <div class="muted">{d?.reason}</div>
      <div class="kv">
        <span>ช่วงเวลา</span><b>{{ work: "เวลางาน", lunch: "พักเที่ยง", off: "นอกเวลางาน" }[d?.slot] ?? "—"}</b>
        <span>5 ชม. ตอนนี้</span><b>{pct(d?.five_pct)} / เพดาน {pct(d?.five_cap)}</b>
        <span>รีเซ็ต 5 ชม.</span><b>{clock(s?.snap?.five_reset)}</b>
        <span>บอทใช้วันนี้</span><b>{pct(d?.bot_used_today, 1)} / {pct(d?.today_allowance, 1)}</b>
        <span>กันให้งาน/ชม.</span><b>{(s?.reserve_per_hour ?? 0).toFixed(2)}% {s?.reserve_learned ? "(เรียนรู้จากสถิติ)" : "(ค่าเริ่มต้น)"}</b>
        <span>คุณกำลังใช้ Claude</span><b>{s?.user_active ? "ใช่" : "ไม่"}</b>
      </div>
      <div class="row wrap">
        <button class:primary={!s?.paused} onclick={async () => { await api.setPaused(!s?.paused); refresh(); }}>{s?.paused ? "▶ ให้บอททำต่อ" : "⏸ พักบอท"}</button>
        {#if s?.turbo_until}<button onclick={() => turbo(0)}>ปิด Turbo</button>{:else}
          <button onclick={() => turbo(2)}>⚡ Turbo 2 ชม.</button>
          <button onclick={() => turbo(8)}>⚡ Turbo ทั้งคืน</button>{/if}
      </div>
    </div>
  </div>

  <section>
    <h2>8 วันข้างหน้า</h2>
    <div class="days">
      {#each cal as c}
        {@const dt = new Date(c.date)}
        <div class="day k-{c.kind}" title={c.note ?? ""}>
          <div class="small faint">{DAYS[(dt.getDay() + 6) % 7]} {dt.getDate()}</div>
          <div class="hrs">
            {#each c.hours ?? [] as slot}
              <i class:w={slot === "work"} class:l={slot === "lunch"}></i>
            {/each}
          </div>
          <div class="small">{KIND[c.kind]}{c.override ? " *" : ""}</div>
        </div>
      {/each}
    </div>
    <div class="faint small">สีส้ม = เวลางาน (บอทหลบ) · เขียว = บอทใช้ได้เต็มที่ · * = ตั้งค่าเอง (แก้ได้ที่หน้าตั้งค่า)</div>
  </section>

  <section>
    <div class="row"><h2 class="grow">ย้อนหลัง</h2>
      <select bind:value={days} onchange={load}><option value={7}>7 วัน</option><option value={28}>28 วัน</option><option value={90}>90 วัน</option></select>
      <button class="small" onclick={() => api.exportCsv().then(() => toast("ส่งออก CSV แล้ว"))}>⬇ CSV</button></div>

    {#if rep}
      <div class="card">
        <h3>ใช้ต่อวัน (คุณ / บอท)</h3>
        <div class="bars">
          {#each rep.daily as r}
            <div class="bcol" title="{r.date}: คุณ {r.user.toFixed(1)}% · บอท {r.bot.toFixed(1)}%">
              <div class="stack" style="height:{((r.user + r.bot) / maxDaily) * 100}%">
                <i class="b" style="flex:{r.bot}"></i><i class="u" style="flex:{r.user}"></i>
              </div>
              <span class="faint">{r.date.slice(8)}</span>
            </div>
          {:else}<div class="faint small">ยังไม่มีข้อมูล — ระบบเก็บสถิติทุก 5 นาที</div>{/each}
        </div>
        <div class="row small faint"><i class="sw u"></i>คุณ <i class="sw b"></i>บอท</div>
      </div>

      <div class="two">
        <div class="card">
          <h3>ช่วงเวลาที่ใช้บ่อย (เฉลี่ย %/สัปดาห์)</h3>
          <div class="heat">
            {#each rep.heat as row, wd}
              <span class="faint small">{DAYS[wd]}</span>
              {#each row as v, h}<i title="{DAYS[wd]} {h}:00 · {v.toFixed(2)}%" style="opacity:{0.08 + 0.92 * (v / maxHeat)}"></i>{/each}
            {/each}
            <span></span>{#each Array(24) as _, h}<span class="faint hx">{h % 3 === 0 ? h : ""}</span>{/each}
          </div>
        </div>
        <div class="card">
          <h3>เหลือทิ้งตอนรีเซ็ต</h3>
          <p class="faint small">ตัวชี้วัดหลัก — ยิ่งใกล้ 0% ยิ่งคุ้ม</p>
          {#each rep.weeks as w}
            <div class="row small wk"><span class="grow">{clock(w.reset)}</span>
              <div class="lb"><i style="width:{w.left}%"></i></div><b>{w.closed ? pct(w.left) : "กำลังใช้"}</b></div>
          {:else}<div class="faint small">ยังไม่ครบสัปดาห์</div>{/each}
          {#if rep.models?.length}
            <h3 style="margin-top:16px">แยกตามโมเดล · effort</h3>
            {#each rep.models as m}
              <div class="row small"><span class="grow">{m.model}</span><span class="faint">{m.runs} รอบ · สำเร็จ {m.ok} · {(m.tokens / 1e6).toFixed(1)}M tok</span><b style="width:56px;text-align:right">{pct(m.pct, 1)}</b></div>
            {/each}
          {/if}
          {#if rep.stages.length}
            <h3 style="margin-top:16px">แยกตามขั้นงาน</h3>
            {#each rep.stages as st}
              <div class="row small"><span class="grow">{stageLabel(st.stage)}</span><span class="faint">{st.runs} รอบ · {(st.tokens / 1e6).toFixed(1)}M tok</span><b style="width:56px;text-align:right">{pct(st.pct, 1)}</b></div>
            {/each}
          {/if}
        </div>
      </div>
    {/if}
  </section>
</div>

<style>
  .wrapx { max-width: 1100px; display: flex; flex-direction: column; gap: 24px; }
  .head p { margin: 4px 0 0; }
  .warn { background: var(--amber-bg); color: var(--amber); padding: 8px 12px; border-radius: 8px; }
  .top { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; }
  .ringcard { display: flex; align-items: center; gap: 18px; }
  .big { font-size: 28px; font-weight: 600; fill: var(--text); }
  .sub { font-size: 12px; fill: var(--text-3); }
  .legend { display: flex; flex-direction: column; gap: 6px; flex: 1; }
  .legend i { width: 10px; height: 10px; border-radius: 3px; display: inline-block; }
  .decide { display: flex; flex-direction: column; gap: 10px; }
  .dot { width: 10px; height: 10px; border-radius: 50%; background: var(--text-3); }
  .dot.ok { background: var(--teal); }
  .sprint { background: var(--red-bg); color: var(--red); }
  .kv { display: grid; grid-template-columns: auto 1fr; gap: 3px 14px; font-size: 13px; }
  .kv span { color: var(--text-2); }
  .kv b { font-weight: 500; }
  section { display: flex; flex-direction: column; gap: 10px; }
  .days { display: grid; grid-template-columns: repeat(8, 1fr); gap: 8px; }
  .day { background: var(--card); border: 1px solid var(--line); border-radius: 10px; padding: 8px; display: flex; flex-direction: column; gap: 6px; }
  .day.k-off { border-color: var(--teal-bg); }
  .hrs { display: grid; grid-template-columns: repeat(24, 1fr); gap: 1px; height: 22px; }
  .hrs i { background: var(--teal); opacity: .55; border-radius: 1px; }
  .hrs i.w { background: var(--amber); opacity: .9; }
  .hrs i.l { background: var(--amber); opacity: .35; }
  .bars { display: flex; align-items: flex-end; gap: 4px; height: 160px; padding-top: 8px; }
  .bcol { flex: 1; display: flex; flex-direction: column; align-items: center; height: 100%; justify-content: flex-end; font-size: 10px; gap: 2px; }
  .stack { width: 100%; max-width: 22px; display: flex; flex-direction: column; border-radius: 4px 4px 0 0; overflow: hidden; min-height: 1px; }
  .stack i, .sw { display: block; }
  .b { background: var(--teal); }
  .u { background: var(--blue); }
  .sw { width: 10px; height: 10px; border-radius: 3px; display: inline-block; margin-left: 8px; }
  .two { display: grid; grid-template-columns: 1.4fr 1fr; gap: 14px; }
  .card h3 { margin-bottom: 10px; }
  .heat { display: grid; grid-template-columns: 28px repeat(24, 1fr); gap: 2px; align-items: center; }
  .heat i { height: 16px; background: var(--teal); border-radius: 3px; }
  .hx { font-size: 10px; text-align: center; }
  .wk { margin: 4px 0; }
  .lb { width: 120px; height: 6px; background: var(--bg-2); border-radius: 6px; overflow: hidden; }
  .lb i { display: block; height: 100%; background: var(--amber); }
</style>
