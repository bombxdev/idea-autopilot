<script>
  import { onMount } from "svelte";
  import ConfirmHost from "./components/ConfirmHost.svelte";
  import { store, start, refresh, toast } from "./lib/store.svelte.js";
  import { api, pct, listen } from "./lib/api.js";
  import { t, isRtl } from "./lib/i18n.js";
  import { setUiLang } from "./lib/uitrans.svelte.js";
  import Morning from "./pages/Morning.svelte";
  import Board from "./pages/Board.svelte";
  import Quota from "./pages/Quota.svelte";
  import Library from "./pages/Library.svelte";
  import Skills from "./pages/Skills.svelte";
  import Settings from "./pages/Settings.svelte";
  import Activity from "./pages/Activity.svelte";
  import Sessions from "./pages/Sessions.svelte";
  import IdeaDrawer from "./components/IdeaDrawer.svelte";
  import Palette from "./components/Palette.svelte";
  import QuickAdd from "./components/QuickAdd.svelte";
  import Setup from "./components/Setup.svelte";
  import UpdateCard from "./components/UpdateCard.svelte";
  import WhatsNew from "./components/WhatsNew.svelte";

  const NAV = [
    { key: "morning", icon: "☀️" },
    { key: "board", icon: "🗂" },
    { key: "quota", icon: "⛽" },
    { key: "library", icon: "📚" },
    { key: "skills", icon: "🧰" },
    { key: "sessions", icon: "🔗" },
    { key: "activity", icon: "📜" },
    { key: "settings", icon: "⚙️" },
  ];

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
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        paletteOpen = !paletteOpen;
      } else if (e.key === "Escape") {
        if (paletteOpen) paletteOpen = false;
        else store.openIdea = null;
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const s = $derived(store.state);
  const d = $derived(s?.decision);
  const openQ = $derived(store.questions.length);
  const botState = $derived.by(() => {
    if (!s) return { cls: "idle", text: "กำลังโหลด…" };
    if (s.role === "follower") return { cls: "idle", text: "เครื่องรอง · บอทรันที่เครื่องหลัก" };
    if (s.paused) return { cls: "paused", text: "พักบอทอยู่" };
    if (s.current) return { cls: "run", text: `${s.current.power ? "🚀 " : ""}กำลังทำ · ${s.current.title}`, step: s.current.stage_total ? `(ขั้น ${s.current.stage_no}/${s.current.stage_total})` : "" };
    if (d?.allow) return { cls: "ready", text: "พร้อมทำงาน · คิวว่าง" };
    return { cls: "idle", text: d?.reason ?? "รอ" };
  });

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
    <button class="small ghost" onclick={() => (updHidden = true)} aria-label="ซ่อน">✕</button>
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
<div class="shell">
  <aside>
    <div class="brand">
      <div class="logo">◐</div>
      <div>
        <div class="name">Idea Autopilot</div>
        <div class="faint small">ตื่นมาเจอคำตอบ</div>
      </div>
    </div>
    <nav>
      {#each NAV as n}
        <button class="nav" class:on={store.page === n.key} onclick={() => (store.page = n.key)}>
          <span class="ico">{n.icon}</span>{t("nav_" + n.key, store.lang)}
          {#if n.key === "morning" && openQ > 0}<span class="badge">{openQ}</span>{/if}
        </button>
      {/each}
    </nav>
    <div class="side-foot">
      {#if d}
        <div class="mini">
          <div class="row small"><span class="muted grow" title="โควต้า AI ที่ใช้ไปแล้ว — ไม่ใช่ความคืบหน้าของไอเดีย (ความคืบหน้าดูที่การ์ดไอเดียและหน้า เช้านี้)">โควต้าสัปดาห์</span><b>{pct(d.week_pct)}</b></div>
          <div class="bar"><i style="width:{Math.min(100, d.week_pct)}%"></i><i class="res" style="width:{Math.min(100 - d.week_pct, d.reserve)}%"></i></div>
          <div class="row small"><span class="muted grow" title="โควต้า AI ในรอบ 5 ชั่วโมง — ไม่ใช่ความคืบหน้าของไอเดีย">โควต้า 5 ชม.</span><b>{pct(d.five_pct)}</b></div>
          <div class="bar"><i class="five" style="width:{Math.min(100, d.five_pct)}%"></i></div>
        </div>
      {/if}
      <button class="ghost small kbd" onclick={() => (paletteOpen = true)}>ค้นหา / สั่งงาน <kbd>Ctrl K</kbd></button>
      <button class="ghost small ver" onclick={() => (store.page = "settings")} title="ดูประวัติเวอร์ชัน / อัปเดต">v{s?.version ?? ""}{#if upd} <span class="newdot" title="มีเวอร์ชันใหม่ v{upd.latest}">● ใหม่ v{upd.latest}</span>{/if}</button>
    </div>
  </aside>

  <main>
    <header>
      <QuickAdd />
      <button class="status {botState.cls}" onclick={togglePause} title="คลิกเพื่อ พัก / ทำต่อ">
        <span class="dot"></span><span class="ellipsis">{botState.text}{#if botState.step} <span>{botState.step}</span>{/if}</span>
      </button>
    </header>
    <section class="page">
      {#if store.page === "morning"}<Morning />
      {:else if store.page === "board"}<Board />
      {:else if store.page === "quota"}<Quota />
      {:else if store.page === "sessions"}<Sessions />
      {:else if store.page === "library"}<Library />
      {:else if store.page === "skills"}<Skills />
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
  {#key store.toast.id}<div class="toast {store.toast.kind}">{store.toast.text}</div>{/key}
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
  .nav.on { background: var(--card-2); color: var(--text); font-weight: 500; }
  .ico { width: 20px; text-align: center; }
  .badge { margin-left: auto; background: var(--amber); color: #2a1a00; font-size: 11px; font-weight: 600; border-radius: 999px; padding: 0 7px; }
  .side-foot { margin-top: auto; display: flex; flex-direction: column; gap: 12px; }
  .mini { background: var(--card); border: 1px solid var(--line); border-radius: 12px; padding: 10px 12px; display: flex; flex-direction: column; gap: 4px; }
  .bar { height: 6px; background: var(--bg-2); border-radius: 6px; overflow: hidden; display: flex; margin-bottom: 4px; }
  .bar i { display: block; height: 100%; background: var(--teal); }
  .bar i.res { background: repeating-linear-gradient(45deg, var(--amber-bg), var(--amber-bg) 3px, transparent 3px, transparent 6px); }
  .bar i.five { background: var(--blue); }
  .kbd { display: flex; justify-content: space-between; align-items: center; color: var(--text-3); }
  kbd { font-family: inherit; font-size: 11px; border: 1px solid var(--line-2); border-radius: 5px; padding: 0 5px; }
  main { display: flex; flex-direction: column; min-width: 0; height: 100vh; }
  header { display: flex; gap: 12px; align-items: center; padding: 14px 28px; border-bottom: 1px solid var(--line); }
  .page { flex: 1; overflow: auto; padding: 24px 28px 60px; }
  .status { display: flex; align-items: center; gap: 8px; max-width: 360px; border-radius: 999px; padding: 6px 14px; font-size: 13px; color: var(--text-2); }
  .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--text-3); flex: none; }
  .status.run .dot { background: var(--teal); animation: pulse 1.4s infinite; }
  .status.ready .dot { background: var(--teal); }
  .status.paused .dot { background: var(--red); }
  .toast { position: fixed; bottom: 24px; left: 50%; transform: translateX(-50%); background: var(--card-2); border: 1px solid var(--line-2); padding: 8px 18px; border-radius: 999px; animation: rise .2s ease-out; z-index: 50; }
  .toast.err { border-color: var(--red); color: var(--red); }
</style>
