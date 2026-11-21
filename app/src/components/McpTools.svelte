<script>
  // Tools (MCP servers) next to skills (mcp.rs). A skill is a recipe; a tool is a program that gives the AI new
  // abilities. A bot run gets a tool only when a skill offered to that run needs it — every tool adds its whole list of
  // functions to every turn, so "always on" is a choice the owner makes knowingly.
  import Icon from "./Icon.svelte";
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";
  import { toast } from "../lib/store.svelte.js";
  import { ask } from "../lib/dialog.svelte.js";

  let { skills = [], cats = {} } = $props();

  let mine = $state([]);
  let local = $state([]);
  let q = $state("");
  let found = $state(null);
  let searching = $state(false);
  let cat = $state("");
  let open = $state(null); // registry item being installed
  let way = $state(0);
  let values = $state({});
  let busy = $state(false);
  let linking = $state(""); // tool id whose skills are being picked

  async function load() {
    try {
      mine = await api.mcpList();
      local = await api.mcpLocal();
    } catch (e) { toast(String(e), "err"); }
  }
  onMount(load);

  async function search() {
    searching = true;
    open = null;
    try { found = (await api.mcpSearch(q.trim())).servers; } catch (e) { found = []; toast(String(e), "err"); }
    searching = false;
  }
  const catCounts = $derived.by(() => {
    const m = new Map();
    for (const r of found ?? []) m.set(r.category || "other", (m.get(r.category || "other") ?? 0) + 1);
    return [...m.entries()].sort((a, b) => b[1] - a[1]);
  });
  const shown = $derived((found ?? []).filter((r) => !cat || (r.category || "other") === cat));

  async function importLocal(t) {
    try { await api.mcpImport(t.id); toast(`นำเข้า ${t.id} แล้ว`); } catch (e) { toast(String(e), "err"); }
    load();
  }
  function pick(r) {
    open = r;
    way = 0;
    values = {};
  }
  async function install() {
    const w = open.ways[way];
    const kind = w.kind === "remote" ? "เชื่อมต่อกับ" : "รันคำสั่ง";
    if (!(await ask(`${kind}\n${w.command}\n\nเครื่องมือนี้จะทำงานในเครื่องของคุณเมื่อบอทใช้สกิลที่ต้องการมัน — ติดตั้งเฉพาะจากแหล่งที่เชื่อถือ`, { ok: "ติดตั้ง" }))) return;
    busy = true;
    try {
      await api.mcpInstall(open.name, w.kind, w.index, values);
      toast(`ติดตั้ง ${open.title || open.name} แล้ว — เลือกสกิลที่ใช้เครื่องมือนี้ได้ด้านบน`);
      open = null;
    } catch (e) { toast(String(e), "err"); }
    busy = false;
    load();
  }
  async function set(t, field, value) {
    try { await api.mcpSet(t.id, field, String(value)); } catch (e) { toast(String(e), "err"); }
    load();
  }
  async function toggleSkill(t, slug, on) {
    const next = on ? [...new Set([...t.skills, slug])] : t.skills.filter((s) => s !== slug);
    await set(t, "skills", next.join(","));
  }
  async function remove(t) {
    if (!(await ask(`เอา ${t.label || t.id} ออกจากแอป? (ไม่กระทบ Claude Code ของคุณ)`, { ok: "เอาออก" }))) return;
    try { await api.mcpRemove(t.id); } catch (e) { toast(String(e), "err"); }
    load();
  }
  const skillName = (slug) => skills.find((s) => s.slug === slug)?.name ?? slug;
</script>

<div class="card info small">
  <Icon name="plug" /> <b>สกิล</b> = สูตรที่ AI อ่าน · <b>เครื่องมือ (MCP)</b> = โปรแกรมที่ให้ AI ทำสิ่งใหม่ได้ (เช่น REA ถอดโปรแกรม, เบราว์เซอร์, ฐานข้อมูล)
  บางสกิลต้องใช้เครื่องมือ — ผูกไว้แล้วบอทจะเปิดเครื่องมือให้<b>เฉพาะรอบที่ใช้สกิลนั้น</b> เพราะเครื่องมือแต่ละตัวเพิ่ม token ทุกข้อความ (REA มี 207 คำสั่ง)
</div>

