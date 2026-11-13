$ErrorActionPreference = "Stop"
$RootDir = Split-Path -Parent $PSScriptRoot
Set-Location $RootDir

Write-Host "=== [1/4] Verifying Owlmic Repo Invariants ===" -ForegroundColor Cyan
if (Get-Command python -ErrorAction SilentlyContinue) {
    python scripts/verify-repo-invariants.py
} elseif (Get-Command python3 -ErrorAction SilentlyContinue) {
    python3 scripts/verify-repo-invariants.py
} else {
    Write-Error "Python not found in PATH."
}
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "=== [2/4] Checking Rust Workspace (pc/) ===" -ForegroundColor Cyan
Set-Location "$RootDir\pc"
cargo fmt --all --check
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo clippy --workspace --all-targets -- -D warnings
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo test --workspace
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "=== [3/4] Checking Android Client (android/) ===" -ForegroundColor Cyan
if (Test-Path "$RootDir\android\gradlew.bat") {
    Set-Location "$RootDir\android"
    .\gradlew.bat testDebugUnitTest lintDebug
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

Set-Location $RootDir
Write-Host "=== [4/4] All Owlmic Checks Passed! ===" -ForegroundColor Green
