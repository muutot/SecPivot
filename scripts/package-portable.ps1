#!/usr/bin/env pwsh
# package-portable.ps1 — Build a portable (no-install) KeyVault ZIP.
#
# The app already runs portably by construction: config and vault paths are
# resolved relative to the executable's folder (see ConfigStore::load using
# current_exe().parent()). This script produces a ZIP that can be unzipped
# anywhere and run without installation (WebView2 Runtime must be present,
# as with the NSIS installer).
#
# Usage:
#   powershell -File scripts/package-portable.ps1          # full build + zip
#   powershell -File scripts/package-portable.ps1 -SkipBuild  # zip existing exe
#
# Output: dist/KeyVault-<version>-portable.zip

param(
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot
$Version = (Get-Content "$Root\package.json" -Raw | ConvertFrom-Json).version
$ReleaseExe = "$Root\src-tauri\target\release\keyvault-desktop.exe"
$OutDir = "$Root\dist\portable"
$Zip = "$Root\dist\KeyVault-$Version-portable.zip"

if (-not $SkipBuild) {
    Write-Host "[1/3] Building release bundle (tauri build)..."
    Push-Location $Root
    try {
        npm run tauri build
        if ($LASTEXITCODE -ne 0) { throw "tauri build failed with exit code $LASTEXITCODE" }
    } finally {
        Pop-Location
    }
} else {
    Write-Host "[1/3] Skipping build (using existing release exe)"
}

if (-not (Test-Path $ReleaseExe)) {
    throw "Release exe not found: $ReleaseExe (run without -SkipBuild first)"
}

Write-Host "[2/3] Staging portable folder..."
if (Test-Path $OutDir) { Remove-Item -Recurse -Force $OutDir }
New-Item -ItemType Directory -Path $OutDir | Out-Null
Copy-Item $ReleaseExe (Join-Path $OutDir "KeyVault.exe")

$Readme = @"
KeyVault v$Version - 便携版 (Portable)
========================================

解压后直接运行 KeyVault.exe,无需安装。WebView2 Runtime 需系统已安装
(Windows 10/11 通常自带)。

数据位置:
- 配置(含设置与加密的 S3 密钥):  本目录 conf/config.json
- 本地数据库:                     用户自行选择的位置
- 远端镜像:                       本目录 Storage/remote/

升级:用新版本覆盖 KeyVault.exe 即可,配置与数据不受影响。
"@
Set-Content -Path (Join-Path $OutDir "README.txt") -Value $Readme -Encoding UTF8

Write-Host "[3/3] Compressing..."
if (Test-Path $Zip) { Remove-Item -Force $Zip }
Compress-Archive -Path "$OutDir\*" -DestinationPath $Zip
$Size = [math]::Round((Get-Item $Zip).Length / 1MB, 1)
Write-Host ""
Write-Host "Portable package ready: $Zip ($Size MB)"
