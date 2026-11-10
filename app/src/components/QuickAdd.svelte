<script>
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";
  import { refresh, toast } from "../lib/store.svelte.js";
  import Attach from "./Attach.svelte";
  import SkillMenu from "./SkillMenu.svelte";
  import IdeaAi from "./IdeaAi.svelte";
  import Scope from "./Scope.svelte";

  let { autofocus = false, onadded = null } = $props();
  let text = $state("");
  let err = $state("");
  let sendKey = $state("enter"); // enter = Enter sends · ctrl_enter = Ctrl+Enter sends, Enter is a new line
  let el = $state();
  let att;
  let staged = $state([]);
  let menu;
  let ai = $state({ account: "", model: "", effort: "" });
  let scope = $state("personal"); // starts as the last set used

  onMount(async () => {
    try {
      const s = await api.settings();
      sendKey = s.send_key || "enter";
      scope = s.scope_default === "work" ? "work" : "personal";
    } catch {}
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
    if (att?.pending()) {
      err = "รอไฟล์แนบอัปโหลดก่อน";
      return;
    }
    if (!text.trim() && !staged.length) {
      err = "พิมพ์ไอเดียก่อน";
      return;
    }
    try {
      // files only: the idea is named after them
      await api.addIdea(text.trim() ? text : `ไอเดียจากไฟล์แนบ: ${staged.map((s) => s.name).join(", ")}`, staged.map((s) => s.token), ai, scope);
      att?.reset();
      ai = { account: "", model: "", effort: "" };
      text = "";
      toast("จดแล้ว — ยังแก้ข้อความได้ในหน้าไอเดียก่อนบอทเริ่มทำ (ประมาณ 1–2 นาที)", "ok", 4500);
      refresh();
      onadded?.();
    } catch (e) {
      err = String(e);
    }
  }
  function key(e) {
    if (menu?.key(e)) return; // the skill list takes ↑ ↓ Enter Tab Esc while it is open
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
  <div class="tw">
  <SkillMenu bind:text below bind:this={menu} {el} />
  <textarea
    bind:this={el}
    bind:value={text}
    rows="1"
    oninput={() => (err = "")}
    onkeydown={key}
    onpaste={(e) => att?.paste(e)}
    placeholder={sendKey === "enter"
      ? "จดไอเดีย — Enter ส่ง · Shift+Enter ขึ้นบรรทัดใหม่   ·   ! = ด่วน   ? = ขอแค่ research"
      : "จดไอเดีย — Ctrl+Enter ส่ง · Enter ขึ้นบรรทัดใหม่   ·   ! = ด่วน   ? = ขอแค่ research"}
  ></textarea>
  </div>
  <Attach bind:this={att} bind:staged fallback priority={1} />
  <Scope bind:value={scope} compact />
  <IdeaAi bind:value={ai} />
  {#if err}<span class="err">{err}</span>{/if}
</div>

<style>
  .qa { flex: 1; display: flex; flex-wrap: wrap; align-items: flex-start; gap: 8px; background: var(--card); border: 1px solid var(--line); border-radius: 12px; padding: 0 12px; transition: border-color .15s; }
  .qa:focus-within { border-color: var(--teal); }
  .tw { position: relative; flex: 1; display: flex; min-width: 200px; }
  .tw textarea { flex: 1; }
  .qa :global(.scope) { margin-top: 6px; }
  .plus { color: var(--teal); font-size: 18px; padding-top: 8px; }
  textarea { flex: 1; min-width: 200px; border: none; background: transparent; padding: 10px 0; font-size: 15px; resize: none; line-height: 1.4; max-height: 150px; font-family: inherit; }
  .err { color: var(--red); font-size: 13px; white-space: nowrap; padding-top: 10px; }
</style>
