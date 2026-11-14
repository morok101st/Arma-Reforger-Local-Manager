# Baut das Paket: EXE, Hilfs-Mod und Anleitung als ZIP unter publish.
# Aufruf: powershell -ExecutionPolicy Bypass -File scripts\build-package.ps1 -Version 0.5.0
# Braucht Rust (rustup, MSVC) und Node.js. Datei bewusst nur ASCII (Windows PowerShell 5.1).
param([Parameter(Mandatory = $true)][string]$Version)
$ErrorActionPreference = 'Stop'
if ($Version -notmatch '^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$') { throw "Ungueltige Version '$Version'" }

$repo = Split-Path $PSScriptRoot -Parent
$app = Join-Path $repo 'tauri\app'
$out = Join-Path $repo 'publish'
$folder = Join-Path $out 'ReforgerSyncLaunch'
$zip = Join-Path $out "Reforger-Sync-Launch-$Version.zip"
$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
if ((Test-Path $cargoBin) -and ($env:Path -notlike "*$cargoBin*")) { $env:Path = "$cargoBin;$env:Path" }

if (Test-Path $out) { Remove-Item $out -Recurse -Force }

# Die App zeigt RMS_VERSION an, die Windows-Dateiversion braucht reine Zahlen.
$numeric = ($Version -split '-')[0]
$env:RMS_VERSION = $Version
# Als Datei, weil PowerShell 5.1 Anfuehrungszeichen in Argumenten fuer externe Programme verschluckt.
$versionConfig = Join-Path ([IO.Path]::GetTempPath()) "tauri-version-$PID.json"
[IO.File]::WriteAllText($versionConfig, "{`"version`":`"$numeric`"}")
Push-Location $app
try {
    & npm ci --no-audit --no-fund
    if ($LASTEXITCODE -ne 0) { throw 'npm ci fehlgeschlagen' }
    & npx tauri build --no-bundle --config $versionConfig
    if ($LASTEXITCODE -ne 0) { throw 'tauri build fehlgeschlagen' }
}
finally {
    Pop-Location
    Remove-Item Env:RMS_VERSION
    Remove-Item $versionConfig -ErrorAction SilentlyContinue
}

$exe = Join-Path $repo 'tauri\target\release\ReforgerSyncLaunch.exe'
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

Write-Output "Paket: $zip"
Write-Output ("Groesse: {0:N1} MB" -f ((Get-Item $zip).Length / 1MB))
Get-ChildItem $folder -Recurse -File | ForEach-Object { "{0}  {1:N1} MB" -f $_.FullName.Substring($folder.Length + 1), ($_.Length / 1MB) }
