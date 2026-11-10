<script>
  // ทุกคำสั่งที่เจ้าของพิมพ์ในแชตเป็นรายการที่ต้องทำให้ครบ: AI ติ๊กว่าเสร็จ / ทำไม่ได้ (ia req) แล้วเจ้าของกดยืนยันว่าใช้ได้จริง
  import { api } from "../lib/api.js";
  import { toast } from "../lib/store.svelte.js";

  let { requests = [], onchange = () => {} } = $props();
  let showOk = $state(false);
  let busy = $state(0);

  const STATE = {
    "": { label: "รอทำ", cls: "wait" },
    done: { label: "AI ว่าเสร็จ — ลองแล้วกดยืนยัน", cls: "done" },
    ok: { label: "ใช้ได้แล้ว", cls: "ok" },
    no: { label: "AI ว่าทำไม่ได้", cls: "no" },
  };
  const clean = (t) => t.replace(/^\((สั่งในแชต|เจ้าของแจ้ง[^)]*)\)\s*/, "");
  const open = $derived(requests.filter((r) => r.state !== "ok"));
  const confirmed = $derived(requests.filter((r) => r.state === "ok"));
  const shown = $derived(showOk ? requests : open);

  async function set(r, state) {
    busy = r.id;
    try {
      await api.requestSet(r.id, state);
      toast(state === "ok" ? `ยืนยันคำสั่ง #${r.id} แล้ว` : `แจ้งว่าคำสั่ง #${r.id} ยังไม่ได้ — AI จะเห็นเป็นงานค้าง`);
      onchange();
    } catch (e) {
      toast(String(e), "err");
    } finally {
      busy = 0;
    }
  }
</script>

{#if requests.length}
  <section class="reqs">
    <div class="head row">
      <b>คำสั่งของคุณ</b>
      <span class="faint small">{open.length ? `ค้าง ${open.length} ข้อ` : "ครบทุกข้อแล้ว"}</span>
      <span class="grow"></span>
      {#if confirmed.length}
        <button class="ghost small" onclick={() => (showOk = !showOk)}>{showOk ? "ซ่อนข้อที่ใช้ได้แล้ว" : `ดูข้อที่ใช้ได้แล้ว (${confirmed.length})`}</button>
      {/if}
    </div>
    <ul>
      {#each shown as r (r.id)}
        {@const s = STATE[r.state] ?? STATE[""]}
        <li class={s.cls}>
          <div class="line">
            <span class="no faint">#{r.id}</span>
            <span class="txt">{clean(r.text)}</span>
          </div>
          <div class="row wrap small meta">
            <span class="chip {s.cls}">{s.label}</span>
            {#if r.note}<span class="note muted">{r.note}</span>{/if}
            <span class="grow"></span>
            {#if r.state !== "ok"}
              <button class="small okb" disabled={busy === r.id} onclick={() => set(r, "ok")} title="ลองแล้วใช้ได้จริง">✓ ใช้ได้แล้ว</button>
            {/if}
            {#if r.state === "done" || r.state === "ok" || r.state === "no"}
              <button class="ghost small" disabled={busy === r.id} onclick={() => set(r, "")} title="ยังไม่ได้ตามที่ขอ — กลับเป็นงานค้าง (ถ้าแชตเปิดอยู่ AI จะได้รับแจ้งทันที)">ยังไม่ได้</button>
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  </section>
{/if}

<style>
  .reqs { border: 1px solid var(--line); border-radius: 12px; padding: 10px 12px; background: var(--card); display: flex; flex-direction: column; gap: 8px; }
  .head { gap: 8px; align-items: center; }
  ul { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 6px; }
  li { border-left: 3px solid var(--line-2); padding: 4px 0 4px 10px; display: flex; flex-direction: column; gap: 4px; }
  li.done { border-left-color: var(--teal); }
  li.ok { border-left-color: var(--line); opacity: 0.7; }
  li.no { border-left-color: var(--red); }
  li.wait { border-left-color: var(--amber); }
  .line { display: flex; gap: 6px; line-height: 1.45; }
  .no { flex: none; }
  .txt { white-space: pre-wrap; word-break: break-word; }
  .meta { gap: 6px; align-items: center; }
  .chip { border-radius: 6px; padding: 1px 8px; background: var(--card-2); }
  .chip.wait { background: var(--amber-bg); color: var(--amber); }
  .chip.done { background: var(--teal-bg); color: var(--teal); }
  .chip.no { background: var(--red-bg); color: var(--red); }
  .note { font-size: 12px; }
  .okb { padding: 2px 10px; }
</style>
