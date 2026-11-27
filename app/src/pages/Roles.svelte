<script>
  // Team roles (roles.rs): named AI team members — a job, a persona and the AI that plays them. Roles marked
  // "in the team" join an idea's team mode. Found in public collections (same safety as the skill shop), written
  // here, or imported from a friend's file; exported as a portable .md (a Claude Code subagent).
  import { onMount } from "svelte";
  import Icon from "../components/Icon.svelte";
  import { api } from "../lib/api.js";
  import Bilingual from "../components/Bilingual.svelte";
  import RoleIcon from "../components/RoleIcon.svelte";
  import IconPick from "../components/IconPick.svelte";
  import Personas from "../components/Personas.svelte";
  import PersonaAvatar from "../components/PersonaAvatar.svelte";
  import { toast } from "../lib/store.svelte.js";
  import { ask } from "../lib/dialog.svelte.js";

  let tab = $state("teams");
  let roles = $state([]);
  let teams = $state([]);
  let presets = $state([]);
  let accounts = $state([]);
  let personas = $state([]); // characters that can play a role (ISSUE-172)
  let editing = $state(null); // the role being written / changed
  let exporting = $state(null); // { slug, with_persona, text, warnings }

  async function load() {
    try {
      roles = (await api.roles()).roles;
      const t = await api.teams();
      teams = t.teams;
      presets = t.presets;
      accounts = ((await api.accounts()).accounts ?? []).filter((a) => a.enabled);
      personas = (await api.personas().catch(() => ({ personas: [] }))).personas ?? [];
    } catch (e) { toast(String(e), "err"); }
  }
  onMount(load);

  // a tier travels with the role (shared files work on any machine); the account and exact model are this machine's
  const TIERS = [["auto", "อัตโนมัติ (ตามบัญชี)"], ["fast", "เร็ว / ประหยัด"], ["balanced", "สมดุล"], ["strong", "เก่งสุด"]];
  const PAR = [[1, "ทีละส่วน · ประหยัด"], [2, "สมดุล"], [3, "เร็วสุด · ใช้โควต้ามาก"]];
  const tierName = (t) => TIERS.find((x) => x[0] === t)?.[1] ?? t;
  const who = (r) => `${r.account ? accounts.find((a) => a.id === r.account)?.label || r.account : "AI ของหัวหน้าทีม"} · ${r.model_override || tierName(r.tier)}`;
  const teamsOf = (slug) => teams.filter((t) => t.members.includes(slug));

  function blank() {
    return { slug: "", name: "", title: "", emoji: "", description: "", instructions: "", persona: "", tier: "auto", account: "", model_override: "", tools: "" };
  }

  // ---- named teams
  let editTeam = $state(null);
  let teamExp = $state(null);
  const blankTeam = () => ({ slug: "", name: "", emoji: "i:users", description: "", use_for: "", members: [], helpers: 3, lead: "", cast: {} });
  async function saveTeam() {
    try {
      const t = await api.teamSave($state.snapshot(editTeam));
      toast(`บันทึกทีม “${t.name}” แล้ว`);
      editTeam = null;
      await load();
    } catch (e) { toast(String(e), "err"); }
  }
  async function removeTeam(t) {
    if (!(await ask(`ลบทีม “${t.name}”? (สมาชิกยังอยู่)`, { ok: "ลบ", danger: true }))) return;
    try { await api.teamDelete(t.slug); await load(); } catch (e) { toast(String(e), "err"); }
  }
  async function preset(k) {
    try {
      const t = await api.teamPreset(k);
      toast(`ติดตั้ง “${t.name}” พร้อมสมาชิก ${t.members.length} คนแล้ว — เลือกทีมนี้ที่ “ใครทำ” ตอนจดไอเดีย หรือในหน้าไอเดีย · แก้สมาชิกได้`, "ok", 6000);
      await load();
    } catch (e) { toast(String(e), "err"); }
  }
  function toggleMember(slug) {
    editTeam.members = editTeam.members.includes(slug) ? editTeam.members.filter((m) => m !== slug) : [...editTeam.members, slug];
  }
  async function showTeamExport(t, withPersona = true) {
    try { teamExp = { slug: t.slug, with_persona: withPersona, ...(await api.teamExport(t.slug, withPersona, false)) }; } catch (e) { toast(String(e), "err"); }
  }
  async function saveTeamExport() {
    try { const x = await api.teamExport(teamExp.slug, teamExp.with_persona, true); toast(`บันทึกไฟล์แล้ว: ${x.path}`); } catch (e) { toast(String(e), "err"); }
  }
  async function copyTeamExport() {
    try { await navigator.clipboard.writeText(teamExp.text); toast("คัดลอกแล้ว — วางส่งให้เพื่อนได้เลย"); } catch { toast("คัดลอกไม่ได้", "err"); }
  }
  async function save() {
    try {
      const r = await api.roleSave($state.snapshot(editing));
      toast(`บันทึก “${r.name}” แล้ว`);
      editing = null;
      await load();
    } catch (e) { toast(String(e), "err"); }
  }
  async function setField(r, patch) {
    try { await api.roleSave({ ...r, ...patch }); await load(); } catch (e) { toast(String(e), "err"); }
  }
  async function remove(r) {
    if (!(await ask(`ลบ role “${r.name}”?`, { ok: "ลบ", danger: true }))) return;
    try { await api.roleDelete(r.slug); await load(); } catch (e) { toast(String(e), "err"); }
  }
  async function showExport(r, withPersona = true) {
    try { exporting = { slug: r.slug, with_persona: withPersona, ...(await api.roleExport(r.slug, withPersona, false)) }; } catch (e) { toast(String(e), "err"); }
  }
  async function saveExport() {
    try {
      const x = await api.roleExport(exporting.slug, exporting.with_persona, true);
      toast(`บันทึกไฟล์แล้ว: ${x.path}`);
    } catch (e) { toast(String(e), "err"); }
  }
  async function copyExport() {
    try { await navigator.clipboard.writeText(exporting.text); toast("คัดลอกแล้ว — วางส่งให้เพื่อนได้เลย"); } catch { toast("คัดลอกไม่ได้", "err"); }
  }

  // ---- shop: public collections of roles on GitHub
  const SORTS = [["popular", "ยอดนิยม", "star"], ["trending", "กำลังมาแรง", "flame"], ["new", "มาใหม่ (90 วัน)", "sparkle"]];
  let sort = $state("popular");
  let q = $state("");
  let searching = $state(false);
  let repos = $state(null);
  let note = $state("");
  let repo = $state(null);
  let filter = $state("");
  let cat = $state("");
  let preview = $state(null);
  let previewing = $state("");
  let accept = $state(false);
  let installing = $state(false);

  async function search() {
    searching = true;
    repo = null;
    preview = null;
    try {
      const r = await api.rolesShopSearch(q.trim(), sort);
      repos = r.repos;
      note = r.note ?? "";
    } catch (e) { repos = []; toast(String(e), "err"); }
    searching = false;
  }
  async function browse(name) {
    preview = null;
    filter = "";
    cat = "";
    try { repo = await api.rolesShopBrowse(name); } catch (e) { toast(String(e), "err"); }
  }
  async function open(path) {
    previewing = path;
    accept = false;
    try { preview = { path, ...(await api.rolesShopPreview(repo.repo, repo.commit, path)) }; } catch (e) { toast(String(e), "err"); }
    previewing = "";
  }
  async function install() {
    installing = true;
    try {
      const r = await api.rolesShopInstall(repo.repo, repo.commit, preview.path, preview.sha256, accept);
      toast(`ติดตั้ง “${r.name}” แล้ว — ตั้งชื่อ เลือกโมเดล และเพิ่มบุคลิกได้ที่แท็บ “ทีมของฉัน”`);
      preview = null;
      await load();
    } catch (e) { toast(String(e).replace(/^NEEDS_ACCEPT: /, ""), "err"); }
    installing = false;
  }
  const cats = $derived.by(() => {
    const m = new Map();
    for (const r of repo?.roles ?? []) m.set(r.category || "อื่นๆ", (m.get(r.category || "อื่นๆ") ?? 0) + 1);
    return [...m.entries()].sort((a, b) => b[1] - a[1]).slice(0, 24);
  });
  const shownRoles = $derived((repo?.roles ?? []).filter((r) => (!cat || (r.category || "อื่นๆ") === cat) && (!filter.trim() || (r.path + " " + r.name).toLowerCase().includes(filter.trim().toLowerCase()))));

  // ---- import a friend's role file
  let text = $state("");
  let checked = $state(null);
  let acceptLocal = $state(false);
  // a whole team file carries its members after "<!-- role: …" marks
  const isTeam = (t) => t.includes("<!-" + "- role: ");
  async function check() {
    try {
      checked = isTeam(text) ? { kind: "team", ...(await api.teamCheckText(text)) } : { kind: "role", ...(await api.roleCheckText(text)) };
      acceptLocal = false;
    } catch (e) { checked = null; toast(String(e), "err"); }
  }
  async function importText() {
    try {
      const kind = checked.kind;
      const r = kind === "team" ? await api.teamImportText(text, acceptLocal) : await api.roleImportText(text, acceptLocal);
      toast(kind === "team" ? `นำเข้าทีม “${r.name}” (${r.members.length} คน) แล้ว — เลือก AI ให้สมาชิกได้ที่แท็บ “สมาชิก”` : `นำเข้า “${r.name}” แล้ว`);
      text = "";
      checked = null;
      tab = kind === "team" ? "teams" : "mine";
      await load();
    } catch (e) { toast(String(e).replace(/^NEEDS_ACCEPT: /, ""), "err"); }
  }
  async function pickFile(e) {
    const f = e.currentTarget.files?.[0];
    if (!f) return;
    if (f.size > 60000) return toast("ไฟล์ใหญ่เกินไป", "err");
    text = await f.text();
    check();
  }
  const VERDICT = { clean: ["ไม่พบสิ่งน่าสงสัย", "teal"], warn: ["มีคำเตือน — อ่านก่อนติดตั้ง", "amber"], danger: ["อันตราย — ติดตั้งไม่ได้", "red"] };
