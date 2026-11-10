<script>
  import IconText from "../components/IconText.svelte";
  // Sessions the app keeps open with Claude (Remote Control, one per idea): who is connected, what state it is in,
  // and the buttons to disconnect — so finished conversations do not pile up in the Claude app. A closed session can be
  // reconnected at any time and the conversation continues where it stopped.
  import { onMount } from "svelte";
  import { api, ago } from "../lib/api.js";
  import { store, toast, refresh } from "../lib/store.svelte.js";

  let data = $state({ open: [], resumable: [], idle_min: 45, has_claude: true });
  let busy = $state("");
  let now = $state(Math.floor(Date.now() / 1000));

  async function load() {
    try {
      data = await api.sessionsOverview();
    } catch {}
    now = Math.floor(Date.now() / 1000);
  }
  onMount(() => {
    load();
    const t = setInterval(load, 4000);
    return () => clearInterval(t);
  });
  $effect(() => {
    store.state?.sessions; // the feed moved
    load();
  });

  const title = (id) => store.ideas.find((i) => i.id === id)?.title ?? "";
  const STATUS = {
    starting: ["กำลังเปิด / Connecting", "amber"],
    working: ["AI กำลังทำ / Working", "teal"],
    idle: ["พร้อม รอคำสั่ง / Ready", "muted"],
    error: ["มีปัญหา / Problem", "red"],
  };
  const mins = (s) => (s < 60 ? "<1" : Math.floor(s / 60));
  // what a session of each AI is, in one line: where else it shows up, and how it carries on after it is closed
  const AGENT = {
    claude: { name: "Claude", note: "Remote Control — เห็นและคุยต่อในแอป Claude / claude.ai/code ได้ด้วย · ตัดแล้ว “ต่อกลับมา” ได้จากรายการด้านล่าง" },
    codex: { name: "Codex", note: "คุยในแอปนี้ ใช้บทสนทนาเดียวกับ Codex — เปิดต่อใน Codex เองได้ด้วยคำสั่ง codex resume (มีปุ่มคัดลอกในหน้าไอเดีย) · ไม่ขึ้นในแอป Claude" },
    gemini: { name: "Gemini", note: "คุยในแอปนี้เท่านั้น — แต่ละข้อความรัน Gemini หนึ่งรอบพร้อมบทสนทนาก่อนหน้า · ไม่มีในแอปของ Google · ตัดแล้วส่งข้อความใหม่ได้เลย บทสนทนาเดิมยังอยู่" },
    opencode: { name: "OpenCode", note: "คุยในแอปนี้เท่านั้น — แต่ละข้อความรัน OpenCode หนึ่งรอบพร้อมบทสนทนาก่อนหน้า (ใช้ได้กับบัญชี OpenRouter และค่ายอื่นที่ผ่าน OpenCode) · ตัดแล้วส่งข้อความใหม่ได้เลย บทสนทนาเดิมยังอยู่" },
    mistral: { name: "Mistral", note: "คุยในแอปนี้เท่านั้น — แต่ละข้อความรัน Mistral Vibe หนึ่งรอบพร้อมบทสนทนาก่อนหน้า · ตัดแล้วส่งข้อความใหม่ได้เลย บทสนทนาเดิมยังอยู่" },
  };
  const ag = (s) => AGENT[s.agent] ?? AGENT.claude;

  async function run(key, job, ok) {
    busy = key;
    try {
      await job();
      if (ok) toast(ok);
    } catch (e) {
      toast(String(e), "err");
    }
    busy = "";
    await load();
    refresh();
  }
  const closeSum = (s) => run(`c${s.idea_id}`, () => api.sessionClose(s.idea_id, true), "ให้ AI สรุปสิ่งที่คุยกันแล้วปิด session");
  const closeNow = (s) => run(`x${s.idea_id}`, () => api.sessionClose(s.idea_id, false), "ตัด session แล้ว");
  const reconnect = (r) => run(`r${r.idea_id}`, () => api.remote(r.idea_id, true), "กำลังต่อ session กลับมา — คุยต่อจากเดิมได้");
  const closeAll = () => run("all", () => api.sessionCloseAll(true), "กำลังสรุปและปิดทุก session");
  async function setIdle(v) {
    await api.setSetting("session_idle_min", String(v));
    load();
  }
