<script>
  import { store, stageLabel, refresh, toast, progress } from "../lib/store.svelte.js";
  import { api, ago, clock, STATUS } from "../lib/api.js";
  import Markdown from "./Markdown.svelte";
  import Intents from "./Intents.svelte";
  import Question from "./Question.svelte";
  import Actions from "./Actions.svelte";
  import AskAI from "./AskAI.svelte";

  let { id } = $props();
  let data = $state(null);
  let tab = $state("report");
  let editing = $state(false);
  let draft = $state("");
  async function saveEdit() {
    try {
      await api.editIdea(id, draft);
      editing = false;
      toast("แก้ข้อความแล้ว — AI จะอ่านข้อความใหม่ (ถ้ายังไม่เริ่มลงลึก จะตีความและตั้งชื่อโฟลเดอร์ใหม่ให้ตรง)", "ok", 5000);
      refresh();
      load();
    } catch (e) {
      toast(String(e), "err");
    }
  }
  function startEdit() {
    draft = idea?.body || idea?.title || "";
    editing = true;
  }

  async function load() {
    try {
      data = await api.idea(id);
    } catch (e) {
      toast(String(e), "err");
    }
  }
  $effect(() => {
    id;
    store.ideas; // reload when the list refreshes
    load();
  });

  const idea = $derived(data?.idea);
  const f = $derived(data?.files ?? {});
  const tabs = $derived(
    [
      { key: "report", label: "รายงาน", text: f.report },
      { key: "knowledge", label: "ความรู้", text: f.knowledge },
      { key: "run", label: "วิธีใช้", text: f.how_to_run },
      { key: "plan", label: "แผน", text: f.plan },
      { key: "progress", label: "ความคืบหน้า", text: f.progress },
      { key: "idea", label: "ไอเดียเดิม", text: f.idea },
      { key: "issues", label: "⚠ ปัญหา/วิธีแก้", text: f.issues },
    ].filter((t) => t.text),
  );
  $effect(() => {
    if (tabs.length && !tabs.find((t) => t.key === tab) && tab !== "history") tab = tabs[0].key;
  });
  const running = $derived(store.state?.current?.idea_id === id);
  const openQs = $derived((data?.questions ?? []).filter((q) => !q.answer));
  const answered = $derived((data?.questions ?? []).filter((q) => q.answer));

  async function act(a) {
    await api.action(id, a);
    toast({ archive: "เก็บแล้ว", restore: "กู้คืนแล้ว", delete: "ลบแล้ว", urgent: "เปลี่ยนความด่วนแล้ว" }[a] ?? "ok");
    if (a === "delete") store.openIdea = null;
    refresh();
  }
  const tokens = (r) => r.input_tokens + r.output_tokens + r.cache_write;
</script>

