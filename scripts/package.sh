#!/usr/bin/env sh
# macOS/Linux twin of package.ps1: builds what you hand to other people, checked for personal data first.
#   macOS: dist/IdeaAutopilot-<version>-macos-<arch>.dmg
#   Linux: dist/IdeaAutopilot-<version>-linux-<arch>.AppImage and .deb
# The `ia` CLI ships next to the app (Tauri "externalBin"), where the app looks for it.
# Needs Rust, Node 22 and, on Linux, the WebKitGTK build packages (see docs/CROSS-PLATFORM.md).
# Usage:  sh scripts/package.sh
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
version=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["version"])' "$root/app/src-tauri/tauri.conf.json")
triple=$(rustc -vV | sed -n 's/^host: //p')
case "$(uname -s)" in
  Darwin) os=macos; bundles="app,dmg" ;;
  Linux) os=linux; bundles="appimage,deb" ;;
  *) echo "use scripts/package.ps1 on Windows" >&2; exit 1 ;;
esac
arch=$(uname -m); [ "$arch" = arm64 ] && arch=aarch64
# keep the builder's local paths (user name, folders) out of the shipped binaries
export RUSTFLAGS="--remap-path-prefix=$HOME=~ --remap-path-prefix=$root=src"
# separate target dir so these flags don't invalidate the normal dev build cache
export CARGO_TARGET_DIR="$root/app/src-tauri/target-dist"
mkdir -p "$root/dist"

cd "$root/app"
npm run build
(cd src-tauri && cargo build --release --bin ia)
# Tauri wants a sidecar named <name>-<target triple>; it drops the suffix when bundling
mkdir -p src-tauri/binaries
cp -f "$CARGO_TARGET_DIR/release/ia" "src-tauri/binaries/ia-$triple"
npx tauri build --bundles "$bundles" --config '{"bundle":{"externalBin":["binaries/ia"]}}'

out="$CARGO_TARGET_DIR/release/bundle"
stage="$root/dist/installer-check"
rm -rf "$stage"; mkdir -p "$stage"
name="IdeaAutopilot-$version-$os-$arch"
if [ "$os" = macos ]; then
  cp "$(ls -t "$out"/dmg/*.dmg | head -1)" "$root/dist/$name.dmg"
  cp -R "$(ls -dt "$out"/macos/*.app | head -1)" "$stage/"
else
  cp "$(ls -t "$out"/appimage/*.AppImage | head -1)" "$root/dist/$name.AppImage"
  cp "$(ls -t "$out"/deb/*.deb | head -1)" "$root/dist/$name.deb"
  cp "$root/dist/$name.AppImage" "$root/dist/$name.deb" "$stage/"
fi

# never hand out a package with anything personal in it
if ! sh "$root/scripts/privacy-check.sh" "$stage"; then
  rm -f "$root/dist/$name".*
  rm -rf "$stage"
  echo "Privacy check failed - nothing was packaged." >&2
  exit 1
fi
rm -rf "$stage"
ls -l "$root/dist/$name".*
