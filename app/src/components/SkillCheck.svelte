<script>
  import Icon from "./Icon.svelte";
  // Deep check of a skill text with SkillSpector (NVIDIA). Runs by itself when the text changes and
  // reports the combined verdict (built-in scan + SkillSpector) back through `result`.
  import { api } from "../lib/api.js";

  let { text, result = $bindable(null) } = $props();
  let busy = $state(false);
  let err = $state("");
  let seq = 0;

  async function run(t) {
    const mine = ++seq;
    busy = true;
    err = "";
    result = null;
    try {
      const r = await api.skillDeepCheck(t);
      if (mine === seq) result = r;
    } catch (e) {
      if (mine === seq) err = String(e);
    }
    if (mine === seq) busy = false;
  }
  $effect(() => {
    const t = text;
    if (t) run(t);
  });

  const LABEL = { pass: ["ผ่านการตรวจ", "teal", "check"], caution: ["ควรระวัง", "amber", "warning"], fail: ["ไม่ผ่าน", "red", "close"] };
  const SEV = { CRITICAL: "red", HIGH: "red", MEDIUM: "amber", LOW: "faint" };
</script>

<div class="sc">
  <div class="row wrap">
    <b class="small"><Icon name="shieldCheck" /> SkillSpector (NVIDIA)</b>
    {#if busy}<span class="faint small">กำลังตรวจเชิงลึก… (ประมาณ 15 วินาที)</span>{/if}
    {#if result?.ss}
      <span class="chip {LABEL[result.ss.status]?.[1]}">{#if LABEL[result.ss.status]}<Icon name={LABEL[result.ss.status][2]} /> {/if}{LABEL[result.ss.status]?.[0] ?? result.ss.status}</span>
      <span class="faint small">คะแนนเสี่ยง {result.ss.score}/100 · v{result.ss.version}{result.ss.complete ? "" : " · ตรวจไม่ครบทุกไฟล์"}</span>
    {:else if result && !result.ss_available}
      <span class="faint small">ยังไม่ได้ติดตั้ง — ใช้ตัวสแกนในโปรแกรมอย่างเดียว (ติดตั้งได้ที่แถบด้านบนของหน้านี้)</span>
    {/if}
  </div>
  {#if err}
    <div class="small" style="color:var(--red)">{err}</div>
    <button class="small" onclick={() => run(text)}>ตรวจอีกครั้ง</button>
  {/if}
  {#each result?.ss?.issues ?? [] as i}
    <div class="iss">
      <div><b class={SEV[i.severity]}>{i.severity}</b> · {i.category} — {i.pattern}</div>
      {#if i.finding}<code class="small">{i.finding}</code>{/if}
      {#if i.fix}<div class="small faint">แนวทางแก้: {i.fix}</div>{/if}
    </div>
  {/each}
</div>

<style>
  .sc { display: flex; flex-direction: column; gap: 6px; background: var(--bg-2); border-radius: 8px; padding: 8px 12px; }
  .iss { font-size: 13px; display: flex; flex-direction: column; gap: 2px; border-top: 1px solid var(--line, rgba(128,128,128,.2)); padding-top: 4px; }
  .iss code { color: var(--text-2); word-break: break-all; }
  .red { color: var(--red); }
  .amber { color: var(--amber); }
  .faint { color: var(--text-3); }
</style>
