# Builds the OpenDrape Windows installer (per-user, no admin rights) and a portable ZIP.
# Usage: scripts/package-windows.ps1 [name-suffix]
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.*)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$suffix = if ($args.Count -gt 0) { $args[0] } else { $version }

cargo build --release -p opendrape
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
cargo packager --release -p opendrape --formats nsis
if ($LASTEXITCODE -ne 0) { throw "cargo packager failed" }

New-Item -ItemType Directory -Force dist | Out-Null
$installer = Get-ChildItem target/release -Recurse -Filter '*-setup.exe' | Sort-Object LastWriteTime | Select-Object -Last 1
Copy-Item $installer.FullName "dist/OpenDrape-$suffix-windows-x64-setup.exe"

$portable = 'target/portable/OpenDrape'
Remove-Item -Recurse -Force $portable -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $portable | Out-Null
Copy-Item target/release/opendrape.exe "$portable/OpenDrape.exe"
Copy-Item LICENSE, docs/PORTABLE.txt $portable
Compress-Archive -Path $portable -DestinationPath "dist/OpenDrape-$suffix-windows-x64-portable.zip" -Force
Get-ChildItem dist
