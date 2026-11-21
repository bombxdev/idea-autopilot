# Idea Autopilot on iPhone / iPad

The phone app is one web page (`mobile/app/src/main/assets/index.html`) that talks to GitHub (the sync repo).
Android wraps it in a tiny WebView shell (`MainActivity.java`). For iOS there are three ways; the plan picks the
free ones first.

## Options compared

| | A. Home-screen web app (recommended) | B. Swift WKWebView shell (`mobile-ios/`) | C. Tauri 2 mobile target |
|---|---|---|---|
| cost | free | free to build and put on **your own** iPhone; **App Store / TestFlight need the Apple Developer Program, US$99/year** | same as B |
| needs a Mac | no | **yes** (Xcode is macOS-only) | **yes** |
| effort | done (manifest + safe-area CSS) | done (one Swift file + XcodeGen spec); build on a Mac | high: the phone page is not the Tauri app, it would have to be ported into a Tauri project with Rust built for iOS |
| updates | the page updates when the hosted copy does | rebuild in Xcode | rebuild |
| limits | needs the page hosted on HTTPS; no native bridge | with a free Apple ID the app **stops opening after 7 days** (re-install from Xcode), max 3 such apps | same as B |

Decision: **A for everyone now** (free, no Mac, same page as Android), **B for whoever has a Mac** and wants a
native app icon/shell, **C not worth it** — it would rebuild what B gives for far more work.

## A. Home-screen web app

1. Host the folder `mobile/app/src/main/assets/` on any free HTTPS static host — e.g. GitHub Pages of a public repo
   (Settings → Pages), or Cloudflare Pages. The page holds no secrets: the GitHub token is typed on the phone and
   kept in that phone's storage only.
2. On the iPhone open the page in **Safari** → Share → **Add to Home Screen**.
3. It opens full-screen with its own icon, clear of the notch and home bar.

Done in this repo: `manifest.webmanifest`, `icon-192.png`, Apple web-app meta tags, `env(safe-area-inset-*)` padding
(0 on Android, so nothing changes there). Checked: the page loads and lays out at phone size in a browser. Not
checked on a real iPhone.

## B without a Mac: build in GitHub Actions

`.github/workflows/bundles.yml` (job `ios`) builds the shell on a free GitHub macOS runner and keeps
`IdeaAutopilot-unsigned.ipa` as a run artifact (Actions → Bundles → Run workflow, or push a `v*` tag). It is unsigned:
open it in **Sideloadly** or **AltStore** on Windows, sign in with a free Apple ID, and it installs on your iPhone
(7-day limit as below). macOS runner minutes are free for public repositories; a private repository spends its
monthly allowance 10× faster on macOS.

Another no-Mac route, not set up here: **xtool** (open source, runs in WSL) builds and installs iOS apps from
Windows, but needs Xcode.xip (downloaded with a free Apple ID) and a SwiftPM package instead of the XcodeGen spec.

## B. Native shell on a Mac (free for your own device)

```sh
brew install xcodegen          # free
cd mobile-ios && xcodegen      # makes IdeaAutopilot.xcodeproj
open IdeaAutopilot.xcodeproj
```

In Xcode: target → Signing & Capabilities → Team = your Apple ID ("Personal Team", free), plug in the iPhone, Run.
On the phone: Settings → General → VPN & Device Management → trust your developer certificate. With a free Apple ID
the install lasts 7 days; run it from Xcode again after that.

What the shell does (`mobile-ios/IdeaAutopilot/App.swift`), matching the Android shell:
- loads the same `assets/index.html` (the folder is referenced, not copied — one page for both phones);
- keeps the page inside the safe area; links open in Safari;
- `IAApp.fontScale()` from the iPhone's text size, like Android;
- shows the page's `confirm()` / `prompt()` dialogs (WKWebView shows none by itself);
- photo picker: WKWebView's own (it hands HEIC photos over as JPEG, so no conversion code is needed).

Not built or run here (this machine is Windows). Still to add on a Mac if wanted: a 1024×1024 app icon in an asset
catalog (only a 192 px icon exists today).

Publishing to the App Store or TestFlight needs the paid Apple Developer Program (US$99/year). That breaks the
free-first rule, so it is not planned.