</script>

<div class="wrapx">
  <h1>เซสชัน · Sessions</h1>
  <p class="muted small">
    เมื่อคุณคุยกับ AI ในช่อง “สั่ง AI แก้ / ทำต่อ” ของไอเดีย แอปจะเปิด session ให้ไอเดียละหนึ่งอัน หน้านี้บอกว่าตอนนี้ต่ออยู่กี่อัน สถานะเป็นอย่างไร
    และให้ <b>ตัดการเชื่อมต่อ</b> เมื่อคุยจบ (ก่อนตัด AI สรุปสิ่งที่คุยกันเก็บไว้กับไอเดียให้ได้ทุกค่าย)
  </p>
  <div class="card small legend">
    <b>AI แต่ละค่ายทำได้ไม่เท่ากัน</b>
    <div class="lg"><span class="chip">Claude</span><span>เปิดเป็น Remote Control — เห็นในแอป Claude / claude.ai/code ด้วย และตัดแล้ว <b>ต่อกลับมา</b> ได้ (เฉพาะ Claude)</span></div>
    <div class="lg"><span class="chip">Codex</span><span>คุยในแอปนี้ บทสนทนาเดียวกับ Codex — เปิดต่อใน Codex เองได้ด้วย <code>codex resume</code></span></div>
    <div class="lg"><span class="chip">Gemini · OpenCode · Mistral</span><span>คุยในแอปนี้เท่านั้น — แต่ละข้อความรันหนึ่งรอบพร้อมบทสนทนาก่อนหน้า ไม่ต้องต่อกลับ ส่งข้อความใหม่ได้เลย</span></div>
  </div>
  {#if !data.has_claude}
    <div class="card amber small">ยังไม่พบ Claude Code ในเครื่องนี้ — Remote Control และ “ต่อกลับมา” ใช้ไม่ได้ แต่คุยกับ AI ค่ายอื่นในหน้าไอเดียได้ตามปกติ</div>
  {/if}

  <section>
    <div class="row">
      <h2 class="grow">กำลังเชื่อมต่อ ({data.open.length})</h2>
      {#if data.open.length > 1}<button class="small" disabled={busy === "all"} onclick={closeAll}>สรุปและตัดทั้งหมด</button>{/if}
    </div>
    {#each data.open as s (s.idea_id)}
      <div class="card sess">
        <div class="row wrap">
          <b class="grow">#{s.idea_id} {title(s.idea_id)}</b>
          <span class="chip" title={ag(s).note}>{ag(s).name}</span>
          <span class="chip {STATUS[s.status]?.[1] ?? 'muted'}">
            {#if s.status === "working" || s.status === "starting"}<i class="pulse"></i>{/if}{STATUS[s.status]?.[0] ?? s.status}
          </span>
        </div>
        <div class="small faint">
          ชื่อ session: {s.name} · เปิดมา {mins(now - s.started)} นาที · ไม่มีความเคลื่อนไหว {mins(s.idle_secs)} นาที
          {#if data.idle_min > 0 && s.status === "idle"} · จะตัดเองเมื่อเงียบครบ {data.idle_min} นาที{/if}
        </div>
        <div class="small faint">{ag(s).note}</div>
        {#if s.feed?.length}
          <div class="feed">{#each s.feed as f}<div class="small"><IconText text={f.text} /></div>{/each}</div>
        {/if}
        <div class="row wrap">
          <button class="small primary" disabled={busy === `c${s.idea_id}`} onclick={() => closeSum(s)} title="ให้ AI สรุปสิ่งที่คุยกันและบันทึกไว้กับไอเดีย แล้วปิด session">สรุปแล้วตัดการเชื่อมต่อ</button>
          <button class="small" disabled={busy === `x${s.idea_id}`} onclick={() => closeNow(s)} title="ปิดทันที (บทสนทนายังถูกเก็บไว้ ต่อกลับมาได้)">ตัดทันที</button>
          <button class="small ghost" onclick={() => (store.openIdea = s.idea_id)}>เปิดไอเดีย</button>
        </div>
      </div>
    {:else}
      <div class="card faint small">ไม่มี session ที่เชื่อมต่ออยู่ — เปิดได้จากช่อง “สั่ง AI แก้ / ทำต่อ” ในหน้าไอเดีย (ปุ่ม “เปิด session โดยยังไม่ส่งข้อความ”)</div>
    {/each}
  </section>

  <section>
    <h2>เคยคุยไว้ — ต่อกลับมาได้ ({data.resumable.length}) <span class="chip">เฉพาะ Claude</span></h2>
    <p class="faint small">Codex และค่ายอื่นไม่ต้องต่อกลับ — ส่งข้อความใหม่ในหน้าไอเดียได้เลย บทสนทนาเดิมถูกส่งต่อให้เอง</p>
    {#each data.resumable.slice(0, 20) as r (r.idea_id)}
      <div class="card row wrap sess2">
        <span class="grow"><b>#{r.idea_id}</b> {r.title}</span>
        <span class="faint small">{ago(r.last)}</span>
        <button class="small" disabled={!data.has_claude || busy === `r${r.idea_id}`} onclick={() => reconnect(r)}>ต่อกลับมา</button>
      </div>
    {:else}
      <div class="faint small">ยังไม่มีบทสนทนาที่ปิดไว้</div>
    {/each}
  </section>

  <section class="card">
    <h2>ตัดเองเมื่อไม่ได้ใช้งาน</h2>
    <div class="row field small">
      <span class="grow">ถ้า session เงียบ (พร้อมรอคำสั่งแต่ไม่มีใครคุย) นานเกิน</span>
      <select value={String(data.idle_min)} onchange={(e) => setIdle(e.currentTarget.value)}>
        <option value="0">ไม่ตัดเอง</option><option value="15">15 นาที</option><option value="30">30 นาที</option><option value="45">45 นาที (ค่าเริ่มต้น)</option><option value="90">90 นาที</option><option value="240">4 ชั่วโมง</option>
      </select>
    </div>
    <p class="faint small">ก่อนตัดเอง AI จะสรุปสิ่งที่คุยกันและบันทึกกับไอเดียให้ก่อน (ใช้ token เล็กน้อย) บทสนทนายังต่อกลับมาได้เสมอ · session ที่ AI กำลังทำงานอยู่จะไม่ถูกตัด</p>
  </section>
</div>

<style>
  .wrapx { display: flex; flex-direction: column; gap: 14px; }
  .wrapx p { margin: 0; }
  section { display: flex; flex-direction: column; gap: 8px; }
  .sess { display: flex; flex-direction: column; gap: 6px; }
  .feed { background: var(--bg-2); border-radius: 8px; padding: 6px 10px; display: flex; flex-direction: column; gap: 2px; color: var(--text-2); }
  .chip.teal { color: var(--teal); }
  .chip.amber { color: var(--amber); }
  .chip.red { color: var(--red); }
  .chip.muted { color: var(--text-2); }
  .pulse { display: inline-block; width: 7px; height: 7px; border-radius: 50%; background: currentColor; margin-right: 6px; animation: pulse 1.2s ease-in-out infinite; }
  @keyframes pulse { 50% { opacity: 0.25; } }
  .faint { color: var(--text-3); }
  .legend { display: flex; flex-direction: column; gap: 6px; }
  .lg { display: flex; gap: 8px; align-items: baseline; }
  .lg .chip { flex: none; }
  .amber { color: var(--amber); }
</style>