</script>

<div class="page-roles">
  <h1>ทีม AI</h1>
  <p class="muted">จัด<b>ทีม</b>ตามลักษณะงาน (ทีมเว็บแอป, ทีมการตลาด…) จาก<b>สมาชิก</b>ที่มีหน้าที่ บุคลิก และ AI/โมเดลของตัวเอง เช่น “บอม · front-end developer · พูดตรง ชอบลองของใหม่”
    ตอนจดไอเดียเลือกได้ว่าให้ AI ตัวเดียวหรือทีมไหนทำ แล้วหน้าไอเดียจะบอกว่าใครทำส่วนไหน</p>

  <div class="tabs">
    <button class="f" class:on={tab === "teams"} onclick={() => (tab = "teams")}><Icon name="layers" /> ทีม ({teams.length})</button>
    <button class="f" class:on={tab === "mine"} onclick={() => (tab = "mine")}><Icon name="user" /> บทบาท ({roles.length})</button>
    <button class="f" class:on={tab === "personas"} onclick={() => (tab = "personas")}><Icon name="sparkle" /> ตัวละคร ({personas.length})</button>
    <button class="f" class:on={tab === "shop"} onclick={() => { tab = "shop"; if (!repos) search(); }}><Icon name="search" /> ค้นหาต้นแบบ (ฟรี)</button>
    <button class="f" class:on={tab === "import"} onclick={() => (tab = "import")}><Icon name="download" /> นำเข้าจากเพื่อน</button>
  </div>

  {#if tab === "teams"}
    <div class="row wrap">
      <button class="primary" onclick={() => (editTeam = blankTeam())}><Icon name="plus" /> สร้างทีม</button>
    </div>
    <!-- ready-made teams come with their members (starter_roles.rs): one press = a working team to try -->
    <h3 class="presethead">ทีมสำเร็จรูป — ติดตั้งพร้อมสมาชิก ลองใช้ได้ทันที</h3>
    <div class="presets">
      {#each presets as p (p.key)}
        <div class="card preset">
          <div class="row"><b class="grow"><RoleIcon emoji={p.emoji} words={p.name} /> {p.name}</b>{#if p.installed}<span class="chip teal small">มีแล้ว</span>{/if}</div>
          <div class="muted small">{p.description}</div>
          <div class="row wrap members">{#each p.starter ?? [] as m (m.slug)}<span class="chip small" title={m.name}><RoleIcon emoji={m.emoji} words={(m.title || "") + " " + m.name} /> {m.title || m.name}</span>{/each}</div>
          <button class="small" class:primary={!p.installed} onclick={() => preset(p.key)}><Icon name="download" /> {p.installed ? "ติดตั้งอีกชุด" : "ติดตั้งทีมนี้"}</button>
        </div>
      {/each}
    </div>
    <p class="faint small">หัวหน้าทีม = AI ของไอเดียที่เลือกทีมนี้ (ตัวไหนก็ได้ที่รับเครื่องมือ MCP: Claude, Codex, Gemini, OpenCode, Mistral) · Claude ของ Anthropic เรียกสมาชิกได้เร็วกว่าในรอบเดียวกัน ค่ายอื่นส่งงานให้สมาชิกทีละส่วนผ่าน “ask_other_ai” (พร้อมกันไม่เกิน 3 งาน)</p>

    {#if editTeam}
      <div class="card edit">
        <div class="grid">
          <label>ชื่อทีม <input bind:value={editTeam.name} placeholder="เช่น ทีมเว็บแอป" maxlength="60" /></label>
          <label>ไอคอน <IconPick bind:value={editTeam.emoji} words={editTeam.name + " " + editTeam.description} /></label>
        </div>
        <div class="par">
          <span class="small">ทำพร้อมกันได้สูงสุด</span>
          <div class="seg">{#each PAR as [n, l] (n)}<button type="button" class:on={editTeam.helpers === n} onclick={() => (editTeam.helpers = n)}><b>{n}</b> {l}</button>{/each}</div>
          <span class="faint small">หัวหน้าทีมแบ่งงานใหญ่ให้สมาชิกทำพร้อมกันได้กี่ส่วน — แต่ละส่วนคือ AI อีกตัวที่รันจริงบนเครื่องนี้ จึงใช้โควต้าตามจำนวนที่ทำพร้อมกัน · จำกัดที่ 3 เพราะมากกว่านี้โควต้าหมดเร็ว เครื่องหน่วง และงานชนกันง่าย · งานเล็กหัวหน้าทำเอง ไม่แบ่ง</span>
        </div>
        <label>ทีมนี้ทำงานแบบไหน <input bind:value={editTeam.description} placeholder="เช่น สร้างเว็บตั้งแต่หน้าตาถึงหลังบ้าน แล้วทดสอบ" maxlength="400" /></label>
        <label>คำที่บอกว่าไอเดียเหมาะกับทีมนี้ (คั่นด้วย ,) — ใช้แนะนำทีมตอนจดไอเดีย
          <input bind:value={editTeam.use_for} placeholder="เช่น เว็บ, แอป, dashboard, api" maxlength="200" /></label>
        <label>หัวหน้าทีม
          <select bind:value={editTeam.lead}>
            <option value="">AI ของไอเดีย (ตามที่เลือกตอนจด)</option>
            {#each accounts as a (a.id)}<option value={a.id}>{a.label || a.id} · {a.vendor_label || a.kind_label}</option>{/each}
          </select></label>
        <div class="small">สมาชิก ({editTeam.members.length}) <span class="faint">— เลือกได้ว่าแต่ละคนใช้ AI ตัวไหน (ตั้งที่ตัวสมาชิก ใช้กับทุกทีมที่มีคนนี้)</span></div>
        {#if editTeam.members.length}
          <div class="mlist">
            {#each editTeam.members as slug (slug)}
              {@const r = roles.find((x) => x.slug === slug)}
              <div class="mrow">
                {#if r}
                  {@const pp = personas.find((p) => p.slug === editTeam.cast?.[slug])}
                  <span class="mi">{#if pp}<PersonaAvatar look={pp.look} size={30} />{:else}<RoleIcon emoji={r.emoji} words={r.title + " " + r.name} size="1.3em" />{/if}</span>
                  <div class="mn"><b>{r.name}</b>
                    <select class="cast" value={editTeam.cast?.[slug] ?? ""} onchange={(e) => (editTeam.cast = { ...(editTeam.cast ?? {}), [slug]: e.currentTarget.value })} title="ตัวละครที่สวมบทบาทนี้ในทีมนี้ (นิสัย น้ำเสียง)">
                      <option value="">ใครเล่น: ไม่มีตัวละคร</option>
                      {#each personas as p (p.slug)}<option value={p.slug}>ใครเล่น: {p.name}</option>{/each}
                    </select></div>
                  <select value={r.account} onchange={(e) => setField(r, { account: e.currentTarget.value })} title="AI ที่เล่นบทนี้">
                    <option value="">อัตโนมัติ — AI เดียวกับหัวหน้าทีม</option>
                    {#each accounts as a (a.id)}<option value={a.id}>{a.label || a.id} · {a.vendor_label || a.kind_label}</option>{/each}
                  </select>
                  <select value={r.tier || "auto"} onchange={(e) => setField(r, { tier: e.currentTarget.value })} title="ระดับโมเดล: แต่ละเครื่องแปลงเป็นโมเดลของบัญชีนั้นเอง">
                    {#each TIERS as [v, l] (v)}<option value={v}>{l}</option>{/each}
                  </select>
                {:else}
                  <span class="mi"><RoleIcon words={slug} size="1.3em" /></span><div class="mn"><b>{slug}</b><span class="faint small">ไม่พบสมาชิกนี้ในเครื่อง</span></div>
                {/if}
                <button type="button" class="ghost small" onclick={() => toggleMember(slug)} title="เอาออกจากทีม"><Icon name="close" /></button>
              </div>
            {/each}
          </div>
        {/if}
        {#if roles.some((r) => !editTeam.members.includes(r.slug))}
          <div class="row wrap add">
            <span class="faint small">เพิ่มสมาชิก:</span>
            {#each roles.filter((r) => !editTeam.members.includes(r.slug)) as r (r.slug)}<button type="button" class="chip" onclick={() => toggleMember(r.slug)} title={r.description}><RoleIcon emoji={r.emoji} words={r.title + " " + r.name} /> {r.name}{r.title ? ` · ${r.title}` : ""} <Icon name="plus" /></button>{/each}
          </div>
        {/if}
        {#if !roles.length}<span class="faint small">ยังไม่มีสมาชิก — สร้างที่แท็บ “สมาชิก” หรือติดตั้งจาก “ค้นหาต้นแบบ”</span>{/if}
        <div class="row"><button class="primary" onclick={saveTeam} disabled={!editTeam.name.trim()}>บันทึกทีม</button><button onclick={() => (editTeam = null)}>ยกเลิก</button></div>
      </div>
    {/if}

    {#if !teams.length && !editTeam}<div class="card faint">ยังไม่มีทีม — กดทีมสำเร็จรูปด้านบน หรือสร้างเอง แล้วเลือกทีมได้ตอนจดไอเดีย</div>{/if}
    {#each teams as t (t.slug)}
      <div class="card role">
        <div class="row wrap">
          <span class="emo"><RoleIcon emoji={t.emoji || "i:users"} words={t.name} size="1.3em" /></span><b>{t.name}</b>
          <span class="chip" title="หัวหน้าทีมแบ่งงานให้สมาชิกทำพร้อมกันได้สูงสุดกี่ส่วน">ทำพร้อมกันได้ {t.helpers}</span>
          <span class="chip teal"><Icon name="bot" /> หัวหน้า: {t.lead ? accounts.find((a) => a.id === t.lead)?.label || t.lead : "AI ของไอเดีย"}</span>
          <span class="grow"></span>
          <button class="small" onclick={() => (editTeam = { ...t, members: [...t.members], cast: { ...(t.cast ?? {}) } })}><Icon name="pencil" /> แก้</button>
          <button class="small" onclick={() => showTeamExport(t)}><Icon name="copy" /> แชร์ทั้งทีม</button>
          <button class="small danger" onclick={() => removeTeam(t)}>ลบ</button>
        </div>
        {#if t.description}<div class="muted small">{t.description}</div>{/if}
        <div class="row wrap small">
          {#each t.member_roles as m (m.slug)}<span class="chip" title={m.title}><RoleIcon emoji={m.emoji} words={m.title + " " + m.name} /> {m.name} · {m.account ? accounts.find((a) => a.id === m.account)?.label || m.account : "AI หัวหน้า"} · {tierName(m.tier)}</span>{/each}
          {#if !t.member_roles.length}<span class="faint">ไม่มีสมาชิก — หัวหน้าใช้ผู้ช่วยทั่วไป</span>{/if}
        </div>
        {#if t.use_for}<div class="faint small">เหมาะกับ: {t.use_for}</div>{/if}
        {#if teamExp?.slug === t.slug}
          <div class="exp">
            <label class="row small"><input type="checkbox" checked={teamExp.with_persona} onchange={(e) => showTeamExport(t, e.currentTarget.checked)} /> ส่งบุคลิก (Persona) ของสมาชิกไปด้วย</label>
            {#if teamExp.warnings.length}<div class="warnbox small"><Icon name="warning" /> {teamExp.warnings.join(" · ")} — ตรวจก่อนส่ง</div>{/if}
            <div class="faint small">ไฟล์เดียวทั้งทีม (.team.md) · ส่งแค่ “ระดับโมเดล” (เร็ว/สมดุล/เก่งสุด) ไม่ส่งบัญชีหรือชื่อโมเดล — เครื่องผู้รับเลือก AI ของเขาเอง</div>
            <pre class="txt">{teamExp.text}</pre>
            <div class="row"><button class="primary small" onclick={copyTeamExport}><Icon name="copy" /> คัดลอก</button><button class="small" onclick={saveTeamExport}><Icon name="download" /> บันทึกเป็นไฟล์</button><button class="small" onclick={() => (teamExp = null)}>ปิด</button></div>
          </div>
        {/if}
      </div>
    {/each}
  {/if}

  {#if tab === "personas"}
    <Personas onchange={load} />
  {/if}

  {#if tab === "mine"}
    <div class="row wrap">
      <button class="primary" onclick={() => (editing = blank())}><Icon name="plus" /> สร้างสมาชิกใหม่</button>
      <span class="faint small">สมาชิกหนึ่งคนอยู่ได้หลายทีม · จัดทีมที่แท็บ “ทีม”</span>
    </div>

    {#if editing}
      <div class="card edit">
        <div class="grid">
          <label>ชื่อ <input bind:value={editing.name} placeholder="เช่น บอม" maxlength="60" /></label>
          <label>หน้าที่ <input bind:value={editing.title} placeholder="เช่น Front-end developer" maxlength="80" /></label>
          <label>ไอคอน <IconPick bind:value={editing.emoji} words={editing.title + " " + editing.name} /></label>
        </div>
        <label>ทำอะไรได้ (หัวหน้าทีมอ่านบรรทัดนี้เพื่อเลือกคนทำงาน)
          <input bind:value={editing.description} placeholder="เช่น ทำหน้าเว็บ React/Svelte ให้สวย ใช้ง่าย รองรับมือถือ" maxlength="400" /></label>
        <label>วิธีทำงาน (คำสั่งถึง AI ในบทนี้)
          <textarea rows="5" bind:value={editing.instructions} placeholder="เช่น You build clean, accessible UIs. Use the project's existing components first…"></textarea></label>
        {#if editing.persona}<div class="faint small">บุคลิกเดิมของบทบาทนี้ถูกย้ายไปเป็นตัวละครในแท็บ “ตัวละคร” แล้ว — ตั้งว่าใครเล่นบทบาทไหนที่หน้าแก้ทีม</div>{/if}
        <label style="display:none">บุคลิก (Persona) — นิสัย น้ำเสียง วิธีคิด ความชอบ
          <textarea rows="4" bind:value={editing.persona} placeholder="เช่น พูดตรง สั้น เป็นกันเอง · ชอบลองของใหม่แต่เลือกทางที่ดูแลง่าย · อธิบายเป็นภาษาไทยแบบคนทำงานจริง · ไม่ชอบงานที่ซับซ้อนเกินจำเป็น"></textarea></label>
        <div class="faint small" style="display:none">เขียนจากตัวคุณเองก็ได้ — วิธีที่ง่ายคือตอบ 4 ข้อ: ชอบพูดแบบไหน, ตัดสินใจจากอะไร, อะไรที่ไม่ชอบ, งานที่ภูมิใจ · ตอนแชร์เลือกได้ว่าจะส่งบุคลิกไปด้วยไหม</div>
        <div class="grid">
          <label>AI ที่เล่นบทนี้
            <select bind:value={editing.account}>
              <option value="">AI ของหัวหน้าทีม (บัญชีเดียวกับหัวหน้า)</option>
              {#each accounts as a (a.id)}<option value={a.id}>{a.label || a.id} · {a.vendor_label || a.kind_label}</option>{/each}
            </select></label>
          <label>ระดับโมเดล (ติดไปกับไฟล์เวลาแชร์)
            <select bind:value={editing.tier}>{#each TIERS as [v, l] (v)}<option value={v}>{l}</option>{/each}</select></label>
          <label>ชื่อโมเดลเจาะจง (เครื่องนี้เท่านั้น ไม่บังคับ)
            <input bind:value={editing.model_override} placeholder="ว่าง = ตามระดับ" maxlength="80" /></label>
        </div>
        <div class="row">
          <button class="primary" onclick={save} disabled={!editing.name.trim()}>บันทึก</button>
          <button onclick={() => (editing = null)}>ยกเลิก</button>
        </div>
      </div>
    {/if}

    {#if !roles.length && !editing}
      <div class="card faint">ยังไม่มีสมาชิก — สร้างเอง หรือไปที่ “ค้นหาต้นแบบ” เพื่อเลือกจาก role ยอดนิยมหลายร้อยแบบ (front-end, back-end, นักการตลาด, นักออกแบบ…)</div>
    {/if}
    {#each roles as r (r.slug)}
      <div class="card role">
        <div class="row wrap">
          <span class="emo"><RoleIcon emoji={r.emoji} words={r.title + " " + r.name} size="1.3em" /></span>
          <b>{r.name}</b>
          {#if r.title}<span class="chip">{r.title}</span>{/if}
          <span class="chip teal" title="AI ที่เล่นบทนี้"><Icon name="bot" /> {who(r)}</span>
          {#if r.persona}<span class="chip" title={r.persona}><Icon name="user" /> มีบุคลิก</span>{/if}
          {#each teamsOf(r.slug) as t (t.slug)}<span class="chip"><RoleIcon emoji={t.emoji || "i:users"} words={t.name} /> {t.name}</span>{/each}
          <span class="grow"></span>
          <button class="small" onclick={() => (editing = { ...r })}><Icon name="pencil" /> แก้</button>
          <button class="small" onclick={() => showExport(r)}><Icon name="copy" /> แชร์ / Export</button>
          <button class="small danger" onclick={() => remove(r)}>ลบ</button>
        </div>
        <div class="muted small"><Bilingual text={r.description} /></div>
        {#if r.source}<div class="faint small"><a href={r.source} target="_blank" rel="noreferrer">ที่มา</a>{#if r.license} · {r.license}{/if}</div>{/if}
        {#if exporting?.slug === r.slug}
          <div class="exp">
            <label class="row small"><input type="checkbox" checked={exporting.with_persona} onchange={(e) => showExport(r, e.currentTarget.checked)} /> ส่งบุคลิก (Persona) ไปด้วย</label>
            {#if exporting.warnings.length}<div class="warnbox small"><Icon name="warning" /> บุคลิกนี้{exporting.warnings.join(", ")} — ตรวจก่อนส่งให้คนอื่น</div>{/if}
            <div class="faint small">ไฟล์นี้คือ subagent มาตรฐานของ Claude Code (.md) เพื่อนนำเข้าในแอปนี้ หรือวางใน <code>.claude/agents/</code> ก็ใช้ได้ · ส่งแค่ระดับโมเดล ไม่ส่งบัญชีหรือชื่อโมเดลของคุณ</div>
            <pre class="txt">{exporting.text}</pre>
            <div class="row"><button class="primary small" onclick={copyExport}><Icon name="copy" /> คัดลอก</button><button class="small" onclick={saveExport}><Icon name="download" /> บันทึกเป็นไฟล์</button><button class="small" onclick={() => (exporting = null)}>ปิด</button></div>
          </div>
        {/if}
      </div>
    {/each}
  {/if}

  {#if tab === "shop"}
    <div class="card safe small"><Icon name="shieldCheck" /> ดึงเฉพาะข้อความของไฟล์ role (.md) จาก GitHub ผูกกับ commit และ hash เดียวกับที่คุณเปิดดู ผ่านตัวสแกนเดียวกับสกิล ไม่ดาวน์โหลดสคริปต์ และ AI ติดตั้งเองไม่ได้</div>
    <div class="row wrap">{#each SORTS as [k, l, ic] (k)}<button class="chip" class:on={sort === k} onclick={() => { sort = k; search(); }}><Icon name={ic} /> {l}</button>{/each}</div>
    <form class="row" onsubmit={(e) => { e.preventDefault(); search(); }}>
      <input class="grow" bind:value={q} placeholder="ค้นหาคลัง role เช่น frontend, marketing, data, security (เว้นว่าง = คลังยอดนิยม)" />
      <button class="primary" disabled={searching}>{searching ? "กำลังค้นหา…" : "ค้นหา"}</button>
    </form>
    {#if note}<div class="faint small">{note}</div>{/if}
    {#if repo}
      <div class="card">
        <div class="row wrap"><button class="small" onclick={() => { repo = null; preview = null; }}><Icon name="arrowLeft" /> กลับ</button><b class="grow">{repo.repo}</b>{#if repo.stars != null}<span class="faint small">★ {repo.stars.toLocaleString()}</span>{/if}{#if repo.license}<span class="faint small">{repo.license}</span>{/if}<a class="small" href={repo.url} target="_blank" rel="noreferrer">GitHub</a></div>
        <div class="muted small">{repo.description ?? ""} · {repo.total} role</div>
        {#if cats.length > 1}<div class="row wrap cats"><button class="chip" class:on={!cat} onclick={() => (cat = "")}>ทุกหมวด</button>{#each cats as [k, n] (k)}<button class="chip" class:on={cat === k} onclick={() => (cat = cat === k ? "" : k)}><span>{k}</span> <span class="faint">({n})</span></button>{/each}</div>{/if}
        <input bind:value={filter} placeholder="กรองชื่อ เช่น frontend" />
        <div class="list">
          {#each shownRoles.slice(0, 200) as r (r.path)}
            <div class="row item small">
              <span class="grow">{r.name}</span><span class="faint">{r.category}</span>
              {#if r.too_big}<span class="faint">ใหญ่เกิน</span>{:else}<button class="small" disabled={previewing === r.path} onclick={() => open(r.path)}>{previewing === r.path ? "กำลังโหลด…" : "ดู"}</button>{/if}
            </div>
            {#if preview?.path === r.path}
              <div class="pv">
                <div class="row wrap"><b><RoleIcon emoji={preview.role.emoji} words={(preview.role.title || "") + " " + preview.role.name} /> {preview.role.name}</b><span class="chip {VERDICT[preview.verdict][1]}">{VERDICT[preview.verdict][0]}</span><span class="faint small">ระดับโมเดลที่ต้นแบบแนะนำ: {tierName(preview.role.tier)}</span></div>
                <div class="muted small"><Bilingual text={preview.role.description} /></div>
                {#each preview.findings as f, i (i)}<div class="small {f.level === 'danger' ? 'red' : 'amber'}">• {f.what} — {f.why}</div>{/each}
                <Bilingual text={preview.text} md />
                {#if preview.needs_accept}<label class="row small"><input type="checkbox" bind:checked={accept} /> อ่านเนื้อหาทั้งหมดแล้ว ยืนยันติดตั้ง</label>{/if}
                <div class="row"><button class="primary small" disabled={installing || preview.verdict === "danger" || (preview.needs_accept && !accept)} onclick={install}>{installing ? "กำลังติดตั้ง…" : "ติดตั้งเป็นสมาชิกทีม"}</button><a class="small" href={preview.source} target="_blank" rel="noreferrer">ที่มา</a></div>
              </div>
            {/if}
          {/each}
        </div>
      </div>
    {:else if repos}
      {#each repos as r (r.repo)}
        <div class="card repo">
          <div class="row wrap"><b class="grow">{r.repo}</b>{#if r.listed === "curated"}<span class="chip teal">ยอดนิยม</span>{/if}{#if r.stars != null}<span class="faint small">★ {r.stars.toLocaleString()}</span>{/if}<button class="small primary" onclick={() => browse(r.repo)}>ดู role ในคลัง</button></div>
          <div class="muted small">{r.description ?? ""}</div>
        </div>
      {/each}
    {/if}
  {/if}

  {#if tab === "import"}
    <p class="muted small">วางเนื้อหาไฟล์ทีม (.team.md) หรือไฟล์สมาชิก (.md) ที่เพื่อนส่งมา หรือเลือกไฟล์ — ตรวจด้วยตัวสแกนก่อนเสมอ บุคลิกของคนอื่นเป็นแค่ข้อความ ไม่มีสิทธิ์เหนือกฎของบอท</p>
    <input type="file" accept=".md,text/markdown,text/plain" onchange={pickFile} />
    <textarea rows="10" bind:value={text} placeholder="---&#10;name: ...&#10;description: ...&#10;---&#10;..."></textarea>
    <div class="row"><button onclick={check} disabled={!text.trim()}>ตรวจ</button></div>
    {#if checked?.kind === "team"}
      <div class="card">
        <div class="row wrap"><b><RoleIcon emoji={checked.team.emoji || "i:users"} words={checked.team.name} /> ทีม {checked.team.name}</b><span class="chip {VERDICT[checked.verdict][1]}">{VERDICT[checked.verdict][0]}</span></div>
        <div class="muted small">{checked.team.description}</div>
        {#each checked.roles as r (r.slug)}<div class="small">• <RoleIcon emoji={r.emoji} words={(r.title || "") + " " + r.name} /> <b>{r.name}</b>{r.title ? ` · ${r.title}` : ""} · {tierName(r.tier)}{r.persona ? " · มีบุคลิก" : ""}</div>{/each}
        {#each checked.findings as f, i (i)}<div class="small {f.level === 'danger' ? 'red' : 'amber'}">• {f.what} — {f.why}</div>{/each}
        {#if checked.needs_accept}<label class="row small"><input type="checkbox" bind:checked={acceptLocal} /> อ่านแล้ว ยืนยันนำเข้า</label>{/if}
        <button class="primary small" disabled={checked.verdict === "danger" || (checked.needs_accept && !acceptLocal)} onclick={importText}>นำเข้าทั้งทีม</button>
      </div>
    {:else if checked}
      <div class="card">
        <div class="row wrap"><b><RoleIcon emoji={checked.role.emoji} words={(checked.role.title || "") + " " + checked.role.name} /> {checked.role.name}</b><span class="chip {VERDICT[checked.verdict][1]}">{VERDICT[checked.verdict][0]}</span></div>
        <div class="muted small">{checked.role.description}</div>
        {#if checked.role.persona}<div class="small"><b>บุคลิก:</b> {checked.role.persona}</div>{/if}
        {#each checked.findings as f, i (i)}<div class="small {f.level === 'danger' ? 'red' : 'amber'}">• {f.what} — {f.why}</div>{/each}
        {#if checked.needs_accept}<label class="row small"><input type="checkbox" bind:checked={acceptLocal} /> อ่านแล้ว ยืนยันนำเข้า</label>{/if}
        <button class="primary small" disabled={checked.verdict === "danger" || (checked.needs_accept && !acceptLocal)} onclick={importText}>นำเข้าเป็นสมาชิกทีม</button>
      </div>
    {/if}
  {/if}
</div>

<style>
  .page-roles { display: flex; flex-direction: column; gap: 12px; max-width: 980px; }
  .tabs { display: flex; gap: 6px; flex-wrap: wrap; }
  .f { border-radius: 999px; padding: 6px 14px; }
  .f.on { background: var(--teal-bg); border-color: var(--teal); color: var(--text); }
  .edit { display: flex; flex-direction: column; gap: 10px; }
  .edit label { display: flex; flex-direction: column; gap: 4px; font-size: 13px; }
  .edit label.row { flex-direction: row; align-items: center; }
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 10px; }
  textarea { width: 100%; resize: vertical; }
  .role { display: flex; flex-direction: column; gap: 6px; }
  .role.off { opacity: 0.65; }
  .emo { display: inline-flex; width: 32px; height: 32px; align-items: center; justify-content: center; border-radius: 9px; background: var(--teal-bg); }
  /* team editor (ISSUE-168): how many parts at once, members with their AI */
  .par { display: flex; flex-direction: column; gap: 6px; }
  .seg { display: inline-flex; border: 1px solid var(--line); border-radius: 10px; overflow: hidden; align-self: flex-start; }
  .seg button { border: none; border-radius: 0; padding: 7px 14px; background: transparent; color: var(--text-2); }
  .seg button + button { border-left: 1px solid var(--line); }
  .seg button.on { background: var(--teal-bg); color: var(--teal); }
  .mlist { display: flex; flex-direction: column; border: 1px solid var(--line); border-radius: 10px; }
  .mrow { display: grid; grid-template-columns: 34px minmax(140px, 1fr) minmax(180px, 260px) minmax(130px, 170px) 32px; gap: 10px; align-items: center; padding: 8px 10px; }
  .mrow + .mrow { border-top: 1px solid var(--line); }
  .mi { display: inline-flex; width: 30px; height: 30px; align-items: center; justify-content: center; border-radius: 8px; background: var(--teal-bg); }
  .mn { display: flex; flex-direction: column; min-width: 0; }
  .mn .cast { margin-top: 3px; padding: 2px 6px; font-size: 12px; max-width: 220px; }
  .mn b, .mn span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .mrow select { width: 100%; }
  .add { gap: 6px; }
  @media (max-width: 760px) { .mrow { grid-template-columns: 34px 1fr 32px; } .mrow select { grid-column: 2 / 3; } }
  .exp { display: flex; flex-direction: column; gap: 8px; border-top: 1px solid var(--line); padding-top: 8px; }
  .txt { max-height: 280px; overflow: auto; white-space: pre-wrap; background: var(--bg-2); border-radius: 8px; padding: 8px 10px; font-size: 12px; }
  .warnbox { color: var(--amber); }
  .safe { border-color: var(--teal); }
  .cats { margin: 6px 0; }
  .chip.on { border-color: var(--teal); color: var(--text); background: var(--teal-bg); }
  .list { display: flex; flex-direction: column; gap: 2px; max-height: 520px; overflow: auto; margin-top: 6px; }
  .item { padding: 4px 6px; border-radius: 6px; align-items: center; }
  .item:hover { background: var(--bg-2); }
  .pv { border: 1px solid var(--line); border-radius: 10px; padding: 10px; margin: 4px 0 8px; display: flex; flex-direction: column; gap: 6px; }
  .red { color: var(--red); }
  .amber { color: var(--amber); }
  .repo { display: flex; flex-direction: column; gap: 4px; }
  .presethead { margin: 14px 0 6px; font-size: 14px; }
  .presets { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(260px, 100%), 1fr)); gap: 10px; margin-bottom: 12px; }
  .preset { display: flex; flex-direction: column; gap: 8px; }
  .preset .members { gap: 4px; }
  .preset button { align-self: flex-start; margin-top: auto; }
</style>
