<script>
  // A feed line from the AI: ``` code blocks as a box with a copy button, `inline code`, **bold**, [links](…) and
  // | tables | (ISSUE-156); the rest as before (emoji drawn as the app's icons). A block cut off at the end (a long
  // message) still shows as code (ISSUE-073).
  import IconText from "./IconText.svelte";
  import { toast } from "../lib/store.svelte.js";
  import { openUrl } from "@tauri-apps/plugin-opener";

  // onlink(href): a link to a file of the idea (relative or a full path) — opened like a report's link (ISSUE-148)
  let { text = "", onlink = null } = $props();

  const isRow = (l) => /^\s*\|.*\|\s*$/.test(l);
  const isSep = (l) => /^\s*\|(\s*:?-{2,}:?\s*\|)+\s*$/.test(l);
  const cells = (l) => l.trim().replace(/^\||\|$/g, "").split(/(?<!\\)\|/).map((c) => c.trim().replace(/\\\|/g, "|"));

  // plain text → its prose and tables (a header row, then |---|, then rows)
  function split(t) {
    const out = [];
    const lines = t.split("\n");
    let prose = [];
    for (let i = 0; i < lines.length; i++) {
      if (isRow(lines[i]) && i + 1 < lines.length && isSep(lines[i + 1])) {
        // the table is a block of its own: no blank lines on either side of it
        if (prose.length) out.push({ t: prose.join("\n").replace(/\n+$/, "") });
        prose = [];
        const head = cells(lines[i]);
        const rows = [];
        i += 2;
        while (i < lines.length && isRow(lines[i])) rows.push(cells(lines[i++]));
        i--;
        out.push({ head, rows });
        while (i + 1 < lines.length && !lines[i + 1].trim()) i++;
      } else prose.push(lines[i]);
    }
    if (prose.length) out.push({ t: prose.join("\n") });
    return out;
  }

  const blocks = $derived.by(() => {
    const out = [];
    const re = /\x60{3}([\w+-]*)[ \t]*\n?([\s\S]*?)(?:\x60{3}|$)/g;
    let last = 0;
    let m;
    while ((m = re.exec(text)) && m[0].length) {
      if (m.index > last) out.push(...split(text.slice(last, m.index)));
      out.push({ code: m[2].replace(/\s+$/, ""), lang: m[1] });
      last = re.lastIndex;
    }
    if (last < text.length) out.push(...split(text.slice(last)));
    return out;
  });
  // (written as \x60 / char 96: a literal backtick here confuses the i18n extractor)
  const bits = (t) => t.split(/(\x60[^\x60\n]+\x60|\*\*[^*\n]+\*\*|\[[^\]\n]+\]\([^)\s]+\))/).filter((p) => p !== "");
  const TICK = String.fromCharCode(96);
  const link = (p) => p.match(/^\[([^\]\n]+)\]\(([^)\s]+)\)$/);

  function go(e, href) {
    e.preventDefault();
    e.stopPropagation();
    if (/^(https?:|mailto:)/i.test(href) && !/^https?:\/\/tauri\.localhost/i.test(href)) openUrl(href);
    else if (onlink) onlink(href);
  }

  async function copy(c) {
    try {
      await navigator.clipboard.writeText(c);
      toast("คัดลอกแล้ว");
    } catch {
      toast("คัดลอกไม่ได้", "err");
    }
  }
</script>

{#snippet inline(t)}{#each bits(t) as p}{#if p.length > 2 && p.startsWith(TICK) && p.endsWith(TICK)}<code>{p.slice(1, -1)}</code>{:else if p.length > 4 && p.startsWith("**") && p.endsWith("**")}<b>{p.slice(2, -2)}</b>{:else if link(p)}<a href={link(p)[2]} title={link(p)[2]} onclick={(e) => go(e, link(p)[2])}>{link(p)[1]}</a>{:else}<IconText text={p} />{/if}{/each}{/snippet}

{#each blocks as b}{#if b.code !== undefined}<span class="cb"><span class="bar"><span class="lang">{b.lang}</span><button type="button" class="cp" onclick={() => copy(b.code)}>คัดลอก</button></span><pre>{b.code}</pre></span>{:else if b.head}<span class="tw"><table><thead><tr>{#each b.head as c}<th>{@render inline(c)}</th>{/each}</tr></thead><tbody>{#each b.rows as r}<tr>{#each r as c}<td>{@render inline(c)}</td>{/each}</tr>{/each}</tbody></table></span>{:else}{@render inline(b.t)}{/if}{/each}

<style>
  .cb { display: block; margin: 4px 0; border: 1px solid var(--line); border-radius: 8px; background: var(--card-2); overflow: hidden; }
  .bar { display: flex; align-items: center; justify-content: space-between; padding: 2px 6px 2px 10px; border-bottom: 1px solid var(--line); font-size: 11px; color: var(--text-3); }
  .cp { font-size: 11px; padding: 1px 8px; border: none; background: transparent; color: var(--teal); }
  pre { margin: 0; padding: 6px 10px; white-space: pre-wrap; word-break: break-all; font: 12px/1.45 ui-monospace, Consolas, monospace; color: var(--text); }
  code { font: 12px ui-monospace, Consolas, monospace; background: var(--card-2); border-radius: 4px; padding: 0 4px; }
  a { color: var(--blue, var(--teal)); text-decoration: underline; cursor: pointer; word-break: break-all; }
  .tw { display: block; margin: 6px 0; overflow-x: auto; white-space: normal; }
  table { border-collapse: collapse; font-size: 13px; line-height: 1.45; }
  th, td { border: 1px solid var(--line); padding: 4px 10px; text-align: left; vertical-align: top; }
  th { background: var(--card-2); font-weight: 600; }
</style>
