<script>
  // Type "/" anywhere in a chat box (at the start, or after a space or a new line) and the skills of the library are
  // listed; each letter narrows the list ("/r" → the skills starting with r). ↑ ↓ choose, Enter or Tab puts "/name "
  // where the cursor is, Esc closes. The rest of the text is the request: the app sends the skill's text together with
  // it to whichever AI is chatting (ISSUE-066: it used to work only at the very start).
  import { onMount, tick } from "svelte";
  import { api } from "../lib/api.js";

  let { text = $bindable(""), below = false, el = null } = $props();
  let skills = $state([]);
  let idx = $state(0);
  let closed = $state(false);

  async function load() {
    skills = (await api.skillsSlash().catch(() => [])) ?? [];
  }
  onMount(load);

  // where the cursor is (without the box: the end of the text)
  let caret = $state(0);
  $effect(() => {
    if (!el) return;
    const at = () => (caret = el.selectionStart ?? text.length);
    const evs = ["input", "keyup", "click", "select", "focus"];
    evs.forEach((ev) => el.addEventListener(ev, at));
    return () => evs.forEach((ev) => el.removeEventListener(ev, at));
  });
  const pos = $derived(el ? Math.min(caret, text.length) : text.length);
  // the word being typed right before the cursor, when it is "/…" starting the text or after a space
  const m = $derived(/(^|\s)\/([\w-]*)$/.exec(text.slice(0, pos)));
  const q = $derived(m ? m[2].toLowerCase() : null);
  const items = $derived.by(() => {
    if (q === null) return [];
    // only names that START with what was typed, narrowing with every letter
    return skills.filter((s) => s.slug.startsWith(q) || s.name.toLowerCase().startsWith(q));
  });
  const open = $derived(q !== null && !closed);

  $effect(() => {
    q;
    idx = 0;
    closed = false;
    if (q !== null) load();
  });

  async function accept(s) {
    const start = pos - m[2].length - 1;
    // the rest of a name the cursor sits inside is replaced too
    const after = text.slice(pos).replace(/^[\w-]+/, "");
    const ins = `/${s.slug}` + (/^\s/.test(after) ? "" : " ");
    text = text.slice(0, start) + ins + after;
    const at = start + ins.length;
    caret = at;
    await tick();
    if (el) {
      el.focus();
      el.setSelectionRange(at, at);
    }
  }

  /** Call from the box's keydown: true = the key was used by the menu. */
  export function key(e) {
    if (!open) return false;
    const n = items.length;
    if (e.key === "ArrowDown" && n) idx = (idx + 1) % n;
    else if (e.key === "ArrowUp" && n) idx = (idx - 1 + n) % n;
    else if ((e.key === "Enter" || e.key === "Tab") && n && !e.isComposing && !e.shiftKey) accept(items[idx]);
    else if (e.key === "Escape") closed = true;
    else return false;
    e.preventDefault();
    return true;
  }
  $effect(() => {
    // keep the chosen row in view
    if (open) document.querySelector(".skillmenu .on")?.scrollIntoView({ block: "nearest" });
  });
</script>

{#if open}
  <div class="skillmenu" class:below role="listbox" aria-label="Skills">
    {#each items as s, i (s.slug)}
      <button type="button" role="option" aria-selected={i === idx} class:on={i === idx} onmousedown={(e) => { e.preventDefault(); accept(s); }} onmouseenter={() => (idx = i)}>
        <b>/{s.slug}</b>
        <span class="d">{s.description || s.name}</span>
      </button>
    {:else}
      <div class="empty">{skills.length ? "ไม่มีสกิลที่ขึ้นต้นด้วยคำนี้" : "ยังไม่มีสกิลที่เปิดใช้ — เพิ่มได้ที่หน้าสกิล"}</div>
    {/each}
    <div class="foot">↑ ↓ เลือก · Enter / Tab ใส่ชื่อสกิล · Esc ปิด</div>
  </div>
{/if}

<style>
  .skillmenu { position: absolute; left: 0; right: 0; bottom: 100%; margin-bottom: 6px; z-index: 30; max-height: 260px; overflow: auto;
    background: var(--card); border: 1px solid var(--line-2, var(--line)); border-radius: 10px; box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35); padding: 4px; display: flex; flex-direction: column; }
  .skillmenu.below { bottom: auto; top: 100%; margin-bottom: 0; margin-top: 6px; }
  button { display: flex; align-items: baseline; gap: 10px; text-align: start; border: none; background: transparent; padding: 6px 10px; border-radius: 6px; width: 100%; }
  button.on { background: var(--teal-bg); }
  b { color: var(--teal); font-weight: 600; flex: none; }
  .d { color: var(--text-2); font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
  .empty { padding: 8px 10px; color: var(--text-3); font-size: 13px; }
  .foot { padding: 4px 10px 2px; color: var(--text-3); font-size: 11px; border-top: 1px solid var(--line); margin-top: 2px; }
</style>
