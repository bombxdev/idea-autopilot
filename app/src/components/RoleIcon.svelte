<script>
  // A team's or a role's icon in the app's own style (ISSUE-168): one picked in the app ("i:code"), an old emoji
  // mapped to the icon that means the same, or one guessed from the role's words.
  import Icon from "./Icon.svelte";
  import { roleIcon } from "../lib/icons.js";
  let { emoji = "", words = "", size = "1.15em" } = $props();
</script>

<span class="ri"><Icon name={roleIcon(emoji, words)} {size} /></span>

<style>
  .ri { display: inline-flex; align-items: center; justify-content: center; color: var(--teal); }
</style>
