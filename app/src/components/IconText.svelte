<script>
  // Shows a text that may start with (or contain) an emoji from the backend, drawing each one as the app's own icon.
  import Icon from "./Icon.svelte";
  import { splitIcons } from "../lib/icons.js";
  let { text = "" } = $props();
  const parts = $derived(splitIcons(text));
</script>

{#each parts as p}{#if p.i}<Icon name={p.i} />{:else}{p.t}{/if}{/each}
