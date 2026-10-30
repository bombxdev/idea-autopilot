# Prepares the PUBLIC copy of the project: a separate folder with its own, brand-new git history.
# The private working repo (with its history) is never pushed anywhere; only what this script
# exports can be. Nothing is published by this script — it only writes the folder and commits locally.
#
# Usage:  powershell -ExecutionPolicy Bypass -File scripts\export-public.ps1 -Target D:\path\to\idea-autopilot
# Run it again for each release: files are synced, the history of the target is kept and gets a new commit.
param(
  [Parameter(Mandatory = $true)][string]$Target,
  [string]$Author = "bombxdev",
  # a GitHub no-reply address, so the private/work e-mail never reaches the public history
  [string]$AuthorEmail = "bombxdev@users.noreply.github.com"
)
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$version = (Get-Content "$root\app\src-tauri\tauri.conf.json" -Raw | ConvertFrom-Json).version

New-Item -ItemType Directory -Force $Target | Out-Null
$Target = (Resolve-Path $Target).Path
if ($Target -eq $root) { throw "Target must not be the working repo itself." }

# wipe everything except .git, then copy what git would track (tracked + new, minus ignored files)
Get-ChildItem $Target -Force | Where-Object { $_.Name -ne ".git" } | Remove-Item -Recurse -Force
Push-Location $root
$files = git ls-files --cached --others --exclude-standard | Where-Object { $_ -and (Test-Path $_ -PathType Leaf) }
Pop-Location
foreach ($f in $files) {
  if ($f -like "*.apk" -or $f -eq "scripts/privacy-words.txt") { continue }
  $dest = Join-Path $Target $f
  New-Item -ItemType Directory -Force (Split-Path $dest) | Out-Null
  Copy-Item (Join-Path $root $f) $dest
}

# nothing personal may be in the export
& "$PSScriptRoot\privacy-check.ps1" -Folder $Target
if ($LASTEXITCODE -ne 0) { throw "Privacy check failed - nothing was committed." }

Push-Location $Target
if (-not (Test-Path ".git")) { git init -q -b main }
git config user.name $Author
git config user.email $AuthorEmail
git add -A
git commit -q -m "Idea Autopilot v$version" 2>&1 | Out-Null
git log --oneline | Select-Object -First 3
Pop-Location
Write-Host "Public copy ready in $Target (not pushed anywhere)." -ForegroundColor Green
