# Contributing

Thanks for helping! Idea Autopilot is GPL-3.0-or-later; by sending a change you agree it is released under the same license.

- **Bugs / ideas:** open an issue. For a bug, say which AI agent you use (Claude Code, Codex, Gemini CLI, OpenCode, custom), your Windows version, and what the app's Activity page ("บันทึก") shows.
- **Code:** `cd app && npm install && npm run build`, then `cd src-tauri && cargo test`. UI preview without Tauri: `npm run dev` (sample data).
- **Every fix or improvement** adds an entry to `docs/ISSUES.md` (symptom / cause / fix / files) and, if users will notice it, a line in `CHANGELOG.md`. Releases bump the version in `app/src-tauri/tauri.conf.json`, `app/src-tauri/Cargo.toml` and `app/package.json` together.
- **Privacy:** never commit anything from a real data folder (ideas, `data/`, `brain/`, tokens) or machine-specific paths. `scripts/privacy-check.ps1` must pass before you share a build.
- **New AI agents:** add them in `app/src-tauri/src/providers.rs` (command line, model tiers, whether effort can be set).
