<script>
  // An idea's chat rounds (ISSUE-159): what the owner asked, the AI's whole answer, what happened along the way
  // (counted, not listed line by line), problems in plain words, and the prompts the AI suggests next as buttons.
  import Markdown from "./Markdown.svelte";
  import Icon from "./Icon.svelte";
  import { api, dayBreak } from "../lib/api.js";
  import { store, toast } from "../lib/store.svelte.js";

  let { id, rounds = [] } = $props();
  let open = $state({}); // round id -> shown whole (the newest is open by default)
  let along = $state({}); // round id -> "along the way" notes shown

  const hm = (ts) => new Date(ts * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  // the ➡️ lines become buttons, so they are left out of the answer itself
  const body = (a) => a.split("\n").filter((l) => !/^\s*[-*]?\s*➡️/.test(l)).join("\n").trim();
  const preview = (r) => (r.note || body(r.answer)).replace(/[#*\x60|>]/g, " ").replace(/\s+/g, " ").trim().slice(0, 220);
  const isOpen = (r, i) => open[r.id] ?? i === 0;
  const link = (h) => api.openReportLink(id, h).catch((e) => toast(String(e), "err"));
  function use(t) {
    store.chatDraft = { id, text: t };
    toast("ใส่ในช่องสั่ง AI แล้ว — แก้ได้ก่อนส่ง");
  }
</script>

{#if !rounds.length}
  <div class="faint pad">ยังไม่มีรอบคุย — เมื่อสั่ง AI ในแชตของไอเดียนี้ คำสั่งและคำตอบแต่ละรอบจะเก็บไว้ที่นี่</div>
{:else}
  <p class="faint small lead">คำสั่งแต่ละรอบกับคำตอบของ AI แบบเต็ม (ล่าสุดอยู่บน) — ขั้นตอนระหว่างทางนับไว้ ปัญหาแปลเป็นภาษาคน</p>
  {#each rounds as r, i (r.id)}
    {@const day = dayBreak(rounds, i, (x) => x.started)}
    {#if day}<div class="day"><span>{day}</span></div>{/if}
    <section class="round" class:last={i === 0}>
      <button type="button" class="head" onclick={() => (open[r.id] = !isOpen(r, i))}>
        <span class="t">{hm(r.started)}</span>
        <span class="q">{#if r.prompt}🙋 {r.prompt}{:else}<span class="faint">AI ทำต่อ (ไม่มีคำสั่งใหม่)</span>{/if}</span>
        <Icon name={isOpen(r, i) ? "arrowUp" : "arrowDown"} />
      </button>
      {#if isOpen(r, i)}
        {#if r.answer}
          <div class="ans"><Markdown text={body(r.answer)} onlink={link} /></div>
        {:else if r.note}
          <div class="ans"><Markdown text={r.note} onlink={link} /></div>
        {:else}
          <div class="faint small">{r.done ? "รอบนี้ไม่มีคำตอบเป็นข้อความ" : "AI ยังทำรอบนี้อยู่…"}</div>
        {/if}
        {#if r.problems.length}
          <div class="probs">
            <b class="small"><Icon name="warning" /> เจอปัญหาระหว่างทาง</b>
            <ul>{#each r.problems as p}<li title={p.raw}>{p.text}</li>{/each}</ul>
          </div>
        {/if}
        <div class="meta small faint">
          {#if r.steps}<span>ระหว่างทาง {r.steps} ขั้น</span>{/if}
          {#if r.files.length}<span>แก้ไฟล์: {r.files.slice(0, 6).join(", ")}{r.files.length > 6 ? ` +${r.files.length - 6}` : ""}</span>{/if}
          {#if r.says.length}<button type="button" class="ghost small" onclick={() => (along[r.id] = !along[r.id])}>{along[r.id] ? "ซ่อนที่ AI เล่าระหว่างทาง" : `ดูที่ AI เล่าระหว่างทาง (${r.says.length})`}</button>{/if}
          {#if r.imported}<span title="บันทึกก่อนมีหน้านี้: แต่ละข้อความถูกตัดเหลือบรรทัดเดียว ไม่เกิน 600 ตัวอักษร">จากบันทึกเก่า (อาจถูกตัด)</span>{/if}
        </div>
        {#if along[r.id]}
          <ol class="says small">{#each r.says as s}<li>{s}</li>{/each}</ol>
        {/if}
        {#if r.next.length}
          <div class="next">
            <span class="small faint">สั่งต่อ:</span>
            {#each r.next as n}<button type="button" class="chip" onclick={() => use(n)} title="ใส่ในช่องสั่ง AI (แก้ได้ก่อนส่ง)">➡️ {n}</button>{/each}
          </div>
        {/if}
      {:else}
        <div class="pv small faint">{preview(r) || (r.problems.length ? `⚠️ ${r.problems[0].text}` : "")}</div>
      {/if}
    </section>
  {/each}
{/if}

<style>
  .lead { margin: 0 0 10px; }
  .pad { padding: 16px 4px; }
  .day { text-align: center; margin: 14px 0 6px; font-size: 12px; color: var(--text-3); }
  .day span { background: var(--card-2); padding: 2px 10px; border-radius: 999px; }
  .round { border: 1px solid var(--line); border-radius: 10px; padding: 8px 12px; margin: 0 0 10px; background: var(--card); }
  .round.last { border-color: var(--teal); }
  .head { display: flex; align-items: flex-start; gap: 10px; width: 100%; background: none; border: none; padding: 2px 0; text-align: left; color: var(--text); cursor: pointer; font: inherit; }
  .head .t { color: var(--text-3); font-size: 12px; padding-top: 2px; flex: none; }
  .head .q { flex: 1; font-weight: 600; white-space: pre-wrap; overflow-wrap: anywhere; }
  .ans { margin: 6px 0 4px; }
  .pv { margin: 2px 0 2px 46px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .probs { background: color-mix(in srgb, var(--amber, #f2b04a) 12%, transparent); border-radius: 8px; padding: 6px 10px; margin: 6px 0; }
  .probs ul { margin: 4px 0 0; padding-left: 18px; }
  .meta { display: flex; flex-wrap: wrap; gap: 4px 14px; align-items: center; margin: 6px 0 2px; }
  .meta .ghost { padding: 0 6px; }
  .says { margin: 4px 0; padding-left: 20px; color: var(--text-2); }
  .says li { margin: 2px 0; white-space: pre-wrap; }
  .next { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; margin: 8px 0 2px; }
  .chip { border: 1px solid var(--teal); color: var(--teal); background: transparent; border-radius: 999px; padding: 3px 12px; font-size: 13px; cursor: pointer; text-align: left; }
  .chip:hover { background: color-mix(in srgb, var(--teal) 12%, transparent); }
</style>
