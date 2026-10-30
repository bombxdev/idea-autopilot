<script>
  import { api } from "../lib/api.js";
  import { refresh, toast } from "../lib/store.svelte.js";

  let { idea, compact = false } = $props();
  let sayOpen = $state(false);
  let text = $state("");
  let err = $state("");

  const finished = $derived(["done", "ready", "failed"].includes(idea.status));

  async function act(action, t) {
    try {
      await api.action(idea.id, action, t);
      const msg = {
        go: "รับทราบ — ไปต่อ",
        deeper: "จะขุดลึกให้รอบหน้า",
        alt: "จะลองอีกแนวให้",
        stop: "หยุดไอเดียนี้แล้ว",
        say: "ส่งคำสั่งแล้ว",
        run_now: "เริ่มทำทันที",
        retry: "ลองใหม่อีกครั้ง",
      }[action];
      if (msg) toast(msg);
      sayOpen = false;
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
      <button class="primary" onclick={() => api.runPrototype(idea.id).catch((e) => toast(String(e), "err"))}>▶ ลองเล่น</button>
    {/if}
    {#if idea.status === "failed"}
      <button onclick={() => act("retry")}>↻ ลองใหม่</button>
    {/if}
    <button onclick={() => act("go")} title="ทำขั้นถัดไป">👍 ไปต่อ</button>
    <button onclick={() => act("deeper")}>🔍 ขุดลึก</button>
    <button onclick={() => act("alt")}>🔀 ลองอีกแนว</button>
    {#if !compact}
      {#if !finished}<button onclick={() => act("stop")}>✂️ พอแค่นี้</button>{/if}
      <button onclick={() => act("run_now")} title="ไม่รอโควต้าว่าง เริ่มเลย">⚡ ทำเลยตอนนี้</button>
    {/if}
    <button class="ghost" onclick={() => (sayOpen = !sayOpen)}>💬 สั่งสั้นๆ</button>
  </div>
  {#if sayOpen}
    <div class="row say">
      <input class="grow" bind:value={text} oninput={() => (err = "")} placeholder="เช่น เน้นตลาดไทย / ใช้ Python แทน / ไม่เอาแบบต้องมีเซิร์ฟเวอร์"
        onkeydown={(e) => e.key === "Enter" && !e.isComposing && (text.trim() ? act("say", text) : (err = "พิมพ์คำสั่งก่อน"))} />
      <button class="primary" onclick={() => (text.trim() ? act("say", text) : (err = "พิมพ์คำสั่งก่อน"))}>ส่ง</button>
    </div>
  {/if}
  {#if err}<div class="small" style="color:var(--red)">{err}</div>{/if}
</div>

<style>
  .acts { display: flex; flex-direction: column; gap: 8px; }
  .acts button { font-size: 13px; padding: 4px 11px; }
</style>
