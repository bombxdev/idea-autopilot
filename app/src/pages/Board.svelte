<script>
  import Icon from "../components/Icon.svelte";
  import { store, column, questionsFor, refresh, progress, chatOpen, fixing } from "../lib/store.svelte.js";
  import { api } from "../lib/api.js";
  import Intents from "../components/Intents.svelte";

  const COLS = [
    { key: "inbox", label: "Inbox", hint: "รอตีความ" },
    { key: "thinking", label: "กำลังคิด", hint: "ค้นคว้า / ตัดสิน" },
    { key: "building", label: "กำลังสร้าง", hint: "วางแผน / สร้าง / แก้ไข / ตรวจ" },
    { key: "ready", label: "พร้อมใช้", hint: "กด “ลองเล่น” ได้" },
    { key: "done", label: "จบแล้ว", hint: "เสร็จ / หยุด / ติดปัญหา" },
  ];
  // cards shown per column before "show more" (finished work matters least)
  const LIMIT = { done: 4 };
  const limitOf = (k) => LIMIT[k] ?? 6;

  // per-viewer conveniences, kept between visits
  const saved = (k, d) => { try { return JSON.parse(localStorage.getItem(k)) ?? d; } catch { return d; } };
  const keep = (k, v) => { try { localStorage.setItem(k, JSON.stringify(v)); } catch {} };

  let showArchived = $state(false);
  let compact = $state(saved("board.compact", false));
  let query = $state("");
  let open = $state({});
  let dragId = $state(null);
  let overId = $state(null);
  $effect(() => keep("board.compact", compact));

  // search: the name contains the text (any case), or "#12" / "12" for the idea number
  const q = $derived(query.trim().toLowerCase());
  const matches = (i) => {
    if (!q) return true;
    const n = q.replace(/^#/, "");
    return i.title.toLowerCase().includes(q) || (/^\d+$/.test(n) && String(i.id) === n);
  };
  const ideas = $derived(
    store.ideas.filter((i) => i.status !== "deleted" && (showArchived || i.status !== "archived") && matches(i)),
  );
  // pinned first; finished work newest first; everything else in the bot's order
  const byCol = $derived(
    Object.fromEntries(
      COLS.map((c) => {
        const list = ideas.filter((i) => column(i) === c.key);
        const rest = list.filter((i) => !i.pinned);
        if (c.key === "done") rest.sort((a, b) => b.updated - a.updated);
        return [c.key, [...list.filter((i) => i.pinned), ...rest]];
      }),
    ),
  );
  const shown = (k) => (q || open[k] ? byCol[k] : byCol[k].slice(0, Math.max(limitOf(k), byCol[k].filter((i) => i.pinned).length)));
  const pinnedCount = $derived(store.ideas.filter((i) => i.pinned && i.status !== "deleted" && i.status !== "archived").length);

  async function drop(targetId) {
    if (dragId == null || dragId === targetId) return;
    const order = store.ideas.map((i) => i.id).filter((id) => id !== dragId);
    const at = order.indexOf(targetId);
    order.splice(at < 0 ? order.length : at, 0, dragId);
    dragId = overId = null;
    await api.reorder(order);
    refresh();
  }
  async function act(e, id, a) {
    e.stopPropagation();
    await api.action(id, a);
    refresh();
  }
</script>

<div class="head row">
  <div class="grow">
    <h1>บอร์ด</h1>
    <p class="muted small">ลากการ์ดขึ้นลงเพื่อจัดลำดับ — AI ทำจากบนลงล่าง (การ์ด “ด่วน” มาก่อนเสมอ) · 📌 ปักหมุดไอเดียที่โฟกัสให้อยู่บนสุดของคอลัมน์</p>
  </div>
  <div class="tools row wrap">
    <div class="search">
      <span class="faint">🔍</span>
      <input type="search" placeholder="ค้นหาชื่อไอเดีย หรือ #เลข" bind:value={query} onkeydown={(e) => e.key === "Escape" && (query = "")} />
    </div>
    <label class="row small muted"><input type="checkbox" bind:checked={compact} /> แบบย่อ</label>
    <label class="row small muted"><input type="checkbox" bind:checked={showArchived} /> แสดงที่เก็บแล้ว</label>
  </div>
</div>
{#if q}
  <p class="small muted found">พบ {ideas.length} ไอเดียที่ชื่อมี “{query.trim()}”{#if !ideas.length} — ลองคำที่สั้นลง หรือติ๊ก “แสดงที่เก็บแล้ว”{/if}</p>
{:else if pinnedCount}
  <p class="small faint found">📌 ปักหมุดไว้ {pinnedCount} ไอเดีย</p>
{/if}

<div class="board" class:compact>
  {#each COLS as c}
    <div class="col">
      <div class="colhead"><b>{c.label}</b><span class="faint small">{byCol[c.key].length}</span></div>
      <div class="faint small hint">{c.hint}</div>
      {#each shown(c.key) as idea (idea.id)}
        {@const qn = questionsFor(idea.id).length}
        {@const running = store.state?.current?.idea_id === idea.id}
        {@const fix = fixing(idea)}
        {@const chat = chatOpen(idea)}
        <div
          class="mini"
          class:over={overId === idea.id}
          class:running
          class:ask={qn > 0}
          class:pinned={idea.pinned}
          class:fix
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
          <div class="t">
            <button class="pin" class:on={idea.pinned} title={idea.pinned ? "เลิกปักหมุด" : "ปักหมุดไว้บนสุด (ไอเดียที่โฟกัสอยู่)"}
              onclick={(e) => act(e, idea.id, idea.pinned ? "unpin" : "pin")}>📌</button>
            <span class="idn">#{idea.id}</span>{#if idea.priority > 0}<span class="urgent">ด่วน</span>{/if}{idea.title}
          </div>
          {#if idea.summary && !compact}<div class="s small muted">{idea.summary}</div>{/if}
          {#if !compact && !fix}<div class="pb"><i style="width:{progress(idea)}%"></i></div>{/if}
          <div class="row wrap small">
            {#if !compact}<Intents intents={idea.intents} />{/if}
            {#if qn}<span class="chip q"><Icon name="user" /> {qn}</span>{/if}
            {#if running}<span class="chip run"><Icon name="dot" size="0.8em" /> ทำอยู่</span>{/if}
            {#if chat}<span class="chip run" title="มีแชตเปิดอยู่กับไอเดียนี้">💬 แชตอยู่</span>{/if}
            {#if fix}
              <span class="chip fixc" title="สั่งแก้หลังจากพร้อมใช้แล้ว — จะกลับไปช่องพร้อมใช้เมื่อกด “ใช้ได้แล้ว”">🔧 กำลังแก้</span>
              {#if idea.fixing}<button class="works small" title="ลองแล้วใช้งานได้ — ย้ายกลับไปพร้อมใช้/จบแล้ว" onclick={(e) => act(e, idea.id, "works")}>✓ ใช้ได้แล้ว</button>{/if}
            {:else if idea.verdict && !compact}<span class="chip">{idea.verdict}</span>{/if}
          </div>
        </div>
      {:else}
        <div class="emptycol faint small">—</div>
      {/each}
      {#if !q && byCol[c.key].length > shown(c.key).length}
        <button class="more small" onclick={() => (open[c.key] = true)}>แสดงอีก {byCol[c.key].length - shown(c.key).length} ไอเดีย</button>
      {:else if !q && open[c.key] && byCol[c.key].length > limitOf(c.key)}
        <button class="more small" onclick={() => (open[c.key] = false)}>ย่อ</button>
      {/if}
    </div>
  {/each}
</div>

<style>
  .head { margin-bottom: 12px; align-items: flex-end; gap: 12px; flex-wrap: wrap; }
  .head p { margin: 4px 0 0; }
  .tools { gap: 12px; }
  .search { display: flex; align-items: center; gap: 6px; border: 1px solid var(--line); border-radius: 8px; padding: 0 8px; background: var(--card); }
  .search input { border: 0; background: transparent; outline: none; padding: 6px 0; width: 220px; color: inherit; }
  .found { margin: 0 0 10px; }
  .board { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 12px; align-items: start; }
  .col { background: var(--bg-2); border: 1px solid var(--line); border-radius: 14px; padding: 12px; display: flex; flex-direction: column; gap: 8px; min-height: 200px; }
  .colhead { display: flex; justify-content: space-between; }
  .hint { margin-top: -6px; }
  .mini { background: var(--card); border: 1px solid var(--line); border-radius: 10px; padding: 10px 12px; cursor: grab; display: flex; flex-direction: column; gap: 6px; transition: border-color .15s, transform .1s; }
  .compact .mini { padding: 7px 10px; gap: 4px; }
  .mini:hover { border-color: var(--line-2); transform: translateY(-1px); }
  .mini.over { border-color: var(--teal); border-style: dashed; }
  .mini.pinned { border-left: 3px solid var(--amber); }
  .mini.fix { border-color: var(--amber); }
  .mini.running { border-color: var(--teal); }
  .mini.ask { border-color: var(--amber); }
  .mini.arch { opacity: .55; }
  .t { font-weight: 500; line-height: 1.4; }
  .pin { float: right; border: 0; background: none; padding: 0 0 0 6px; cursor: pointer; font-size: 13px; opacity: 0; filter: grayscale(1); }
  .mini:hover .pin, .mini:focus-within .pin { opacity: .5; }
  .pin:hover { opacity: 1 !important; filter: none; }
  .pin.on { opacity: 1; filter: none; }
  .s { display: -webkit-box; -webkit-line-clamp: 3; line-clamp: 3; -webkit-box-orient: vertical; overflow: hidden; }
  .urgent { font-size: 11px; background: var(--red-bg); color: var(--red); border-radius: 6px; padding: 1px 6px; margin-right: 6px; }
  .q { background: var(--amber-bg); color: var(--amber); }
  .fixc { background: var(--amber-bg); color: var(--amber); }
  .run { background: var(--teal-bg); color: var(--teal); }
  .works { padding: 1px 8px; border-radius: 6px; font-size: 12px; }
  .pb { height: 4px; background: var(--bg-2); border-radius: 4px; overflow: hidden; }
  .pb i { display: block; height: 100%; background: var(--teal); }
  .emptycol { text-align: center; padding: 12px; }
  .more { background: none; border: 1px dashed var(--line-2); border-radius: 8px; padding: 6px; color: var(--muted, inherit); cursor: pointer; }
</style>
