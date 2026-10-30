# Fails when a folder that is about to be shared contains anything personal: the builder's user
# name or paths, company names, the builder's own ideas, or any saved data.
# Usage:  powershell -ExecutionPolicy Bypass -File scripts\privacy-check.ps1 <folder>
# Extra words to look for (your company, your ideas) can go in scripts\privacy-words.txt, one per line
# (that file is git-ignored, so it never ships either).
param([Parameter(Mandatory = $true)][string]$Folder)
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent

# found on this machine: the Windows user, this folder, and the builder's git e-mail (the public
# developer name in the license/credits is intentional and not searched for)
$git = { param($k) (& git config --get $k 2>$null) }
$email = & $git "user.email"
$words = @("Users\$env:USERNAME", "Users/$env:USERNAME", $root, ($root -replace "\\", "/"))
# a GitHub no-reply address is public by design; anything else (e.g. a work e-mail) must not ship
if ($email -match "^(.+)@(.+)$" -and $email -notmatch "users\.noreply\.github\.com$") { $words += $email, $Matches[2] }
$userWords = Join-Path $PSScriptRoot "privacy-words.txt"
if (Test-Path $userWords) { $words += Get-Content $userWords -Encoding UTF8 | Where-Object { $_.Trim() -ne "" } }
$words = $words | Where-Object { $_ } | Select-Object -Unique

Add-Type -TypeDefinition @"
using System;
public static class Finder {
  public static bool Has(byte[] hay, byte[] needle) {
    if (needle.Length == 0 || hay.Length < needle.Length) return false;
    byte f = needle[0];
    int last = hay.Length - needle.Length;
    for (int i = 0; i <= last; i++) {
      if (hay[i] != f) continue;
      int j = 1;
      while (j < needle.Length && hay[i + j] == needle[j]) j++;
      if (j == needle.Length) return true;
    }
    return false;
  }
}
"@

$bad = @()
foreach ($file in Get-ChildItem $Folder -Recurse -File) {
  $rel = $file.FullName.Substring((Resolve-Path $Folder).Path.Length).TrimStart("\")
  # saved data never ships: data\ may only hold the .keep marker
  if ($rel -like "data\*" -and $file.Name -ne ".keep") { $bad += "$rel : saved data inside the package"; continue }
  if ($file.Extension -in ".apk", ".db", ".sqlite", ".db-wal", ".db-shm" -or $file.Name -eq "api.json") { $bad += "$rel : should not be shipped"; continue }
  $bytes = [IO.File]::ReadAllBytes($file.FullName)
  foreach ($w in $words) {
    foreach ($enc in [Text.Encoding]::UTF8, [Text.Encoding]::Unicode) {
      if ([Finder]::Has($bytes, $enc.GetBytes($w))) { $bad += "$rel : contains '$w' ($($enc.EncodingName))"; break }
    }
  }
}
if ($bad.Count) {
  Write-Host "PRIVACY CHECK FAILED" -ForegroundColor Red
  $bad | ForEach-Object { Write-Host "  $_" }
  exit 1
}
Write-Host "Privacy check passed: no personal names, paths or data in $Folder" -ForegroundColor Green
