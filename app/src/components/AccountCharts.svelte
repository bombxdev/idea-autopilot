<script>
  // Quota history of the accounts. Choose which accounts to show (one, several, all) and whether each gets its own
  // chart or they are drawn together as one averaged chart. The choice is remembered.
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";
  import { qv, loadQv, isShown } from "../lib/quotaview.svelte.js";

  let { days = 7 } = $props();
  let series = $state({});
  let accounts = $state([]);
  const W = 320;
  const H = 70;
  const STEPS = 96;

  async function load() {
    try {
      [series, accounts] = await Promise.all([api.accountsSeries(days), api.accounts().then((r) => r.accounts)]);
    } catch {}
  }
  onMount(() => {
    loadQv();
    load();
    const t = setInterval(load, 60000);
    return () => clearInterval(t);
  });

  const ids = $derived([...new Set([...accounts.map((a) => a.id), ...Object.keys(series)])]);
  const shown = $derived(ids.filter((id) => isShown(id)));

  const now = () => Math.floor(Date.now() / 1000);
  const info = (id) => accounts.find((a) => a.id === id);
  function line(rows, idx) {
    if (!rows?.length) return "";
    const t0 = now() - days * 86400;
    return rows.map((r) => `${(((r[0] - t0) / (days * 86400)) * W).toFixed(1)},${(H - Math.min(100, r[idx]) * (H / 100)).toFixed(1)}`).join(" ");
  }

  // one averaged series: at each step, the latest known value of every shown account, averaged
  const combined = $derived.by(() => {
    const t0 = now() - days * 86400;
    const out = [];
    for (let i = 0; i <= STEPS; i++) {
      const t = t0 + (i / STEPS) * days * 86400;
      let f = 0, w = 0, n = 0;
      for (const id of shown) {
        let last = null;
        for (const r of series[id] ?? []) {
          if (r[0] <= t) last = r;
          else break;
        }
        if (last) {
          f += last[1];
          w += last[2];
          n++;
        }
      }
      if (n) out.push([t, f / n, w / n]);
    }
    return out;
  });
  const nowAvg = $derived.by(() => {
    const q = shown.map((id) => info(id)?.quota).filter(Boolean);
    return q.length ? { five: q.reduce((a, b) => a + b.five, 0) / q.length, week: q.reduce((a, b) => a + b.week, 0) / q.length, n: q.length } : null;
  });
</script>

{#if ids.length}
  {#if !shown.length}
    <div class="faint small">เลือกอย่างน้อยหนึ่งบัญชี</div>
  {:else if qv.mode === "combined"}
    <div class="ch one">
      <div class="row small"><b class="grow">รวม {shown.length} บัญชี (ค่าเฉลี่ย)</b>
        {#if nowAvg}<span class="faint">ตอนนี้ {Math.round(nowAvg.five)}% / {Math.round(nowAvg.week)}%</span>{/if}</div>
      {#if combined.length}
        <svg viewBox="0 0 {W} {H}" preserveAspectRatio="none" role="img" aria-label="รวม">
          <line x1="0" y1={H * 0.4} x2={W} y2={H * 0.4} class="g" /><line x1="0" y1={H * 0.8} x2={W} y2={H * 0.8} class="g" />
          <polyline points={line(combined, 2)} class="w" /><polyline points={line(combined, 1)} class="f" />
        </svg>
        <div class="row small faint"><span><i class="k f"></i>5 ชม.</span><span><i class="k w"></i>สัปดาห์</span></div>
        <div class="mini small">
          {#each shown as id (id)}
            <span>{info(id)?.label || id}: {info(id)?.quota ? `${Math.round(info(id).quota.five)}% / ${Math.round(info(id).quota.week)}%` : "—"}</span>
          {/each}
        </div>
      {:else}<div class="faint small">ยังไม่มีข้อมูล</div>{/if}
    </div>
  {:else}
    <div class="grid">
      {#each shown as id (id)}
        {@const a = info(id)}
        <div class="ch" class:off={a && !a.enabled}>
          <div class="row small"><b class="grow">{a?.label || id}</b><span class="faint">{a?.kind_label ?? ""}{a && !a.enabled ? " · ปิดชั่วคราว" : ""}</span></div>
          {#if a?.limit && a.limit.kind !== "window"}
            <!-- no 5-hour / weekly windows at this provider: say what it really has instead of drawing empty bars -->
            <div class="small"><span class="chip">{a.limit.kind === "daily" ? "ลิมิตรายวัน" : "จ่ายตามใช้"}</span> {a.limit.text}</div>
            {#if a.limit.balance}<div class="small"><b>{a.limit.balance}</b></div>{/if}
          {:else if series[id]?.length}
            <svg viewBox="0 0 {W} {H}" preserveAspectRatio="none" role="img" aria-label="โควต้าของ {id}">
              <line x1="0" y1={H * 0.4} x2={W} y2={H * 0.4} class="g" /><line x1="0" y1={H * 0.8} x2={W} y2={H * 0.8} class="g" />
              <polyline points={line(series[id], 2)} class="w" /><polyline points={line(series[id], 1)} class="f" />
            </svg>
            <div class="row small faint"><span><i class="k f"></i>5 ชม.</span><span><i class="k w"></i>สัปดาห์</span>
              <span class="grow"></span>{#if a?.quota}<span>ตอนนี้ {Math.round(a.quota.five)}% / {Math.round(a.quota.week)}%</span>{/if}</div>
          {:else}<div class="faint small">ยังไม่มีข้อมูล (บันทึกทุก 5 นาทีเมื่อบัญชีเปิดอยู่)</div>{/if}
        </div>
      {/each}
    </div>
  {/if}
{/if}

<style>
  .ctl { gap: 10px; align-items: center; margin-bottom: 8px; }
  .pick { display: inline-flex; align-items: center; gap: 6px; border: 1px solid var(--line); border-radius: 999px; padding: 3px 10px; background: var(--bg-2); }
  .pick.off { opacity: 0.5; }
  .only { border: none; background: transparent; color: var(--text-3); font-size: 11px; padding: 0 2px; }
  .only:hover { color: var(--teal); background: transparent; }
  .seg { display: inline-flex; border: 1px solid var(--line); border-radius: 8px; overflow: hidden; }
  .seg button { border: none; border-radius: 0; padding: 4px 12px; background: transparent; }
  .seg button.on { background: var(--teal-bg); color: var(--teal); }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 10px; }
  .ch { border: 1px solid var(--line); border-radius: 10px; padding: 10px 12px; background: var(--bg-2); display: flex; flex-direction: column; gap: 6px; }
  .ch.one { max-width: 640px; }
  .ch.off { opacity: 0.55; }
  .mini { display: flex; flex-wrap: wrap; gap: 4px 14px; color: var(--text-2); }
  svg { width: 100%; height: 70px; background: var(--bg); border-radius: 6px; }
  .g { stroke: rgba(128, 128, 128, 0.25); stroke-width: 1; }
  polyline { fill: none; stroke-width: 1.6; vector-effect: non-scaling-stroke; }
  .f { stroke: var(--teal); }
  .w { stroke: var(--blue, #5b9bff); }
  .k { display: inline-block; width: 10px; height: 3px; margin-right: 4px; vertical-align: middle; }
  .k.f { background: var(--teal); }
  .k.w { background: var(--blue, #5b9bff); }
  .faint { color: var(--text-3); }
</style>
