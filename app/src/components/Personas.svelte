<script>
  // Characters that play the roles of an AI team (ISSUE-172): who they are and how they talk, apart from the job.
  // Search mine and the starter characters, make one from trait chips and four sliders, give it a face, share it as
  // a Character Card (the open format AI-character sites use) or import one from a friend.
  import { onMount } from "svelte";
  import Icon from "./Icon.svelte";
  import PersonaAvatar from "./PersonaAvatar.svelte";
  import { api } from "../lib/api.js";
  import Bilingual from "./Bilingual.svelte";
  import { toast } from "../lib/store.svelte.js";
  import { ask } from "../lib/dialog.svelte.js";

  let { onchange = null } = $props();
  let mine = $state([]);
  let presets = $state([]);
  let traits = $state([]);
  let q = $state("");
  let editing = $state(null);
  let sharing = $state(null); // { slug, text, warnings }
  let importing = $state(null); // { text, accept }

  const LEVELS = [
    ["humor", "ความตลก", ["ไม่เล่นมุก", "นานๆ ที", "บ่อย", "ตลอด"]],
    ["direct", "ความตรง", ["อ้อมมาก", "ค่อนข้างอ้อม", "ตรง", "ตรงมาก"]],
    ["formal", "ความทางการ", ["กันเองมาก", "กันเอง", "สุภาพ", "ทางการ"]],
    ["words", "ความยาวคำตอบ", ["สั้นที่สุด", "สั้น", "อธิบายบ้าง", "ละเอียด"]],
  ];
  const HAIR = [["short", "สั้น"], ["fluffy", "หัวฟู"], ["long", "ยาว"], ["bun", "มวย"], ["spiky", "ตั้ง"], ["curly", "หยิก"], ["bald", "ล้าน"], ["cap", "หมวก"]];
  const GLASSES = [["none", "ไม่ใส่"], ["round", "กลม"], ["square", "เหลี่ยม"], ["sun", "กันแดด"]];
  const EXTRA = [["none", "ไม่มี"], ["beard", "หนวดเครา"], ["headphones", "หูฟัง"], ["earrings", "ต่างหู"]];
  const FACE = [["smile", "ยิ้ม"], ["grin", "ยิ้มกว้าง"], ["calm", "นิ่ง"], ["wink", "ขยิบตา"]];
  const COLORS = ["#2b2118", "#6b3f2a", "#c9a227", "#8a8a8a", "#111111", "#c94f7c", "#4f7cff", "#5dcaa5"];
  const SHIRTS = ["#4f7cff", "#5dcaa5", "#f2b04a", "#e2574c", "#a49bf0", "#ec8fb0", "#4b5563", "#1d64aa"];

  async function load() {
    try {
      const r = await api.personas();
      mine = r.personas;
      presets = r.presets;
      traits = r.traits;
    } catch (e) { toast(String(e), "err"); }
  }
  onMount(load);

  const label = (k) => traits.find((t) => t.key === k)?.label ?? k;
  const hit = (p) => {
    const t = q.trim().toLowerCase();
    if (!t) return true;
    return [p.name, p.bio, p.catchphrase, ...(p.traits ?? []).map(label)].join(" ").toLowerCase().includes(t);
  };
  const shownMine = $derived(mine.filter(hit));
  const shownPresets = $derived(presets.filter((p) => !p.installed && hit(p)));

  const blank = () => ({ slug: "", name: "", traits: [], levels: { humor: 1, direct: 2, formal: 1, words: 1 }, catchphrase: "", donts: "หยาบคาย", bio: "",
    look: { skin: 1, hair: "short", hair_color: "#2b2118", glasses: "none", shirt: "#4f7cff", extra: "none", face: "smile" } });
  function edit(p) {
    const b = blank();
    editing = { ...b, ...structuredClone($state.snapshot(p)), levels: { ...b.levels, ...(p.levels ?? {}) }, look: { ...b.look, ...(p.look ?? {}) } };
  }
  function toggleTrait(k) {
    editing.traits = editing.traits.includes(k) ? editing.traits.filter((x) => x !== k) : [...editing.traits, k];
  }
  async function save() {
    try {
      await api.personaSave($state.snapshot(editing));
      toast(`บันทึกตัวละคร “${editing.name}” แล้ว`);
      editing = null;
      await load();
      onchange?.();
    } catch (e) { toast(String(e), "err"); }
  }
  async function install(p) {
    try { await api.personaPreset(p.slug); toast(`เพิ่ม “${p.name}” แล้ว`); await load(); onchange?.(); } catch (e) { toast(String(e), "err"); }
  }
  async function remove(p) {
    if (!(await ask(`ลบตัวละคร “${p.name}”? ทีมที่ใช้ตัวนี้จะกลับไปใช้บทบาทเฉยๆ`, { ok: "ลบ", danger: true }))) return;
    try { await api.personaDelete(p.slug); await load(); onchange?.(); } catch (e) { toast(String(e), "err"); }
  }
  async function share(p) {
    try { sharing = { slug: p.slug, ...(await api.personaExport(p.slug, false)) }; } catch (e) { toast(String(e), "err"); }
  }
  async function shareSave() {
    try { const r = await api.personaExport(sharing.slug, true); toast(`บันทึกไฟล์แล้ว: ${r.path}`); } catch (e) { toast(String(e), "err"); }
  }
  async function copy(t) {
    try { await navigator.clipboard.writeText(t); toast("คัดลอกแล้ว"); } catch { toast("คัดลอกไม่ได้", "err"); }
  }
  async function doImport() {
    try {
      const p = await api.personaImport(importing.text, !!importing.accept);
      toast(`นำเข้า “${p.name}” แล้ว`);
      importing = null;
      await load();
      onchange?.();
    } catch (e) {
      const m = String(e);
      if (m.includes("NEEDS_ACCEPT")) { importing.needs = true; toast("มีจุดที่ควรอ่านก่อน — ติ๊กยืนยันแล้วกดนำเข้าอีกครั้ง", "err"); } else toast(m, "err");
    }
  }
  async function pickFile(e) {
    const f = e.currentTarget.files?.[0];
    if (f) importing = { text: await f.text(), accept: false };
  }
