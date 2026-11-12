<script>
  // The quota card at the bottom of the menu (ISSUE-090), like the phone (ISSUE-088): the account the bot uses now,
  // one chosen account, or several accounts as small rings. The choice is kept on this machine.
  import Icon from "./Icon.svelte";
  import { store } from "../lib/store.svelte.js";
  import { pct } from "../lib/api.js";

  const read = () => { try { return JSON.parse(localStorage.getItem("quotaMini")) || null; } catch { return null; } };
  let view = $state(read() || { mode: "auto", one: "", many: [] });
  let setting = $state(false);
  function keep() {
    try { localStorage.setItem("quotaMini", JSON.stringify(view)); } catch {}
  }

  const s = $derived(store.state);
  const accts = $derived((s?.accounts ?? []).filter((a) => a.decision || a.snap));
  const weekOf = (a) => a.snap?.week_pct ?? a.decision?.week_pct ?? 0;
  const fiveOf = (a) => a.snap?.five_pct ?? a.decision?.five_pct ?? 0;
  const color = (p) => (p >= 85 ? "var(--red)" : p >= 60 ? "var(--amber)" : "var(--teal)");
  // what the bars show: the chosen account, else the bot's current one
  const one = $derived(view.mode === "one" ? accts.find((a) => a.id === view.one) : null);
  const d = $derived(one ? { ...one.decision, week_pct: weekOf(one), five_pct: fiveOf(one) } : s?.decision);
  const curLabel = $derived(one ? one.label || one.id : accts.find((a) => a.active)?.label);
  const many = $derived(accts.filter((a) => !view.many.length || view.many.includes(a.id)));
  const R = 13, C = 2 * Math.PI * R;
  function toggleMany(id) {
    view.many = view.many.includes(id) ? view.many.filter((x) => x !== id) : [...view.many, id];
    keep();
  }
</script>

{#if d}
  <div class="mini">
    <div class="head">
      <span class="faint small ellipsis" title={curLabel || ""}>{view.mode === "many" && accts.length ? "โควต้าสัปดาห์ · ทุกบัญชี" : curLabel || "โควต้า"}</span>
      <button class="gear" class:on={setting} onclick={() => (setting = !setting)} title="เลือกว่าจะแสดงโควต้าของบัญชีไหน"><Icon name="gear" size="0.95em" /></button>
    </div>
    {#if view.mode === "many" && accts.length}
      {#each many as a (a.id)}
        {@const w = weekOf(a)}
        <div class="acct" title="{a.label || a.id} — สัปดาห์ {pct(w)} · 5 ชม. {pct(fiveOf(a))}">
          <svg viewBox="0 0 32 32" width="30" height="30" aria-hidden="true">
            <circle cx="16" cy="16" r={R} fill="none" stroke="var(--bg-2)" stroke-width="4" />
            <circle cx="16" cy="16" r={R} fill="none" stroke={color(w)} stroke-width="4" stroke-linecap="round"
              stroke-dasharray="{(C * Math.min(100, w)) / 100} {C}" transform="rotate(-90 16 16)" />
          </svg>
          <div class="an">
            <div class="small ellipsis">{a.label || a.id}{#if a.active} <span class="now" title="บอทใช้บัญชีนี้อยู่">●</span>{/if}</div>
            <div class="faint tiny">{pct(w)} · 5 ชม. {pct(fiveOf(a))}</div>
          </div>
        </div>
      {/each}
    {:else}
      <div class="row small"><span class="muted grow" title="โควต้า AI ที่ใช้ไปแล้ว — ไม่ใช่ความคืบหน้าของไอเดีย (ความคืบหน้าดูที่การ์ดไอเดียและหน้า เช้านี้)">โควต้าสัปดาห์</span><b>{pct(d.week_pct)}</b></div>
      <div class="bar"><i style="width:{Math.min(100, d.week_pct)}%"></i><i class="res" style="width:{Math.min(100 - d.week_pct, d.reserve ?? 0)}%"></i></div>
      <div class="row small"><span class="muted grow" title="โควต้า AI ในรอบ 5 ชั่วโมง — ไม่ใช่ความคืบหน้าของไอเดีย">โควต้า 5 ชม.</span><b>{pct(d.five_pct)}</b></div>
      <div class="bar"><i class="five" style="width:{Math.min(100, d.five_pct)}%"></i></div>
    {/if}
    {#if setting}
      <div class="menu">
        <b class="small">แสดงโควต้าของ</b>
        <label><input type="radio" name="qm" checked={view.mode === "auto"} onchange={() => { view.mode = "auto"; keep(); }} /> บัญชีที่บอทใช้อยู่ (อัตโนมัติ)</label>
        {#each accts as a (a.id)}
          <label><input type="radio" name="qm" checked={view.mode === "one" && view.one === a.id} onchange={() => { view.mode = "one"; view.one = a.id; keep(); }} /> {a.label || a.id}</label>
        {/each}
        <label><input type="radio" name="qm" checked={view.mode === "many"} onchange={() => { view.mode = "many"; keep(); }} /> หลายบัญชี (วงกลม %)</label>
        {#if view.mode === "many"}
          <div class="faint tiny">ติ๊กบัญชีที่อยากเห็น — ไม่ติ๊กเลย = ทุกบัญชี</div>
          {#each accts as a (a.id)}
            <label class="sub"><input type="checkbox" checked={view.many.includes(a.id)} onchange={() => toggleMany(a.id)} /> {a.label || a.id}</label>
          {/each}
        {/if}
        <button class="small" onclick={() => (setting = false)}>เสร็จ</button>
      </div>
    {/if}
  </div>
{/if}

<style>
  .mini { position: relative; background: var(--card); border: 1px solid var(--line); border-radius: 12px; padding: 8px 12px 10px; display: flex; flex-direction: column; gap: 4px; }
  .head { display: flex; align-items: center; gap: 6px; margin-bottom: 2px; }
  .head span { flex: 1; min-width: 0; }
  .gear { border: none; background: transparent; padding: 2px 4px; color: var(--text-3); }
  .gear:hover, .gear.on { color: var(--teal); background: transparent; }
  .bar { height: 6px; background: var(--bg-2); border-radius: 6px; overflow: hidden; display: flex; margin-bottom: 4px; }
  .bar i { display: block; height: 100%; background: var(--teal); }
  .bar i.res { background: repeating-linear-gradient(45deg, var(--amber-bg), var(--amber-bg) 3px, transparent 3px, transparent 6px); }
  .bar i.five { background: var(--blue); }
  .acct { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .an { min-width: 0; flex: 1; }
  .tiny { font-size: 11px; }
  .now { color: var(--teal); font-size: 9px; vertical-align: 1px; }
  .menu { position: absolute; left: 0; right: 0; bottom: calc(100% + 6px); background: var(--bg-2); border: 1px solid var(--line-2); border-radius: 12px; padding: 10px 12px; display: flex; flex-direction: column; gap: 6px; box-shadow: 0 12px 30px rgba(0, 0, 0, 0.45); z-index: 30; max-height: 70vh; overflow: auto; }
  .menu label { display: flex; align-items: center; gap: 8px; font-size: 13px; cursor: pointer; }
  .menu label.sub { padding-left: 18px; }
  .menu input { width: auto; margin: 0; }
</style>
