<script>
  // Dashboard (ISSUE-173): the whole program at a glance — which ideas are being worked on, by which AI and model,
  // how far along, and what needs the owner. Three views, the last one picked is remembered:
  //   overview — numbers on top, a card per idea in work (ring %, model, step, time), recent ideas below
  //   focus    — calm one-line rows with a thin bar and the AI's latest words, to keep open beside work
  //   studio   — a cartoon 3D office (three.js): a desk per idea, an AI character (or the team's characters) at it
  import { onMount } from "svelte";
  import Icon from "../components/Icon.svelte";
  import PersonaAvatar from "../components/PersonaAvatar.svelte";
  import { api, pct } from "../lib/api.js";
  import { store, progress } from "../lib/store.svelte.js";
  import { vendorOf, glyphSvg } from "../lib/vendors.js";

  const VIEWS = [["overview", "ภาพรวม", "board"], ["focus", "เกาะติด", "list"], ["studio", "สตูดิโอ 3D", "sparkle"]];
  let view = $state(saved() ?? "overview");
  function saved() {
    try { return localStorage.getItem("dash_view"); } catch { return null; }
  }
  function pick(v) {
    view = v;
    try { localStorage.setItem("dash_view", v); } catch {}
  }

  let teams = $state([]);
  onMount(async () => {
    try { teams = (await api.teams()).teams ?? []; } catch {}
  });
  // the 3D studio is a separate chunk, loaded only when it is opened
  let Studio = $state(null);
  $effect(() => {
    if (view === "studio" && !Studio) import("../components/Studio3D.svelte").then((m) => (Studio = m.default));
  });

  const s = $derived(store.state);
  const ideaOf = (id) => store.ideas.find((i) => i.id === id);
  const stageLabel = (k) => store.stages.find((x) => x.key === k)?.label ?? k;
  // the AI's latest words as plain text (no table rows, no markdown marks) for a 2-line preview
  const plain = (t) => t.split("\n").filter((l) => !/^\s*\|?[\s:-]*\|[\s|:-]*$/.test(l) && !l.trim().startsWith("|")).join(" ").replace(/[#*\x60>_]|\[([^\]]*)\]\([^)]*\)/g, "$1").replace(/\s+/g, " ").trim();
  const lastSaid = (feed) => plain([...(feed ?? [])].reverse().find((f) => f.text.startsWith("💬"))?.text.replace(/^💬\s*/, "") ?? feed?.at(-1)?.text ?? "");
  const teamOf = (id) => {
    const slug = s?.idea_ai?.[id]?.team;
    return slug ? teams.find((t) => t.slug === slug) : null;
  };
  const openQ = (id) => store.questions.filter((q) => q.idea_id === id && !q.answer).length;

  // everything in work right now: the bot's run and every open chat
  const work = $derived.by(() => {
    const out = [];
    const c = s?.current;
    if (c?.idea_id) {
      const i = ideaOf(c.idea_id);
      out.push({ key: "run" + c.run_id, id: c.idea_id, title: i?.title ?? c.title, kind: "bot", vendor: c.provider, model: c.model,
        step: stageLabel(c.stage) + (c.stage_total ? stepOf(c.stage_no, c.stage_total) : ""), pct: i ? progress(i) : 0, since: c.started,
        last: c.last_line ?? "", status: "working", team: teamOf(c.idea_id) });
    }
    for (const x of s?.sessions ?? []) {
      if (out.some((o) => o.id === x.idea_id)) continue;
      const i = ideaOf(x.idea_id);
      const st = x.status === "error" ? "error" : x.status === "idle" ? (openQ(x.idea_id) ? "waiting" : "idle") : x.quiet > 60 ? "thinking" : "working";
      out.push({ key: "chat" + x.idea_id, id: x.idea_id, title: i?.title ?? x.name, kind: "chat", vendor: x.agent || "claude", model: x.ai,
        step: st === "idle" ? "แชตรอคำสั่ง" : st === "waiting" ? "รอคุณตอบ" : "กำลังทำในแชต", pct: i ? progress(i) : 0, since: x.busy_since || x.started,
        last: lastSaid(x.feed), status: st, team: teamOf(x.idea_id) });
    }
    return out;
  });
  // ideas that wait for the owner (a question), not in work
  const waiting = $derived(store.ideas.filter((i) => openQ(i.id) && !work.some((w) => w.id === i.id)).slice(0, 4)
    .map((i) => ({ key: "q" + i.id, id: i.id, title: i.title, kind: "idle", vendor: s?.idea_ai?.[i.id]?.account ? "" : "", model: "", step: "รอคุณตอบคำถาม", pct: progress(i), since: i.updated, last: "", status: "waiting", team: teamOf(i.id) })));
  const recent = $derived(store.ideas.filter((i) => !["archived", "deleted"].includes(i.status) && !work.some((w) => w.id === i.id))
    .sort((a, b) => (b.updated ?? 0) - (a.updated ?? 0)).slice(0, 8));

  const dayStart = $derived(new Date(new Date(store.now * 1000).setHours(0, 0, 0, 0)).getTime() / 1000);
  const doneToday = $derived(store.ideas.filter((i) => ["ready", "done"].includes(i.status) && (i.updated ?? 0) >= dayStart).length);
  const openQuestions = $derived(store.questions.filter((q) => !q.answer).length);
  const weekPct = $derived(s?.snap?.week_pct ?? s?.decision?.week_pct ?? null);

  const hm = (h, m) => h + " ชม. " + m + " นาที";
  const stepOf = (n, total) => " · ขั้น " + n + "/" + total;
  const ago = (t) => {
    if (!t) return "";
    const m = Math.max(0, Math.floor((store.now - t) / 60));
    return m < 1 ? "เพิ่งเริ่ม" : m < 60 ? m + " นาที" : hm(Math.floor(m / 60), m % 60);
  };
  const STATUS = { working: ["กำลังทำ", "teal"], thinking: ["กำลังคิด", "blue"], waiting: ["รอคุณ", "amber"], error: ["ติดปัญหา", "red"], idle: ["ว่าง", "faint"] };
  const IDEA_ST = { inbox: "รอตีความ", active: "กำลังทำ", ready: "พร้อมใช้", done: "เสร็จแล้ว", failed: "ติดปัญหา" };
  const R = 26, CIRC = 2 * Math.PI * R;
  const open = (id) => (store.openIdea = id);
</script>

<div class="dash">
  <div class="top">
    <h1>Dashboard</h1>
    <div class="views" role="tablist">
      {#each VIEWS as [k, l, ic] (k)}<button role="tab" class:on={view === k} aria-selected={view === k} onclick={() => pick(k)}><Icon name={ic} /> {l}</button>{/each}
    </div>
  </div>

  <div class="kpis">
    <div class="kpi"><span class="n teal">{work.filter((w) => w.status !== "idle").length}</span><span class="l">กำลังทำ</span></div>
    <div class="kpi" class:hot={openQuestions > 0}><span class="n amber">{openQuestions}</span><span class="l">รอคุณตอบ</span></div>
    <div class="kpi"><span class="n">{doneToday}</span><span class="l">เสร็จวันนี้</span></div>
    <div class="kpi"><span class="n">{weekPct == null ? "—" : pct(weekPct)}</span><span class="l">โควต้าสัปดาห์</span>
      {#if weekPct != null}<span class="kbar"><i style="width:{Math.min(100, weekPct)}%"></i></span>{/if}</div>
  </div>

  {#if view === "overview"}
    {#if work.length || waiting.length}
      <div class="cards">
        {#each [...work, ...waiting] as w (w.key)}
          {@const v = vendorOf(w.vendor)}
          <button class="wcard st-{w.status}" style="--v:{v.color}" onclick={() => open(w.id)}>
            <div class="ring">
              <svg viewBox="0 0 64 64" width="64" height="64"><circle cx="32" cy="32" r={R} class="track" /><circle cx="32" cy="32" r={R} class="val" stroke-dasharray={CIRC} stroke-dashoffset={CIRC * (1 - w.pct / 100)} /></svg>
              <b>{w.pct}%</b>
            </div>
            <div class="wbody">
              <div class="wtitle"><span class="idn">#{w.id}</span> {w.title}</div>
              <div class="wmeta">
                {#if w.vendor}<span class="vchip"><svg viewBox="0 0 20 20" width="14" height="14"><circle cx="10" cy="10" r="10" fill={v.color} />{@html glyphSvg(v.glyph)}</svg>{w.model || v.name}</span>{/if}
                <span class="schip {STATUS[w.status][1]}"><i></i>{STATUS[w.status][0]}</span>
                {#if w.since}<span class="faint small">{ago(w.since)}</span>{/if}
              </div>
              <div class="small muted">{w.step}</div>
              {#if w.last}<div class="last small">{w.last}</div>{/if}
              {#if w.team}
                <div class="team">{#each w.team.member_roles as m (m.slug)}<span title="{m.persona_name || m.name} · {m.title || m.name}">{#if m.look}<PersonaAvatar look={m.look} size={26} />{:else}<span class="dotname">{(m.persona_name || m.name).slice(0, 1)}</span>{/if}</span>{/each}<span class="small faint">{w.team.name}</span></div>
              {/if}
            </div>
          </button>
        {/each}
      </div>
    {:else}
      <div class="empty"><Icon name="sparkle" size="1.6em" /><div>ตอนนี้ยังไม่มี AI ทำงาน — จดไอเดีย หรือสั่งงานในแชตของไอเดียไหนก็ได้</div></div>
    {/if}

    <h3 class="sec">ไอเดียล่าสุด</h3>
    <div class="recent">
      {#each recent as i (i.id)}
        <button class="rrow" onclick={() => open(i.id)}>
          <span class="idn">#{i.id}</span><span class="rt">{i.title}</span>
          <span class="rst small faint">{IDEA_ST[i.status] ?? i.status}</span>
          <span class="rbar"><i style="width:{progress(i)}%"></i></span><span class="small rp">{progress(i)}%</span>
        </button>
      {/each}
    </div>
  {:else if view === "focus"}
    <div class="focus">
      {#each [...work, ...waiting] as w (w.key)}
        {@const v = vendorOf(w.vendor)}
        <button class="frow st-{w.status}" style="--v:{v.color}" onclick={() => open(w.id)}>
          <span class="fdot"></span>
          <span class="ft"><b>#{w.id}</b> {w.title}</span>
          <span class="fm small faint">{w.model || v.name} · {STATUS[w.status][0]}{w.since ? ` · ${ago(w.since)}` : ""}</span>
          <span class="fbar"><i style="width:{w.pct}%"></i></span><span class="small fp">{w.pct}%</span>
          {#if w.last}<span class="fl small">{w.last}</span>{/if}
        </button>
      {:else}<div class="empty small">ยังไม่มีงานที่กำลังทำ</div>{/each}
      {#each recent.slice(0, 5) as i (i.id)}
        <button class="frow quiet" onclick={() => open(i.id)}>
          <span class="fdot"></span><span class="ft"><b>#{i.id}</b> {i.title}</span><span class="fm small faint">{IDEA_ST[i.status] ?? i.status}</span>
          <span class="fbar"><i style="width:{progress(i)}%"></i></span><span class="small fp">{progress(i)}%</span>
        </button>
      {/each}
    </div>
  {:else}
    <div class="studio">
      {#if Studio}<Studio items={[...work, ...waiting]} onopen={open} />{:else}<div class="empty small">กำลังเปิดสตูดิโอ…</div>{/if}
    </div>
  {/if}
</div>

<style>
  .dash { display: flex; flex-direction: column; gap: 16px; }
  .top { display: flex; align-items: center; gap: 16px; flex-wrap: wrap; }
  .top h1 { margin: 0; flex: 1; }
  .views { display: inline-flex; background: var(--bg-2); border: 1px solid var(--line); border-radius: 12px; padding: 3px; gap: 2px; }
  .views button { border: none; background: transparent; border-radius: 9px; padding: 6px 12px; color: var(--text-2); display: inline-flex; gap: 6px; align-items: center; }
  .views button.on { background: var(--card); color: var(--text); box-shadow: 0 1px 4px rgba(0, 0, 0, .25); }
  .kpis { display: grid; grid-template-columns: repeat(auto-fit, minmax(160px, 1fr)); gap: 10px; }
  .kpi { background: var(--card); border: 1px solid var(--line); border-radius: 14px; padding: 12px 16px; display: flex; flex-direction: column; gap: 2px; }
  .kpi.hot { border-color: color-mix(in srgb, var(--amber, #f2b04a) 60%, transparent); }
  .kpi .n { font-size: 26px; font-weight: 700; }
  .kpi .l { color: var(--text-3); font-size: 13px; }
  .n.teal { color: var(--teal); } .n.amber { color: var(--amber, #f2b04a); }
  .kbar { height: 4px; background: var(--bg-2); border-radius: 4px; overflow: hidden; margin-top: 6px; }
  .kbar i { display: block; height: 100%; background: var(--teal); border-radius: 4px; }
  .cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(340px, 1fr)); gap: 12px; }
  .wcard { display: flex; gap: 14px; text-align: left; align-items: flex-start; background: linear-gradient(135deg, color-mix(in srgb, var(--v) 14%, var(--card)) 0%, var(--card) 55%);
    border: 1px solid color-mix(in srgb, var(--v) 35%, var(--line)); border-radius: 16px; padding: 14px; color: var(--text); transition: transform .15s, box-shadow .15s; }
  .wcard:hover { transform: translateY(-2px); box-shadow: 0 8px 22px rgba(0, 0, 0, .25); }
  .ring { position: relative; width: 64px; height: 64px; flex: none; }
  .ring svg { transform: rotate(-90deg); }
  .ring .track { fill: none; stroke: var(--bg-2); stroke-width: 6; }
  .ring .val { fill: none; stroke: var(--v); stroke-width: 6; stroke-linecap: round; transition: stroke-dashoffset .6s ease; }
  .ring b { position: absolute; inset: 0; display: grid; place-items: center; font-size: 14px; }
  .wbody { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 5px; }
  .wtitle { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .idn { color: var(--teal); font-weight: 600; margin-right: 4px; }
  .wmeta { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .vchip { display: inline-flex; align-items: center; gap: 5px; font-size: 12px; padding: 2px 8px 2px 3px; border-radius: 999px; background: color-mix(in srgb, var(--v) 18%, transparent); }
  .schip { display: inline-flex; align-items: center; gap: 5px; font-size: 12px; }
  .schip i { width: 7px; height: 7px; border-radius: 50%; background: currentColor; }
  .schip.teal { color: var(--teal); } .schip.blue { color: var(--blue, #6aa8ec); } .schip.amber { color: var(--amber, #f2b04a); } .schip.red { color: var(--red); } .schip.faint { color: var(--text-3); }
  .st-working .schip i, .st-thinking .schip i { animation: pulse 1.4s infinite; }
  .last { color: var(--text-2); display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .team { display: flex; align-items: center; gap: 4px; margin-top: 2px; }
  .dotname { display: inline-grid; place-items: center; width: 26px; height: 26px; border-radius: 50%; background: var(--teal-bg); color: var(--teal); font-size: 12px; }
  .empty { display: flex; gap: 10px; align-items: center; color: var(--text-3); padding: 18px; border: 1px dashed var(--line); border-radius: 14px; }
  .sec { margin: 6px 0 0; font-size: 15px; color: var(--text-2); }
  .recent { display: flex; flex-direction: column; border: 1px solid var(--line); border-radius: 14px; overflow: hidden; }
  .rrow { display: grid; grid-template-columns: 52px 1fr 90px 160px 44px; gap: 10px; align-items: center; text-align: left; border: none; border-radius: 0; background: transparent; padding: 9px 14px; color: var(--text); }
  .rrow + .rrow { border-top: 1px solid var(--line); }
  .rrow:hover { background: var(--bg-2); }
  .rt { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rbar, .fbar { height: 5px; background: var(--bg-2); border-radius: 5px; overflow: hidden; }
  .rbar i, .fbar i { display: block; height: 100%; background: var(--teal); border-radius: 5px; transition: width .6s ease; }
  .rp, .fp { text-align: right; color: var(--text-2); }
  .focus { display: flex; flex-direction: column; gap: 6px; }
  .frow { display: grid; grid-template-columns: 12px minmax(160px, 1fr) minmax(140px, 260px) 140px 40px; gap: 10px; align-items: center; text-align: left; background: var(--card);
    border: 1px solid var(--line); border-radius: 12px; padding: 10px 14px; color: var(--text); }
  .frow .fdot { width: 8px; height: 8px; border-radius: 50%; background: var(--v, var(--text-3)); }
  .frow.st-working .fdot, .frow.st-thinking .fdot { animation: pulse 1.4s infinite; }
  .frow.st-waiting .fdot { background: var(--amber, #f2b04a); } .frow.st-error .fdot { background: var(--red); }
  .frow.quiet { opacity: .75; }
  .frow .fbar i { background: var(--v, var(--teal)); }
  .ft, .fm { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .fl { grid-column: 2 / -1; color: var(--text-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .studio { position: relative; height: min(70vh, 640px); border-radius: 16px; overflow: hidden; border: 1px solid var(--line); background: var(--bg-2); }
  @keyframes pulse { 50% { opacity: .35; } }
  @media (max-width: 760px) { .rrow { grid-template-columns: 44px 1fr 44px; } .rrow .rst, .rrow .rbar { display: none; } .frow { grid-template-columns: 12px 1fr 40px; } .frow .fm, .frow .fbar { display: none; } }
</style>
