#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

git config core.hooksPath scripts/hooks
chmod +x scripts/hooks/pre-commit scripts/hooks/commit-msg scripts/check.sh 2>/dev/null || true
echo "✅ Owlmic Git hooks activated (core.hooksPath -> scripts/hooks)."
