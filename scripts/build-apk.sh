#!/usr/bin/env sh
# macOS/Linux twin of build-apk.ps1: builds the Android companion into dist/IdeaAutopilot-mobile.apk.
# Needs a JDK 17+ and the Android SDK (ANDROID_HOME, or Android Studio's default folder) and Gradle 8.7 on PATH
# (or the one Android Studio downloaded into ~/.gradle).
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
m="$root/mobile"
sdk=${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}
if [ -z "$sdk" ]; then
  if [ -d "$HOME/Library/Android/sdk" ]; then sdk="$HOME/Library/Android/sdk"; else sdk="$HOME/Android/Sdk"; fi
fi
[ -d "$sdk" ] || { echo "Android SDK not found - set ANDROID_HOME" >&2; exit 1; }
printf 'sdk.dir=%s\n' "$sdk" > "$m/local.properties"
gradle=$(command -v gradle || ls "$HOME"/.gradle/wrapper/dists/gradle-8.7-bin/*/gradle-8.7/bin/gradle 2>/dev/null | head -1 || true)
[ -n "$gradle" ] || { echo "Gradle 8.7 not found - open mobile/ once in Android Studio, or install gradle" >&2; exit 1; }
(cd "$m" && "$gradle" assembleRelease --no-daemon -q)
mkdir -p "$root/dist"
cp -f "$m/app/build/outputs/apk/release/app-release.apk" "$root/dist/IdeaAutopilot-mobile.apk"
echo "APK: $root/dist/IdeaAutopilot-mobile.apk"
