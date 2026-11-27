<script>
  // Pick an icon for a team or a role from the app's own set (ISSUE-168) instead of typing an emoji.
  import Icon from "./Icon.svelte";
  import RoleIcon from "./RoleIcon.svelte";
  import { ROLE_ICONS } from "../lib/icons.js";

  let { value = $bindable(""), words = "" } = $props();
  let open = $state(false);
  let box;
  function pick(name) {
    value = "i:" + name;
    open = false;
  }
  function outside(e) {
    if (open && box && !box.contains(e.target)) open = false;
  }
</script>

<svelte:window onclick={outside} onkeydown={(e) => e.key === "Escape" && (open = false)} />

<span class="ip" bind:this={box}>
  <button type="button" class="cur" onclick={() => (open = !open)} title="เลือกไอคอน" aria-expanded={open}>
    <RoleIcon emoji={value} {words} size="1.4em" /><Icon name={open ? "chevronUp" : "chevronDown"} />
  </button>
  {#if open}
    <div class="pop">
      {#each ROLE_ICONS as [name, label] (name)}
        <button type="button" class="opt" class:on={value === "i:" + name} onclick={() => pick(name)} title={label}>
          <Icon name={name} size="1.35em" /><span>{label}</span>
        </button>
      {/each}
    </div>
  {/if}
</span>

<style>
  .ip { position: relative; display: inline-block; }
  .cur { display: inline-flex; align-items: center; gap: 6px; padding: 7px 10px; }
  .pop { position: absolute; top: calc(100% + 6px); left: 0; z-index: 60; width: min(520px, 80vw); max-height: 320px; overflow: auto; display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 4px; padding: 8px; background: var(--card); border: 1px solid var(--line-2);
    border-radius: 10px; box-shadow: 0 8px 24px rgba(0, 0, 0, .35); }
  .opt { display: flex; align-items: center; gap: 8px; text-align: left; padding: 6px 8px; border: 1px solid transparent; background: transparent; font-size: 12.5px; color: var(--text-2); }
  .opt :global(.ico) { color: var(--teal); }
  .opt:hover { background: var(--bg-2); color: var(--text); }
  .opt.on { border-color: var(--teal); color: var(--text); }
</style>
