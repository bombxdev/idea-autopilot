<script>
  import Icon from "./Icon.svelte";
  // Attach pictures/files to an idea or an instruction: the paperclip button, Ctrl+V (a screenshot in the clipboard,
  // or files copied in Explorer) and drag & drop. Files are staged on the backend right away; the parent
  // sends the tokens in `staged` together with its text.
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";
  import { toast } from "../lib/store.svelte.js";
  import { dropzone } from "../lib/dropzones.js";

  let { staged = $bindable([]), priority = 1, fallback = false, zone = null, onpick = null } = $props();
  let info = $state(null);
  let hovering = $state(false);
  let busy = $state(0);
  let input;
  let root;

  onMount(() => {
    api.attachInfo().then((i) => (info = i)).catch(() => {});
    return dropzone({
      el: () => zone?.() ?? root,
      priority,
      fallback,
      onHover: (h) => (hovering = h),
      onDrop: (paths) => addPaths(paths),
    });
  });

  const ICON = { image: "image", pdf: "file", audio: "music", video: "film", text: "doc", office: "chart", other: "paperclip" };
  const mb = (n) => (n >= 1048576 ? `${(n / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`);

  function room(n) {
    const max = info?.max_files ?? 10;
    if (staged.length + busy + n > max) {
      toast(`แนบได้ไม่เกิน ${max} ไฟล์ต่อครั้ง`, "err");
      return false;
    }
    return true;
  }
  async function guard(job) {
    busy++;
    try {
      staged = [...staged, await job()];
    } catch (e) {
      toast(String(e), "err");
    }
    busy--;
  }
  async function addFiles(files) {
    if (!files.length || !room(files.length)) return;
    const max = (info?.max_mb ?? 25) * 1048576;
    for (const f of files) {
      if (f.size > max) {
        toast(`“${f.name}” ใหญ่เกิน ${info?.max_mb ?? 25} MB`, "err");
        continue;
      }
      // a pasted screenshot has no real name
      const generic = /^(image|unknown|blob)\.(png|jpe?g|gif|webp)$/i.test(f.name || "");
      const file = generic ? new File([f], `screenshot-${stamp()}.${(f.type.split("/")[1] || "png").replace("jpeg", "jpg")}`, { type: f.type }) : f;
      const thumb = file.type.startsWith("image/") ? URL.createObjectURL(file) : null;
      guard(async () => ({ ...(await api.attachStage(file)), thumb }));
    }
  }
  async function addPaths(paths) {
    if (!paths.length || !room(paths.length)) return;
    for (const p of paths) guard(() => api.attachStagePath(p));
  }
  const stamp = () => new Date().toISOString().replace(/[-:T]/g, "").slice(0, 14);

  /** call from the textarea's onpaste: files in the clipboard are attached, plain text pastes as usual */
  export function paste(e) {
    const files = [...(e.clipboardData?.files ?? [])];
    if (!files.length) return;
    e.preventDefault();
    addFiles(files);
  }
  export function reset() {
    for (const s of staged) if (s.thumb) URL.revokeObjectURL(s.thumb);
    staged = [];
  }
  export function pending() {
    return busy > 0;
  }
  async function remove(s) {
    if (s.thumb) URL.revokeObjectURL(s.thumb);
    staged = staged.filter((x) => x.token !== s.token);
    api.attachUnstage(s.token).catch(() => {});
  }
  async function picked(e) {
    const files = [...e.currentTarget.files];
    e.currentTarget.value = "";
    onpick?.(false);
    await addFiles(files);
  }
  const unsupported = (s) => info && !["text"].includes(s.kind) && !info.accepts.includes(s.kind);
</script>

<span class="att" bind:this={root} class:hover={hovering}>
  <button type="button" class="clip" title="แนบรูป/ไฟล์ (หรือกด Ctrl+V วางภาพที่แคปไว้ หรือลากไฟล์มาวาง)"
    onclick={() => { onpick?.(true); input.click(); }}><Icon name="paperclip" />{#if busy}<span class="spin">…</span>{/if}</button>
  <input bind:this={input} type="file" multiple hidden onchange={picked} oncancel={() => onpick?.(false)} />
</span>
{#if hovering}<div class="drop">ปล่อยไฟล์ตรงนี้เพื่อแนบ</div>{/if}
{#if staged.length}
  <div class="chips">
    {#each staged as s (s.token)}
      <span class="chipf" class:warn={unsupported(s)} title={unsupported(s) ? `${info.label} อาจเปิดไฟล์ชนิดนี้ตรงๆ ไม่ได้ — ไฟล์จะถูกเก็บไว้ แต่บอทอาจอ่านไม่ได้` : s.name}>
        {#if s.thumb}<img src={s.thumb} alt="" />{:else}<span><Icon name={ICON[s.kind] ?? "paperclip"} /></span>{/if}
        <span class="nm">{s.name}</span><span class="faint">{mb(s.size)}</span>
        {#if unsupported(s)}<span class="w"><Icon name="warning" /></span>{/if}
        <button type="button" class="x" aria-label="เอาออก" onclick={() => remove(s)}><Icon name="close" /></button>
      </span>
    {/each}
  </div>
{/if}

<style>
  .att { display: inline-flex; }
  .clip { border: none; background: transparent; font-size: 17px; padding: 6px 4px; cursor: pointer; color: var(--text-2); }
  .clip:hover { color: var(--teal); background: transparent; }
  .spin { font-size: 12px; }
  .drop { flex-basis: 100%; text-align: center; border: 2px dashed var(--teal); border-radius: 10px; padding: 10px; color: var(--teal); background: var(--teal-bg); font-size: 14px; }
  .chips { flex-basis: 100%; display: flex; flex-wrap: wrap; gap: 6px; padding: 0 0 8px; }
  .chipf { display: inline-flex; align-items: center; gap: 6px; background: var(--bg-2); border: 1px solid var(--line, rgba(128,128,128,.25)); border-radius: 8px; padding: 2px 6px; font-size: 12px; max-width: 260px; }
  .chipf.warn { border-color: var(--amber); }
  .chipf img { width: 24px; height: 24px; object-fit: cover; border-radius: 4px; }
  .nm { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 150px; }
  .faint { color: var(--text-3); }
  .w { color: var(--amber); }
  .x { border: none; background: transparent; padding: 0 2px; font-size: 12px; color: var(--text-3); cursor: pointer; }
  .x:hover { color: var(--red); background: transparent; }
</style>
