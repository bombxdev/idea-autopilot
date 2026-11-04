<script>
  // สั่ง AI แก้/ทำต่อในไอเดีย: ส่งข้อความเข้า Remote Control session "Idea#<id>" (มีอยู่แล้วส่งเข้าอันเดิม)
  // แล้วแสดงว่า AI กำลังทำอะไรเป็นภาษาคน (แปลงจาก transcript ด้วยกฎ ไม่ใช้ token เพิ่ม)
  import { api } from "../lib/api.js";
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
  const agent = $derived(accts.find((a) => a.id === ai.account)?.kind ?? "");
  let feedEl;
  let staged = $state([]);
  let att;
  let askEl = $state();
  let menu; // "/" opens the list of skills (they work with every AI, see skills.rs)

  // only AI agents installed on this machine are offered. Claude shows a live feed here; the others
  // are opened in Orca (or a terminal window) on the idea's own repo
  const agents = $derived(store.state?.agents ?? []);
  const picked = $derived(agents.find((a) => a.id === agent) ?? agents[0]);

  const s = $derived((store.state?.sessions ?? []).find((x) => x.idea_id === id));
  const feed = $derived((s?.feed ?? []).slice(-14));
  const STATUS = {
    starting: { label: "กำลังเปิด", cls: "amber" },
    working: { label: "AI กำลังทำ", cls: "teal" },
    idle: { label: "รอคำสั่ง", cls: "muted" },
    error: { label: "มีปัญหา", cls: "red" },
  };
  const st = $derived(s ? STATUS[s.status] ?? STATUS.idle : null);

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
    <b>🛠 สั่ง AI แก้ / ทำต่อ</b>
    {#if st}<span class="badge {st.cls}">{#if s.status === "working" || s.status === "starting"}<i class="pulse"></i>{/if}{st.label}</span>{/if}
    <span class="grow"></span>
    {#if s}
      <button class="ghost small" onclick={() => (store.page = "sessions")} title="ดูและจัดการ session ทั้งหมด">จัดการ session</button>
      <button class="ghost small" onclick={close} title="ปิด session (คุยต่อจากเดิมได้ภายหลัง)">ปิด session</button>
    {:else if !picked || picked.via === "app"}
      <button class="ghost small" onclick={openOnly} title="เปิด session โดยไม่ส่งข้อความ เพื่อไปคุยต่อเองในแอป Claude — สิ่งที่คุยจะถูกสรุปกลับมาที่ไอเดียนี้">เปิด session โดยยังไม่ส่งข้อความ</button>
    {/if}
  </div>

  <div class="compose">
    <div class="tw">
    <SkillMenu bind:text bind:this={menu} />
    <textarea
      rows="2"
      bind:value={text}
      onpaste={(e) => att?.paste(e)}
      placeholder="บอกสิ่งที่อยากให้แก้ เช่น “ทำปุ่มให้ใหญ่ขึ้น และเพิ่มโหมดมืด” · พิมพ์ / เพื่อเรียกสกิล · Ctrl+V วางภาพได้"
      onkeydown={(e) => {
        if (menu?.key(e)) return;
        if (e.key === "Enter" && (e.ctrlKey || e.metaKey) && !e.isComposing) send();
      }}
    ></textarea>
    </div>
    <div class="bar">
      <Attach bind:this={att} bind:staged priority={3} zone={() => askEl} />
      <span class="grow"></span>
      <button class="primary send" onclick={send} disabled={sending || (!text.trim() && !staged.length)}>{sending ? "กำลังส่ง…" : `ส่งให้ ${!picked || picked.id === "claude" ? "AI" : picked.label.split(" ")[0]}`}</button>
    </div>
  </div>
  <IdeaAi bind:value={ai} title="AI สำหรับแชตนี้" autoNote="ถ้าไม่เลือก ใช้ AI/โมเดลที่ตั้งไว้ให้ไอเดียนี้ ไม่ก็ค่าเริ่มต้นของ Claude" />
  <div class="hint small faint">
    {#if !picked}ยังไม่พบ AI ในเครื่องนี้ — ติดตั้งอย่างใดอย่างหนึ่งแล้วกลับมาใหม่
    {:else if picked.via === "orca"}{picked.hint} — แอปจะ commit เก็บไว้ก่อน แล้วเปิดใน Orca บน repo ของ Idea #{id} · ผลสรุปจะกลับมาที่ไอเดียเมื่อเสร็จ
    {:else if picked.via === "terminal"}{picked.hint} — แอปจะ commit เก็บไว้ก่อน แล้วเปิดหน้าต่าง terminal ใหม่ในโฟลเดอร์ของ Idea #{id} ให้ AI อ่านคำสั่งจาก HANDOFF.md
    {:else if s}ส่งเข้า session เดิม <b>{s.name}</b> · เปิดดู/คุยต่อในแอป Claude ได้ · Ctrl+Enter ส่ง
    {:else}จะเปิด session “Idea#{id} : …” ให้ (คุยต่อจากครั้งก่อน ถ้าเคยเปิด) · ดูในแอป Claude บนมือถือได้ด้วย · บอทกลางคืนจะเว้นไอเดียนี้ระหว่างเปิด{/if}
  </div>

  {#if feed.length}
    <div class="feed" bind:this={feedEl}>
      {#each feed as f}
        <div class="line {kind(f.text)}"><span class="t">{hhmm(f.ts)}</span><span class="x">{f.text}</span></div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .ask { display: flex; flex-direction: column; gap: 8px; padding: 12px; border: 1px solid var(--line); border-radius: 12px; background: var(--bg-2); }
  .ask.on { border-color: var(--teal); }
  .head { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; }
  .badge { display: inline-flex; align-items: center; gap: 6px; font-size: 12px; padding: 2px 10px; border-radius: 999px; background: var(--card-2); }
  .badge.teal { background: var(--teal-bg); color: var(--teal); }
  .badge.amber { color: var(--amber); }
  .badge.red { color: var(--red); }
  .badge.muted { color: var(--text-2); }
  .pulse { width: 7px; height: 7px; border-radius: 50%; background: currentColor; animation: pulse 1.2s ease-in-out infinite; }
  @keyframes pulse { 50% { opacity: .25; transform: scale(.7); } }
  .feed { max-height: 190px; overflow: auto; display: flex; flex-direction: column; gap: 3px; padding: 8px 10px; background: var(--bg); border-radius: 8px; font-size: 13px; line-height: 1.5; }
  .line { display: flex; gap: 8px; }
  .line .t { color: var(--text-3); font-variant-numeric: tabular-nums; flex: none; }
  .line .x { overflow-wrap: anywhere; }
  .line.step .x { color: var(--text-2); }
  .line.say .x { color: var(--text); }
  .line.me .x { color: var(--teal); font-weight: 500; }
  .line.done .x { color: var(--teal); }
  .line.warn .x { color: var(--amber); }
  .compose { display: flex; flex-direction: column; gap: 8px; }
  .compose textarea { flex: 1; resize: vertical; min-height: 64px; width: 100%; }
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
