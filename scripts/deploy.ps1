# Rebuild and hot-swap the installed app in bin\, then start it again.
# Waits for the bot to finish its current stage so no work is cut off mid-way.
# Usage:  powershell -ExecutionPolicy Bypass -File scripts\deploy.ps1 [-NoBuild] [-Minimized]
param([switch]$NoBuild, [switch]$Minimized)
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$bin = "$root\bin"
$ia = "$bin\ia.exe"
$env:NODE_OPTIONS = "--use-system-ca"

if (-not $NoBuild) {
    Push-Location "$root\app"; npm run build; if ($LASTEXITCODE) { throw "frontend build failed" }
    Push-Location src-tauri; cargo build --release --features tauri/custom-protocol; if ($LASTEXITCODE) { throw "cargo build failed" }
    Pop-Location; Pop-Location
}

# wait (max 60 min) until no stage is running
$deadline = (Get-Date).AddMinutes(60)
while ((Get-Process idea-autopilot -ErrorAction SilentlyContinue) -and (Test-Path $ia)) {
    $cur = $null
    try { $cur = (& $ia status --json | ConvertFrom-Json).current } catch {}
    if (-not $cur) { break }
    if ((Get-Date) -gt $deadline) { Write-Host "still running after 60 min - swapping anyway"; break }
    Write-Host "waiting for the bot to finish: $($cur.title) ($($cur.stage))"
    Start-Sleep 20
}

Get-Process idea-autopilot -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep 2
Copy-Item "$root\app\src-tauri\target\release\idea-autopilot.exe", "$root\app\src-tauri\target\release\ia.exe" $bin -Force
if ($Minimized) { Start-Process "$bin\idea-autopilot.exe" -ArgumentList "--minimized" } else { Start-Process "$bin\idea-autopilot.exe" }
Write-Host "updated and restarted: $bin\idea-autopilot.exe"
