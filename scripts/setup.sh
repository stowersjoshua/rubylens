#!/usr/bin/env bash
# One-time setup for contributors (human or agent).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

git config core.hooksPath .githooks
echo "✓ git hooks enabled (.githooks/commit-msg rejects AI attribution)"

npm install --silent --no-audit --no-fund
echo "✓ lint tooling installed"

cargo build --release --quiet
echo "✓ rbview built (target/release/rbview)"

missing=()
command -v ruby >/dev/null || missing+=("ruby (golden + corpus syntax checks)")
command -v rust-analyzer >/dev/null || missing+=("rust-analyzer: rustup component add rust-analyzer rust-src")
if [ ${#missing[@]} -gt 0 ]; then
  echo "optional, for scripts/check.sh --full:"
  printf '  - %s\n' "${missing[@]}"
fi
echo "next: scripts/check.sh"
