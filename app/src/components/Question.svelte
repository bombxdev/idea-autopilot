<script>
  import { api } from "../lib/api.js";
  import { refresh, toast } from "../lib/store.svelte.js";

  let { q } = $props();
  let other = $state(false);
  let text = $state("");
  let err = $state("");
  let sent = $state(false);

  async function send(answer) {
    if (!answer?.trim()) {
      err = "พิมพ์คำตอบก่อน";
      return;
    }
    sent = true;
    try {
      await api.answer(q.id, answer.trim());
      toast("ตอบแล้ว — AI จะใช้คำตอบนี้รอบถัดไป");
      refresh();
    } catch (e) {
      sent = false;
      err = String(e);
    }
  }
</script>

<div class="q" class:sent>
  <div class="text">{q.question}</div>
  {#if q.answer}
    <div class="small muted">ตอบแล้ว: <b>{q.answer}</b></div>
  {:else}
    <div class="row wrap opts">
      {#each q.options as o}
        <button class:guess={o === q.guess} onclick={() => send(o)} disabled={sent}>{o}</button>
      {/each}
      <button class="ghost" onclick={() => (other = !other)}>อื่นๆ…</button>
    </div>
    {#if other}
      <div class="row other">
        <input class="grow" bind:value={text} oninput={() => (err = "")} placeholder="พิมพ์คำตอบสั้นๆ"
          onkeydown={(e) => e.key === "Enter" && !e.isComposing && send(text)} />
        <button class="primary" onclick={() => send(text)}>ส่ง</button>
      </div>
    {/if}
    {#if err}<div class="err small">{err}</div>{/if}
    {#if q.guess}<div class="faint small">ระหว่างรอ AI ใช้ “{q.guess}” ไปก่อน</div>{/if}
  {/if}
</div>

<style>
  .q { display: flex; flex-direction: column; gap: 8px; padding: 12px 14px; background: var(--amber-bg); border-radius: 10px; }
  .q.sent { opacity: .6; }
  .text { font-weight: 500; }
  .opts button { background: var(--card); }
  .opts button.guess { border-color: var(--amber); }
  .other input { background: var(--card); }
  .err { color: var(--red); }
</style>
