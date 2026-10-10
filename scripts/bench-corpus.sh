#!/usr/bin/env bash
# `rubylens bench` over the pinned 66-crate corpus (tests/corpus). Extra arguments go to bench:
#
#   scripts/bench-corpus.sh --repeat 3 --top 10                 # where the time goes
#   scripts/bench-corpus.sh --repeat 3 --digest /tmp/after.txt  # prove output unchanged
#   RUBYLENS=target/debug/rubylens scripts/bench-corpus.sh --top 0   # another binary (e.g. debug checks)
#
# See docs/performance.md.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MANIFEST="$ROOT/tests/corpus/Cargo.toml"
if [ -z "${RUBYLENS:-}" ]; then
  cargo build --release --quiet --manifest-path "$ROOT/Cargo.toml"
  RUBYLENS="$ROOT/target/release/rubylens"
fi
cargo fetch --quiet --manifest-path "$MANIFEST"
mapfile -t CRATES < <(cargo metadata --format-version 1 --manifest-path "$MANIFEST" | python3 -c '
import json, os, sys
for p in json.load(sys.stdin)["packages"]:
    if (p.get("source") or "").startswith("registry"):
        print(os.path.dirname(p["manifest_path"]))
' | sort)
exec "$RUBYLENS" bench "${CRATES[@]}" "$@"
