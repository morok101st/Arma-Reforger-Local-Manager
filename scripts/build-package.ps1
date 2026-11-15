# Baut unter publish den Installer (mit Signatur fuer die Update-Funktion), die Update-Beschreibung latest.json
# und das portable ZIP mit EXE, Hilfs-Mod und Anleitung.
# Signaturschluessel aus TAURI_SIGNING_PRIVATE_KEY, lokal sonst aus %USERPROFILE%\.tauri\reforger-sync-launch.key.
# Aufruf: powershell -ExecutionPolicy Bypass -File scripts\build-package.ps1 -Version 0.5.0
# Braucht Rust (rustup, MSVC) und Node.js. Datei bewusst nur ASCII (Windows PowerShell 5.1).
param([Parameter(Mandatory = $true)][string]$Version)
$ErrorActionPreference = 'Stop'
if ($Version -notmatch '^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$') { throw "Ungueltige Version '$Version'" }

$repo = Split-Path $PSScriptRoot -Parent
$app = Join-Path $repo 'app'
$out = Join-Path $repo 'publish'
$folder = Join-Path $out 'ReforgerSyncLaunch'
$zip = Join-Path $out "Reforger-Sync-Launch-$Version.zip"
$repository = 'https://github.com/morok101st/Arma-Reforger-Local-Manager'
$setupName = "Reforger-Sync-Launch-$Version-Setup.exe"
$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
if ((Test-Path $cargoBin) -and ($env:Path -notlike "*$cargoBin*")) { $env:Path = "$cargoBin;$env:Path" }

if (Test-Path $out) { Remove-Item $out -Recurse -Force }

# Ohne Schluessel entsteht kein Update, das die installierten Apps annehmen.
$keyFile = Join-Path $env:USERPROFILE '.tauri\reforger-sync-launch.key'
$keyFromFile = $false
if (-not $env:TAURI_SIGNING_PRIVATE_KEY) {
    if (-not (Test-Path $keyFile)) { throw "Kein Signaturschluessel: TAURI_SIGNING_PRIVATE_KEY setzen oder $keyFile anlegen" }
    $env:TAURI_SIGNING_PRIVATE_KEY = [IO.File]::ReadAllText($keyFile).Trim()
    $keyFromFile = $true
}
if (-not $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD) { $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = '' }
$bundleDir = Join-Path $repo 'target\release\bundle\nsis'
if (Test-Path $bundleDir) { Remove-Item $bundleDir -Recurse -Force }

# Die App zeigt RMS_VERSION an, die Windows-Dateiversion braucht reine Zahlen.
$numeric = ($Version -split '-')[0]
$env:RMS_VERSION = $Version
# Als Datei, weil PowerShell 5.1 Anfuehrungszeichen in Argumenten fuer externe Programme verschluckt.
$versionConfig = Join-Path ([IO.Path]::GetTempPath()) "rms-version-$PID.json"
[IO.File]::WriteAllText($versionConfig, "{`"version`":`"$numeric`"}")
Push-Location $app
try {
    & npm ci --no-audit --no-fund
    if ($LASTEXITCODE -ne 0) { throw 'npm ci fehlgeschlagen' }
    & npx tauri build --config $versionConfig
    if ($LASTEXITCODE -ne 0) { throw 'tauri build fehlgeschlagen' }
}
finally {
    Pop-Location
    Remove-Item Env:RMS_VERSION
    if ($keyFromFile) { Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY }
    Remove-Item $versionConfig -ErrorAction SilentlyContinue
}

$exe = Join-Path $repo 'target\release\ReforgerSyncLaunch.exe'
$exeVersion = (Get-Item $exe).VersionInfo.ProductVersion
if ($exeVersion -cne $numeric) { throw "Die EXE meldet Version '$exeVersion' statt '$numeric'" }
# Die angezeigte Version steht als Text in der EXE. Ein Paket mit falscher Angabe darf nicht entstehen.
$bytes = [IO.File]::ReadAllBytes($exe)
if ([Text.Encoding]::ASCII.GetString($bytes).IndexOf($Version, [StringComparison]::Ordinal) -lt 0) {
    throw "Die EXE enthaelt die Version '$Version' nicht"
}

New-Item -ItemType Directory -Force $folder | Out-Null
Copy-Item $exe $folder
Copy-Item (Join-Path $repo 'helper') (Join-Path $folder 'helper') -Recurse
Copy-Item (Join-Path $repo 'packaging\LIES-MICH.txt') (Join-Path $folder 'LIES-MICH.txt')
Compress-Archive -Path $folder -DestinationPath $zip

# Installer und Signatur. Tauri benennt sie nach dem Produktnamen, mit Leerzeichen und "&".
$setup = @(Get-ChildItem $bundleDir -Filter '*-setup.exe')
if ($setup.Count -ne 1) { throw "Erwartet genau einen Installer in $bundleDir" }
$signature = "$($setup[0].FullName).sig"
if (-not (Test-Path $signature)) { throw 'Die Signatur des Installers fehlt' }
$setupFile = Join-Path $out $setupName
Copy-Item $setup[0].FullName $setupFile
# latest.json beschreibt das Update fuer die installierten Apps. Sie liegt in jedem Release, die Apps holen
# sie ueber releases/latest/download/latest.json.
$latest = [ordered]@{
    version   = $Version
    notes     = "$repository/releases/tag/v$Version"
    pub_date  = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
    platforms = [ordered]@{
        'windows-x86_64' = [ordered]@{
            signature = [IO.File]::ReadAllText($signature).Trim()
            url       = "$repository/releases/download/v$Version/$setupName"
        }
    }
}
[IO.File]::WriteAllText((Join-Path $out 'latest.json'), ($latest | ConvertTo-Json -Depth 4))

Write-Output "Installer: $setupFile"
Write-Output ("Groesse: {0:N1} MB" -f ((Get-Item $setupFile).Length / 1MB))

Write-Output "Paket: $zip"
Write-Output ("Groesse: {0:N1} MB" -f ((Get-Item $zip).Length / 1MB))
Get-ChildItem $folder -Recurse -File | ForEach-Object { "{0}  {1:N1} MB" -f $_.FullName.Substring($folder.Length + 1), ($_.Length / 1MB) }
