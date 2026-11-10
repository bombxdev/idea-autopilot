<script>
  // One icon of the app's own set (lib/icons.js). Follows the text colour; size defaults to the text size.
  import { ICONS } from "../lib/icons.js";
  let { name, size = "1.15em", label = "", class: cls = "" } = $props();
</script>

<svg class="ico {cls}" viewBox="0 0 20 20" width={size} height={size} fill="none" stroke="currentColor" stroke-width="1.5"
  stroke-linecap="round" stroke-linejoin="round" role={label ? "img" : undefined} aria-label={label || undefined}
  aria-hidden={label ? undefined : "true"}>{@html ICONS[name] ?? ""}</svg>

<style>
  .ico { display: inline-block; flex: none; vertical-align: -0.2em; }
</style>
