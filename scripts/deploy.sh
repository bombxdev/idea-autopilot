#!/usr/bin/env sh
# macOS/Linux twin of deploy.ps1: rebuild and hot-swap the app in bin/, then start it again.
# Waits for the bot to finish its current stage so no work is cut off mid-way.
# Usage:  sh scripts/deploy.sh [--no-build] [--minimized]
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
bin="$root/bin"
ia="$bin/ia"
build=1
minimized=""
for a in "$@"; do
  case "$a" in
    --no-build) build=0 ;;
    --minimized) minimized="--minimized" ;;
    *) echo "unknown option: $a" >&2; exit 2 ;;
  esac
done

if [ "$build" = 1 ]; then
  (cd "$root/app" && npm run build)
  (cd "$root/app/src-tauri" && cargo build --release --features tauri/custom-protocol)
fi

# wait (max 60 min) until no stage is running
deadline=$(( $(date +%s) + 3600 ))
while pgrep -x idea-autopilot >/dev/null 2>&1 && [ -x "$ia" ]; do
  cur=$("$ia" status --json 2>/dev/null | python3 -c 'import json,sys
try:
    c = json.load(sys.stdin).get("current")
except Exception:
    c = None
print("%s (%s)" % (c.get("title"), c.get("stage")) if c else "")' 2>/dev/null || true)
  [ -z "$cur" ] && break
  if [ "$(date +%s)" -gt "$deadline" ]; then echo "still running after 60 min - swapping anyway"; break; fi
  echo "waiting for the bot to finish: $cur"
  sleep 20
done

pkill -x idea-autopilot 2>/dev/null || true
sleep 2
mkdir -p "$bin"
cp -f "$root/app/src-tauri/target/release/idea-autopilot" "$root/app/src-tauri/target/release/ia" "$bin/"
nohup "$bin/idea-autopilot" $minimized >/dev/null 2>&1 &
echo "updated and restarted: $bin/idea-autopilot"
