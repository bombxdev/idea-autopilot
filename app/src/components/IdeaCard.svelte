<script>
  import Icon from "./Icon.svelte";
  import { store, questionsFor, stageLabel, progress, stepGroups } from "../lib/store.svelte.js";
  import { ago, clock, STATUS } from "../lib/api.js";
  import Intents from "./Intents.svelte";
  import Question from "./Question.svelte";
  import Actions from "./Actions.svelte";

  let { idea, showQuestions = true, showActions = true, compact = false } = $props();
  const qs = $derived(questionsFor(idea.id));
  const running = $derived(store.state?.current?.idea_id === idea.id);
  // which AI is thinking about / working on this idea right now: the bot's run, or an open chat (ISSUE-065)
  const cur = $derived(running ? store.state.current : null);
  const chat = $derived((store.state?.sessions ?? []).find((x) => x.idea_id === idea.id));
  const usingNow = $derived(
    cur ? { text: [cur.account || cur.provider, cur.model].filter(Boolean).join(" · "), why: "AI ที่บอทใช้คิด/ทำไอเดียนี้อยู่ตอนนี้" }
    : chat?.ai ? { text: chat.ai, why: "AI ในแชตที่เปิดคุยกับไอเดียนี้อยู่" } : null,
  );
  const pinned = $derived(store.state?.idea_ai?.[idea.id]);
  // the account this idea waits for hit its usage limit: say why and until when
  const limited = $derived(pinned?.account ? store.state?.account_limits?.[pinned.account] : null);
  const queuedNow = $derived(!running && store.state?.force === idea.id);
  const stage = $derived(idea.status === "inbox" ? "clarify" : idea.recipe[idea.stage_idx]);
  const st = $derived(STATUS[idea.status] ?? { label: idea.status, color: "blue" });
</script>

