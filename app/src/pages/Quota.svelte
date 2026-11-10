<script>
  import IconText from "../components/IconText.svelte";
  import Icon from "../components/Icon.svelte";
  import { onMount } from "svelte";
  import { store, refresh, toast, stageLabel } from "../lib/store.svelte.js";
  import { api, pct, clock } from "../lib/api.js";
  import AccountCharts from "../components/AccountCharts.svelte";
  import QuotaPick from "../components/QuotaPick.svelte";
  import PlanCard from "../components/PlanCard.svelte";
  import { qv, isShown } from "../lib/quotaview.svelte.js";

  let rep = $state(null);
  let eco = $state(null);
  let days = $state(28);
  let cal = $state([]);

  async function load() {
    [rep, cal, eco] = await Promise.all([api.reports(days), api.calendar(8), api.modelEconomics()]);
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

  // ---- the plan per account (split) or for the accounts together (combined)
  const accts = $derived(s?.accounts ?? []);
  const shownAccts = $derived(accts.filter((a) => a.decision && isShown(a.id)));
  // the share of the used quota that is yours, from the history of your own use (Claude, which you also use yourself)
  const userShare = $derived(split.user + split.bot > 0 ? split.user / (split.user + split.bot) : 0);
  const avg = (xs) => (xs.length ? xs.reduce((a, b) => a + b, 0) / xs.length : 0);
  const cards = $derived.by(() => {
    if (!accts.length) return [{ key: "main", title: "", d, snap: s?.snap, shared: true, userShare, lines: [] }];
    if (qv.mode === "combined" && shownAccts.length > 1) {
      const ds = shownAccts.map((a) => a.decision);
      const merged = {
        allow: ds.some((x) => x.allow), reason: shownAccts.map((a) => `${a.label}: ${a.decision.allow ? "ทำงานได้" : a.decision.reason}`).join(" · "),
        slot: ds[0].slot, week_pct: avg(ds.map((x) => x.week_pct)), five_pct: avg(ds.map((x) => x.five_pct)), five_cap: avg(ds.map((x) => x.five_cap)),
        reserve: avg(ds.map((x) => x.reserve)), bot_used_today: ds.reduce((a, x) => a + x.bot_used_today, 0), today_allowance: ds.reduce((a, x) => a + x.today_allowance, 0),
        sprint: ds.some((x) => x.sprint), turbo: ds.some((x) => x.turbo), hours_to_reset: Math.min(...ds.map((x) => x.hours_to_reset)),
      };
      return [{ key: "all", title: `รวม ${shownAccts.length} บัญชี (ค่าเฉลี่ยของแต่ละบัญชี)`, d: merged, snap: shownAccts[0].snap, shared: shownAccts.some((a) => a.shared), userShare, lines: shownAccts.map((a) => `${a.label}: ${Math.round(a.decision.week_pct)}% ของสัปดาห์ · ${a.decision.allow ? "ทำงานได้" : "รอ"}`) }];
    }
    return shownAccts.map((a) => ({ key: a.id, title: shownAccts.length > 1 || accts.length > 1 ? a.label : "", d: a.decision, snap: a.snap, shared: a.shared, userShare: a.shared ? userShare : 0, lines: [], limit: a.limit ?? "window" }));
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
    <button class="small" onclick={() => { api.refreshUsage(); toast("กำลังอ่านโควต้าใหม่"); }}><Icon name="refresh" /> อ่านใหม่</button>
  </div>

  {#if s?.snap_note}<div class="warn small"><Icon name="warning" /> <IconText text={s.snap_note} /></div>{/if}

  <QuotaPick accounts={accts} />
  <div class="row wrap small ctrl">
    <button class:primary={!s?.paused} onclick={async () => { await api.setPaused(!s?.paused); refresh(); }}><Icon name={s?.paused ? "play" : "pause"} /> {s?.paused ? "ให้บอททำต่อ" : "พักบอท"}</button>
    {#if s?.turbo_until}<button onclick={() => turbo(0)}>ปิด Turbo</button>{:else}
      <button onclick={() => turbo(2)}><Icon name="bolt" /> Turbo 2 ชม.</button>
      <button onclick={() => turbo(8)}><Icon name="bolt" /> Turbo ทั้งคืน</button>{/if}
    <span class="faint">กันให้งาน/ชม. {(s?.reserve_per_hour ?? 0).toFixed(2)}% {s?.reserve_learned ? "(เรียนรู้จากสถิติ)" : "(ค่าเริ่มต้น)"} · คุณกำลังใช้ Claude: {s?.user_active ? "ใช่" : "ไม่"}</span>
  </div>
  <div class="plans" class:single={cards.length === 1}>
    {#each cards as c (c.key)}<PlanCard title={c.title} d={c.d} snap={c.snap} shared={c.shared} userShare={c.userShare} lines={c.lines} limit={c.limit ?? "window"} />{/each}
  </div>

  <section>
    <h2>โควต้าแยกตามบัญชี</h2>
    <AccountCharts />
  </section>

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
      <button class="small" onclick={() => api.exportCsv().then(() => toast("ส่งออก CSV แล้ว"))}><Icon name="download" /> CSV</button></div>

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
          {#if eco?.overall?.length}
            <h3 style="margin-top:16px">ความคุ้มค่าของแต่ละโมเดล</h3>
            <p class="faint small">“ต่อรอบที่สำเร็จ” = ค่าใช้จ่ายทั้งหมดของโมเดลนั้น ÷ จำนวนรอบที่ออกมาดี (รอบที่ต้องแก้หรือทำซ้ำก็นับเป็นค่าใช้จ่าย) ยิ่งต่ำยิ่งคุ้ม · เครื่องหมาย ~ คือประมาณจากจำนวน token เพราะ AI ตัวนั้นไม่รายงานราคา · ไม่นับรอบโหมดเต็มที่</p>
            <table class="eco">
              <thead><tr><th>โมเดล</th><th>รอบ</th><th>สำเร็จ</th><th>เฉลี่ย/รอบ</th><th>ต่อรอบที่สำเร็จ</th><th>turns</th><th>นาที</th></tr></thead>
              <tbody>
                {#each eco.overall as m}
                  {@const best = Math.min(...eco.overall.filter((x) => x.runs >= eco.min_evidence && x.per_good != null).map((x) => x.per_good))}
                  <tr class:best={m.runs >= eco.min_evidence && m.per_good != null && m.per_good === best}>
                    <td>{m.label}</td><td>{m.runs}</td><td>{m.ok_pct}%</td>
                    <td>{m.estimated ? "~" : ""}${m.avg_cost.toFixed(2)}</td>
                    <td><b>{m.per_good == null ? "—" : `${m.estimated ? "~" : ""}$${m.per_good.toFixed(2)}`}</b></td>
                    <td>{m.avg_turns.toFixed(0)}</td><td>{m.avg_min.toFixed(0)}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
            {#if eco.overall.every((m) => m.runs < eco.min_evidence)}<div class="faint small">ยังมีสถิติน้อย (ต้องอย่างน้อย {eco.min_evidence} รอบต่อโมเดลจึงใช้ตัดสิน)</div>{/if}
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
  .plans { display: grid; grid-template-columns: repeat(auto-fit, minmax(420px, 1fr)); gap: 14px; }
  .plans.single { grid-template-columns: 1fr; }
  .ctrl { gap: 10px; align-items: center; }
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
  .eco { width: 100%; border-collapse: collapse; font-size: 13px; }
  .eco th, .eco td { text-align: left; padding: 4px 6px; border-bottom: 1px solid rgba(128,128,128,.2); }
  .eco tr.best td { color: var(--teal); }
</style>
