<script>
  import IconText from "./components/IconText.svelte";
  import Icon from "./components/Icon.svelte";
  import { onMount } from "svelte";
  import ConfirmHost from "./components/ConfirmHost.svelte";
  import { store, start, refresh, toast } from "./lib/store.svelte.js";
  import { api, pct, listen } from "./lib/api.js";
  import { t, isRtl } from "./lib/i18n.js";
  import { setUiLang } from "./lib/uitrans.svelte.js";
  import Morning from "./pages/Morning.svelte";
  import Board from "./pages/Board.svelte";
  import Quota from "./pages/Quota.svelte";
  import Usage from "./pages/Usage.svelte";
  import Library from "./pages/Library.svelte";
  import Skills from "./pages/Skills.svelte";
  import Roles from "./pages/Roles.svelte";
  import Settings from "./pages/Settings.svelte";
  import Activity from "./pages/Activity.svelte";
  import Sessions from "./pages/Sessions.svelte";
  import Workspace from "./pages/Workspace.svelte";
  import QuotaMini from "./components/QuotaMini.svelte";
  import { inbox, loadReview } from "./lib/review.svelte.js";
  import IdeaDrawer from "./components/IdeaDrawer.svelte";
  import Palette from "./components/Palette.svelte";
  import QuickAdd from "./components/QuickAdd.svelte";
  import Shelf from "./components/Shelf.svelte";
  import Setup from "./components/Setup.svelte";
  import UpdateCard from "./components/UpdateCard.svelte";
  import WhatsNew from "./components/WhatsNew.svelte";

  // 8 entries in 3 groups (ISSUE-098): pages about the same thing share one entry and show as tabs on the page
  // ("usage" under quota, "sessions" under activity) — the page keys stay, so old links still open the right view
  const NAV = [
    { group: "ไอเดีย" },
    { key: "morning", icon: "sun" },
    { key: "board", icon: "board" },
    { key: "workspace", icon: "windows" },
    { key: "library", icon: "book" },
    { group: "AI" },
    { key: "quota", icon: "gauge", also: ["usage"] },
    { key: "skills", icon: "toolbox" },
    { key: "roles", icon: "user" },
    { group: "ระบบ" },
    { key: "activity", icon: "list", also: ["sessions"] },
    { key: "settings", icon: "gear" },
  ];
  const QUOTA_TABS = [["quota", "โควต้า", "gauge"], ["usage", "รายงานการใช้", "chart"]];
  const LOG_TABS = [["activity", "ไทม์ไลน์", "list"], ["sessions", "เซสชัน", "link"]];
  const PAGE_TABS = { quota: QUOTA_TABS, usage: QUOTA_TABS, activity: LOG_TABS, sessions: LOG_TABS };

  let paletteOpen = $state(false);
  let upd = $state(null); // a newer version is out
  let updHidden = $state(false);
  let other = $state(null); // another copy of the program was opened from a different folder
  async function switchCopy() {
    try { await api.switchCopy(); } catch (e) { toast(String(e), "err"); }
  }

  onMount(() => {
    start();
    // interface language (English until the owner chooses)
    api.settings().then((x) => {
      store.lang = x.language || "en";
      document.documentElement.dir = isRtl(store.lang) ? "rtl" : "ltr";
      setUiLang(store.lang);
    }).catch(() => {});
    listen("other-copy", (e) => (other = e.payload));
    loadReview();
    const checkUpdate = () => api.updateStatus(false).then((u) => (upd = u?.available ? u : null)).catch(() => {});
    setTimeout(checkUpdate, 4000);
    listen("update-available", () => checkUpdate());
    const updTimer = setInterval(checkUpdate, 3 * 3600 * 1000);
    // an idea became ready / finished / got stuck: also shown here when the window is open
    listen("idea-notify", (e) => {
      toast(`${e.payload.title} — ${e.payload.body}`, "ok", 9000);
      refresh();
    });
    const onKey = (e) => {
      // Ctrl+F opens the search (ISSUE-090). By the key's place, not its letter: with the Thai layout on, e.key is a
      // Thai character, which is why Ctrl+K never worked. Ctrl+K still opens it too.
      if ((e.ctrlKey || e.metaKey) && !e.altKey && (e.code === "KeyF" || e.code === "KeyK")) {
        e.preventDefault();
        paletteOpen = !paletteOpen;
      } else if (e.key === "Escape") {
        if (paletteOpen) paletteOpen = false;
        else store.openIdea = null;
      }
    };
    // capture: ahead of the web view's own Ctrl+F (find in page)
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  });

  // window width → layout: wide (full menu) · rail (icons only, tablet or Workspace) · phone (menu in a drawer)
  let winW = $state(typeof window === "undefined" ? 1280 : window.innerWidth);
  let menuOpen = $state(false);
  const phone = $derived(winW < 760);
  const rail = $derived(!phone && (store.page === "workspace" || winW < 1100));
  $effect(() => {
    store.page;
    menuOpen = false; // picking a page closes the drawer
  });
  const s = $derived(store.state);
  const d = $derived(s?.decision);
  // the badge counts everything waiting for the owner: questions and finished work not looked at yet (ISSUE-100)
  const openQ = $derived(inbox().count);
  const botState = $derived.by(() => {
    if (!s) return { cls: "idle", text: "กำลังโหลด…" };
    if (s.role === "follower") return { cls: "idle", text: "เครื่องรอง · บอทรันที่เครื่องหลัก" };
    if (s.paused) return { cls: "paused", text: "พักบอทอยู่" };
    if (s.current) return { cls: "run", power: s.current.power, text: `กำลังทำ · ${s.current.title}`, step: s.current.stage_total ? `(ขั้น ${s.current.stage_no}/${s.current.stage_total})` : "" };
    if (d?.allow) return { cls: "ready", text: "พร้อมทำงาน · คิวว่าง" };
    return { cls: "idle", text: d?.reason ?? "รอ" };
  });

  // something is running: one press stops it all and remembers it, so the PC can be switched off (ISSUE-067)
  const busyNow = $derived(!!s && s.role !== "follower" && !s.shelf && ((s.sessions ?? []).length > 0 || !!s.current));
  async function pauseAll() {
    try {
      await api.pauseAll();
      toast("พักทั้งหมดแล้ว — ปิดเครื่องได้เลย · กลับมากด ▶ ทำต่อ", "ok", 6000);
    } catch (e) {
      toast(String(e), "err");
    }
    refresh();
  }

  async function togglePause() {
    await api.setPaused(!s?.paused);
    toast(s?.paused ? "บอทกลับมาทำงาน" : "พักบอทแล้ว");
    refresh();
  }
