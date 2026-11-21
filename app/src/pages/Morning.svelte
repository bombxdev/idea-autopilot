<script>
  import Icon from "../components/Icon.svelte";
  import { onMount } from "svelte";
  import { store, questionsFor } from "../lib/store.svelte.js";
  import { api, pct, clock } from "../lib/api.js";
  import IdeaCard from "../components/IdeaCard.svelte";
  import LiveLog from "../components/LiveLog.svelte";
  import { inbox, markReviewed } from "../lib/review.svelte.js";

  let digest = $state(null);
  onMount(async () => {
    digest = await api.digest();
  });

  const since = $derived(digest?.body?.since ?? Date.now() / 1000 - 86400);
  const visible = $derived(store.ideas.filter((i) => !["archived", "deleted"].includes(i.status)));
  const asking = $derived(visible.filter((i) => questionsFor(i.id).length > 0));
  // one list of everything that waits for the owner (ISSUE-100)
  const box = $derived(inbox());
  const KIND = { ready: "พร้อมลองเล่น", done: "เสร็จแล้ว", failed: "ติดปัญหา" };
  const ready = $derived(visible.filter((i) => i.status === "ready" && !asking.includes(i)));
  const fresh = $derived(
    visible.filter((i) => i.updated >= since && ["done", "failed"].includes(i.status) && !asking.includes(i)),
  );
  const moving = $derived(
    visible.filter((i) => ["active", "inbox"].includes(i.status) && !asking.includes(i)).slice(0, 6),
  );
  const d = $derived(store.state?.decision);
  const hour = new Date().getHours();
  const greet = hour < 12 ? "อรุณสวัสดิ์" : hour < 17 ? "สวัสดีตอนบ่าย" : "สวัสดีตอนค่ำ";
  const b = $derived(digest?.body);
</script>

<div class="wrapx">
  <div class="hero">
    <div>
      <h1>{greet} <Icon name="sunrise" /></h1>
      <p class="muted">{b?.headline ?? "ยังไม่มีสรุป — จดไอเดียทิ้งไว้ แล้วตื่นมาดูตรงนี้"}</p>
    </div>
    <button class="ghost small" onclick={async () => (digest = await api.makeDigest())}>สรุปใหม่ตอนนี้</button>
  </div>

  <div class="tiles">
    <div class="tile"><span class="lbl">ใช้ไปแล้วสัปดาห์นี้</span><span class="num">{pct(d?.week_pct)}</span><span class="faint small">รีเซ็ต {clock(store.state?.snap?.week_reset)}</span></div>
    <div class="tile"><span class="lbl">บอทใช้ตั้งแต่สรุปล่าสุด</span><span class="num teal">{pct(b?.bot_pct, 1)}</span><span class="faint small">{b?.runs ?? 0} รอบงาน</span></div>
    <div class="tile"><span class="lbl">กันไว้ให้งานคุณ</span><span class="num amber">{pct(d?.reserve)}</span><span class="faint small">ช่วงเวลางานที่เหลือ</span></div>
    <div class="tile"><span class="lbl">บอทยังใช้ได้อีก</span><span class="num">{pct(Math.max(0, d?.bot_budget ?? 0))}</span><span class="faint small">วันนี้ {pct(d?.today_allowance, 1)}</span></div>
  </div>

  {#if store.state?.current}
    <section>
      <h2><Icon name="gear" /> กำลังทำอยู่ตอนนี้</h2>
      <LiveLog />
    </section>
  {/if}

  <section>
    <h2><Icon name="user" /> รอคุณ {#if box.count}<span class="count">{box.count}</span>{/if}
      {#if box.count}<span class="faint small">ตอบคำถาม หรือดูงานที่เสร็จแล้วกด “ตรวจแล้ว”</span>{/if}</h2>
    {#if box.count}
      <div class="grid">
        {#each box.asking as idea (idea.id)}<IdeaCard {idea} />{/each}
        {#each box.rest as idea (idea.id)}
          <div class="rev">
            <div class="revbar"><span class="chip {idea.status === 'failed' ? 'red' : 'teal'}">{KIND[idea.status]}</span><span class="grow"></span>
              <button class="small" onclick={() => markReviewed(idea)} title="เอาออกจากรายการรอคุณ (งานยังอยู่ในบอร์ดและคลังความรู้)"><Icon name="check" /> ตรวจแล้ว</button></div>
            <IdeaCard {idea} />
          </div>
        {/each}
      </div>
    {:else}
      <div class="card done-all"><Icon name="checkCircle" /> ไม่มีอะไรรอคุณ — บอททำงานต่อเองได้</div>
    {/if}
  </section>

  {#if b?.needs?.length}
    <section>
      <h2><Icon name="checklist" /> สิ่งที่ต้องทำเอง</h2>
      <div class="card">
        {#each b.needs as n}
          <div class="need"><button class="ghost" onclick={() => (store.openIdea = n.id)}><b>{n.title}</b></button>
            <ul>{#each n.needs as x}<li>{x}</li>{/each}</ul></div>
        {/each}
      </div>
    </section>
  {/if}

  <section>
    <h2><Icon name="compass" /> ในคิว</h2>
    {#if moving.length}
      <div class="grid">{#each moving as idea (idea.id)}<IdeaCard {idea} compact />{/each}</div>
    {:else}
      <div class="empty card">
        <div class="big"><Icon name="bulb" size="40" /></div>
        <div><b>คิวว่าง</b></div>
        <div class="muted small">จดไอเดียที่ช่องด้านบน หรือกดปุ่มลัดจดไอเดียจากที่ไหนก็ได้ (ดูได้ที่หน้าตั้งค่า) แล้วไปทำอย่างอื่นได้เลย</div>
      </div>
    {/if}
  </section>
</div>

<style>
  .wrapx { max-width: 1100px; display: flex; flex-direction: column; gap: 28px; }
  .hero { display: flex; justify-content: space-between; align-items: flex-end; gap: 16px; }
  .hero p { margin: 6px 0 0; font-size: 16px; }
  .tiles { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(190px, 100%), 1fr)); gap: 12px; }
  .tile { background: var(--card); border: 1px solid var(--line); border-radius: 14px; padding: 14px 16px; display: flex; flex-direction: column; gap: 2px; }
  .lbl { font-size: 13px; color: var(--text-2); }
  .num { font-size: 28px; font-weight: 600; }
  .teal { color: var(--teal); }
  .amber { color: var(--amber); }
  section { display: flex; flex-direction: column; gap: 12px; }
  section h2 { display: flex; align-items: baseline; gap: 10px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(420px, 100%), 1fr)); gap: 14px; }
  .empty { text-align: center; padding: 32px; display: flex; flex-direction: column; gap: 4px; align-items: center; }
  .big { font-size: 34px; }
  .count { background: var(--amber); color: #2a1a00; font-size: 12px; font-weight: 600; border-radius: 999px; padding: 0 8px; align-self: center; }
  .rev { display: flex; flex-direction: column; gap: 6px; }
  .revbar { display: flex; align-items: center; gap: 8px; }
  .done-all { color: var(--text-2); display: flex; gap: 8px; align-items: center; }
  .need ul { margin: 2px 0 10px; }
</style>
