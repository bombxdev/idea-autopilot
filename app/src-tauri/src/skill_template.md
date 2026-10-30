---
name: idea-autopilot
description: Control the user's Idea Autopilot app (bot that spends spare Claude quota researching/building their ideas). Use when the user wants to jot/add an idea ("จดไอเดีย", "ฝากไอเดีย", "อยากรู้ว่า…ทำได้ไหม"), check what the bot did or is doing ("บอททำอะไรอยู่", "เมื่อคืนได้อะไรบ้าง", "สรุปเช้า"), answer the bot's questions, steer an idea (go on / dig deeper / try another approach / stop), read an idea's report, play a finished prototype, see the history/progress log, pause/resume/turbo the bot, check quota plan, or set a holiday/work day.
---
<!-- managed by Idea Autopilot: rewritten on every app start with this machine's paths -->

# Idea Autopilot control

The app runs on this Windows PC and is controlled with its CLI:

```
"{IA}" <command>
```

(Use the full quoted path; in Bash use forward slashes: `{IA_SLASH}`.) Output is Thai text; add `--json` when you need structured data.

| User wants | Command |
|---|---|
| Status / what's running / quota | `ia status` |
| Add an idea | `ia add "<idea text in the user's words>"` — prefix `!` for urgent, `?` for research-only |
| List ideas | `ia list` (`--all` includes archived) |
| Details, questions, needs-from-user | `ia show <id>` |
| Read the report | `ia report <id>` (or `knowledge`, `plan`, `progress`, `run` as 2nd arg) |
| Open questions | `ia questions` |
| Answer | `ia answer <qid> <option number or free text>` |
| Steer | `ia go <id> [note]`, `ia deeper <id> [note]`, `ia alt <id> [note]`, `ia say <id> "<instruction>"`, `ia stop <id>`, `ia urgent <id>`, `ia archive <id>`, `ia retry <id>`, `ia now <id>` (run immediately) |
| Have the AI change/continue an idea right now (live session `Idea#<id>`, reused if already open) | `ia ask <id> "<what to change, in the user's words>"` — `ia remote <id> off` closes it |
| Play a finished prototype | `ia play <id>` (opens on the PC screen) |
| Bot control | `ia pause`, `ia resume`, `ia stop` (stop current stage), `ia turbo [hours]` (`0` = off) |
| Morning summary | `ia digest` |
| History: what ran when, results, why the bot was idle, queue progress/ETA | `ia log [days]` (default 1) |
| Full log of one run | `ia runlog <run id>` |
| Holiday / extra work day | `ia day YYYY-MM-DD off|work|half|clear [note]` |
| Show a page on the PC | `ia ui morning|board|quota|library|settings [id]` |

## How to respond

- Keep it short and in Thai. When the user only describes an idea, just `ia add` it with their wording — don't interrogate them; the bot asks its own tap-to-answer questions later.
- For "what happened overnight", run `ia digest` and `ia questions`, then summarise: what's ready to play, verdicts, and open questions with numbered options so the user can reply with just a number.
- When the user replies with an option number to a question you showed, call `ia answer <qid> <number>`.
- To summarise a report, run `ia report <id>` and give the "สรุป 30 วินาที" section plus the verdict; offer to read more.
- If the CLI says the app isn't running, tell the user the PC app is closed (it starts with Windows; executable `{APP}`) and offer to start it.
- Never edit `data/quota.db` or files under `workspace/` directly to control the bot — always go through `ia`.
