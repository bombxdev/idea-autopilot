<script>
  // A persona's face, drawn by the app from its look (ISSUE-172): skin, hair style and colour, glasses, shirt,
  // an extra (beard, headphones, earrings) and an expression. No picture to download, the same everywhere
  // (lib/avatar.js — the 3D studio uses the same drawing).
  import { avatarSvg } from "../lib/avatar.js";
  let { look = {}, size = 48, ring = "" } = $props();
</script>

<span class="pa" style="width:{size}px;height:{size}px" aria-hidden="true">{@html avatarSvg(look, { size, ring })}</span>

<style>
  .pa { flex: none; display: inline-block; line-height: 0; }
  .pa :global(svg) { display: block; }
</style>