</script>

{#if upd && !updHidden && !other}
  <div class="other upd">
    <b>มีเวอร์ชันใหม่ v{upd.latest}</b>
    <UpdateCard compact />
    <button class="small ghost" onclick={() => (updHidden = true)} aria-label="ซ่อน"><Icon name="close" /></button>
  </div>
{/if}
{#if other}
  <div class="other">
    <b>คุณเพิ่งเปิดโปรแกรมจากอีกโฟลเดอร์</b> — ตัวที่รันอยู่ตอนนี้ใช้ข้อมูลที่ <code>{other.my_root}</code> (path เดิมยังอยู่ที่ตัวนี้)
    ตัวใหม่ที่คุณเปิดใช้ข้อมูลที่ <code>{other.root}</code>
    <button class="primary small" onclick={switchCopy}>ปิดตัวนี้ แล้วใช้ตัวใหม่</button>
    <button class="small" onclick={() => (other = null)}>ใช้ตัวนี้ต่อ</button>
  </div>
{/if}
<!-- on the Workspace page the menu shrinks to icons so the windows get the whole width (ISSUE-087) -->
<svelte:window bind:innerWidth={winW} />
<div class="shell" class:rail class:phone class:menu={menuOpen}>
  {#if phone && menuOpen}<button class="scrim" aria-label="ปิดเมนู" onclick={() => (menuOpen = false)}></button>{/if}
  <aside>
    <div class="brand">
      <div class="logo"><Icon name="logo" size="22" /></div>
      <div>
        <div class="name">Idea Autopilot</div>
        <div class="faint small">ตื่นมาเจอคำตอบ</div>
      </div>
    </div>
    <nav>
      {#each NAV as n}
        {#if n.group}<div class="navgrp">{n.group}</div>{:else}
        <button class="nav" class:on={store.page === n.key || n.also?.includes(store.page)} onclick={() => (store.page = n.key)} title={t("nav_" + n.key, store.lang)}>
          <span class="ico"><Icon name={n.icon} /></span>{t("nav_" + n.key, store.lang)}
          {#if n.key === "morning" && openQ > 0}<span class="badge">{openQ}</span>{/if}
        </button>
        {/if}
      {/each}
    </nav>
    <div class="side-foot">
      <QuotaMini />
      <button class="ghost small kbd" onclick={() => (paletteOpen = true)}><span><Icon name="search" /> ค้นหา</span> <kbd>Ctrl F</kbd></button>
      <button class="ghost small ver" onclick={() => { store.settingsTab = "about"; store.page = "settings"; }} title="ดูประวัติเวอร์ชัน / อัปเดต">v{s?.version ?? ""}{#if upd} <span class="newdot" title="มีเวอร์ชันใหม่ v{upd.latest}"><Icon name="dot" size="0.8em" /> ใหม่ v{upd.latest}</span>{/if}</button>
    </div>
  </aside>

  <main>
    <header>
      {#if phone}<button class="ghost menubtn" aria-label="เมนู" onclick={() => (menuOpen = !menuOpen)}><Icon name="list" /></button>{/if}
      <QuickAdd />
      <!-- the bot's state and its stop as one column pinned to the top: it does not float mid-height when the note box grows -->
      <div class="botctl">
        <button class="status {botState.cls}" onclick={togglePause} title="คลิกเพื่อ พัก / ทำต่อ">
          <span class="dot"></span><span class="ellipsis">{#if botState.power}<Icon name="rocket" /> {/if}{botState.text}{#if botState.step} <span>{botState.step}</span>{/if}</span>
        </button>
        {#if busyNow}
          <button class="ghost small shelve" onclick={pauseAll} title="หยุดบอทและปิดแชตที่ AI กำลังทำ แล้วจำไว้ — ปิดเครื่องได้ทันที กลับมากด ▶ ทำต่อ แล้ว AI จะทำต่อจากเดิม">
            <Icon name="pause" /> พักทั้งหมด · ปิดเครื่องได้
          </button>
        {/if}
      </div>
    </header>
    {#if store.page !== "settings"}
      {#each s?.account_problems ?? [] as p (p.id)}
        <div class="connbar"><Icon name="warning" /> <span class="grow"><b>{p.title}</b> เชื่อมต่อไม่ได้: {p.cause}</span>
          <button class="small primary" onclick={() => { store.settingsTab = "ai"; store.page = "settings"; }}>ไปแก้</button></div>
      {/each}
    {/if}
    <section class="page" class:full={store.page === "workspace"}>
      {#if store.page !== "workspace"}<Shelf />{/if}
      {#if PAGE_TABS[store.page]}
        <div class="ptabs" role="tablist">
          {#each PAGE_TABS[store.page] as [key, label, icon]}
            <button class="ptab" class:on={store.page === key} role="tab" aria-selected={store.page === key} onclick={() => (store.page = key)}><Icon name={icon} /> {label}</button>
          {/each}
        </div>
      {/if}
      {#if store.page === "morning"}<Morning />
      {:else if store.page === "workspace"}<Workspace />
      {:else if store.page === "board"}<Board />
      {:else if store.page === "quota"}<Quota />
      {:else if store.page === "usage"}<Usage />
      {:else if store.page === "sessions"}<Sessions />
      {:else if store.page === "library"}<Library />
      {:else if store.page === "skills"}<Skills />
      {:else if store.page === "roles"}<Roles />
      {:else if store.page === "activity"}<Activity />
      {:else}<Settings />{/if}
    </section>
  </main>
</div>

{#if store.openIdea}<IdeaDrawer id={store.openIdea} />{/if}
{#if s && s.onboarded === false}<Setup />{/if}
{#if s && s.onboarded && s.version && s.seen_version !== s.version}<WhatsNew />{/if}
{#if paletteOpen}<Palette close={() => (paletteOpen = false)} />{/if}
<ConfirmHost />
{#if store.toast}
  {#key store.toast.id}<div class="toast {store.toast.kind}"><IconText text={store.toast.text} /></div>{/key}
{/if}

<style>
  .ver { color: var(--text-3); text-align: left; }
  .newdot { color: var(--amber); }
  .other.upd { background: var(--card-2); border-bottom-color: var(--teal); }
  .other { position: fixed; top: 0; left: 0; right: 0; z-index: 60; background: var(--amber-bg); color: var(--text); padding: 10px 16px; display: flex; gap: 10px; align-items: center; flex-wrap: wrap; font-size: 13px; border-bottom: 1px solid var(--amber); }
  .shell { display: grid; grid-template-columns: 220px 1fr; height: 100vh; }
  aside { background: var(--bg-2); border-right: 1px solid var(--line); display: flex; flex-direction: column; padding: 18px 12px; gap: 18px; }
  .brand { display: flex; gap: 10px; align-items: center; padding: 0 6px; }
  .logo { width: 34px; height: 34px; border-radius: 10px; background: var(--teal-bg); color: var(--teal); display: grid; place-items: center; font-size: 20px; }
  .name { font-weight: 600; }
  nav { display: flex; flex-direction: column; gap: 2px; }
  .nav { border: none; text-align: left; display: flex; align-items: center; gap: 10px; padding: 9px 10px; border-radius: 9px; color: var(--text-2); }
  .nav:hover { background: var(--card); color: var(--text); }
  .connbar { display: flex; gap: 10px; align-items: center; padding: 8px 28px; background: var(--amber-bg); border-bottom: 1px solid var(--amber); font-size: 13px; }
  .navgrp { font-size: 11px; color: var(--text-3); padding: 12px 10px 4px; letter-spacing: .04em; }
  .navgrp:first-child { padding-top: 0; }
  .rail .navgrp { font-size: 0; padding: 0; height: 1px; width: 28px; background: var(--line); margin: 8px auto; }
  .rail .navgrp:first-child { display: none; }
  .ptabs { display: flex; gap: 6px; border-bottom: 1px solid var(--line); padding-bottom: 8px; margin-bottom: 16px; }
  .ptab { background: transparent; border: 1px solid transparent; border-radius: 8px; padding: 6px 12px; color: var(--text-2); white-space: nowrap; }
  .ptab:hover { background: var(--bg-2); }
  .ptab.on { background: var(--bg-2); border-color: var(--teal); color: var(--text); font-weight: 600; }
  .nav.on { background: var(--card-2); color: var(--text); font-weight: 500; }
  .ico { width: 20px; text-align: center; }
  .badge { margin-left: auto; background: var(--amber); color: #2a1a00; font-size: 11px; font-weight: 600; border-radius: 999px; padding: 0 7px; }
  .side-foot { margin-top: auto; display: flex; flex-direction: column; gap: 12px; }
  .kbd { display: flex; justify-content: space-between; align-items: center; color: var(--text-3); }
  kbd { font-family: inherit; font-size: 11px; border: 1px solid var(--line-2); border-radius: 5px; padding: 0 5px; }
  main { display: flex; flex-direction: column; min-width: 0; height: 100vh; }
  header { display: flex; gap: 20px; align-items: flex-start; padding: 14px 28px; border-bottom: 1px solid var(--line); }
  .botctl { flex: 0 0 clamp(210px, 20vw, 300px); display: flex; flex-direction: column; align-items: stretch; gap: 6px; padding-top: 4px; }
  .botctl .status { max-width: none; }
  .botctl .shelve { justify-content: center; color: var(--text-2); }
  .page { flex: 1; overflow: auto; padding: 24px 28px 60px; }
  .shelve { white-space: nowrap; flex: none; }
  .page.full { padding: 0; overflow: hidden; display: flex; flex-direction: column; }
  .shell.rail { grid-template-columns: 60px 1fr; }
  .rail aside { padding: 18px 8px; align-items: center; }
  .rail .brand > div:last-child, .rail .side-foot :global(.mini), .rail .kbd, .rail .ver { display: none; }
  .rail .brand { padding: 0; }
  .rail .nav { font-size: 0; justify-content: center; padding: 10px 0; width: 44px; position: relative; }
  .rail .nav .ico { font-size: 15px; }
  .rail .badge { position: absolute; top: 2px; right: 0; font-size: 10px; padding: 0 5px; }
  .rail header { padding: 10px 16px; }
  .status { display: flex; align-items: center; gap: 8px; max-width: 360px; border-radius: 999px; padding: 6px 14px; font-size: 13px; color: var(--text-2); }
  .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--text-3); flex: none; }
  .status.run .dot { background: var(--teal); animation: pulse 1.4s infinite; }
  .status.ready .dot { background: var(--teal); }
  .status.paused .dot { background: var(--red); }
  .toast { position: fixed; bottom: 24px; left: 50%; transform: translateX(-50%); background: var(--card-2); border: 1px solid var(--line-2); padding: 8px 18px; border-radius: 999px; animation: rise .2s ease-out; z-index: 50; }
  .toast.err { border-color: var(--red); color: var(--red); }

  /* tablet: icons-only menu (rail) and tighter gutters */
  .rail .page:not(.full) { padding: 20px 20px 60px; }
  .ptabs { overflow-x: auto; scrollbar-width: none; }
  /* phone: the menu becomes a drawer, the header stacks */
  .shell.phone { grid-template-columns: 1fr; }
  .phone aside { position: fixed; inset: 0 auto 0 0; width: 260px; max-width: 85vw; z-index: 70; transform: translateX(-105%); transition: transform .18s ease-out; overflow-y: auto; box-shadow: 4px 0 24px rgba(0, 0, 0, .35); }
  .phone.menu aside { transform: none; }
  .scrim { position: fixed; inset: 0; z-index: 65; background: rgba(0, 0, 0, .45); border: none; border-radius: 0; padding: 0; }
  .menubtn { flex: none; padding: 6px 8px; }
  .phone header { padding: 8px 12px; gap: 8px; flex-wrap: wrap; }
  .phone header :global(.qa) { flex: 1 1 calc(100% - 52px); min-width: 0; }
  .phone .status { max-width: none; flex: 1 1 auto; }
  .phone .page:not(.full) { padding: 14px 12px 60px; }
  .phone .connbar { padding: 8px 12px; flex-wrap: wrap; }
  .phone .shelve { white-space: normal; }
  .phone .botctl { flex: 1 1 100%; flex-direction: row; padding-top: 0; }
</style>
