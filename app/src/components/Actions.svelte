<script>
  import Icon from "./Icon.svelte";
  import { api } from "../lib/api.js";
  import { store, refresh, toast } from "../lib/store.svelte.js";
  import Attach from "./Attach.svelte";

  let { idea, compact = false } = $props();
  let sayOpen = $state(false);
  let text = $state("");
  let err = $state("");
  let staged = $state([]);
  let att;
  let sayEl = $state();

  function send() {
    if (att?.pending()) {
      err = "รอไฟล์แนบอัปโหลดก่อน";
      return;
    }
    if (text.trim() || staged.length) act("say", text);
    else err = "พิมพ์คำสั่งหรือแนบไฟล์ก่อน";
  }
  const finished = $derived(["done", "ready", "failed"].includes(idea.status));

  async function act(action, t) {
    try {
      await api.action(idea.id, action, t, action === "say" ? staged.map((s) => s.token) : []);
      const msg = {
        go: "รับทราบ — ไปต่อ",
        deeper: "จะขุดลึกให้รอบหน้า",
        alt: "จะลองอีกแนวให้",
        stop: "หยุดไอเดียนี้แล้ว",
        say: "ส่งคำสั่งแล้ว",
        run_now: "เริ่มทำทันที",
        retry: "ลองใหม่อีกครั้ง",
      }[action];
      const busy = store.state?.current;
      if (action === "run_now" && busy && busy.idea_id !== idea.id) toast(`ต่อคิวแล้ว — รอ “${busy.title}” จบก่อน แล้วเริ่มทันที`);
      else if (msg) toast(msg);
      sayOpen = false;
      att?.reset();
      text = "";
      refresh();
    } catch (e) {
      err = String(e);
    }
  }
</script>

<div class="acts">
  <div class="row wrap">
    {#if idea.status === "ready"}
      <button class="primary" onclick={() => api.runPrototype(idea.id).catch((e) => toast(String(e), "err"))}><Icon name="play" /> ลองเล่น</button>
    {/if}
    {#if idea.status === "failed"}
      <button onclick={() => act("retry")}><Icon name="refresh" /> ลองใหม่</button>
    {/if}
    <!-- the bot carries on by itself while an idea is in progress: "go on" only matters once it has stopped (ISSUE-072) -->
    {#if idea.status === "done" || idea.status === "ready"}
      <button onclick={() => act("go")} title="ไอเดียนี้จบแล้ว — ให้ทำต่ออีกขั้น (ถ้าน่าทำก็วางแผนและสร้างต่อ) เข้าคิวรอโควต้าว่าง"><Icon name="arrowRight" /> ไปต่อ</button>
    {/if}
    <button onclick={() => act("deeper")} title="รอบถัดไปให้หาข้อมูลจริงเพิ่มในประเด็นที่ยังไม่ชัด — เข้าคิวรอโควต้าว่าง (กด “ทำเลยตอนนี้” ถ้าไม่อยากรอ)"><Icon name="search" /> ขุดลึก</button>
    <button onclick={() => act("alt")} title="รอบถัดไปให้ลองแนวทางที่ต่างจากเดิมแล้วเทียบกัน — เข้าคิวรอโควต้าว่าง (กด “ทำเลยตอนนี้” ถ้าไม่อยากรอ)"><Icon name="fork" /> ลองอีกแนว</button>
    {#if !compact}
      {#if !finished}<button onclick={() => act("stop")} title="หยุดไอเดียนี้ ไม่ทำต่อ (งานที่ได้แล้วยังอยู่)"><Icon name="scissors" /> พอแค่นี้</button>{/if}
      {#if !finished}<button onclick={() => act("run_now")} title="ไม่รอโควต้า/ช่วงพัก — เริ่มงานถัดไปของไอเดียนี้ทันที (รวมสิ่งที่เพิ่งกดขุดลึก/ลองอีกแนว/สั่งไว้) ถ้ามีงานอื่นรันอยู่จะเป็นคิวถัดไป"><Icon name="bolt" /> ทำเลยตอนนี้</button>{/if}
    {/if}
    <button class="ghost" onclick={() => (sayOpen = !sayOpen)} title="พิมพ์คำสั่งเพิ่มเอง เช่น เน้นตลาดไทย — บอทอ่านตอนรอบงานถัดไป"><Icon name="chat" /> สั่งสั้นๆ</button>
  </div>
  {#if sayOpen}
    <div class="row say wrap" bind:this={sayEl}>
      <input class="grow" bind:value={text} oninput={() => (err = "")} placeholder="เช่น เน้นตลาดไทย / ใช้ Python แทน / ดูรูปที่แนบ (วางภาพด้วย Ctrl+V ได้)"
        onpaste={(e) => att?.paste(e)}
        onkeydown={(e) => e.key === "Enter" && !e.isComposing && send()} />
      <Attach bind:this={att} bind:staged priority={2} zone={() => sayEl?.closest(".drawer") ?? sayEl} />
      <button class="primary" onclick={send}>ส่ง</button>
    </div>
  {/if}
  {#if err}<div class="small" style="color:var(--red)">{err}</div>{/if}
</div>

<style>
  .acts { display: flex; flex-direction: column; gap: 8px; }
  .acts button { font-size: 13px; padding: 4px 11px; }
</style>
