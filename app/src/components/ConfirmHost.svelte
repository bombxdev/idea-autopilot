<script>
  // Draws the app's confirmation box (see lib/dialog.svelte.js). Esc / click outside = cancel, Enter = OK.
  import { dialog, answer } from "../lib/dialog.svelte.js";

  let okBtn = $state();
  $effect(() => {
    if (dialog.open) okBtn?.focus();
  });
  function key(e) {
    if (!dialog.open) return;
    if (e.key === "Escape") {
      e.preventDefault();
      answer(false);
    }
  }
</script>

<svelte:window onkeydown={key} />

{#if dialog.open}
  {@const d = dialog.open}
  <div class="back" role="presentation" onclick={(e) => e.target === e.currentTarget && answer(false)}>
    <div class="box" role="alertdialog" aria-modal="true" aria-label={d.title || d.message}>
      <div class="ic" class:danger={d.danger}>{d.danger ? "🗑" : "❔"}</div>
      <div class="body">
        {#if d.title}<b class="t">{d.title}</b>{/if}
        <div class="msg">{d.message}</div>
        {#if d.code}<code class="code">{d.code}</code>{/if}
      </div>
      <div class="btns">
        <button onclick={() => answer(false)}>{d.cancel}</button>
        <button bind:this={okBtn} class:danger={d.danger} class:primary={!d.danger} onclick={() => answer(true)}>{d.ok}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .back { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.5); backdrop-filter: blur(2px); display: grid; place-items: center; z-index: 200; animation: fade 0.12s ease-out; }
  .box { width: min(440px, calc(100vw - 32px)); background: var(--card-2, var(--bg-2)); border: 1px solid var(--line-2, var(--line)); border-radius: 14px; padding: 18px 20px; box-shadow: 0 18px 50px rgba(0, 0, 0, 0.45); display: grid; grid-template-columns: auto 1fr; gap: 6px 14px; animation: rise 0.15s ease-out; }
  .ic { font-size: 22px; width: 40px; height: 40px; border-radius: 50%; display: grid; place-items: center; background: var(--teal-bg); }
  .ic.danger { background: rgba(255, 90, 90, 0.15); }
  .body { display: flex; flex-direction: column; gap: 8px; min-width: 0; }
  .t { font-size: 15px; }
  .msg { white-space: pre-wrap; overflow-wrap: anywhere; color: var(--text-2, var(--text)); line-height: 1.5; }
  .code { display: block; padding: 8px 10px; background: var(--bg); border-radius: 8px; font-size: 13px; overflow-wrap: anywhere; }
  .btns { grid-column: 1 / -1; display: flex; justify-content: flex-end; gap: 8px; margin-top: 8px; }
  .btns button { min-width: 84px; }
  .btns .danger { background: var(--red); border-color: var(--red); color: #fff; }
  @keyframes fade { from { opacity: 0; } }
  @keyframes rise { from { opacity: 0; transform: translateY(8px) scale(0.98); } }
</style>
