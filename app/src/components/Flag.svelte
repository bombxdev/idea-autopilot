<script>
  // Simplified flags drawn as SVG: Windows has no emoji flags (it shows two letters instead), and the app works offline.
  let { code, size = 24 } = $props();
  const w = $derived(Math.round(size * 1.5));
</script>

<svg viewBox="0 0 30 20" width={w} height={size} role="img" aria-label={code} class="flag">
  <defs><clipPath id="c-{code}"><rect width="30" height="20" rx="2.5" /></clipPath></defs>
  <g clip-path="url(#c-{code})">
    {#if code === "th"}
      <rect width="30" height="20" fill="#a51931" /><rect y="3.3" width="30" height="13.4" fill="#f4f5f8" /><rect y="6.7" width="30" height="6.6" fill="#2d2a4a" />
    {:else if code === "en"}
      <rect width="30" height="20" fill="#012169" />
      <path d="M0 0L30 20M30 0L0 20" stroke="#fff" stroke-width="4" /><path d="M0 0L30 20M30 0L0 20" stroke="#c8102e" stroke-width="1.6" />
      <path d="M15 0V20M0 10H30" stroke="#fff" stroke-width="6.5" /><path d="M15 0V20M0 10H30" stroke="#c8102e" stroke-width="3.6" />
    {:else if code === "zh"}
      <rect width="30" height="20" fill="#de2910" />
      <polygon points="6,2.2 7.3,5.9 11.2,6 8.1,8.3 9.2,12 6,9.8 2.8,12 3.9,8.3 0.8,6 4.7,5.9" fill="#ffde00" />
    {:else if code === "ja"}
      <rect width="30" height="20" fill="#fff" /><circle cx="15" cy="10" r="5.5" fill="#bc002d" />
    {:else if code === "ko"}
      <rect width="30" height="20" fill="#fff" />
      <circle cx="15" cy="10" r="4.6" fill="#cd2e3a" /><path d="M10.4 10a4.6 4.6 0 0 0 9.2 0a2.3 2.3 0 0 0-4.6 0a2.3 2.3 0 0 1-4.6 0z" fill="#0047a0" />
      <g stroke="#111" stroke-width="1"><path d="M3 4l3-2M4 6l3-2M23 2l3 2M24 4l3 2M3 16l3 2M4 14l3 2M23 18l3-2M24 16l3-2" /></g>
    {:else if code === "vi"}
      <rect width="30" height="20" fill="#da251d" />
      <polygon points="15,4.2 16.6,8.7 21.3,8.8 17.6,11.6 18.9,16 15,13.4 11.1,16 12.4,11.6 8.7,8.8 13.4,8.7" fill="#ffff00" />
    {:else if code === "id"}
      <rect width="30" height="10" fill="#e70011" /><rect y="10" width="30" height="10" fill="#fff" />
    {:else if code === "es"}
      <rect width="30" height="20" fill="#aa151b" /><rect y="5" width="30" height="10" fill="#f1bf00" />
    {:else if code === "fr"}
      <rect width="10" height="20" fill="#0055a4" /><rect x="10" width="10" height="20" fill="#fff" /><rect x="20" width="10" height="20" fill="#ef4135" />
    {:else if code === "de"}
      <rect width="30" height="6.7" fill="#000" /><rect y="6.7" width="30" height="6.6" fill="#dd0000" /><rect y="13.3" width="30" height="6.7" fill="#ffce00" />
    {:else if code === "pt"}
      <rect width="30" height="20" fill="#009c3b" /><polygon points="15,2.5 27,10 15,17.5 3,10" fill="#ffdf00" /><circle cx="15" cy="10" r="4.4" fill="#002776" />
    {:else if code === "ru"}
      <rect width="30" height="6.7" fill="#fff" /><rect y="6.7" width="30" height="6.6" fill="#0039a6" /><rect y="13.3" width="30" height="6.7" fill="#d52b1e" />
    {:else if code === "ar"}
      <rect width="30" height="20" fill="#006c35" /><rect x="6" y="6.5" width="18" height="1.6" rx="0.8" fill="#fff" /><rect x="7" y="13" width="16" height="1.2" rx="0.6" fill="#fff" />
    {:else if code === "hi"}
      <rect width="30" height="6.7" fill="#ff9933" /><rect y="6.7" width="30" height="6.6" fill="#fff" /><rect y="13.3" width="30" height="6.7" fill="#138808" />
      <circle cx="15" cy="10" r="2.4" fill="none" stroke="#000080" stroke-width="0.7" />
    {:else if code === "it"}
      <rect width="10" height="20" fill="#009246" /><rect x="10" width="10" height="20" fill="#fff" /><rect x="20" width="10" height="20" fill="#ce2b37" />
    {:else if code === "tr"}
      <rect width="30" height="20" fill="#e30a17" /><circle cx="11.3" cy="10" r="5" fill="#fff" /><circle cx="12.7" cy="10" r="4" fill="#e30a17" />
      <polygon points="18.6,10 20.6,9.3 20.6,11.4 19.4,10.7 18.4,12 18.2,10.6" fill="#fff" transform="translate(1.2 -0.1)" />
    {:else}
      <rect width="30" height="20" fill="#888" />
    {/if}
  </g>
  <rect width="30" height="20" rx="2.5" fill="none" stroke="rgba(128,128,128,.45)" stroke-width="0.8" />
</svg>

<style>
  .flag { flex: none; display: block; }
</style>