<div class="scrim" onclick={() => (store.openIdea = null)} role="presentation"></div>
<aside class="drawer">
  {#if idea}
    <div class="top">
      <div class="row">
        <span class="idn big" title="เลขไอเดีย ใช้อ้างอิงเวลาสั่งงาน เช่น ia ask {id} …">Idea #{id}</span>
        <span class="chip">{STATUS[idea.status]?.label ?? idea.status}</span>
        {#if running}<span class="chip run">● กำลังทำ: {stageLabel(store.state.current.stage)}</span>{/if}
        <span class="grow"></span>
        <button class="ghost small" onclick={startEdit} title="แก้ข้อความไอเดีย (เช่น กด Enter เร็วไป)">✏️ แก้ข้อความ</button>
        <button class="ghost small" onclick={() => api.openFolder(id)} title="เปิดโฟลเดอร์งาน">📁</button>
        <button class="ghost small" onclick={() => act("urgent")} title="สลับด่วน">{idea.priority > 0 ? "⬇ ไม่ด่วน" : "⚡ ด่วน"}</button>
        {#if idea.status === "archived"}
          <button class="ghost small" onclick={() => act("restore")}>กู้คืน</button>
        {:else}
          <button class="ghost small" onclick={() => act("archive")}>เก็บ</button>
        {/if}
        <button class="ghost small" onclick={() => (store.openIdea = null)}>✕</button>
      </div>
      {#if editing}
        <textarea class="edit" rows="5" bind:value={draft} onkeydown={(e) => e.key === "Escape" && (editing = false)}></textarea>
        <div class="row small">
          <button class="primary" onclick={saveEdit} disabled={!draft.trim()}>บันทึกข้อความใหม่</button>
          <button onclick={() => (editing = false)}>ยกเลิก</button>
          <span class="faint">ขึ้นต้น ! = ด่วน · ? = ขอแค่ research · ถ้า AI ยังไม่เริ่มลงลึก จะเริ่มตีความใหม่และตั้งชื่อโฟลเดอร์ใหม่ตามข้อความนี้</span>
        </div>
      {:else}
        <h1>{idea.title}</h1>
      {/if}
      <div class="row wrap">
        <Intents intents={idea.intents} />
        {#if idea.verdict}<span class="chip v">{idea.verdict}</span>{/if}
      </div>
      {#if idea.summary}<p class="summary">{idea.summary}</p>{/if}
      <div class="row small"><div class="pb grow"><i style="width:{progress(idea)}%"></i></div><b>{progress(idea)}%</b></div>
      {#if idea.recipe.length}
        <div class="stepper">
          {#each idea.recipe as s, i}
            <div class="st" class:done={i < idea.stage_idx} class:now={i === idea.stage_idx && idea.status === "active"}>
              <span class="n">{i < idea.stage_idx ? "✓" : i + 1}</span>{stageLabel(s)}
            </div>
          {/each}
        </div>
      {/if}
      <Actions {idea} />
      <AskAI {id} />
      {#if openQs.length}<div class="qs">{#each openQs as q (q.id)}<Question {q} />{/each}</div>{/if}
      {#if idea.needs.length}
        <div class="needs small"><b>ต้องการจากคุณ</b><ul>{#each idea.needs as n}<li>{n}</li>{/each}</ul></div>
      {/if}
    </div>

    <div class="tabs">
      {#each tabs as t}<button class="tab" class:on={tab === t.key} onclick={() => (tab = t.key)}>{t.label}</button>{/each}
      <button class="tab" class:on={tab === "history"} onclick={() => (tab = "history")}>ประวัติ</button>
    </div>

    <div class="body">
      {#if tab === "history"}
        <h3>รอบการทำงาน</h3>
        <table class="runs">
          <thead><tr><th>เวลา</th><th>ขั้น</th><th>โมเดล</th><th>ผล</th><th>tokens</th><th>%สัปดาห์</th></tr></thead>
          <tbody>
            {#each data.runs as r}
              <tr title={r.error ?? ""}>
                <td>{clock(r.started)}</td><td>{stageLabel(r.stage)}</td>
                <td title={r.reason ?? ""}>{r.model}{r.effort ? ` · ${r.effort}` : ""}</td>
                <td class="st-{r.status}">{r.status}</td>
                <td>{(tokens(r) / 1000).toFixed(0)}k</td>
                <td>{r.week_after != null && r.week_before != null ? `+${(r.week_after - r.week_before).toFixed(1)}` : "—"}</td>
              </tr>
            {:else}<tr><td colspan="6" class="faint">ยังไม่เคยรัน</td></tr>{/each}
          </tbody>
        </table>
        {#if data.directives.length}
          <h3>คำสั่งของคุณ</h3>
          <ul>{#each data.directives as [ts, t]}<li><span class="faint small">{ago(ts)}</span> {t}</li>{/each}</ul>
        {/if}
        {#if answered.length}
          <h3>คำถามที่ตอบแล้ว</h3>
          <ul>{#each answered as q}<li>{q.question} → <b>{q.answer}</b></li>{/each}</ul>
        {/if}
      {:else}
        {@const t = tabs.find((x) => x.key === tab)}
        {#if t}<Markdown text={t.text} />{:else}
          <div class="faint pad">ยังไม่มีเอกสาร — AI จะเขียนรายงานลงที่นี่เมื่อเริ่มทำงาน</div>
        {/if}
      {/if}
    </div>
  {:else}
    <div class="pad faint">กำลังโหลด…</div>
  {/if}
</aside>

<style>
  .edit { width: 100%; box-sizing: border-box; font: inherit; line-height: 1.5; padding: 8px 10px; border-radius: 8px; }
  .scrim { position: fixed; inset: 0; background: rgba(0, 0, 0, .35); z-index: 20; animation: rise .15s; }
  .drawer { position: fixed; top: 0; right: 0; bottom: 0; width: min(860px, 92vw); background: var(--bg); border-left: 1px solid var(--line); z-index: 21; display: flex; flex-direction: column; animation: slide .2s ease-out; }
  @keyframes slide { from { transform: translateX(30px); opacity: 0 } to { transform: none; opacity: 1 } }
  .top { padding: 18px 26px 14px; display: flex; flex-direction: column; gap: 10px; border-bottom: 1px solid var(--line); max-height: 55vh; overflow: auto; }
  .top h1 { font-size: 22px; }
  .summary { margin: 0; color: var(--text-2); }
  .run { background: var(--teal-bg); color: var(--teal); }
  .v { background: var(--card-2); color: var(--text); font-weight: 600; }
  .pb { height: 8px; background: var(--bg-2); border-radius: 6px; overflow: hidden; }
  .pb i { display: block; height: 100%; background: var(--teal); }
  .stepper { display: flex; flex-wrap: wrap; gap: 6px; }
  .st { display: flex; align-items: center; gap: 6px; font-size: 13px; color: var(--text-3); padding: 3px 10px 3px 4px; border-radius: 999px; background: var(--bg-2); }
  .st .n { width: 20px; height: 20px; border-radius: 50%; display: grid; place-items: center; background: var(--card-2); font-size: 11px; }
  .st.done { color: var(--teal); }
  .st.done .n { background: var(--teal-bg); }
  .st.now { color: var(--text); outline: 1px solid var(--teal); }
  .st.now .n { background: var(--teal); color: #08251c; }
  .qs { display: flex; flex-direction: column; gap: 8px; }
  .needs { background: var(--blue-bg); padding: 8px 12px; border-radius: 8px; }
  .needs ul { margin: 4px 0 0; padding-left: 18px; }
  .tabs { display: flex; gap: 2px; padding: 0 22px; border-bottom: 1px solid var(--line); }
  .tab { border: none; border-bottom: 2px solid transparent; border-radius: 0; padding: 10px 12px; color: var(--text-2); }
  .tab.on { color: var(--text); border-bottom-color: var(--teal); }
  .tab:hover { background: none; color: var(--text); }
  .body { flex: 1; overflow: auto; padding: 8px 26px 40px; }
  .body h3 { margin: 18px 0 8px; }
  .pad { padding: 24px; }
  .runs { width: 100%; border-collapse: collapse; font-size: 13px; }
  .runs th, .runs td { text-align: left; padding: 6px 8px; border-bottom: 1px solid var(--line); }
  .runs th { color: var(--text-3); font-weight: 500; }
  .st-ok { color: var(--teal); }
  .st-error, .st-interrupted { color: var(--red); }
  .st-timeout, .st-rate_limited, .st-cancelled { color: var(--amber); }
</style>
