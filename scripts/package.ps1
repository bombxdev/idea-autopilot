# Builds what you hand to other people, both checked for personal names, paths and data first:
#   dist\IdeaAutopilot-<version>-Setup.exe        the installer (updates in place; ideas and settings are kept)
#   dist\IdeaAutopilot-<version>-portable.zip     unzip-and-run copy that starts empty
#   dist\IdeaAutopilot-<version>-mobile.apk       the phone app, same version (if Android Studio is installed)
# Needs Windows 10/11 build tools (Rust, Node). Usage:
#   powershell -ExecutionPolicy Bypass -File scripts\package.ps1
# Code signing (optional, removes the "unknown publisher" warning): set IA_SIGN_THUMBPRINT to the
# thumbprint of a code-signing certificate in your Windows certificate store before running.
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$version = (Get-Content "$root\app\src-tauri\tauri.conf.json" -Raw | ConvertFrom-Json).version
$env:NODE_OPTIONS = "--use-system-ca"
# keep the builder's local paths (user name, folders) out of the shipped binaries
$env:RUSTFLAGS = "--remap-path-prefix=$env:USERPROFILE=~ --remap-path-prefix=$root=src"
# separate target dir so these flags don't invalidate the normal dev build cache
$env:CARGO_TARGET_DIR = "$root\app\src-tauri\target-dist"
New-Item -ItemType Directory -Force "$root\dist" | Out-Null

# ---- build: the CLI first (the installer bundles it), then app + installer ----
Push-Location "$root\app"
npm run build
Push-Location src-tauri
cargo build --release --bin ia
Pop-Location
# the CLI is bundled into the installer only for this build (a resource that must exist at compile
# time would break plain dev builds if it lived in tauri.conf.json)
$bundle = @{ resources = @{ "target-dist/release/ia.exe" = "ia.exe" } }
if ($env:IA_SIGN_THUMBPRINT) {
  $bundle.windows = @{ certificateThumbprint = $env:IA_SIGN_THUMBPRINT; digestAlgorithm = "sha256"; timestampUrl = "http://timestamp.digicert.com" }
  Write-Host "Signing with certificate $env:IA_SIGN_THUMBPRINT"
} else {
  Write-Host "No IA_SIGN_THUMBPRINT set: the installer will be unsigned (Windows SmartScreen will warn)." -ForegroundColor Yellow
}
$cfg = (@{ bundle = $bundle } | ConvertTo-Json -Compress -Depth 6).Replace('"', '\"')
$tauriArgs = @("tauri", "build", "--bundles", "nsis", "--config", $cfg)
npx @tauriArgs
Pop-Location

# ---- installer ----
$setup = Get-ChildItem "$env:CARGO_TARGET_DIR\release\bundle\nsis\*setup.exe" | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $setup) { throw "Installer not found after the build." }
$setupOut = "$root\dist\IdeaAutopilot-$version-Setup.exe"
Copy-Item $setup.FullName $setupOut -Force

# ---- portable zip ----
$out = "$root\dist\IdeaAutopilot"
Remove-Item $out -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force "$out\bin", "$out\data" | Out-Null
Copy-Item "$env:CARGO_TARGET_DIR\release\idea-autopilot.exe", "$env:CARGO_TARGET_DIR\release\ia.exe" "$out\bin\"
Copy-Item "$root\README.md", "$root\CHANGELOG.md", "$root\scripts\START-HERE.txt", "$root\docs\ISSUES.md" "$out\"
# an empty data\ folder marks the portable layout: everything stays inside this folder
Set-Content "$out\data\.keep" "Idea Autopilot data lives here"

# never hand out a package with anything personal in it
& "$PSScriptRoot\privacy-check.ps1" -Folder $out
if ($LASTEXITCODE -ne 0) { throw "Privacy check failed - nothing was packaged." }
$stage = "$root\dist\installer-check"
Remove-Item $stage -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item $setupOut $stage
& "$PSScriptRoot\privacy-check.ps1" -Folder $stage
$ok = $LASTEXITCODE -eq 0
Remove-Item $stage -Recurse -Force
if (-not $ok) { Remove-Item $setupOut -Force; throw "Privacy check failed on the installer." }

$zip = "$root\dist\IdeaAutopilot-$version-portable.zip"
Remove-Item $zip -Force -ErrorAction SilentlyContinue
Compress-Archive -Path $out -DestinationPath $zip
$sig = (Get-AuthenticodeSignature $setupOut).Status
Write-Host "Installer: $setupOut  (signature: $sig)"
Write-Host "Portable : $zip"

# ---- phone app: the same version as the PC (build.gradle reads tauri.conf.json), for the same release ----
$apk = "$root\dist\IdeaAutopilot-$version-mobile.apk"
& powershell -ExecutionPolicy Bypass -File "$PSScriptRoot/build-apk.ps1"
if ($LASTEXITCODE) { Write-Warning "Phone app not built (Android Studio missing?) - the release has no APK this time" }
else {
  Copy-Item "$root\dist\IdeaAutopilot-mobile.apk" $apk -Force
  Write-Host "Phone app: $apk"
}
