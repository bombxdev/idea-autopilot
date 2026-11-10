<script>
  import Icon from "./Icon.svelte";
  // หน้า “มีอะไรใหม่”: ขึ้นทุกครั้งที่ติดตั้งใหม่หรืออัปเดต แล้วจำว่าเห็นเวอร์ชันนี้แล้ว
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";
  import { store, refresh } from "../lib/store.svelte.js";
  import Markdown from "./Markdown.svelte";

  let about = $state(null);
  onMount(async () => (about = await api.about()));

  const cmp = (a, b) => {
    const [x, y] = [a, b].map((v) => v.split(".").map(Number));
    for (let i = 0; i < 3; i++) if ((x[i] || 0) !== (y[i] || 0)) return (x[i] || 0) - (y[i] || 0);
    return 0;
  };
  // each "## vX.Y.Z — date" block; updating shows what is newer than the last seen version,
  // a first install shows the newest one
  const entries = $derived.by(() => {
    if (!about) return [];
    const parts = about.changelog.split(/^## /m).slice(1).map((b) => ({ v: (b.match(/^v?(\d+\.\d+\.\d+)/) ?? [])[1], text: "## " + b })).filter((e) => e.v);
    const newer = about.seen ? parts.filter((e) => cmp(e.v, about.seen) > 0) : parts.slice(0, 1);
    return newer.length ? newer : parts.slice(0, 1);
  });

  async function ok() {
    await api.setSetting("seen_version", about.version);
    refresh();
  }
  async function all() {
    await ok();
    store.settingsTab = "about";
    store.page = "settings";
  }
</script>

<div class="scrim" role="presentation"></div>
<div class="box card">
  <h1><Icon name="sparkle" /> มีอะไรใหม่ใน Idea Autopilot v{about?.version ?? ""}</h1>
  <div class="body">
    {#each entries as e (e.v)}<Markdown text={e.text} />{/each}
  </div>
  <div class="row end">
    <button class="ghost" onclick={all}>ดูประวัติทุกเวอร์ชัน</button>
    <button class="primary" onclick={ok}>รับทราบ</button>
  </div>
</div>

<style>
  .scrim { position: fixed; inset: 0; background: rgba(0, 0, 0, .5); z-index: 70; }
  .box { position: fixed; z-index: 71; top: 50%; left: 50%; transform: translate(-50%, -50%); width: min(640px, calc(100vw - 32px)); max-height: 80vh; display: flex; flex-direction: column; gap: 12px; }
  h1 { margin: 0; font-size: 19px; }
  .body { overflow: auto; }
  .end { justify-content: flex-end; }
</style>
