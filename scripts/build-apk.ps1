# Builds the Android companion app into dist\IdeaAutopilot-mobile.apk
# Needs Android Studio (bundled JDK + SDK). Uses the Windows certificate store so it works behind TLS inspection.
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$m = "$root\mobile"
$env:JAVA_HOME = "C:\Program Files\Android\Android Studio\jbr"
$sdk = "$env:LOCALAPPDATA\Android\Sdk"
Set-Content "$m\local.properties" ("sdk.dir=" + $sdk.Replace("\", "/")) -Encoding ascii

# Java ignores the Windows store; build a truststore with its roots once
$ts = "$m\.truststore.jks"
if (-not (Test-Path $ts)) {
    Copy-Item "$env:JAVA_HOME\lib\security\cacerts" $ts
    $tmp = Join-Path $env:TEMP "ia-certs"; New-Item -ItemType Directory -Force $tmp | Out-Null
    foreach ($c in @(Get-ChildItem Cert:\LocalMachine\Root) + @(Get-ChildItem Cert:\CurrentUser\Root) + @(Get-ChildItem Cert:\LocalMachine\CA)) {
        $f = "$tmp\$($c.Thumbprint).cer"
        if (Test-Path $f) { continue }
        [IO.File]::WriteAllBytes($f, $c.Export('Cert'))
        & "$env:JAVA_HOME\bin\keytool.exe" -importcert -noprompt -keystore $ts -storepass changeit -alias "win-$($c.Thumbprint)" -file $f 2>$null | Out-Null
    }
    Remove-Item $tmp -Recurse -Force
}
$tsPath = $ts.Replace("\", "/")
# the truststore path is machine-specific: pass it on the command line, never write it into the
# committed gradle.properties (it would leak this folder's path into the public repo)
$trust = @("-Dorg.gradle.jvmargs=-Xmx2g -Djavax.net.ssl.trustStore=$tsPath -Djavax.net.ssl.trustStorePassword=changeit",
           "-Djavax.net.ssl.trustStore=$tsPath", "-Djavax.net.ssl.trustStorePassword=changeit")

$gradle = Get-ChildItem "$env:USERPROFILE\.gradle\wrapper\dists\gradle-8.7-bin\*\gradle-8.7\bin\gradle.bat" | Select-Object -First 1
if (-not $gradle) { throw "Gradle 8.7 not found - open mobile\ once in Android Studio to download it" }
Push-Location $m
& $gradle.FullName assembleRelease --no-daemon -q @trust
if ($LASTEXITCODE) { throw "gradle build failed" }
Pop-Location
New-Item -ItemType Directory -Force "$root\dist" | Out-Null
Copy-Item "$m\app\build\outputs\apk\release\app-release.apk" "$root\dist\IdeaAutopilot-mobile.apk" -Force
Write-Host "APK: $root\dist\IdeaAutopilot-mobile.apk"
