<script>
  import FeedText from "./FeedText.svelte";
  import { autogrow } from "../lib/autogrow.js";
  import { codebox } from "../lib/codebox.js";
  import Icon from "./Icon.svelte";
  // สั่ง AI แก้/ทำต่อในไอเดีย: ส่งข้อความเข้า Remote Control session "Idea#<id>" (มีอยู่แล้วส่งเข้าอันเดิม)
  // แล้วแสดงว่า AI กำลังทำอะไรเป็นภาษาคน (แปลงจาก transcript ด้วยกฎ ไม่ใช้ token เพิ่ม)
  import { lift } from "../lib/lift.js";
  import { api, dayBreak } from "../lib/api.js";
  import { store, toast, refresh } from "../lib/store.svelte.js";
  import Attach from "./Attach.svelte";
  import SkillMenu from "./SkillMenu.svelte";
  import IdeaAi from "./IdeaAi.svelte";
  import { locale } from "../lib/i18n.js";
  import { ui } from "../lib/uilang.svelte.js";

  let { id } = $props();
  let text = $state("");
  let sending = $state(false);
  // the AI for this chat: collapsed and automatic until the owner opens it and chooses (account → model → effort)
  let ai = $state({ account: "", model: "", effort: "" });
  let accts = $state([]);
  $effect(() => {
    api.accounts().then((r) => (accts = r?.accounts ?? [])).catch(() => {});
  });
  // automatic = the AI set for this idea, if any (the backend resolves it the same way)
  const acct = $derived(accts.find((a) => a.id === (ai.account || store.state?.idea_ai?.[id]?.account)));
  const agent = $derived(acct?.kind ?? "");
  // the button names the account (OpenRouter), and the program only when it differs (OpenRouter runs through OpenCode)
  const sendTo = $derived.by(() => {
    if (!picked || (picked.id === "claude" && !acct)) return "AI";
    const prog = picked.label.split(" ")[0];
    const name = acct ? acct.label || acct.id : prog;
    return name.toLowerCase() === prog.toLowerCase() ? name : `${name} (ผ่าน ${prog})`;
  });
  let feedEl;
  let staged = $state([]);
  let att;
  let askEl = $state();
  let box = $state();
  // the box grows with the text; ⤢ opens it big for a long instruction with files (ISSUE-078)
  let big = $state(false);
  function toggleBig() {
    big = !big;
    requestAnimationFrame(() => box?.focus());
  }
  let menu; // "/" opens the list of skills (they work with every AI, see skills.rs)

  // only AI agents installed on this machine are offered. Claude shows a live feed here; the others
  // are opened in Orca (or a terminal window) on the idea's own repo
  const agents = $derived(store.state?.agents ?? []);
  const picked = $derived(agents.find((a) => a.id === agent) ?? agents[0]);

  const s = $derived((store.state?.sessions ?? []).find((x) => x.idea_id === id));
  // the Codex conversation of this idea: the same one can be opened in Codex itself (`codex resume`)
  let cthread = $state(null);
  $effect(() => {
    s?.feed?.length; // refresh after something happened in the chat
    id;
    api.codexThread(id).then((r) => (cthread = r)).catch(() => (cthread = null));
  });
  async function copyResume() {
    try {
      await navigator.clipboard.writeText(cthread.command);
      toast("คัดลอกคำสั่งแล้ว — วางใน terminal เพื่อคุยต่อใน Codex เอง");
    } catch {
      toast(cthread.command);
    }
  }
  let full = $state(false); // feed shown over the whole window
  // AIs that print line by line (Vibe, Gemini…) split a ``` block over several feed lines: join them back into one
  function joinFences(list) {
    const out = [];
    for (const f of list) {
      const prev = out[out.length - 1];
      if (prev && (prev.text.match(/\x60{3}/g) ?? []).length % 2 === 1) prev.text += "\n" + f.text.replace(/^💬\s?/u, "");
      else out.push({ ...f });
    }
    return out;
  }
  const feed = $derived(joinFences((s?.feed ?? []).slice(full ? -200 : -30)));
  // a long message shows a few lines; "ดูเต็ม" opens it right there and "ย่อ" folds it back (ISSUE-074)
  let opened = $state({});
  const isLong = (t) => t.length > 280 || t.split("\n").length > 4;
  const STATUS = {
    starting: { label: "กำลังเปิด", cls: "amber" },
    working: { label: "AI กำลังทำ", cls: "teal" },
    idle: { label: "รอคำสั่ง", cls: "muted" },
    error: { label: "มีปัญหา", cls: "red" },
  };
  const st = $derived(s ? STATUS[s.status] ?? STATUS.idle : null);
  // "working" yet nothing has moved for minutes: say so instead of a cheerful "working" (ISSUE-063)
  const quietMin = $derived(s && s.status === "working" && (s.quiet ?? 0) >= 180 ? Math.floor(s.quiet / 60) : 0);
  // a one-message-at-a-time AI (Vibe, Codex…) still on a long reply: what you send waits until it ends (ISSUE-068)
  const busyMin = $derived(s?.busy_since ? Math.max(0, Math.floor((store.now - s.busy_since) / 60)) : -1);
  let cutting = $state(false);
  async function cutIn() {
    cutting = true;
    try {
      await api.sessionCutIn(id);
      toast(s?.waiting ? "หยุดรอบเดิมแล้ว — ข้อความของคุณจะถึง AI ทันที" : "หยุดรอบที่ทำอยู่แล้ว");
    } catch (e) {
      toast(String(e), "err");
    } finally {
      cutting = false;
      refresh();
    }
  }
  let reviving = $state(false);
  async function revive() {
    reviving = true;
    try {
      await api.sessionRevive(id);
      toast("กำลังเปิด session ใหม่ — คุยต่อจากเดิม และบอก AI ให้ทำต่อจากที่ค้าง");
    } catch (e) {
      toast(String(e), "err");
    } finally {
      reviving = false;
    }
  }

  $effect(() => {
    feed.length;
    feedEl?.scrollTo({ top: feedEl.scrollHeight, behavior: "smooth" });
  });

  async function send() {
    const t = text.trim();
    if ((!t && !staged.length) || sending) return;
    if (att?.pending()) {
      toast("รอไฟล์แนบอัปโหลดก่อน", "err");
      return;
    }
    sending = true;
    const files = staged.map((s) => s.token);
    try {
      if (!picked) throw "ยังไม่พบ AI ในเครื่องนี้ — ติดตั้ง Claude Code, Codex, Gemini CLI หรือ OpenCode ก่อน";
      if (picked.via !== "app") {
        toast(await api.handoff(id, picked.id, t, files, ai));
      } else {
        await api.ask(id, t, files, ai);
        toast(s ? "ส่งเข้า session เดิมแล้ว" : "กำลังเปิด session แล้วส่งคำสั่งให้ AI…");
      }
      att?.reset();
      text = "";
      refresh();
    } catch (e) {
      toast(String(e), "err");
    } finally {
      sending = false;
    }
  }
  // after a crash or a bug: carry on with the last request without typing it again (ISSUE-072)
  const CARRY = "↻ ทำต่อจากคำสั่งล่าสุดของผมที่ยังไม่เสร็จ (ในบทสนทนานี้และ DIRECTIVES.md) — รอบก่อนติดปัญหาของโปรแกรม ไม่ต้องให้ผมพิมพ์ซ้ำ ทำได้เลย แล้วบอกผลว่าใช้ได้จริงไหม";
  const canCarry = $derived(picked?.via === "app" && (!s || s.status === "idle" || s.status === "error"));
  async function carryOn() {
    text = CARRY;
    await send();
  }
  // open the conversation without sending anything: to carry on in the Claude app yourself. What is said there
  // comes back to the idea (notes + CHAT-LOG.md), and the Sessions page can disconnect it when you are done.
  async function openOnly() {
    try {
      await api.remote(id, true);
      toast("กำลังเปิด session — เปิดคุยต่อในแอป Claude ได้เลย สิ่งที่คุยจะถูกสรุปกลับมาที่ไอเดียนี้");
      refresh();
    } catch (e) {
      toast(String(e), "err");
    }
  }
  async function close() {
    try {
      await api.remote(id, false);
      toast("ปิด session แล้ว — ครั้งหน้าจะคุยต่อจากเดิม");
      refresh();
    } catch (e) {
      toast(String(e), "err");
    }
  }
  const hhmm = (ts) => new Date(ts * 1000).toLocaleTimeString(locale(ui.lang), { hour: "2-digit", minute: "2-digit" });
  const kind = (t) => (t.startsWith("💬") ? "say" : t.startsWith("🙋") ? "me" : t.startsWith("✅") || t.startsWith("📝") ? "done" : t.startsWith("⚠️") ? "warn" : "step");
