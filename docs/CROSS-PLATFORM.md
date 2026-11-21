# Idea Autopilot on Windows, macOS and Linux

The desktop app is Tauri 2 + Rust, so the same code builds on all three. This page lists every Windows-only piece
that was found in `app/src-tauri/src` (audit of 2026-11-21) and what each one does on macOS/Linux now.

## Build on macOS / Linux

| | macOS | Linux (Debian/Ubuntu) |
|---|---|---|
| tools | Xcode Command Line Tools (`xcode-select --install`), Rust, Node 22 | Rust, Node 22 |
| system packages | — | `sudo apt install build-essential pkg-config libssl-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev` |
| dev build + run | `sh scripts/deploy.sh` (builds, waits for the bot to be idle, swaps `bin/`, restarts) | same |
| package to share | `sh scripts/package.sh` → `dist/IdeaAutopilot-<v>-macos-<arch>.dmg` | `sh scripts/package.sh` → `.AppImage` + `.deb` |
| phone app (Android) | `sh scripts/build-apk.sh` | same |

All free. The macOS `.dmg` is not signed or notarized (that needs a paid Apple Developer account): the first time,
right-click the app → Open. The `ia` CLI ships next to the app (Tauri `externalBin`), where the app looks for it.

CI: `.github/workflows/bundles.yml` builds Windows x64 and Windows ARM64 (two separate NSIS installers; the ARM64 one
runs on GitHub's free `windows-11-arm` runner, which exists for public repositories only), macOS (Apple silicon + Intel `.dmg`) and Linux
(`.AppImage`, `.deb`) on every `v*` tag or by hand (Actions → Bundles → Run workflow); the files are kept as run
artifacts. The same workflow builds the iPhone shell (`mobile-ios/`) into an unsigned `.ipa` on a macOS runner (see
docs/IOS.md). On Windows ARM64 the add-on downloads only use a recipe's `windows-aarch64` entry; none exist yet, so
the video add-on uses ffmpeg on PATH and offers no speech engine there. `.github/workflows/ci.yml` runs the Rust tests on all three systems.

## Audit: Windows-only code and what happens elsewhere

| # | Where | Windows-only thing | macOS / Linux now |
|---|---|---|---|
| 1 | many (`runner`, `agents`, `providers`, `docs`, `video`, …) | `creation_flags(0x0800_0000)` (no console window) | already `#[cfg(windows)]` — nothing to hide elsewhere |
| 2 | `providers.rs`, `agents.rs` | `where` to find a CLI, `.exe/.cmd/.bat` lookup | already `which` / bare name off Windows |
| 3 | `runner.rs` `kill_tree` | `taskkill /T` (whole tree) | **fixed:** was `kill -9 <pid>` only (children of the AI CLI kept running) → now finds the tree with `pgrep -P` and kills all |
| 4 | `runner.rs` shell line | `cmd /C` | already `sh -c` |
| 5 | `commands.rs` ▶ run prototype | `cmd /c start … cmd /k` window | **fixed:** opens Terminal.app (macOS, `osascript`) or the first of x-terminal-emulator / gnome-terminal / konsole / xfce4-terminal / xterm (Linux); the window stays open |
| 6 | `commands.rs` account login | `cmd /C start` login window | **fixed:** same terminal helper, env vars and args shell-quoted (was "Windows only" error) |
| 7 | `update.rs` self-update | PowerShell script swaps files; release assets are `-Setup.exe` / `-portable.zip` | **explained + guarded:** off Windows the mode is `os-package` → the app only tells you about a new version and opens the Releases page (no Windows file is ever downloaded on a Mac). In-place update for .dmg/.AppImage is future work |
| 8 | `remote.rs`, `skill.rs`, `team.rs`, `video.rs` | `ia.exe` next to the app | **fixed:** `ia` (via `video::exe`) |
| 9 | `remote.rs` SessionStart hook | `"shell": "powershell"` command | **fixed:** off Windows the hook is a plain `sh` command (`cat "$IDEA_AUTOPILOT_BRIEF"`) |
| 10 | `tool_recipes.json` | one download per tool, all Windows x86_64 | **fixed:** `platforms` entries with their own url + sha256 for uv 0.12.10 and transcribe.cpp 0.4.2 (macos-aarch64, macos-x86_64, linux-x86_64, linux-aarch64). A system without an entry gets a clear message, never the Windows file |
| 11 | `tool_recipes.json` ffmpeg | gyan.dev Windows zip | **explained:** no free, versioned, long-lived static build exists for macOS arm64/Linux (BtbN prunes old builds; evermeet is Intel only) → the app uses ffmpeg on PATH (`brew install ffmpeg` / `apt install ffmpeg`) and says so |
| 12 | `speech_engine.rs`, `video.rs` | `transcribe.dll` | **fixed:** `libtranscribe.dylib` on macOS (was looking for `.so`), `libtranscribe.so` on Linux |
| 13 | `video.rs` unpack | Windows `tar` reads .zip | **fixed:** on Linux a .zip goes to `unzip` (GNU tar can't read zip); macOS bsdtar reads both |
| 14 | `agents.rs` Orca | `%LOCALAPPDATA%\Programs\orca\…\orca.exe` | **fixed:** that path only on Windows; elsewhere `orca` on PATH |
| 15 | `mcp.rs` | `cmd /c npx` | already `npx` off Windows |
| 16 | `accounts.rs` Codex sandbox | `[windows] sandbox` | already Windows-only by design |
| 17 | `lib.rs` restart / power | `DETACHED_PROCESS`, `windows-sys` power API | already `#[cfg(windows)]` (Cargo dep is `cfg(windows)` only) |
| 18 | `scripts/*.ps1` | PowerShell | **added:** `deploy.sh`, `package.sh`, `privacy-check.sh`, `build-apk.sh`. `export-public.ps1` stays Windows (maintainer-only publishing tool) |
| 19 | `tauri.conf.json` | `"targets": ["nsis"]` | left as the default for Windows; the scripts and CI pass `--bundles` per system |

## What was verified

- Windows: `cargo check --all-targets`, `cargo test --lib`, `npm run build`.
- Linux (Ubuntu 24.04 in WSL2): `cargo check --all-targets` and `cargo test --lib` natively.
- macOS: `cargo check --target aarch64-apple-darwin --all-targets` passes — it type-checks the Rust code (C parts stubbed; no link). The real
  macOS build, the Terminal.app launch and the `.dmg` are first proven by the `Bundles` workflow on a macOS runner.
- Not tried on a real Mac or Linux desktop: tray icon, global shortcut, notifications, the terminal windows, the
  speech add-on actually transcribing. Its libraries were checked to find their neighbours by themselves when
  loaded by full path (`RUNPATH $ORIGIN` on Linux, `@loader_path` on macOS); the Linux build also needs the Vulkan
  loader `libvulkan1`.