<article class="card idea rise" class:running class:ask={qs.length > 0}>
  <div class="row top">
    <button class="title ghost" onclick={() => (store.openIdea = idea.id)}>
      <span class="idn" title="เลขไอเดีย ใช้อ้างอิงเวลาสั่งงาน">#{idea.id}</span>{#if idea.priority > 0}<span class="urgent">ด่วน</span>{/if}{idea.title}
    </button>
    {#if idea.verdict}
      <span class="chip v-{idea.verdict.toLowerCase()}"><Icon name={idea.verdict === "GO" ? "check" : idea.verdict === "NO-GO" ? "close" : "approx"} /> {idea.verdict === "GO" ? "GO" : idea.verdict === "NO-GO" ? "NO-GO" : "MAYBE"}</span>
    {/if}
    <span class="chip s-{st.color}">{#if running}<Icon name="dot" size="0.8em" /> กำลังทำ{:else if queuedNow}<Icon name="bolt" /> คิวถัดไป{:else}{st.label}{/if}</span>
  </div>

  <div class="row wrap meta">
    <Intents intents={idea.intents} />
    {#if idea.scope === "work"}<span class="chip" title="เรื่องงานบริษัท — เก็บแยกในชุดงาน"><Icon name="briefcase" /> งาน</span>{/if}
    {#if idea.research_only}<span class="chip">research อย่างเดียว</span>{/if}
    {#if limited}<span class="chip amber" title={limited.why}><Icon name="hourglass" /> ติดลิมิต · รอถึง {clock(limited.until)}</span>{/if}
    {#if usingNow}<span class="chip ai-now" title={usingNow.why}><Icon name={cur ? "bot" : "chat"} /> {usingNow.text}</span>
    {:else if pinned && (pinned.account || pinned.model || pinned.effort)}<span class="chip" title="AI ที่เลือกไว้สำหรับไอเดียนี้"><Icon name="bot" /> {[pinned.account, pinned.model, pinned.effort].filter(Boolean).join(" · ")}</span>{/if}
    {#if idea.knowledge}<span class="chip"><Icon name="book" /> มีในคลัง</span>{/if}
  </div>

  {#if idea.summary}
    <p class="summary">{idea.summary}</p>
  {:else if idea.status === "inbox"}
    <p class="summary faint">{queuedNow ? "สั่งทำเลยแล้ว — รอรอบงานที่รันอยู่จบก่อน แล้วจะเริ่มทันที" : "รอ AI ตีความ… (พักสั้นๆ ให้แก้ข้อความก่อน แล้วรอโควต้าว่าง)"}</p>
  {/if}

  <div class="prog"><div class="pbar"><i style="width:{progress(idea)}%"></i></div><span class="small faint">{progress(idea)}%</span></div>

  {#if idea.recipe.length && !compact}
    <div class="steps">
      {#each stepGroups(idea.recipe, idea.stage_idx, idea.status === "active") as g}
        <span class="step" class:done={g.done} class:now={g.now}>
          {stageLabel(g.key)}{#if g.count > 1} ×{g.count}{:else if g.now && idea.repeat_count > 0} ×{idea.repeat_count + 1}{/if}
        </span>
      {/each}
    </div>
  {/if}

  {#if idea.needs.length && !compact}
    <div class="needs small">
      <b>ต้องการจากคุณ:</b>
      <ul>{#each idea.needs as n}<li>{n}</li>{/each}</ul>
    </div>
  {/if}

  {#if showQuestions && qs.length}
    <div class="qs">{#each qs as q (q.id)}<Question {q} />{/each}</div>
  {/if}

  {#if showActions}
    <Actions {idea} compact={compact} />
  {/if}
  <div class="faint small foot">{running && (compact || !idea.recipe.length) ? `ขั้น: ${stageLabel(stage)}` : `อัปเดต ${ago(idea.updated)}`}</div>
</article>

<style>
  .idea { display: flex; flex-direction: column; gap: 10px; transition: border-color .2s; }
  .idea.running { border-color: var(--teal); }
  .idea.ask { border-color: var(--amber); }
  .top { align-items: flex-start; }
  .title { flex: 1; text-align: left; border: none; padding: 0; font-size: 16px; font-weight: 600; line-height: 1.4; }
  .title:hover { background: none; color: var(--teal); }
  .urgent { font-size: 11px; background: var(--red-bg); color: var(--red); border-radius: 6px; padding: 1px 6px; margin-right: 6px; vertical-align: 2px; }
  .summary { margin: 0; color: var(--text-2); }
  .prog { display: flex; align-items: center; gap: 8px; }
  .pbar { flex: 1; height: 6px; background: var(--bg-2); border-radius: 6px; overflow: hidden; }
  .pbar i { display: block; height: 100%; background: var(--teal); transition: width .4s; }
  .steps { display: flex; flex-wrap: wrap; gap: 4px; }
  .step { font-size: 12px; padding: 2px 9px; border-radius: 6px; background: var(--bg-2); color: var(--text-3); }
  .step.done { color: var(--teal); background: var(--teal-bg); }
  .step.now { color: var(--text); background: var(--card-2); outline: 1px solid var(--teal); }
  .qs { display: flex; flex-direction: column; gap: 8px; }
  .needs { background: var(--blue-bg); color: var(--text); padding: 8px 12px; border-radius: 8px; }
  .needs ul { margin: 4px 0 0; padding-left: 18px; }
  .foot { margin-top: -4px; }
  .v-go { background: var(--teal-bg); color: var(--teal); }
  .v-no-go { background: var(--red-bg); color: var(--red); }
  .v-maybe { background: var(--amber-bg); color: var(--amber); }
  .s-teal { background: var(--teal-bg); color: var(--teal); }
  .s-blue { background: var(--blue-bg); color: var(--blue); }
  .s-purple { background: var(--purple-bg); color: var(--purple); }
  .s-red { background: var(--red-bg); color: var(--red); }
</style>
