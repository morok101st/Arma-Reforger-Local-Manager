# Baut das Paket fuer Tester: eigenstaendige EXE, Hilfs-Mod und Anleitung als ZIP.
# Aufruf: powershell -ExecutionPolicy Bypass -File scripts\build-package.ps1 -Version 0.1.0
# Datei bewusst nur ASCII (Windows PowerShell 5.1).
param([string]$Version = '0.1.0')
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$out = Join-Path $repo 'publish'
$folder = Join-Path $out 'ReforgerSyncLaunch'
$zip = Join-Path $out "Reforger-Sync-Launch-$Version.zip"

if (Test-Path $out) { Remove-Item $out -Recurse -Force }

& dotnet publish (Join-Path $repo 'src\ReforgerModSync.App\ReforgerModSync.App.csproj') `
    -c Release -r win-x64 --self-contained true --nologo -v q `
    -p:PublishSingleFile=true `
    -p:IncludeNativeLibrariesForSelfExtract=true `
    -p:DebugType=none `
    -p:Version=$Version `
    -o $folder
if ($LASTEXITCODE -ne 0) { throw 'dotnet publish fehlgeschlagen' }

Copy-Item (Join-Path $repo 'packaging\LIES-MICH.txt') $folder
Compress-Archive -Path $folder -DestinationPath $zip

Write-Output "Paket: $zip"
Write-Output ("Groesse: {0:N0} MB" -f ((Get-Item $zip).Length / 1MB))
Get-ChildItem $folder -Recurse -File | ForEach-Object { "{0}  {1:N1} MB" -f $_.FullName.Substring($folder.Length + 1), ($_.Length / 1MB) }
