#!/usr/bin/env sh
# macOS/Linux twin of privacy-check.ps1: fails when a folder about to be shared contains anything personal —
# the builder's user name or paths, the git e-mail and its domain, words from scripts/privacy-words.txt, or saved data.
# Usage:  sh scripts/privacy-check.sh <folder>
set -eu
folder=${1:?usage: privacy-check.sh <folder>}
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)
words=$(mktemp)
trap 'rm -f "$words"' EXIT

user=$(id -un)
{
  printf '%s\n' "/Users/$user" "/home/$user" "$root"
  email=$(git config --get user.email 2>/dev/null || true)
  # a GitHub no-reply address is public by design; anything else (e.g. a work e-mail) must not ship
  case "$email" in
    *@users.noreply.github.com|"") ;;
    *@*) printf '%s\n' "$email" "${email#*@}" ;;
  esac
  [ -f "$here/privacy-words.txt" ] && grep -v '^[[:space:]]*$' "$here/privacy-words.txt" || true
} | sort -u > "$words"

bad=0
cd "$folder"
# saved data never ships: data/ may only hold the .keep marker
for f in $(find . -type f \( -path './data/*' ! -name .keep \) -o -type f \( -name '*.apk' -o -name '*.db' -o -name '*.sqlite' -o -name '*.db-wal' -o -name '*.db-shm' -o -name api.json \)); do
  echo "  $f : should not be shipped"; bad=1
done
# fixed-string, binary-safe search (UTF-8 only; the Windows script also checks UTF-16)
hits=$(grep -rlaF -f "$words" . 2>/dev/null || true)
if [ -n "$hits" ]; then
  echo "$hits" | while read -r f; do echo "  $f : contains a personal word or path"; done
  bad=1
fi
if [ "$bad" = 1 ]; then echo "PRIVACY CHECK FAILED"; exit 1; fi
echo "Privacy check passed: no personal names, paths or data in $folder"
