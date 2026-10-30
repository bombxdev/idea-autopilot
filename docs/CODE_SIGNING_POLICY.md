# Code signing policy

Free code signing provided by [SignPath.io](https://signpath.io), certificate by [SignPath Foundation](https://signpath.org).

Windows installers and executables of Idea Autopilot that are published as GitHub releases are signed. Only builds produced from the public source in this repository by the project's GitHub Actions workflow are submitted for signing.

## Roles
- **Author / committer / reviewer / approver:** [bombxdev](https://github.com/bombxdev) (project owner). Every signing request is approved by the owner. Contributors' changes are merged by the owner after review.

## Privacy policy
Idea Autopilot is a local desktop program. It has no telemetry and no server of its own. It does not send your ideas or files anywhere by itself. Network access happens only through things you set up:
- **The AI agent you choose** (Claude Code, Codex CLI, Gemini CLI, OpenCode or your own command) receives the prompts the program runs, under that tool's own terms and privacy policy.
- **Claude plan usage:** if Claude Code is your agent, the program reads your plan's usage numbers from Anthropic's usage endpoint with the login Claude Code already stored on your machine (read-only).
- **Telegram (optional):** if you enter a bot token, progress messages and ideas you type there pass through Telegram.
- **GitHub sync (optional):** if you configure a sync folder that is a git repository, the program pushes to the repository you chose.

Everything the program saves (ideas, reports, settings) stays in your data folder on your computer.

## Signing requests
The workflow submits only artifacts built from a tagged release of this repository. No third-party binaries are signed.