<h3>เครื่องมือของฉัน ({mine.length})</h3>
{#each mine as t (t.id)}
  <div class="card tool" class:off={!t.enabled}>
    <div class="row wrap">
      <b class="grow">{t.label || t.id}</b>
      <span class="faint small">{cats[t.category] ?? ""}</span>
      <span class="chip">{t.source === "claude-code" ? "จาก Claude Code" : t.source === "registry" ? "จาก Registry" : "เพิ่มเอง"}</span>
      <label class="small"><input type="checkbox" checked={t.enabled} onchange={(e) => set(t, "enabled", e.target.checked)} /> เปิดใช้</label>
      <button class="ghost small" onclick={() => remove(t)} title="เอาออก"><Icon name="trash" /></button>
    </div>
    {#if t.description}<div class="muted small">{t.description}</div>{/if}
    <div class="faint small mono">{t.command}{#if t.env_names.length} · ค่าที่ตั้งไว้: {t.env_names.join(", ")}{/if}</div>
    <div class="row wrap small links">
      <span>สกิลที่ใช้:</span>
      {#each t.skills as k (k)}<span class="chip teal">{skillName(k)}</span>{:else}<span class="faint">ยังไม่มี — บอทจะไม่เปิดเครื่องมือนี้</span>{/each}
      <button class="ghost small" onclick={() => (linking = linking === t.id ? "" : t.id)}><Icon name="pencil" /> เลือก</button>
      <label class="faint" title="เปิดให้ทุกรอบ แม้ไม่มีสกิลที่ต้องใช้ — เสีย token ทุกข้อความ"><input type="checkbox" checked={t.always} onchange={(e) => set(t, "always", e.target.checked)} /> ทุกรอบ</label>
    </div>
    {#if linking === t.id}
      <div class="row wrap pick">
        {#each skills as s (s.slug)}
          <label class="small"><input type="checkbox" checked={t.skills.includes(s.slug)} onchange={(e) => toggleSkill(t, s.slug, e.target.checked)} /> {s.name}</label>
        {:else}<span class="faint small">ยังไม่มีสกิลในเครื่อง</span>{/each}
      </div>
    {/if}
  </div>
{:else}
  <div class="faint small">ยังไม่มี — นำเข้าจาก Claude Code ในเครื่อง หรือค้นหาจาก Registry ด้านล่าง</div>
{/each}

{#if local.some((t) => !t.imported)}
  <h3>มีอยู่แล้วใน Claude Code เครื่องนี้</h3>
  {#each local.filter((t) => !t.imported) as t (t.id)}
    <div class="card row wrap tool">
      <b>{t.id}</b><span class="grow faint small mono">{t.command}</span>
      <button class="primary small" onclick={() => importLocal(t)}><Icon name="download" /> นำเข้า</button>
    </div>
  {/each}
{/if}

<h3>ค้นหาจาก MCP Registry (ทางการ · ฟรี)</h3>
<form class="row" onsubmit={(e) => { e.preventDefault(); search(); }}>
  <input class="grow" bind:value={q} placeholder="เช่น browser, postgres, github (เว้นว่าง = ตัวใหม่ล่าสุด)" />
  <button class="primary" disabled={searching}>{searching ? "กำลังค้นหา…" : "ค้นหา"}</button>
</form>
{#if found}
  {#if catCounts.length > 1}
    <div class="row wrap cats">
      <button class="chip" class:on={!cat} onclick={() => (cat = "")}>ทุกหมวด ({found.length})</button>
      {#each catCounts as [k, n] (k)}<button class="chip" class:on={cat === k} onclick={() => (cat = cat === k ? "" : k)}>{cats[k] ?? k} ({n})</button>{/each}
    </div>
  {/if}
  {#each shown as r (r.name)}
    <div class="card tool">
      <div class="row wrap"><b class="grow">{r.title || r.name}</b><span class="faint small">{cats[r.category || "other"]}</span><span class="faint small">v{r.version} · {r.published?.slice(0, 10)}</span></div>
      <div class="muted small">{r.description}</div>
      <div class="row wrap small"><span class="faint mono">{r.name}</span>{#if r.repo}<a href={r.repo} target="_blank" rel="noreferrer">source</a>{/if}
        <span class="grow"></span><button class="small" onclick={() => pick(r)}>ติดตั้ง…</button></div>
      {#if open?.name === r.name}
        <div class="inst">
          {#if r.ways.length > 1}
            <select bind:value={way}>{#each r.ways as w, i}<option value={i}>{w.kind === "remote" ? "เชื่อมต่อออนไลน์" : "รันในเครื่อง"}: {w.command}</option>{/each}</select>
          {/if}
          <div class="faint small mono">{r.ways[way].command}</div>
          {#each r.ways[way].env as v (v.name)}
            <label class="small envf"><span>{v.name}{v.required ? " *" : ""}</span>
              <input type={v.secret ? "password" : "text"} placeholder={v.description ?? ""} bind:value={values[v.name]} /></label>
          {/each}
          <div class="row"><span class="grow faint small">ค่าที่ใส่เก็บในเครื่องนี้เท่านั้น</span><button class="ghost small" onclick={() => (open = null)}>ยกเลิก</button><button class="primary small" disabled={busy} onclick={install}>ติดตั้ง</button></div>
        </div>
      {/if}
    </div>
  {:else}<div class="faint small">ไม่พบ</div>{/each}
{/if}

<style>
  h3 { margin: 18px 0 8px; font-size: 15px; }
  .tool { display: flex; flex-direction: column; gap: 6px; margin-bottom: 8px; }
  .tool.off { opacity: .6; }
  .mono { font-family: ui-monospace, monospace; word-break: break-all; }
  .links { gap: 6px; align-items: center; }
  .pick { gap: 10px; padding: 8px; background: var(--bg-2); border-radius: 8px; }
  .inst { display: flex; flex-direction: column; gap: 8px; padding: 10px; background: var(--bg-2); border-radius: 8px; }
  .envf { display: flex; flex-direction: column; gap: 3px; }
  .cats { gap: 6px; margin: 6px 0; }
  button.chip { cursor: pointer; border: 1px solid var(--line, #333); background: transparent; font-size: 12px; padding: 2px 10px; border-radius: 999px; }
  button.chip.on { border-color: var(--teal); color: var(--teal); }
  .info { line-height: 1.6; }
</style>
