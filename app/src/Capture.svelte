<script>
  import { onMount } from "svelte";
  import { api, listen } from "./lib/api.js";
  import { getCurrentWindow } from "@tauri-apps/api/window";

  let text = $state("");
  let err = $state("");
  let done = $state(false);
  let el = $state();
  let sendKey = $state("enter");
  const loadKey = async () => {
    try { sendKey = (await api.settings()).send_key || "enter"; } catch {}
  };

  function reset() {
    loadKey();
    text = "";
    err = "";
    done = false;
    setTimeout(() => el?.focus(), 30);
  }

  onMount(() => {
    reset();
    listen("capture-shown", reset);
    const w = getCurrentWindow();
    w.onFocusChanged(({ payload }) => {
      if (!payload) api.hideCapture();
    });
  });

  async function submit() {
    if (!text.trim()) {
      err = "พิมพ์ไอเดียก่อน";
      return;
    }
    try {
      await api.addIdea(text);
      done = true;
      setTimeout(() => api.hideCapture(), 650);
    } catch (e) {
      err = String(e);
    }
  }
  function key(e) {
    if (e.key === "Escape") api.hideCapture();
    else if (e.key === "Enter" && !e.isComposing && (e.ctrlKey || e.metaKey || (sendKey === "enter" && !e.shiftKey))) {
      e.preventDefault();
      submit();
    }
  }
</script>

<div class="cap" class:done>
  <div class="row top">
    <span class="logo">◐</span>
    <span class="faint small grow">{sendKey === "enter" ? "จดไอเดีย — Enter บันทึก · Shift+Enter ขึ้นบรรทัด · Esc ปิด" : "จดไอเดีย — Ctrl+Enter บันทึก · Enter ขึ้นบรรทัด · Esc ปิด"}</span>
    <span class="faint small">! ด่วน · ? research</span>
  </div>
  {#if done}
    <div class="ok">✓ จดแล้ว — ไปทำอย่างอื่นได้เลย</div>
  {:else}
    <textarea bind:this={el} bind:value={text} oninput={() => (err = "")} onkeydown={key} rows="2"
      placeholder="เช่น อยากรู้ว่าทำแอปจองคิวร้านตัดผมขายได้ไหม"></textarea>
    {#if err}<div class="err small">{err}</div>{/if}
  {/if}
</div>

<style>
  :global(html), :global(body) { background: transparent !important; overflow: hidden; }
  .cap { margin: 6px; height: calc(100vh - 12px); background: var(--card); border: 1px solid var(--teal); border-radius: 16px; padding: 12px 16px; display: flex; flex-direction: column; gap: 8px; }
  .logo { color: var(--teal); font-size: 18px; }
  textarea { flex: 1; resize: none; border: none; background: transparent; font-size: 18px; padding: 0; line-height: 1.5; }
  .ok { flex: 1; display: grid; place-items: center; color: var(--teal); font-size: 18px; animation: rise .2s; }
  .err { color: var(--red); }
</style>