</script>

<div class="ask" class:on={!!s} bind:this={askEl}>
  <div class="head">
    <b><Icon name="wrench" /> สั่ง AI แก้ / ทำต่อ</b>
    {#if quietMin}
      <span class="badge amber" title="ป้ายบอกว่ากำลังทำ แต่บทสนทนาไม่ขยับเลย — อาจกำลังรอคำสั่งที่ใช้เวลานาน หรือค้าง">AI เงียบมา {quietMin} นาที</span>
    {:else if st}<span class="badge {st.cls}">{#if s.status === "working" || s.status === "starting"}<i class="pulse"></i>{/if}{st.label}</span>{/if}
    {#if s?.ai}<span class="badge ai" title="AI ที่คุยอยู่ในแชตนี้ (บัญชี · โมเดลที่ตอบจริง)"><Icon name="bot" /> {s.ai}</span>{/if}
    <span class="grow"></span>
    {#if s}
      <button class="ghost small" onclick={() => (store.page = "sessions")} title="ดูและจัดการ session ทั้งหมด">จัดการ session</button>
      <button class="ghost small" onclick={close} title="ปิด session (คุยต่อจากเดิมได้ภายหลัง)">ปิด session</button>
    {:else if !picked || picked.id === "claude"}
      <button class="ghost small" onclick={openOnly} title="เปิด session โดยไม่ส่งข้อความ เพื่อไปคุยต่อเองในแอป Claude — สิ่งที่คุยจะถูกสรุปกลับมาที่ไอเดียนี้">เปิด session โดยยังไม่ส่งข้อความ</button>
    {/if}
  </div>

  {#if busyMin >= 5 || (busyMin >= 0 && s.waiting)}
    <div class="quiet small">
      <div class="row wrap">
        <span class="grow">AI ยังทำรอบนี้อยู่ {busyMin} นาที{#if s.waiting}<span class="sep">· ข้อความของคุณ {s.waiting} ข้อความรอให้รอบนี้จบก่อน (AI ตัวนี้รับทีละข้อความ)</span>{/if}</span>
        <button class="small" disabled={cutting} onclick={cutIn} title="หยุดสิ่งที่ AI ทำอยู่ (ไฟล์ที่ทำไปแล้วยังอยู่) แล้วส่งข้อความที่รอให้ทันที">{s.waiting ? "หยุดรอบนี้ แล้วส่งข้อความเลย" : "หยุดรอบนี้"}</button>
      </div>
    </div>
  {/if}

  {#if quietMin}
    <div class="quiet small">
      <div class="row wrap">
        <span class="grow">ถ้า AI กำลังรอคำสั่งที่ใช้เวลานาน (เช่น build) ปล่อยไว้ได้ · ถ้าค้าง กดเปิดใหม่ — คุยต่อจากเดิม ไม่เสียสิ่งที่ทำไป</span>
        <button class="small" disabled={reviving} onclick={revive}>เปิด session ใหม่</button>
      </div>
      {#if s.screen}<div class="faint">หน้าจอ Claude Code ตอนนี้:</div><pre>{s.screen}</pre>{/if}
    </div>
  {/if}

  <div class="compose" class:big use:lift={big}>
    {#if big}<b class="bigtitle"><Icon name="wrench" /> สั่ง AI แก้ / ทำต่อ — Idea #{id}</b>{/if}
    <div class="tw">
    <SkillMenu bind:text bind:this={menu} el={box} />
    <textarea
      bind:this={box}
      use:autogrow={[big ? 0 : 0.5, text]}
      use:codebox={text}
      rows="2"
      bind:value={text}
      onpaste={(e) => att?.paste(e)}
      placeholder="บอกสิ่งที่อยากให้แก้ เช่น “ทำปุ่มให้ใหญ่ขึ้น และเพิ่มโหมดมืด” · พิมพ์ / เพื่อเรียกสกิล · Ctrl+V วางภาพได้"
      onkeydown={(e) => {
        if (menu?.key(e)) return;
        if (e.key === "Escape" && big) big = false;
        if (e.key === "Enter" && (e.ctrlKey || e.metaKey) && !e.isComposing) send();
      }}
    ></textarea>
    </div>
    <div class="bar">
      <Attach bind:this={att} bind:staged priority={3} zone={() => askEl} />
      <button type="button" class="ghost small" onclick={toggleBig} title={big ? "ย่อกลับ (Esc)" : "ขยายเป็นช่องใหญ่ — สำหรับคำสั่งยาวๆ และแนบรูป"}><Icon name={big ? "collapse" : "layers"} /> {big ? "ย่อกลับ" : "ขยาย"}</button>
      {#if canCarry && !text.trim()}
        <button class="ghost small" disabled={sending} onclick={carryOn} title="รอบก่อนพัง ค้าง หรือโปรแกรมติดบั๊ก: ให้ AI ทำคำสั่งล่าสุดที่ยังไม่เสร็จต่อเลย ไม่ต้องพิมพ์ซ้ำ (มันเห็นบทสนทนาเดิมและคำสั่งทั้งหมดของไอเดียนี้)"><Icon name="refresh" /> ทำต่อจากที่สั่งไว้</button>
      {/if}
      <span class="grow"></span>
      <button class="primary send" onclick={send} disabled={sending || (!text.trim() && !staged.length)}>{sending ? "กำลังส่ง…" : `ส่งให้ ${sendTo}`}</button>
    </div>
  </div>
  <div class="aiwrap"><IdeaAi bind:value={ai} title="AI สำหรับแชตนี้" autoNote="ถ้าไม่เลือก ใช้ AI/โมเดลที่ตั้งไว้ให้ไอเดียนี้ ไม่ก็ค่าเริ่มต้นของ Claude" /></div>
  <div class="hint small faint">
    {#if !picked}ยังไม่พบ AI ในเครื่องนี้ — ติดตั้งอย่างใดอย่างหนึ่งแล้วกลับมาใหม่
    {:else if picked.id === "codex"}คุยกับ Codex ตรงนี้ได้เลย — แต่ละข้อความรัน Codex ต่อจากบทสนทนาเดิมในโฟลเดอร์ของ Idea #{id} (ดูมือถือ/แอป Claude ไม่ได้) · Ctrl+Enter ส่ง · บอทกลางคืนจะเว้นไอเดียนี้ระหว่างเปิด
    {:else if ["gemini", "opencode", "mistral"].includes(picked.id)}คุยกับ {picked.label.split(" ")[0]} ตรงนี้ได้เลย — แต่ละข้อความรัน {picked.label.split(" ")[0]} หนึ่งรอบในโฟลเดอร์ของ Idea #{id} พร้อมส่งบทสนทนาก่อนหน้าไปด้วย (ดูมือถือ/แอป Claude ไม่ได้) · Ctrl+Enter ส่ง · บอทกลางคืนจะเว้นไอเดียนี้ระหว่างเปิด
    {:else if picked.via === "orca"}{picked.hint} — แอปจะ commit เก็บไว้ก่อน แล้วเปิดใน Orca บน repo ของ Idea #{id} · ผลสรุปจะกลับมาที่ไอเดียเมื่อเสร็จ
    {:else if picked.via === "terminal"}{picked.hint} — แอปจะ commit เก็บไว้ก่อน แล้วเปิดหน้าต่าง terminal ใหม่ในโฟลเดอร์ของ Idea #{id} ให้ AI อ่านคำสั่งจาก HANDOFF.md
    {:else if s}ส่งเข้า session เดิม <b>{s.name}</b> · เปิดดู/คุยต่อในแอป Claude ได้ · Ctrl+Enter ส่ง
    {:else}จะเปิด session “Idea#{id} : …” ให้ (คุยต่อจากครั้งก่อน ถ้าเคยเปิด) · ดูในแอป Claude บนมือถือได้ด้วย · บอทกลางคืนจะเว้นไอเดียนี้ระหว่างเปิด{/if}
  </div>

  {#if cthread}
    <div class="small faint resume"><Icon name="terminal" /> คุยต่อใน Codex เองได้ด้วยบทสนทนาเดียวกัน: <code>{cthread.command}</code> <button type="button" class="ghost small" onclick={copyResume}>คัดลอก</button></div>
  {/if}
  {#if feed.length}
    <div class="feedbox" class:full use:lift={full}>
      <div class="ftools">
        <span class="small faint">ความคืบหน้าของ AI ({feed.length})</span>
        <span class="grow"></span>
        <button type="button" class="ghost small" onclick={() => (full = !full)} title={full ? "ย่อกลับ (Esc)" : "ขยายเต็มจอ"}><Icon name={full ? "collapse" : "expand"} /> {full ? "ย่อ" : "ขยาย"}</button>
      </div>
      <div class="feed" bind:this={feedEl}>
        {#each feed as f, i (f.ts + ":" + i)}
          {@const k = f.ts + ":" + f.text.length}
          {@const long = isLong(f.text)}
          {@const day = dayBreak(feed, i)}
          {#if day}<div class="daybreak"><span>{day}</span></div>{/if}
          <div class="line {kind(f.text)}"><span class="t">{hhmm(f.ts)}</span><div class="x"><div class="body" class:clamp={long && !opened[k]}><FeedText text={f.text} /></div>{#if long}<button type="button" class="more" onclick={() => (opened[k] = !opened[k])}>{opened[k] ? "ย่อ ▴" : "ดูเต็ม ▾"}</button>{/if}</div></div>
        {/each}
      </div>
    </div>
  {/if}
</div>
<svelte:window onkeydown={(e) => { if (e.key === "Escape" && full) full = false; }} />

<style>
  .ask { display: flex; flex-direction: column; gap: 8px; padding: 12px; border: 1px solid var(--line); border-radius: 12px; background: var(--bg-2); }
  .ask.on { border-color: var(--teal); }
  .head { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; }
  .badge { display: inline-flex; align-items: center; gap: 6px; font-size: 12px; padding: 2px 10px; border-radius: 999px; background: var(--card-2); }
  .badge.teal { background: var(--teal-bg); color: var(--teal); }
  .badge.amber { color: var(--amber); }
  .quiet { border: 1px solid var(--amber); background: var(--amber-bg); border-radius: 10px; padding: 8px 10px; display: flex; flex-direction: column; gap: 6px; }
  .quiet .row { gap: 8px; align-items: center; }
  .quiet .sep { margin-inline-start: 0.35em; }
  .quiet pre { margin: 0; max-height: 120px; overflow: auto; white-space: pre-wrap; font-size: 11px; background: var(--card); border-radius: 6px; padding: 6px 8px; }
  .badge.red { color: var(--red); }
  .badge.muted { color: var(--text-2); }
  .pulse { width: 7px; height: 7px; border-radius: 50%; background: currentColor; animation: pulse 1.2s ease-in-out infinite; }
  @keyframes pulse { 50% { opacity: .25; transform: scale(.7); } }
  .aiwrap { display: block; }
  .resume code { background: var(--bg); padding: 1px 6px; border-radius: 4px; overflow-wrap: anywhere; }
  .feedbox { display: flex; flex-direction: column; gap: 4px; }
  .ftools { display: flex; align-items: center; gap: 8px; }
  .feedbox.full { position: fixed; top: 4vh; left: 4vw; width: 92vw; height: 92vh; max-height: calc(100dvh - 24px); z-index: 160; box-sizing: border-box; padding: 14px; background: var(--bg-2); border: 1px solid var(--teal); border-radius: 14px; box-shadow: 0 20px 60px rgba(0, 0, 0, 0.55); }
  .feedbox.full .feed { max-height: none; flex: 1 1 0; min-height: 0; overflow-y: auto; font-size: 14px; }
  .feed { max-height: min(46vh, 420px); overflow: auto; display: flex; flex-direction: column; gap: 3px; padding: 8px 10px; background: var(--bg); border-radius: 8px; font-size: 13px; line-height: 1.5; }
  .line { display: flex; gap: 8px; }
  .line .t { color: var(--text-3); font-variant-numeric: tabular-nums; flex: none; }
  .line .x { overflow-wrap: anywhere; min-width: 0; flex: 1; }
  .line .body { white-space: pre-line; }
  /* a quiet line with the date where the day changes (ISSUE-075) */
  .daybreak { display: flex; align-items: center; gap: 8px; margin: 6px 0 2px; font-size: 11px; color: var(--text-3); }
  .daybreak::before, .daybreak::after { content: ""; flex: 1; border-top: 1px solid var(--line); }
  .line .body.clamp { display: -webkit-box; -webkit-line-clamp: 3; line-clamp: 3; -webkit-box-orient: vertical; overflow: hidden; }
  .line .more { border: none; background: transparent; padding: 0; margin-top: 1px; font-size: 12px; color: var(--teal); }
  .line.step .x { color: var(--text-2); }
  .line.say .x { color: var(--text); }
  .line.me .x { color: var(--teal); font-weight: 500; }
  .line.done .x { color: var(--teal); }
  .line.warn .x { color: var(--amber); }
  .compose { display: flex; flex-direction: column; gap: 8px; }
  .compose textarea { flex: 1; resize: none; min-height: 64px; width: 100%; line-height: 1.5; }
  /* the big editor, over a dimmed window */
  .compose.big { position: fixed; top: 7vh; left: 50%; transform: translateX(-50%); width: min(940px, 94vw); height: 84vh; z-index: 300; padding: 14px 16px;
    background: var(--bg-2); border: 1px solid var(--teal); border-radius: 14px; box-shadow: 0 0 0 100vmax rgba(0, 0, 0, 0.55), 0 20px 60px rgba(0, 0, 0, 0.5); }
  .compose.big .tw { flex: 1; min-height: 0; }
  .compose.big textarea { height: 100% !important; overflow-y: auto !important; font-size: 15px; }
  .bigtitle { font-size: 15px; display: flex; align-items: center; gap: 6px; }
  .bar { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
  .bar select { height: 34px; padding: 0 10px; font-size: 13px; max-width: 240px; }
  .bar .send { height: 36px; padding: 0 22px; white-space: nowrap; }
  .bar :global(.chips) { order: 10; }
  .bar :global(.drop) { order: 11; }
  .head .grow + button, .head button { white-space: nowrap; }
  .hint b { color: var(--text-2); font-weight: 500; }
  .tw { position: relative; flex: 1; display: flex; }
  .tw textarea { flex: 1; }
</style>
