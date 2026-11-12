<script>
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";
  import { refresh, toast } from "../lib/store.svelte.js";
  import Attach from "./Attach.svelte";
  import SkillMenu from "./SkillMenu.svelte";
  import IdeaAi from "./IdeaAi.svelte";
  import Scope from "./Scope.svelte";
  import Icon from "./Icon.svelte";
  import { autogrow } from "../lib/autogrow.js";
  import { codebox } from "../lib/codebox.js";
  import { lift } from "../lib/lift.js";

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
  // grows with the text like Claude's prompt box; ⤢ opens a big editor for a long prompt with files (ISSUE-078)
  let big = $state(false);
  function toggleBig() {
    big = !big;
    requestAnimationFrame(() => el?.focus());
  }

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
      big = false;
      toast("จดแล้ว — ยังแก้ข้อความได้ในหน้าไอเดียก่อนบอทเริ่มทำ (ประมาณ 1–2 นาที)", "ok", 4500);
      refresh();
      onadded?.();
    } catch (e) {
      err = String(e);
    }
  }
  function key(e) {
    if (menu?.key(e)) return; // the skill list takes ↑ ↓ Enter Tab Esc while it is open
    if (e.key === "Escape" && big) {
      big = false;
      return;
    }
    if (e.key !== "Enter" || e.isComposing) return;
    const mod = e.ctrlKey || e.metaKey;
    // in the big editor Enter is a new line; Ctrl+Enter sends
    if (big) {
      if (mod) {
        e.preventDefault();
        submit();
      }
      return;
    }
    // Enter mode: Enter sends, Shift+Enter = new line · Ctrl mode: Ctrl+Enter sends, Enter = new line
    if (mod || (sendKey === "enter" && !e.shiftKey)) {
      e.preventDefault();
      submit();
    }
  }
</script>

<div class="qa" class:big use:lift={big}>
  <span class="plus">＋</span>
  {#if big}<b class="bigtitle">จดไอเดีย</b>{/if}
  <div class="tw">
  <SkillMenu bind:text below bind:this={menu} {el} />
  <textarea
    bind:this={el}
    bind:value={text}
    use:autogrow={[big ? 0 : 0.45, text]}
    use:codebox={text}
    rows="1"
    oninput={() => (err = "")}
    onkeydown={key}
    onpaste={(e) => att?.paste(e)}
    placeholder={big ? "เขียนไอเดียยาวๆ ได้เต็มที่ — Enter ขึ้นบรรทัดใหม่ · Ctrl+Enter จดไอเดีย · วางรูปด้วย Ctrl+V · / เรียกสกิล" : sendKey === "enter"
      ? "จดไอเดีย — Enter ส่ง · Shift+Enter ขึ้นบรรทัดใหม่   ·   ! = ด่วน   ? = ขอแค่ research"
      : "จดไอเดีย — Ctrl+Enter ส่ง · Enter ขึ้นบรรทัดใหม่   ·   ! = ด่วน   ? = ขอแค่ research"}
  ></textarea>
  </div>
  <button type="button" class="ghost expand" onclick={toggleBig} title={big ? "ย่อกลับ (Esc)" : "ขยายเป็นช่องใหญ่ — สำหรับไอเดียยาวๆ และแนบรูป"}><Icon name={big ? "collapse" : "layers"} /></button>
  <Attach bind:this={att} bind:staged fallback priority={1} />
  <Scope bind:value={scope} compact />
  <IdeaAi bind:value={ai} />
  {#if err}<span class="err">{err}</span>{/if}
  {#if big}<div class="bigbar"><span class="faint small grow">Ctrl+Enter จดไอเดีย · Esc ย่อกลับ</span><button type="button" class="ghost" onclick={() => (big = false)}>ย่อกลับ</button><button type="button" class="primary" onclick={submit}>จดไอเดีย</button></div>{/if}
</div>

<style>
  .qa { flex: 1; display: flex; flex-wrap: wrap; align-items: flex-start; gap: 8px; background: var(--card); border: 1px solid var(--line); border-radius: 12px; padding: 0 12px; transition: border-color .15s; }
  .qa:focus-within { border-color: var(--teal); }
  .tw { position: relative; flex: 1; display: flex; min-width: 200px; }
  .tw textarea { flex: 1; }
  .qa :global(.scope) { margin-top: 6px; }
  .plus { color: var(--teal); font-size: 18px; padding-top: 8px; }
  textarea { flex: 1; min-width: 200px; border: none; background: transparent; padding: 10px 8px; font-size: 15px; resize: none; line-height: 1.4; font-family: inherit; }
  .expand { padding: 6px 8px; margin-top: 4px; color: var(--text-3); }
  .expand:hover { color: var(--teal); }
  /* the big editor: centred over a dimmed window, the box fills it */
  .qa.big { position: fixed; top: 7vh; left: 50%; transform: translateX(-50%); width: min(940px, 94vw); height: 84vh; z-index: 300; padding: 12px 16px;
    align-content: flex-start; border-color: var(--teal); box-shadow: 0 0 0 100vmax rgba(0, 0, 0, 0.55), 0 20px 60px rgba(0, 0, 0, 0.5); }
  .qa.big .plus { display: none; }
  .bigtitle { flex: 1 1 100%; font-size: 15px; }
  .qa.big .tw { flex: 1 1 100%; height: calc(84vh - 150px); }
  .qa.big textarea { height: 100% !important; overflow-y: auto !important; font-size: 15px; line-height: 1.55; }
  .bigbar { flex: 1 1 100%; display: flex; align-items: center; gap: 8px; }
  .err { color: var(--red); font-size: 13px; white-space: nowrap; padding-top: 10px; }
</style>
