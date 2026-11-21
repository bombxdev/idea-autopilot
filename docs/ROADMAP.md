# Roadmap and ideas parked for later

Things we decided to do **later**, with the moment when each one makes sense. Pull an item in when its trigger is true.
Everything here is free / open-source first, and nothing may install or share anything without the user seeing it first.

## Philosophy: many good recipes, not one right answer
Pasta can be made many ways, fried chicken from two chains tastes different and both are good. A skill is a *recipe*:
same dish, different secret method. So the program never crowns one "correct" skill. It keeps the alternatives, shows
the evidence, and lets the **user and the AI choose**.
- Skills that do the same job are grouped in a **family** (variants of one dish), not treated as duplicates.
- The bot rotates between variants to collect evidence, the user can pin a favourite, and stats are shown as
  information, never as a verdict ("people liked it" ≠ "right").
- The same idea applies to personas and roles below.

## Done in v0.3.0
- Skill library (`SKILL.md` standard), families, per-skill on/off, usage and outcome stats.
- Skill shop: search free public sources (GitHub), source and licence shown, security scan, full preview, pinned to a
  commit and a content hash, text-only install, user must confirm.

## Next (pull in when the trigger is true)
| Idea | Why | Trigger to start |
|---|---|---|
| **Bot proposes new skills** after a hard task or a correction, into a "pending" queue with a diff (approve / reject) | Hermes' best idea; knowledge from our own runs becomes reusable | After v0.3.0 has been used for ~2 weeks and the stats show skills are actually opened |
| **Bot suggests shop searches** for an idea ("this idea is a web app: 3 skills may help") — the user still approves | The shopping the owner wanted, without the AI installing anything by itself | When the scanner has run on 50+ real skills with no false negatives found |
| **Automatic A/B inside a family**: lean towards the variant that works better for this stage | Stats become decisions | A family has ≥ 4 runs per variant |
| **Roles**: a bundle = persona + skills + model tiers + stage hints ("Researcher", "Prototyper") | One click to give the bot a working style | After families have proved useful |
| **Persona**: working style/personality learned from chatting with the user, exportable | Make the bot feel like "mine" | After roles exist (a persona is a part of a role) |
| **Community sharing of roles/personas with stars** | Popular = probably good | When ≥ 20 users exist. Idea below |
| In-app auto-update | Friends get fixes without a new installer | After the GitHub repo and signing are live |

### Sharing without running a server (free)
Use GitHub itself as the registry: a public repo `idea-autopilot-recipes` where each role / persona / skill family is a
folder with a `recipe.md` + `meta.json` (author, licence, what it is for). Submit = pull request (reviewed by a human,
so nothing unsafe is listed). **Stars = GitHub stars / 👍 reactions on the PR or issue**, so "popular" is public and
needs no database. The in-app shop reads the index file, shows stars, author and licence, and runs the same scanner
before any install. Personas must be exported through a "what will be shared" preview that removes private data
(names, companies, ideas) — never shared automatically.

## Known limits (be honest with users)
- The scanner is heuristic. It blocks the obvious dangers, but the user's own reading of the preview is the real gate.
- "Skill used" is detected from the agent's log and from the agent's own report; some agents print less, so counts can
  be too low, never too high.
- Success rate shows what happened, it does not prove the skill caused it.

## Proposed 2026-11-21 — built: tools (MCP) in ISSUE-117, team mode (Claude helpers) in ISSUE-118; then tools for Codex/Gemini/OpenCode + helpers from other vendors (ia team-mcp) in ISSUE-119; tools for Mistral Vibe in ISSUE-120

### Tools (MCP) next to skills, not inside them
A **skill** is a recipe (text the AI reads). An **MCP server** is a program that gives the AI new tools (REA, a browser,
a database). A skill may *need* one (reverse-engineer-anything needs the REA MCP), but installing the skill does not
install the tool — and today the bot runs Claude with `--strict-mcp-config`, so **no MCP reaches a bot run at all**.
- One menu, two tabs: "Skills & tools" → **Skills** | **Tools (MCP)**. No new top-level menu (keeps the sidebar short).
- A skill can declare `requires: { mcp: [rea] }`; installing it offers to install the tool too, and the card shows
  "needs tool: REA (installed / not installed)".
