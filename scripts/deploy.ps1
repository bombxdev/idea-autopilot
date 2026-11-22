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

# tell the watcher (lifeline.rs) this stop is on purpose, so it does not report it or open the old copy again
if (Test-Path "$root\data") { Add-Content -Path "$root\data\app-life.log" -Value "$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') exit deploy" -Encoding ASCII }
Get-Process idea-autopilot -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep 2
# a file can stay locked a few seconds after the stop (a helper `ia` still closing): retry, and whatever happens
# start the app again - a failed copy used to end the script with the app closed until morning (ISSUE-144)
$copied = $false
try {
    for ($i = 1; $i -le 15 -and -not $copied; $i++) {
        try {
            Get-Process idea-autopilot, ia -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$bin\*" } | Stop-Process -Force
            Copy-Item "$root\app\src-tauri\target\release\idea-autopilot.exe", "$root\app\src-tauri\target\release\ia.exe" $bin -Force -ErrorAction Stop
            $copied = $true
        } catch {
            Write-Host "copy try $i failed: $($_.Exception.Message)"
            Start-Sleep 2
        }
    }
} finally {
    if ($Minimized) { Start-Process "$bin\idea-autopilot.exe" -ArgumentList "--minimized" } else { Start-Process "$bin\idea-autopilot.exe" }
}
if ($copied) { Write-Host "updated and restarted: $bin\idea-autopilot.exe" } else { Write-Host "copy FAILED - restarted the previous version: $bin\idea-autopilot.exe"; exit 1 }
