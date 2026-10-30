<script>
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";
  import { refresh, toast } from "../lib/store.svelte.js";

  let { autofocus = false, onadded = null } = $props();
  let text = $state("");
  let err = $state("");
  let sendKey = $state("enter"); // enter = Enter sends · ctrl_enter = Ctrl+Enter sends, Enter is a new line
  let el;

  onMount(async () => {
    try { sendKey = (await api.settings()).send_key || "enter"; } catch {}
  });
  $effect(() => {
    if (autofocus) el?.focus();
  });
  // grow with the text (up to ~6 lines) so a longer idea stays visible while typing
  function fit() {
    if (!el) return;
    el.style.height = "auto";
    el.style.height = Math.min(el.scrollHeight, 150) + "px";
  }
  $effect(() => {
    text;
    fit();
  });

  async function submit() {
    if (!text.trim()) {
      err = "พิมพ์ไอเดียก่อน";
      return;
    }
    try {
      await api.addIdea(text);
      text = "";
      toast("จดแล้ว — ยังแก้ข้อความได้ในหน้าไอเดียก่อนบอทเริ่มทำ (ประมาณ 1–2 นาที)", "ok", 4500);
      refresh();
      onadded?.();
    } catch (e) {
      err = String(e);
    }
  }
  function key(e) {
    if (e.key !== "Enter" || e.isComposing) return;
    const mod = e.ctrlKey || e.metaKey;
    // Enter mode: Enter sends, Shift+Enter = new line · Ctrl mode: Ctrl+Enter sends, Enter = new line
    if (mod || (sendKey === "enter" && !e.shiftKey)) {
      e.preventDefault();
      submit();
    }
  }
</script>

<div class="qa">
  <span class="plus">＋</span>
  <textarea
    bind:this={el}
    bind:value={text}
    rows="1"
    oninput={() => (err = "")}
    onkeydown={key}
    placeholder={sendKey === "enter"
      ? "จดไอเดีย — Enter ส่ง · Shift+Enter ขึ้นบรรทัดใหม่   ·   ! = ด่วน   ? = ขอแค่ research"
      : "จดไอเดีย — Ctrl+Enter ส่ง · Enter ขึ้นบรรทัดใหม่   ·   ! = ด่วน   ? = ขอแค่ research"}
  ></textarea>
  {#if err}<span class="err">{err}</span>{/if}
</div>

<style>
  .qa { flex: 1; display: flex; align-items: flex-start; gap: 8px; background: var(--card); border: 1px solid var(--line); border-radius: 12px; padding: 0 12px; transition: border-color .15s; }
  .qa:focus-within { border-color: var(--teal); }
  .plus { color: var(--teal); font-size: 18px; padding-top: 8px; }
  textarea { flex: 1; border: none; background: transparent; padding: 10px 0; font-size: 15px; resize: none; line-height: 1.4; max-height: 150px; font-family: inherit; }
  .err { color: var(--red); font-size: 13px; white-space: nowrap; padding-top: 10px; }
</style>