- Tools come from the official MCP Registry (registry.modelcontextprotocol.io, free) + GitHub `topic:mcp-server`,
  same categories and sorts as the skill shop; install = a pinned command (uvx / npx / a release binary), same
  "see the command first" rule as the CLI installer.
- **Token cost**: every MCP adds its tool list to the context of every turn (REA: 200+ tools). So a run gets an MCP
  only when a skill chosen for that run needs it (`--mcp-config` built per run), never "all enabled tools always".
- Agents that are not Claude: Codex/Gemini/OpenCode each have their own MCP config file; the app writes the same
  list into each one's own format, so a tool works whichever AI does the run.

### Many models on one idea (multi-agent)
Like a team: one lead splits the work, several helpers run at the same time, the lead checks and merges.
- **Where it pays**: work that splits cleanly — research from several angles, independent modules of a prototype,
  tests + docs + code in parallel, "best of 3" drafts judged by another model. **Where it does not**: one tightly
  coupled change (helpers collide on the same files), small tasks (the split costs more than it saves).
- **Shape**: lead (strong model) writes a plan with independent parts → each part runs in its own worktree/folder
  with the account/model that is best *and has spare quota* for that kind of part (router stats already exist) →
  lead merges and verifies. Depth limit 2 (helpers may not spawn helpers), max N parallel (default 3).
- **Limits to respect**: quota burns N× faster (governor must budget the whole team, and stop all helpers at the
  ceiling); the owner's work hours stay protected; merge conflicts → the lead resolves or reruns one part; cost of
  context duplication (each helper re-reads the brief) — keep briefs short; one machine's CPU/RAM (several CLIs at
  once).
- **Setting**: per idea "Team mode: off / 2–3 helpers / max", default off; the lead decides whether a stage is worth
  splitting (a small task stays single).

## Proposed 2026-11-21 (after ISSUE-122/123) — to make it more fun and smoother
Built now: roles with personas (ISSUE-122), popular MCP tools (ISSUE-123). Next, each with the moment to start it:
| Idea | Why | Trigger to start |
|---|---|---|
| **Persona from chat**: a "make my persona" button that asks 6 short questions (tap-to-answer) and drafts the persona; refine from the owner's own chat messages (shown for approval) | The persona the owner asked for, without writing it | After a first role with a persona has run in a real team round |
| **Team presets**: one click = a whole team ("web app": front-end + back-end + tester + reviewer) from the installed roles | Low friction; the team is ready in one tap | When ≥ 5 roles are installed |
| **Per-idea team**: pick which roles join this idea (default: all "in the team") | A marketing idea does not need a back-end dev | After team mode has run on 3+ real ideas |
| **Role stats** like skill stats: parts given, finished OK, tokens per part | Recipes become decisions (philosophy above) | After 10+ team rounds |
| **Community hub** = a GitHub repo `idea-autopilot-community` with `roles/`, `skills/`; the shop lists it first; stars/👍 = popularity; sharing = a PR the app prepares | Sharing with stars, no server, free | When the first 3 friends share a role |
| **Tool bundles in roles**: a role can need tools (MCP) like skills do; the run gets them only when that role works | Each member brings its own tools | When a role needs a tool for real |
| **Command palette actions** (Ctrl+K): "new member", "install tool", "run team on #12" | Faster for power users | Any time — small |
| **Onboarding tour** (30 s, skippable) and sample idea | New users see value in the first minute | Before the next public announcement |
| **Health dashboard**: one card "everything OK?" (logins, quota reads, tools reachable, last run) with fix buttons | Fewer silent stalls (see ISSUE-056/058) | When a user reports a stall again |

