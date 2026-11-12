#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "=== [1/4] Verifying Owlmic Repo Invariants ==="
if python -c "import sys" >/dev/null 2>&1; then
    python scripts/verify-repo-invariants.py
elif py -3 -c "import sys" >/dev/null 2>&1; then
    py -3 scripts/verify-repo-invariants.py
elif python3 -c "import sys" >/dev/null 2>&1; then
    python3 scripts/verify-repo-invariants.py
fi

echo "=== [2/4] Checking Rust Workspace (pc/) ==="
cd "$ROOT_DIR/pc"
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

echo "=== [3/4] Checking Android Client (android/) ==="
if [ -d "$ROOT_DIR/android" ] && [ -f "$ROOT_DIR/android/gradlew" ]; then
    cd "$ROOT_DIR/android"
    ./gradlew testDebugUnitTest lintDebug
fi

echo "=== [4/4] All Owlmic Checks Passed! ==="
