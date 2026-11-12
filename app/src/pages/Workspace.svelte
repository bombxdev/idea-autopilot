<script>
  // Workspace (ISSUE-087): the ideas being focused on, as windows on a taskbar — open one full size or two side by
  // side, minimize, switch, add or take off (taking off is not deleting).
  import Icon from "../components/Icon.svelte";
  import IdeaDrawer from "../components/IdeaDrawer.svelte";
  import { store, progress, questionsFor } from "../lib/store.svelte.js";
  import { STATUS } from "../lib/api.js";
  import { ws, wsLoad, wsOpen, wsToggle, wsMinimize, wsSide, wsRemove, wsPrune, shown } from "../lib/workspace.svelte.js";

  wsLoad();
  const byId = $derived(new Map(store.ideas.map((i) => [i.id, i])));
  $effect(() => {
    // ideas deleted or put away elsewhere leave the Workspace
    if (ws.loaded && store.ideas.length) wsPrune(new Set(store.ideas.filter((i) => i.status !== "deleted" && i.status !== "archived").map((i) => i.id)));
  });
  const windows = $derived(shown().filter((id) => byId.has(id)));
  const working = (id) => store.state?.current?.idea_id === id || (store.state?.sessions ?? []).some((s) => s.idea_id === id && s.status === "working");
  const waiting = (id) => questionsFor(id).length > 0;

  // the add list: ideas not on the Workspace yet, the ones being worked on first
  let picking = $state(false);
  let q = $state("");
  let pickEl = $state();
  const rank = (i) => (i.pinned ? 0 : working(i.id) ? 1 : i.status === "active" ? 2 : i.status === "ready" ? 3 : i.status === "inbox" ? 4 : 5);
  const choices = $derived(
    store.ideas
      .filter((i) => !ws.ids.includes(i.id) && i.status !== "deleted" && i.status !== "archived")
      .filter((i) => !q.trim() || `#${i.id} ${i.title}`.toLowerCase().includes(q.trim().toLowerCase()))
      .sort((a, b) => rank(a) - rank(b) || b.id - a.id),
  );
  function openPicker() {
    picking = !picking;
    q = "";
    requestAnimationFrame(() => pickEl?.focus());
  }
  function add(id) {
    wsOpen(id);
    picking = false;
  }
</script>

