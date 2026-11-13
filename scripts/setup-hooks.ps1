$ErrorActionPreference = "Stop"
$RootDir = Split-Path -Parent $PSScriptRoot
Set-Location $RootDir

git config core.hooksPath scripts/hooks
Write-Host "✅ Owlmic Git hooks activated (core.hooksPath -> scripts/hooks)." -ForegroundColor Green
