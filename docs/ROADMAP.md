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
