<script>
  import Icon from "./Icon.svelte";
  // Which AI works on an idea: pick an account (only the ones switched on), a model and an effort — each "auto" unless the
  // owner chooses. The model and effort lists come from what that account can use. Used when adding and when editing.
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";
  import ModelPicker from "./ModelPicker.svelte";

  let { value = $bindable({ account: "", model: "", effort: "" }), onchange = null, startOpen = false, title = "AI สำหรับไอเดียนี้", autoNote = "ถ้าไม่เลือก บอทใช้บัญชีและโมเดลตามที่ตั้งไว้" } = $props();
  let models = $state([]);
  let hiddenModels = $state([]);
  let source = $state("");
  let open = $state(startOpen);
  let loading = $state(false);
  const LADDER = ["low", "medium", "high", "xhigh"];

  // read again whenever the picker opens: an account switched off in Settings since then must not be offered
  // (this bar stays mounted for the whole session). One already chosen for this idea stays listed, marked as off.
  let all = $state([]);
  function loadAccounts() {
    api.accounts().then((r) => (all = r?.accounts ?? [])).catch(() => {});
  }
  const accounts = $derived(all.filter((a) => (a.enabled && a.installed) || a.id === value.account));
  onMount(loadAccounts);
  $effect(() => {
    if (open) loadAccounts();
  });

  let loadedFor = "";
  $effect(() => {
    const id = value.account;
    if (id === loadedFor) return;
    loadedFor = id;
    models = [];
    hiddenModels = [];
    source = "";
    if (!id) return;
    loading = true;
    api.accountModels(id).then((m) => {
      if (loadedFor !== id) return;
      models = m?.models ?? [];
      hiddenModels = m?.hidden ?? [];
      source = m?.source ?? "";
    }).catch(() => {}).finally(() => (loading = false));
  });

  const acct = $derived(accounts.find((a) => a.id === value.account));
  const efforts = $derived.by(() => {
    if (acct && !acct.effort_supported) return [];
    const m = models.find((x) => x.id === value.model);
    return m?.efforts?.length ? m.efforts : LADDER;
  });
  const label = (a) => `${a.label || a.id} · ${a.vendor_label || a.kind_label}${a.enabled ? "" : " (ปิดอยู่ในตั้งค่า)"}`;
  const summary = $derived(
    !value.account && !value.model && !value.effort
      ? "อัตโนมัติ"
      : [acct ? acct.label || acct.id : value.account || "AI อัตโนมัติ", value.model || "model auto", value.effort || "effort auto"].join(" · "),
  );

  function set(patch) {
    value = { ...value, ...patch };
    // a model or effort that the newly chosen account cannot use is dropped
    if ("account" in patch) value = { ...value, model: "", effort: patch.account ? value.effort : "" };
    onchange?.(value);
  }
</script>

<div class="ai">
  <button type="button" class="head small" onclick={() => (open = !open)} aria-expanded={open}>
    <Icon name="bot" /> {title}: <b>{summary}</b> <span class="faint"><Icon name={open ? "chevronUp" : "chevronDown"} /></span>
  </button>
  {#if open}
    <div class="row wrap body small">
      <label>ค่าย / บัญชี
        <select value={value.account} onfocus={loadAccounts} onchange={(e) => set({ account: e.currentTarget.value })}>
          <option value="">อัตโนมัติ</option>
          {#each accounts as a (a.id)}<option value={a.id}>{label(a)}</option>{/each}
        </select></label>
      <span class="inl">โมเดล
        <ModelPicker value={value.model} models={models} hidden={hiddenModels} disabled={!value.account} auto="auto (โปรแกรมเลือกให้)" placeholder="auto (โปรแกรมเลือกให้)" width={280} onchange={(v) => set({ model: v })} /></span>
      <label>Effort
        <select value={value.effort} disabled={!value.account || (acct && !acct.effort_supported)} onchange={(e) => set({ effort: e.currentTarget.value })}>
          <option value="">auto (โปรแกรมเลือกให้)</option>
          {#each efforts as x}<option value={x}>{x}</option>{/each}
        </select></label>
      {#if acct && !acct.enabled}<span style="color:var(--amber)">⚠ บัญชีนี้ปิดอยู่ในตั้งค่า — บอทจะใช้บัญชีอื่นแทน เลือกใหม่ได้</span>
      {:else if !accounts.length}<span class="faint">ยังไม่มีบัญชี AI ที่เปิดอยู่ — เพิ่มได้ที่ตั้งค่า → บัญชี AI</span>
      {:else if value.account}<span class="faint">{loading ? "กำลังอ่านรายชื่อโมเดล…" : source === "account" ? "รายชื่อมาจากบัญชีนี้โดยตรง" : "รายชื่อมาตรฐานของผู้ให้บริการ"}</span>
      {:else}<span class="faint">เลือกบัญชีก่อนเพื่อกำหนดโมเดลและ effort · {autoNote}</span>{/if}
    </div>
  {/if}
</div>

<style>
  .ai { display: flex; flex-direction: column; gap: 6px; flex-basis: 100%; }
  .head { align-self: flex-start; border: none; background: transparent; padding: 2px 0; color: var(--text-2); text-align: left; }
  .head:hover { background: transparent; color: var(--text); }
  .body { gap: 12px; align-items: center; padding-bottom: 6px; }
  .inl { display: inline-flex; gap: 6px; align-items: center; }
  .body label { display: inline-flex; gap: 6px; align-items: center; }
  .faint { color: var(--text-3); }
</style>