<div class="wsp">
  <div class="taskbar" role="tablist" aria-label="หน้าต่างไอเดีย">
    {#each ws.ids as id (id)}
      {@const idea = byId.get(id)}
      {#if idea}
        <div class="task" class:front={ws.active === id} class:side={ws.side === id} class:min={!windows.includes(id)}>
          <button class="tbtn" role="tab" aria-selected={windows.includes(id)} onclick={() => wsToggle(id)}
            title={ws.active === id ? "ย่อหน้าต่าง" : "เปิดหน้าต่างนี้"}>
            <span class="dot" class:run={working(id)} class:ask={!working(id) && waiting(id)}></span>
            <span class="tid">#{id}</span><span class="tt">{idea.title}</span>
          </button>
          {#if ws.active !== id && ws.side !== id && ws.active != null}
            <button class="tx" onclick={() => wsSide(id)} title="เปิดข้างกันกับหน้าต่างที่เปิดอยู่"><Icon name="split" /></button>
          {/if}
          <button class="tx" onclick={() => wsRemove(id)} title="เอาออกจาก Workspace (ไม่ได้ลบไอเดีย)"><Icon name="close" /></button>
        </div>
      {/if}
    {/each}
    <div class="addwrap">
      <button class="addbtn" class:on={picking} onclick={openPicker}><Icon name="plus" /> เพิ่มไอเดีย</button>
      {#if picking}
        <div class="picker">
          <input bind:this={pickEl} bind:value={q} placeholder="ค้นหาไอเดีย — เลขหรือชื่อ" onkeydown={(e) => { if (e.key === "Escape") picking = false; if (e.key === "Enter" && choices[0]) add(choices[0].id); }} />
          <div class="plist">
            {#each choices.slice(0, 40) as i (i.id)}
              <button class="pick" onclick={() => add(i.id)}>
                <span class="dot" class:run={working(i.id)}></span><span class="tid">#{i.id}</span><span class="tt">{i.title}</span>
                <span class="chip small">{STATUS[i.status]?.label ?? i.status}</span>
              </button>
            {:else}<div class="faint small pad">ไม่มีไอเดียอื่นแล้ว</div>{/each}
          </div>
        </div>
      {/if}
    </div>
  </div>

  {#if windows.length}
    <div class="windows" class:two={windows.length === 2}>
      {#each windows as id (id)}
        <div class="win" class:front={ws.active === id}>
          <IdeaDrawer {id} embedded onmin={() => wsMinimize(id)} onclose={() => wsRemove(id)} />
        </div>
      {/each}
    </div>
  {:else if ws.ids.length}
    <div class="empty">
      <Icon name="windows" size="40" />
      <p>ทุกหน้าต่างย่ออยู่ — คลิกไอเดียบนแถบด้านบนเพื่อเปิด</p>
    </div>
  {:else}
    <div class="empty">
      <Icon name="windows" size="40" />
      <h2>พื้นที่ทำงาน</h2>
      <p class="muted">เลือกไอเดียที่กำลังโฟกัสมาเปิดเป็นหน้าต่าง สลับไปมา ย่อ หรือเปิดสองอันข้างกันได้ — เอาออกจากที่นี่ไม่ใช่การลบไอเดีย</p>
      <div class="starter">
        {#each choices.slice(0, 8) as i (i.id)}
          <button class="pick card" onclick={() => add(i.id)}>
            <span class="dot" class:run={working(i.id)}></span><span class="tid">#{i.id}</span><span class="tt">{i.title}</span>
            <span class="faint small">{progress(i)}%</span>
          </button>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .wsp { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  .taskbar { display: flex; align-items: center; gap: 6px; padding: 8px 12px; border-bottom: 1px solid var(--line); background: var(--bg-2); overflow-x: auto; flex: none; }
  .task { display: flex; align-items: center; border: 1px solid var(--line); border-radius: 9px; background: var(--card); max-width: 260px; flex: none; }
  .task.front { border-color: var(--teal); background: var(--teal-bg); }
  .task.side { border-color: var(--teal); }
  .task.min { opacity: 0.72; }
  .task.min:hover { opacity: 1; }
  .tbtn { display: flex; align-items: center; gap: 6px; border: none; background: transparent; padding: 6px 4px 6px 10px; min-width: 0; font-size: 13px; }
  .tid { color: var(--teal); font-weight: 600; font-size: 12px; flex: none; }
  .tt { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
  .tx { border: none; background: transparent; padding: 4px 6px; color: var(--text-3); flex: none; }
  .tx:hover { color: var(--text); background: transparent; }
  .dot { width: 7px; height: 7px; border-radius: 50%; background: var(--text-3); flex: none; opacity: 0.5; }
  .dot.run { background: var(--teal); opacity: 1; animation: pulse 1.4s infinite; }
  .dot.ask { background: var(--amber); opacity: 1; }
  .addwrap { position: relative; flex: none; }
  .addbtn { display: flex; align-items: center; gap: 6px; font-size: 13px; padding: 6px 12px; border-style: dashed; white-space: nowrap; }
  .addbtn.on { border-color: var(--teal); color: var(--teal); }
  .picker { position: fixed; margin-top: 6px; width: 380px; max-height: 60vh; display: flex; flex-direction: column; background: var(--bg-2); border: 1px solid var(--line-2); border-radius: 12px; box-shadow: 0 16px 40px rgba(0, 0, 0, 0.45); z-index: 30; overflow: hidden; }
  .picker input { margin: 8px; }
  .plist { overflow: auto; padding: 0 6px 6px; }
  .pick { display: flex; align-items: center; gap: 8px; width: 100%; text-align: left; border: none; background: transparent; padding: 7px 8px; border-radius: 8px; font-size: 13px; }
  .pick:hover { background: var(--card-2); }
  .pick .chip { margin-left: auto; flex: none; }
  .windows { flex: 1; min-height: 0; display: grid; grid-template-columns: 1fr; }
  .windows.two { grid-template-columns: 1fr 1fr; }
  .win { min-width: 0; min-height: 0; display: flex; container: ideawin / inline-size; }
  .windows.two .win + .win { border-left: 1px solid var(--line); }
  .empty { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; color: var(--text-3); text-align: center; padding: 24px; }
  .empty h2 { margin: 0; color: var(--text); }
  .empty p { max-width: 520px; margin: 0; }
  .starter { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 8px; width: min(860px, 100%); margin-top: 10px; }
  .starter .pick { border: 1px solid var(--line); background: var(--card); padding: 10px 12px; color: var(--text); }
  .starter .pick span:last-child { margin-left: auto; }
  .pad { padding: 10px; }
</style>
