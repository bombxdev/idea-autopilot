<script>
  // The plan for one account (or for the accounts together): the weekly ring (used by you / by the bot / kept for your
  // work / still available to the bot) and what the governor says right now.
  import { pct, clock } from "../lib/api.js";

  let { title = "", d, snap = null, userShare = null, shared = true, lines = [] } = $props();
  const R = 70;
  const C = 2 * Math.PI * R;
  // how much of the used quota is yours (only an account that shares its quota with you has any)
  const split = $derived.by(() => {
    const used = d?.week_pct ?? 0;
    if (!shared) return { user: 0, bot: used };
    const u = userShare ?? 0;
    return { user: Math.min(used, used * u), bot: used - Math.min(used, used * u) };
  });
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
    return list.map((x) => {
      const r = { ...x, off };
      off += x.v;
      return r;
    });
  });
  const hours = (n) => (n >= 24 ? `${Math.floor(n / 24)} วัน ${Math.round(n % 24)} ชม.` : `${n.toFixed(1)} ชม.`);
</script>

<div class="plan">
  {#if title}<div class="ptitle"><b>{title}</b></div>{/if}
  <div class="card ringcard">
    <svg viewBox="0 0 180 180" width="170" height="170">
      <circle cx="90" cy="90" r={R} fill="none" stroke="var(--bg-2)" stroke-width="20" />
      {#each segs as g}
        {#if g.v > 0}
          <circle cx="90" cy="90" r={R} fill="none" stroke={g.c} stroke-width="20" stroke-dasharray="{(g.v / 100) * C} {C}" stroke-dashoffset={-(g.off / 100) * C}
            transform="rotate(-90 90 90)" opacity={g.dash ? 0.55 : 1} />
        {/if}
      {/each}
      <text x="90" y="86" text-anchor="middle" class="big">{pct(d?.week_pct)}</text>
      <text x="90" y="108" text-anchor="middle" class="sub">ของสัปดาห์</text>
    </svg>
    <div class="legend">
      {#each segs as g}<div class="row small"><i style="background:{g.c};opacity:{g.dash ? 0.55 : 1}"></i><span class="grow">{g.label}</span><b>{pct(g.v, 1)}</b></div>{/each}
      <div class="faint small">รีเซ็ต {clock(snap?.week_reset)} (อีก {hours(d?.hours_to_reset ?? 0)})</div>
    </div>
  </div>

  <div class="card decide">
    <div class="row"><span class="dot" class:ok={d?.allow}></span><b class="grow">{d?.allow ? "บอททำงานได้" : "บอทรออยู่"}</b>
      {#if d?.sprint}<span class="chip sprint">🔥 โค้งสุดท้าย</span>{/if}
      {#if d?.turbo}<span class="chip sprint">⚡ Turbo</span>{/if}</div>
    <div class="muted small">{d?.reason}</div>
    <div class="kv">
      <span>ช่วงเวลา</span><b>{{ work: "เวลางาน", lunch: "พักเที่ยง", off: "นอกเวลางาน" }[d?.slot] ?? "—"}</b>
      <span>5 ชม. ตอนนี้</span><b>{pct(d?.five_pct)} / เพดาน {pct(d?.five_cap)}</b>
      <span>รีเซ็ต 5 ชม.</span><b>{clock(snap?.five_reset)}</b>
      <span>บอทใช้วันนี้</span><b>{pct(d?.bot_used_today, 1)} / {pct(d?.today_allowance, 1)}</b>
      <span>กันไว้ให้งานคุณ</span><b>{shared ? pct(d?.reserve, 1) : "ไม่ต้องกัน (บัญชีของบอท)"}</b>
    </div>
    {#each lines as l}<div class="small faint">{l}</div>{/each}
  </div>
</div>

<style>
  .plan { display: flex; flex-direction: column; gap: 10px; min-width: 0; }
  .ptitle { font-size: 15px; }
  .ringcard { display: flex; gap: 16px; align-items: center; flex-wrap: wrap; }
  .legend { flex: 1; min-width: 160px; display: flex; flex-direction: column; gap: 6px; }
  .legend i { width: 10px; height: 10px; border-radius: 3px; display: inline-block; margin-right: 8px; }
  .big { font-size: 28px; font-weight: 600; fill: var(--text); }
  .sub { font-size: 12px; fill: var(--text-3); }
  .decide { display: flex; flex-direction: column; gap: 8px; }
  .dot { width: 10px; height: 10px; border-radius: 50%; background: var(--amber); margin-right: 6px; }
  .dot.ok { background: var(--teal); }
  .kv { display: grid; grid-template-columns: auto 1fr; gap: 4px 14px; font-size: 13px; }
  .kv span { color: var(--text-3); }
  .chip.sprint { color: var(--amber); }
</style>
