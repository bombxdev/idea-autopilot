<script>
  // พักทั้งหมดก่อนปิดเครื่อง แล้วกด ▶ ทำต่อ ทีหลัง (ISSUE-067): แสดงสิ่งที่พักไว้ หรือที่ค้างจากคอมฯ ที่ดับไปตอน AI ทำงาน
  import Icon from "./Icon.svelte";
  import { api, clock } from "../lib/api.js";
  import { store, toast, refresh } from "../lib/store.svelte.js";

  const shelf = $derived(store.state?.shelf);
  let busy = $state(false);

  async function go() {
    busy = true;
    try {
      const n = await api.resumeAll();
      toast(n ? `ทำต่อแล้ว — เปิดแชตเดิม ${n} อัน AI จะทำต่อจากที่ค้าง` : "ทำต่อแล้ว", "ok");
    } catch (e) {
      toast(String(e), "err");
    } finally {
      busy = false;
      refresh();
    }
  }
  async function forget() {
    await api.shelfDismiss();
    refresh();
  }
</script>

{#if shelf}
  <div class="shelf">
    <Icon name="pause" size="1.4em" />
    <div class="grow body">
      <b>{shelf.by_owner ? `พักงานไว้ตั้งแต่ ${clock(shelf.ts)}` : "ครั้งก่อนคอมฯ ปิดไปตอนที่ AI ยังทำงานอยู่"}</b>
      <div class="small muted">
        {#each shelf.sessions as x, i}{i ? " · " : ""}#{x.id} {x.title} — {x.working ? "AI ทำค้างอยู่" : "แชตเปิดอยู่"}{/each}
        {#if shelf.run}{shelf.sessions.length ? " · " : ""}บอททำ #{shelf.run[0]} {shelf.run[1]} ค้างไว้{/if}
      </div>
      <div class="small faint">กดทำต่อ: เปิดบทสนทนาเดิม{shelf.sessions.some((x) => x.working) ? " และบอก AI ให้ทำต่อจากที่ค้าง" : ""}{shelf.by_owner && !shelf.was_paused ? " · บอทกลับมาทำงานตามปกติ" : ""}</div>
    </div>
    <button class="primary" disabled={busy} onclick={go}><Icon name="play" /> ทำต่อ</button>
    <button class="ghost small" disabled={busy} onclick={forget} title={shelf.by_owner ? "ไม่เปิดแชตเดิม (คุยต่อได้ทุกเมื่อจากหน้าไอเดีย) — บอทกลับไปเป็นเหมือนก่อนกดพัก" : "ไม่เปิดแชตเดิม — คุยต่อได้ทุกเมื่อจากหน้าไอเดีย"}>ไม่ต้อง</button>
  </div>
{/if}

<style>
  .shelf { display: flex; align-items: center; gap: 12px; padding: 10px 14px; margin: 0 0 12px; border: 1px solid var(--teal); background: var(--teal-bg); border-radius: 12px; color: var(--text); }
  .shelf :global(.ico) { color: var(--teal); }
  .body { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
</style>
