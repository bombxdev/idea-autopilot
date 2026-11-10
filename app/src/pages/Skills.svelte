<script>
  import Icon from "../components/Icon.svelte";
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";
  import { toast } from "../lib/store.svelte.js";
  import { ask } from "../lib/dialog.svelte.js";
  import SkillCheck from "../components/SkillCheck.svelte";
  import SkillEngine from "../components/SkillEngine.svelte";

  let tab = $state("mine");
  let data = $state({ skills: [], families: {}, max_offered: 8 });
  let viewing = $state(null); // { slug, text }

  async function load() {
    data = await api.skills();
  }
  onMount(load);

  const TRUST = {
    local: { t: "เขียนเอง", c: "teal" },
    known: { t: "แหล่งที่รู้จัก", c: "blue" },
    community: { t: "ชุมชน", c: "amber" },
  };
  const trust = (k) => TRUST[k] ?? TRUST.community;

  // result of the deep check, as shown next to each installed skill
  function badge(s) {
    const x = s.ss;
    if (!x || !x.status) return { t: "ยังไม่ได้ตรวจเชิงลึก", c: "", tip: "ตรวจด้วยตัวสแกนในโปรแกรมตอนติดตั้งแล้ว แต่ SkillSpector ยังไม่ได้ตรวจสกิลนี้" };
    const top = (x.issues ?? []).slice(0, 3).map((i) => `${i.severity}: ${i.pattern} (${i.category})`).join(" | ");
    const when = `ตรวจด้วย SkillSpector v${x.version} · คะแนนเสี่ยง ${x.score}/100`;
    const stale = x.current ? "" : " (ไฟล์หรือเวอร์ชันตัวตรวจเปลี่ยน — รอตรวจใหม่)";
    if (x.status === "pass") return { t: x.current ? "ผ่านการตรวจ" : "ผ่าน (รอตรวจใหม่)", c: "teal", tip: when + stale };
    if (x.status === "caution") return { t: "ควรระวัง", c: "amber", tip: `${when}${stale} — ${top}` };
    return { t: "ไม่ผ่าน — ไม่ถูกเสนอให้บอท", c: "red", tip: `${when}${stale} — ${top}` };
  }

  // skills grouped by family: one dish, several recipes
  const groups = $derived.by(() => {
    const g = new Map();
    for (const s of data.skills) {
      const k = s.family || `~${s.slug}`;
      if (!g.has(k)) g.set(k, { family: s.family, items: [] });
      g.get(k).items.push(s);
    }
    return [...g.values()].sort((a, b) => (b.items.length > 1) - (a.items.length > 1));
  });

  async function set(slug, field, value) {
    try {
      await api.skillSet(slug, field, String(value));
      await load();
    } catch (e) {
      toast(String(e), "err");
    }
  }
  async function view(slug) {
    viewing = viewing?.slug === slug ? null : { slug, text: await api.skillRead(slug) };
  }
  async function remove(s) {
    if (!(await ask("สกิลจะถูกลบออกจากเครื่องนี้", { title: `ลบสกิล “${s.name}”?`, ok: "ลบสกิล", danger: true }))) return;
    await api.skillDelete(s.slug);
    toast("ลบแล้ว");
    load();
  }
  const rate = (st) => (st.used ? Math.round((st.ok_used / st.used) * 100) : null);

  // ---------- shop ----------
  let q = $state("");
  let searching = $state(false);
  let repos = $state(null);
  let repoNote = $state("");
  let repo = $state(null); // browse result
  let skillFilter = $state("");
  let pasteUrl = $state("");
  let preview = $state(null);
  let previewing = $state("");
  let accept = $state(false);
  let installing = $state(false);
  let deep = $state(null); // SkillSpector result for the open preview
  let deepLocal = $state(null); // ... for the pasted text

  async function search() {
    searching = true;
    repo = null;
    preview = null;
    try {
      const r = await api.shopSearch(q.trim());
      repos = r.repos;
      repoNote = r.note ?? "";
    } catch (e) {
      repos = [];
      repoNote = String(e);
    }
    searching = false;
  }
  async function browse(name) {
    searching = true;
    preview = null;
    try {
      repo = await api.shopBrowse(name);
      skillFilter = "";
    } catch (e) {
      toast(String(e), "err");
    }
    searching = false;
  }
  async function open(path) {
    previewing = path;
    accept = false;
    try {
      deep = null;
      preview = { ...(await api.shopPreview(repo.repo, repo.commit, path)), repo: repo.repo, commit: repo.commit, path };
      setTimeout(() => document.querySelector(".pv")?.scrollIntoView({ behavior: "smooth", block: "start" }), 50);
    } catch (e) {
      toast(String(e), "err");
    }
    previewing = "";
  }
  async function resolveUrl() {
    try {
      const r = await api.shopResolve(pasteUrl.trim());
      repo = { repo: r.repo, commit: r.commit, skills: [], license: null, trust: "community", stars: null };
      await open(r.path);
    } catch (e) {
      toast(String(e), "err");
    }
  }
  async function install() {
    installing = true;
    try {
      const r = await api.shopInstall(preview.repo, preview.commit, preview.path, preview.sha256, accept);
      toast(`ติดตั้ง “${r.name}” แล้ว`, "ok");
      preview = null;
      await load();
      tab = "mine";
    } catch (e) {
      toast(String(e).replace(/^NEEDS_ACCEPT: /, ""), "err");
    }
    installing = false;
  }
  // verdict of the open preview: built-in scan, plus SkillSpector once its result is in
  const pv = $derived(deep?.verdict ?? preview?.verdict);
  const pvWait = $derived(!!preview?.ss_available && !deep);
  const shownSkills = $derived(
    (repo?.skills ?? []).filter((s) => !skillFilter.trim() || (s.name + " " + s.path).toLowerCase().includes(skillFilter.trim().toLowerCase())),
  );

  // ---------- write / paste ----------
  let text = $state("");
  let checked = $state(null);
  let acceptLocal = $state(false);
  async function check() {
    try {
      deepLocal = null;
      checked = await api.skillCheck(text);
    } catch (e) {
      checked = null;
      toast(String(e), "err");
    }
  }
  async function addLocal() {
    try {
      const r = await api.skillAddLocal(text, acceptLocal);
      toast(`เพิ่ม “${r.name}” แล้ว`, "ok");
      text = "";
      checked = null;
      await load();
      tab = "mine";
    } catch (e) {
      toast(String(e).replace(/^NEEDS_ACCEPT: /, ""), "err");
    }
  }
  const lv = $derived(deepLocal?.verdict ?? checked?.verdict);
  const lwait = $derived(!!checked?.ss_available && !deepLocal);
  const TEMPLATE = `---\nname: my-skill\ndescription: ใช้เมื่อ… (บอกให้ชัดว่าสกิลนี้ช่วยเรื่องอะไร AI จะอ่านบรรทัดนี้เพื่อตัดสินว่าจะเปิดใช้ไหม)\nfamily: \n---\n\n# ชื่อสกิล\n\n1. ขั้นตอนแรก\n2. ขั้นตอนต่อไป\n`;
