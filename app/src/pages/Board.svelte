<script>
  import { store, column, questionsFor, refresh, progress } from "../lib/store.svelte.js";
  import { api } from "../lib/api.js";
  import Intents from "../components/Intents.svelte";

  const COLS = [
    { key: "inbox", label: "Inbox", hint: "รอตีความ" },
    { key: "thinking", label: "กำลังคิด", hint: "ค้นคว้า / ตัดสิน" },
    { key: "building", label: "กำลังสร้าง", hint: "วางแผน / สร้าง / ตรวจ" },
    { key: "ready", label: "พร้อมใช้", hint: "กด ▶ ลองได้" },
    { key: "done", label: "จบแล้ว", hint: "เสร็จ / หยุด / ติดปัญหา" },
  ];

  let showArchived = $state(false);
  let dragId = $state(null);
  let overId = $state(null);

  const ideas = $derived(
    store.ideas.filter((i) => i.status !== "deleted" && (showArchived || i.status !== "archived")),
  );
  const byCol = $derived(Object.fromEntries(COLS.map((c) => [c.key, ideas.filter((i) => column(i) === c.key)])));

  async function drop(targetId) {
    if (dragId == null || dragId === targetId) return;
    const order = store.ideas.map((i) => i.id).filter((id) => id !== dragId);
    const at = order.indexOf(targetId);
    order.splice(at < 0 ? order.length : at, 0, dragId);
    dragId = overId = null;
    await api.reorder(order);
    refresh();
  }
</script>

<div class="head row">
  <div class="grow">
    <h1>บอร์ด</h1>
    <p class="muted small">ลากการ์ดขึ้นลงเพื่อจัดลำดับ — AI ทำจากบนลงล่าง (การ์ด “ด่วน” มาก่อนเสมอ)</p>
  </div>
  <label class="row small muted"><input type="checkbox" bind:checked={showArchived} /> แสดงที่เก็บแล้ว</label>
</div>

<div class="board">
  {#each COLS as c}
    <div class="col">
      <div class="colhead"><b>{c.label}</b><span class="faint small">{byCol[c.key].length}</span></div>
      <div class="faint small hint">{c.hint}</div>
      {#each byCol[c.key] as idea (idea.id)}
        {@const qn = questionsFor(idea.id).length}
        {@const running = store.state?.current?.idea_id === idea.id}
        <div
          class="mini"
          class:over={overId === idea.id}
          class:running
          class:ask={qn > 0}
          class:arch={idea.status === "archived"}
          draggable="true"
          role="listitem"
          ondragstart={() => (dragId = idea.id)}
          ondragover={(e) => { e.preventDefault(); overId = idea.id; }}
          ondragleave={() => (overId = null)}
          ondrop={() => drop(idea.id)}
          onclick={() => (store.openIdea = idea.id)}
          onkeydown={(e) => e.key === "Enter" && (store.openIdea = idea.id)}
          tabindex="0"
        >
          <div class="t"><span class="idn">#{idea.id}</span>{#if idea.priority > 0}<span class="urgent">ด่วน</span>{/if}{idea.title}</div>
          {#if idea.summary}<div class="s small muted">{idea.summary}</div>{/if}
          <div class="pb"><i style="width:{progress(idea)}%"></i></div>
          <div class="row wrap small">
            <Intents intents={idea.intents} />
            {#if qn}<span class="chip q">🙋 {qn}</span>{/if}
            {#if running}<span class="chip run">● ทำอยู่</span>{/if}
            {#if idea.verdict}<span class="chip">{idea.verdict}</span>{/if}
          </div>
        </div>
      {:else}
        <div class="emptycol faint small">—</div>
      {/each}
    </div>
  {/each}
</div>

<style>
  .head { margin-bottom: 16px; align-items: flex-end; }
  .head p { margin: 4px 0 0; }
  .board { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 12px; align-items: start; }
  .col { background: var(--bg-2); border: 1px solid var(--line); border-radius: 14px; padding: 12px; display: flex; flex-direction: column; gap: 8px; min-height: 200px; }
  .colhead { display: flex; justify-content: space-between; }
  .hint { margin-top: -6px; }
  .mini { background: var(--card); border: 1px solid var(--line); border-radius: 10px; padding: 10px 12px; cursor: grab; display: flex; flex-direction: column; gap: 6px; transition: border-color .15s, transform .1s; }
  .mini:hover { border-color: var(--line-2); transform: translateY(-1px); }
  .mini.over { border-color: var(--teal); border-style: dashed; }
  .mini.running { border-color: var(--teal); }
  .mini.ask { border-color: var(--amber); }
  .mini.arch { opacity: .55; }
  .t { font-weight: 500; line-height: 1.4; }
  .s { display: -webkit-box; -webkit-line-clamp: 3; line-clamp: 3; -webkit-box-orient: vertical; overflow: hidden; }
  .urgent { font-size: 11px; background: var(--red-bg); color: var(--red); border-radius: 6px; padding: 1px 6px; margin-right: 6px; }
  .q { background: var(--amber-bg); color: var(--amber); }
  .run { background: var(--teal-bg); color: var(--teal); }
  .pb { height: 4px; background: var(--bg-2); border-radius: 4px; overflow: hidden; }
  .pb i { display: block; height: 100%; background: var(--teal); }
  .emptycol { text-align: center; padding: 12px; }
</style>
