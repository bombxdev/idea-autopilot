<script>
  import Icon from "./Icon.svelte";
  // Files attached to an idea (in its folder `attachments/`): thumbnails and delete.
  import { api } from "../lib/api.js";
  import { store, toast } from "../lib/store.svelte.js";
  import { ask } from "../lib/dialog.svelte.js";

  let { id } = $props();
  let items = $state([]);
  let thumbs = $state({});
  const ICON = { image: "image", pdf: "file", audio: "music", video: "film", text: "doc", office: "chart", other: "paperclip" };
  const mb = (n) => (n >= 1048576 ? `${(n / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`);

  async function load() {
    try {
      items = (await api.attachList(id)) ?? [];
    } catch {
      items = [];
    }
    for (const it of items) {
      if (it.kind === "image" && !(it.name in thumbs)) {
        thumbs[it.name] = null;
        api.attachPreview(id, it.name).then((u) => (thumbs[it.name] = u)).catch(() => {});
      }
    }
  }
  $effect(() => {
    id;
    store.ideas; // reload when the list refreshes (a new file may have been sent with an instruction)
    thumbs = {};
    load();
  });

  async function remove(it) {
    if (!(await ask(`${it.name}
ไฟล์จะถูกลบออกจากโฟลเดอร์ของไอเดียนี้`, { title: "ลบไฟล์แนบนี้?", ok: "ลบไฟล์", danger: true }))) return;
    try {
      await api.attachDelete(id, it.name);
      toast("ลบไฟล์แนบแล้ว");
      load();
    } catch (e) {
      toast(String(e), "err");
    }
  }
</script>

{#if items.length}
  <div class="al">
    <div class="small faint"><Icon name="paperclip" /> ไฟล์แนบ ({items.length}) — บอทอ่านจากโฟลเดอร์ของไอเดียนี้</div>
    <div class="grid">
      {#each items as it (it.name)}
        <div class="it" title={it.name}>
          {#if thumbs[it.name]}<img src={thumbs[it.name]} alt={it.name} />{:else}<span class="ic"><Icon name={ICON[it.kind] ?? "paperclip"} /></span>{/if}
          <span class="nm">{it.name}</span>
          <span class="faint small">{mb(it.size)}</span>
          <button class="x" aria-label="ลบ" onclick={() => remove(it)}><Icon name="close" /></button>
        </div>
      {/each}
    </div>
  </div>
{/if}

<style>
  .al { display: flex; flex-direction: column; gap: 6px; }
  .grid { display: flex; flex-wrap: wrap; gap: 8px; }
  .it { position: relative; display: flex; flex-direction: column; align-items: center; gap: 2px; width: 104px; background: var(--bg-2); border-radius: 8px; padding: 6px; }
  .it img { width: 90px; height: 64px; object-fit: cover; border-radius: 6px; }
  .ic { font-size: 34px; height: 64px; display: grid; place-items: center; }
  .nm { font-size: 12px; max-width: 92px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .x { position: absolute; top: 2px; right: 2px; border: none; background: rgba(0,0,0,.35); color: #fff; border-radius: 50%; width: 18px; height: 18px; font-size: 10px; line-height: 1; padding: 0; cursor: pointer; }
  .faint { color: var(--text-3); }
</style>
