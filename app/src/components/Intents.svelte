<script>
  import { intentMeta } from "../lib/api.js";
  let { intents = [] } = $props();
</script>

{#each intents as k}
  {@const m = intentMeta(k)}
  <span class="chip c-{m.color}">{m.icon} {m.label}</span>
{/each}

<style>
  .c-amber { background: var(--amber-bg); color: var(--amber); }
  .c-blue { background: var(--blue-bg); color: var(--blue); }
  .c-purple { background: var(--purple-bg); color: var(--purple); }
  .c-teal { background: var(--teal-bg); color: var(--teal); }
  .c-pink { background: var(--pink-bg); color: var(--pink); }
</style>