</script>

<p class="muted small lead">ตัวละครคือ “ใคร” ที่สวมบทบาทในทีม — นิสัย น้ำเสียง หน้าตา แยกจาก “หน้าที่” (บทบาท) ตัวเดียวเล่นได้หลายทีม ตั้งที่ทีมว่าบทบาทไหนใครเล่น · เปลี่ยนแค่น้ำเสียงและอารมณ์ ไม่เปลี่ยนคุณภาพงานหรือกฎความปลอดภัย</p>

<div class="row wrap bar">
  <label class="search"><Icon name="search" /><input bind:value={q} placeholder="ค้นหาตัวละคร — ชื่อ นิสัย เช่น ตลก, nerd, ใจเย็น" /></label>
  <button class="primary" onclick={() => (editing = blank())}><Icon name="plus" /> สร้างตัวละคร</button>
  <button onclick={() => (importing = { text: "", accept: false })}><Icon name="download" /> นำเข้า (Character Card)</button>
</div>

{#if editing}
  <div class="card edit">
    <div class="ehead">
      <PersonaAvatar look={editing.look} size={88} />
      <div class="grow col">
        <label>ชื่อ <input bind:value={editing.name} placeholder="เช่น บอม" maxlength="40" /></label>
        <label>ประโยคประจำตัว <input bind:value={editing.catchphrase} placeholder="เช่น เอาจริงดิ? งั้นลุยเลย!" maxlength="120" /></label>
      </div>
    </div>
    <div class="small">นิสัย (เลือกได้หลายอย่าง)</div>
    <div class="row wrap chips">{#each traits as t (t.key)}<button type="button" class="chip" class:on={editing.traits.includes(t.key)} onclick={() => toggleTrait(t.key)}>{t.label}</button>{/each}</div>
    <div class="levels">
      {#each LEVELS as [k, name, words] (k)}
        <label class="lv"><span>{name}</span><input type="range" min="0" max="3" step="1" bind:value={editing.levels[k]} /><b class="small">{words[editing.levels[k] ?? 1]}</b></label>
      {/each}
    </div>
    <label>สิ่งที่ไม่ทำเด็ดขาด <input bind:value={editing.donts} placeholder="เช่น หยาบคาย, ด่าคน, พูดเกินจริง" maxlength="300" /></label>
    <label>เกี่ยวกับตัวละคร (ไม่บังคับ) <textarea rows="3" bind:value={editing.bio} maxlength="1500" placeholder="เช่น หนุ่มสาย nerd ใส่แว่น หัวฟู ชอบของเล่นไอทีใหม่ๆ พูดตรง เสียงดัง แต่ใจดี"></textarea></label>
    <div class="small">หน้าตา</div>
    <div class="looks">
      <label>ผิว <span class="sw">{#each [0, 1, 2, 3, 4] as s (s)}<button type="button" class="dot" class:on={editing.look.skin === s} style="background:{['#f6d7b8', '#e8b98f', '#c98e5e', '#9a6440', '#6b4226'][s]}" onclick={() => (editing.look.skin = s)} aria-label="ผิว {s}"></button>{/each}</span></label>
      <label>ทรงผม <select bind:value={editing.look.hair}>{#each HAIR as [v, l] (v)}<option value={v}>{l}</option>{/each}</select></label>
      <label>สีผม <span class="sw">{#each COLORS as c (c)}<button type="button" class="dot" class:on={editing.look.hair_color === c} style="background:{c}" onclick={() => (editing.look.hair_color = c)} aria-label={c}></button>{/each}</span></label>
      <label>แว่น <select bind:value={editing.look.glasses}>{#each GLASSES as [v, l] (v)}<option value={v}>{l}</option>{/each}</select></label>
      <label>สีเสื้อ <span class="sw">{#each SHIRTS as c (c)}<button type="button" class="dot" class:on={editing.look.shirt === c} style="background:{c}" onclick={() => (editing.look.shirt = c)} aria-label={c}></button>{/each}</span></label>
      <label>เพิ่มเติม <select bind:value={editing.look.extra}>{#each EXTRA as [v, l] (v)}<option value={v}>{l}</option>{/each}</select></label>
      <label>สีหน้า <select bind:value={editing.look.face}>{#each FACE as [v, l] (v)}<option value={v}>{l}</option>{/each}</select></label>
    </div>
    <div class="row"><button class="primary" onclick={save} disabled={!editing.name.trim()}>บันทึก</button><button onclick={() => (editing = null)}>ยกเลิก</button></div>
  </div>
{/if}

{#if importing}
  <div class="card edit">
    <div class="small">วางเนื้อหาการ์ดตัวละคร (.json แบบ Character Card V1/V2) หรือเลือกไฟล์ — ตรวจด้วยตัวสแกนก่อนเสมอ ข้อความในการ์ดเป็นแค่สไตล์ ไม่มีสิทธิ์เหนือกฎของบอท</div>
    <input type="file" accept=".json,application/json" onchange={pickFile} />
    <textarea rows="6" bind:value={importing.text} placeholder={'{ "spec": "chara_card_v2", "data": { "name": "…", "personality": "…" } }'}></textarea>
    {#if importing.needs}<label class="small"><input type="checkbox" bind:checked={importing.accept} /> อ่านแล้ว ยืนยันนำเข้า</label>{/if}
    <div class="row"><button class="primary" onclick={doImport} disabled={!importing.text.trim()}>นำเข้า</button><button onclick={() => (importing = null)}>ยกเลิก</button></div>
  </div>
{/if}

{#if sharing}
  <div class="card edit">
    <div class="small">การ์ดตัวละคร (Character Card V2) — ส่งให้เพื่อนนำเข้าได้ทั้งในแอปนี้และแอป AI อื่นที่รองรับการ์ดแบบนี้</div>
    {#each sharing.warnings ?? [] as w}<div class="small warn"><Icon name="warning" /> {w}</div>{/each}
    <textarea rows="8" readonly value={sharing.text}></textarea>
    <div class="row"><button class="primary small" onclick={() => copy(sharing.text)}><Icon name="copy" /> คัดลอก</button><button class="small" onclick={shareSave}><Icon name="download" /> บันทึกเป็นไฟล์</button><button class="small" onclick={() => (sharing = null)}>ปิด</button></div>
  </div>
{/if}

<div class="grid">
  {#each shownMine as p (p.slug)}
    <div class="card pc">
      <PersonaAvatar look={p.look} size={56} />
      <div class="grow col">
        <b>{p.name}</b>
        {#if p.catchphrase}<span class="small muted">“{p.catchphrase}”</span>{/if}
        <div class="row wrap tags">{#each p.traits as t (t)}<span class="chip small">{label(t)}</span>{/each}</div>
      </div>
      <div class="acts">
        <button class="ghost small" onclick={() => edit(p)} title="แก้"><Icon name="pencil" /></button>
        <button class="ghost small" onclick={() => share(p)} title="แชร์เป็นการ์ดตัวละคร"><Icon name="copy" /></button>
        <button class="ghost small" onclick={() => remove(p)} title="ลบ"><Icon name="trash" /></button>
      </div>
    </div>
  {/each}
</div>
{#if !shownMine.length && !editing}<div class="faint small pad">{q ? "ไม่พบตัวละครที่ตรงคำค้น" : "ยังไม่มีตัวละคร — สร้างเอง หรือเพิ่มจากตัวอย่างด้านล่าง"}</div>{/if}

{#if shownPresets.length}
  <h3 class="small">ตัวอย่างตัวละคร — กดเพิ่มแล้วแก้ต่อได้</h3>
  <div class="grid">
    {#each shownPresets as p (p.slug)}
      <div class="card pc preset">
        <PersonaAvatar look={p.look} size={56} />
        <div class="grow col">
          <b>{p.name}</b>
          <span class="small muted"><Bilingual text={p.bio} /></span>
          <div class="row wrap tags">{#each p.traits as t (t)}<span class="chip small">{label(t)}</span>{/each}</div>
        </div>
        <button class="small primary" onclick={() => install(p)}><Icon name="plus" /> เพิ่ม</button>
      </div>
    {/each}
  </div>
{/if}

<style>
  .lead { margin: 0 0 10px; }
  .bar { gap: 8px; margin-bottom: 12px; align-items: center; }
  .search { flex: 1; min-width: 240px; display: flex; align-items: center; gap: 6px; background: var(--bg-2); border: 1px solid var(--line); border-radius: 8px; padding: 0 10px; }
  .search input { border: none; background: transparent; flex: 1; padding: 8px 0; }
  .edit { display: flex; flex-direction: column; gap: 10px; margin-bottom: 12px; }
  .edit label { display: flex; flex-direction: column; gap: 4px; font-size: 13px; }
  .ehead { display: flex; gap: 16px; align-items: center; }
  .col { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  .chips { gap: 6px; }
  .chip.on { border-color: var(--teal); color: var(--teal); background: var(--teal-bg); }
  .levels { display: grid; grid-template-columns: repeat(auto-fill, minmax(230px, 1fr)); gap: 8px 18px; }
  .lv { flex-direction: row !important; align-items: center; gap: 8px !important; }
  .lv span { width: 96px; flex: none; }
  .lv input { flex: 1; }
  .lv b { width: 86px; flex: none; color: var(--teal); }
  .looks { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 8px 18px; }
  .sw { display: flex; gap: 6px; flex-wrap: wrap; }
  .dot { width: 22px; height: 22px; border-radius: 50%; padding: 0; border: 2px solid transparent; }
  .dot.on { border-color: var(--text); }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(320px, 1fr)); gap: 10px; margin-bottom: 14px; }
  .pc { display: flex; gap: 12px; align-items: flex-start; }
  .pc .col b { font-size: 15px; }
  .tags { gap: 4px; }
  .acts { display: flex; flex-direction: column; gap: 2px; }
  .preset { border-style: dashed; }
  .warn { color: var(--amber, #f2b04a); }
  .pad { padding: 8px 2px 14px; }
</style>