</script>

<div class="wrapx">
  <h1>สกิล</h1>
  <p class="muted small">
    สกิลคือ<b>สูตร</b>ที่ AI เปิดอ่านเมื่อจำเป็น — สูตรต่างกันได้ผลต่างกันแม้เป็นอาหารจานเดียวกัน จึงไม่มีสูตร “ถูก” ตัวเดียว
    ใส่สูตรที่ทำงานเดียวกันไว้ใน <b>ตระกูล</b> (family) เดียวกัน บอทจะเสนอทีละสูตรแล้วเก็บผลให้คุณดู คุณปักหมุดสูตรโปรดได้เสมอ
  </p>
  <SkillEngine onchange={load} />
  <div class="row wrap tabs">
    <button class="f" class:on={tab === "mine"} onclick={() => (tab = "mine")}>สกิลของฉัน ({data.skills.length})</button>
    <button class="f" class:on={tab === "shop"} onclick={() => { tab = "shop"; if (!repos) search(); }}><Icon name="cart" /> ช็อปสกิล (ฟรี)</button>
    <button class="f" class:on={tab === "write"} onclick={() => (tab = "write")}><Icon name="pencil" /> เขียน / วางเอง</button>
  </div>

  {#if tab === "mine"}
    {#if !data.skills.length}
      <div class="card faint">ยังไม่มีสกิล — เริ่มจากช็อปสกิลฟรี หรือเขียนเอง บอทจะเสนอให้ AI เฉพาะสกิลที่เปิดใช้ (สูงสุด {data.max_offered} สูตรต่อรอบ)</div>
    {/if}
    {#each groups as g (g.family || g.items[0].slug)}
      <div class="group">
        {#if g.items.length > 1}
          <div class="gh"><Icon name="layers" /> ตระกูล <b>{g.family}</b> · {g.items.length} สูตร — บอทเสนอทีละสูตร</div>
        {/if}
        {#each g.items as s (s.slug)}
          <div class="card sk" class:off={!s.enabled}>
            <div class="row wrap">
              <label class="sw" title="เปิด/ปิดสกิลนี้"><input type="checkbox" checked={s.enabled} onchange={(e) => set(s.slug, "enabled", e.currentTarget.checked)} /></label>
              <b class="grow">{s.name}</b>
              <span class="chip {trust(s.trust).c}">{trust(s.trust).t}</span>
              <span class="chip {badge(s).c}" title={badge(s).tip}><Icon name="shieldCheck" /> {badge(s).t}</span>
              {#if s.family}
                <button class="small" class:pin={s.pinned} onclick={() => set(s.slug, "pinned", !s.pinned)} title="ปักหมุด: ใช้สูตรนี้เสมอในตระกูลนี้"><Icon name="pin" /> {s.pinned ? "ปักหมุดอยู่" : "ปักหมุด"}</button>
              {/if}
            </div>
            <div class="muted small">{s.description}</div>
            <div class="row wrap small faint">
              <span>เสนอ {s.stats.offered} รอบ</span>
              <span>· เปิดใช้จริง {s.stats.used}</span>
              {#if rate(s.stats) !== null}<span>· จบสำเร็จ {rate(s.stats)}% ({s.stats.ok_used}/{s.stats.used})</span>{/if}
              {#if s.stats.used < 5}<span class="amber">· ข้อมูลยังน้อย อย่าเพิ่งสรุป</span>{/if}
              {#if s.source}<a href={s.source} target="_blank" rel="noreferrer">ที่มา</a>{/if}
              {#if s.license}<span>· {s.license}</span>{/if}
            </div>
            {#if s.similar.length}
              <div class="hint small">
                คล้ายกับ {s.similar.map((x) => `“${x.name}” (${x.score}%)`).join(", ")} — ถ้าเป็น<b>สูตรคนละแบบของงานเดียวกัน</b> ใส่ตระกูลเดียวกันเพื่อให้บอทเทียบผลให้
              </div>
            {/if}
            <div class="row wrap small">
              <label class="faint">ตระกูล <input class="fam" value={s.family} placeholder="เช่น web-research" onchange={(e) => set(s.slug, "family", e.currentTarget.value)} /></label>
              <span class="grow"></span>
              <button class="small" onclick={() => view(s.slug)}>{viewing?.slug === s.slug ? "ซ่อนเนื้อหา" : "ดูเนื้อหา"}</button>
              <button class="small danger" onclick={() => remove(s)}>ลบ</button>
            </div>
            {#if viewing?.slug === s.slug}<pre class="txt">{viewing.text}</pre>{/if}
          </div>
        {/each}
      </div>
    {/each}
  {/if}

  {#if tab === "shop"}
    <div class="card safe small">
      <Icon name="shieldCheck" /> <b>ความปลอดภัย:</b> ดึงเฉพาะข้อความของ <code>SKILL.md</code> จาก GitHub (ผูกกับ commit และ hash เดียวกับที่คุณเปิดดู) ไม่ดาวน์โหลดสคริปต์หรือโปรแกรมใดๆ
      ผ่านตัวสแกนก่อนเสมอ ถ้าเจอของอันตรายจะติดตั้งไม่ได้ ถ้าเป็นแค่คำเตือนต้องติ๊กยืนยันหลังอ่านเนื้อหาเต็ม และ AI ติดตั้งสกิลเองไม่ได้ ตัวสแกนเป็นเพียงด่านแรก — การอ่านเนื้อหาด้วยตาคุณคือด่านจริง
    </div>
    <form class="row" onsubmit={(e) => { e.preventDefault(); search(); }}>
      <input class="grow" bind:value={q} placeholder="ค้นหา เช่น pdf, testing, web, research (เว้นว่าง = แหล่งที่รู้จัก)" />
      <button class="primary" disabled={searching}>{searching ? "กำลังค้นหา…" : "ค้นหา"}</button>
    </form>
    <form class="row" onsubmit={(e) => { e.preventDefault(); resolveUrl(); }}>
      <input class="grow" bind:value={pasteUrl} placeholder="หรือวางลิงก์ GitHub ของไฟล์ SKILL.md โดยตรง" />
      <button disabled={!pasteUrl.trim()}>ดูตัวอย่าง</button>
    </form>

    {#if !repo && repos}
      {#if repoNote}<div class="warnbox small">{repoNote}</div>{/if}
      {#each repos as r (r.repo)}
        <button class="card repo" onclick={() => browse(r.repo)}>
          <div class="row wrap"><b class="grow">{r.repo}</b><span class="chip {r.trust === 'known' ? 'blue' : 'amber'}">{r.trust === "known" ? "แหล่งที่รู้จัก" : "ชุมชน"}</span>
            {#if r.stars != null}<span class="faint small"><Icon name="star" /> {r.stars}</span>{/if}{#if r.license}<span class="faint small">{r.license}</span>{/if}</div>
          {#if r.description}<div class="muted small">{r.description}</div>{/if}
        </button>
      {:else}
        <div class="faint">ไม่พบ</div>
      {/each}
    {/if}

    {#if repo}
      <div class="row wrap">
        <button class="small" onclick={() => { repo = null; preview = null; }}><Icon name="arrowLeft" /> กลับ</button>
        <b>{repo.repo}</b>
        {#if repo.stars != null}<span class="faint small"><Icon name="star" /> {repo.stars}</span>{/if}
        <span class="small {repo.license ? 'faint' : 'amber'}">{repo.license ? `ไลเซนส์ ${repo.license}` : "ไม่ระบุไลเซนส์ที่ระดับ repo — ตรวจในไฟล์สกิลเอง"}</span>
        {#if repo.archived}<span class="chip amber">เลิกดูแลแล้ว</span>{/if}
        <span class="faint small">commit {repo.commit.slice(0, 7)}</span>
      </div>
      {#if repo.skills.length}
        <input bind:value={skillFilter} placeholder="กรองรายชื่อสกิลใน repo นี้…" />
        <div class="list">
          {#each shownSkills as s (s.path)}
            <button class="card item" class:cur={preview?.path === s.path} disabled={s.too_big} onclick={() => open(s.path)}>
              <b class="grow">{s.name}</b><span class="faint small">{s.path}</span>
              {#if s.too_big}<span class="chip red">ใหญ่เกินไป</span>{:else}<span class="faint small">{(s.size / 1024).toFixed(1)} KB</span>{/if}
              {#if previewing === s.path}<span class="small">กำลังโหลด…</span>{/if}
            </button>
          {:else}<div class="faint">ไม่พบ</div>{/each}
        </div>
        {#if repo.truncated}<div class="faint small">repo ใหญ่ แสดงไม่ครบทุกไฟล์</div>{/if}
      {:else if !preview}<div class="faint">ไม่พบไฟล์ SKILL.md ใน repo นี้</div>{/if}
    {/if}

    {#if preview}
      <div class="card pv">
        <div class="row wrap"><b class="grow">{preview.name}</b>
          <span class="chip {pv === 'clean' ? 'teal' : pv === 'warn' ? 'amber' : 'red'}">{pv === "clean" ? "ผ่านการสแกน" : pv === "warn" ? "มีคำเตือน" : "อันตราย — ติดตั้งไม่ได้"}</span></div>
        <div class="muted small">{preview.description}</div>
        <div class="small faint">ที่มา <a href={preview.source} target="_blank" rel="noreferrer">{preview.source}</a> · ไลเซนส์ {preview.license || "ไม่ระบุ"} · sha256 {preview.sha256.slice(0, 12)}… · {(preview.size / 1024).toFixed(1)} KB</div>
        {#if preview.domains.length}<div class="small faint">ลิงก์ในไฟล์ไปยัง: {preview.domains.join(", ")}</div>{/if}
        {#each preview.findings as f}
          <div class="finding {f.level}"><b><Icon name={f.level === "danger" ? "stopSign" : "warning"} /> {f.what}</b><div class="small">{f.why}</div>{#if f.excerpt}<code class="small">{f.excerpt}</code>{/if}</div>
        {/each}
        {#if preview.similar.length}
          <div class="hint small">คล้ายกับสกิลที่มีอยู่: {preview.similar.map((x) => `“${x.name}” (${x.score}%)`).join(", ")} — ติดตั้งได้ถ้าอยากได้อีกสูตร แล้วใส่ตระกูลเดียวกันเพื่อเทียบผล</div>
        {/if}
        {#if preview.ss_available}<SkillCheck text={preview.text} bind:result={deep} />{/if}
        <div class="small faint">{preview.note}</div>
        <details open><summary class="small">เนื้อหาเต็ม (อ่านให้จบก่อนติดตั้ง)</summary><pre class="txt">{preview.text}</pre></details>
        {#if pv === "warn"}<label class="small"><input type="checkbox" bind:checked={accept} /> ฉันอ่านเนื้อหาและคำเตือนทั้งหมดแล้ว และต้องการติดตั้ง</label>{/if}
        <div class="row">
          <button class="primary" disabled={installing || pvWait || pv === "danger" || (pv === "warn" && !accept)} onclick={install}>{installing ? "กำลังติดตั้ง…" : pvWait ? "รอผลตรวจก่อน…" : "ติดตั้งสกิลนี้"}</button>
          <button onclick={() => (preview = null)}>ปิด</button>
        </div>
      </div>
    {/if}
  {/if}

  {#if tab === "write"}
    <p class="muted small">เขียนสกิลของคุณเอง หรือวางสกิลที่ได้มาจากที่อื่น จะผ่านตัวสแกนเดียวกัน ต้องมี <code>name</code> และ <code>description</code> ใน front-matter</p>
    <textarea rows="14" bind:value={text} placeholder={TEMPLATE} oninput={() => (checked = null)}></textarea>
    <div class="row wrap">
      <button onclick={() => (text = TEMPLATE)}>ใส่แบบตัวอย่าง</button>
      <button onclick={check} disabled={!text.trim()}>ตรวจก่อน</button>
      <button class="primary" onclick={addLocal} disabled={!text.trim() || lwait || lv === "danger" || (lv === "warn" && !acceptLocal)}>{lwait ? "รอผลตรวจก่อน…" : "เพิ่มสกิล"}</button>
    </div>
    {#if checked}
      <div class="card">
        <b>{checked.name}</b> — {lv === "clean" ? "ผ่านการสแกน" : lv === "warn" ? "มีคำเตือน" : "อันตราย ติดตั้งไม่ได้"}
        {#each checked.findings as f}<div class="finding {f.level}"><b><Icon name={f.level === "danger" ? "stopSign" : "warning"} /> {f.what}</b><div class="small">{f.why}</div></div>{/each}
        {#if checked.similar.length}<div class="hint small">คล้ายกับ {checked.similar.map((x) => `“${x.name}”`).join(", ")}</div>{/if}
        {#if checked.ss_available}<SkillCheck text={text} bind:result={deepLocal} />{/if}
        {#if lv === "warn"}<label class="small"><input type="checkbox" bind:checked={acceptLocal} /> ฉันอ่านคำเตือนแล้ว และต้องการเพิ่ม</label>{/if}
      </div>
    {/if}
  {/if}
</div>

<style>
  .wrapx { max-width: 900px; display: flex; flex-direction: column; gap: 12px; }
  .wrapx p { margin: 0; }
  .tabs { gap: 6px; }
  .f { border-radius: 999px; font-size: 13px; padding: 4px 14px; }
  .f.on { background: var(--card-2); border-color: var(--teal); }
  .group { display: flex; flex-direction: column; gap: 8px; }
  .gh { font-size: 13px; color: var(--text-2); padding: 0 4px; }
  .sk { display: flex; flex-direction: column; gap: 6px; }
  .sk.off { opacity: 0.55; }
  .sw input { width: 18px; height: 18px; }
  .pin { border-color: var(--teal); color: var(--teal); }
  .fam { width: 150px; padding: 2px 8px; font-size: 13px; }
  .hint { background: var(--bg-2); border-left: 3px solid var(--teal); padding: 6px 10px; border-radius: 6px; color: var(--text-2); }
  .safe { border-left: 3px solid var(--teal); }
  .warnbox { background: var(--amber-bg); color: var(--amber); padding: 8px 12px; border-radius: 8px; }
  .amber { color: var(--amber); }
  .repo, .item { text-align: left; display: flex; flex-direction: column; gap: 4px; width: 100%; }
  .item { flex-direction: row; align-items: center; gap: 10px; }
  .item.cur { border-color: var(--teal); }
  .item:disabled { opacity: 0.5; }
  .repo:hover, .item:hover:not(:disabled) { border-color: var(--line-2); background: var(--card); }
  .list { display: flex; flex-direction: column; gap: 6px; max-height: 280px; overflow: auto; }
  .pv { display: flex; flex-direction: column; gap: 8px; border-color: var(--teal); }
  .finding { padding: 6px 10px; border-radius: 8px; display: flex; flex-direction: column; gap: 2px; }
  .finding.danger { background: rgba(220, 70, 70, 0.14); }
  .finding.warn { background: var(--amber-bg); }
  .finding code { color: var(--text-2); word-break: break-all; }
  .txt { background: var(--bg-2); padding: 10px; border-radius: 8px; max-height: 340px; overflow: auto; white-space: pre-wrap; word-break: break-word; font-size: 12px; margin: 6px 0 0; }
  .danger { color: var(--red); }
  textarea { font-family: ui-monospace, Consolas, monospace; font-size: 13px; }
  .chip.red { color: var(--red); }
  .chip.teal { color: var(--teal); }
  .chip.amber { color: var(--amber); }
  .chip.blue { color: var(--blue); }
</style>
