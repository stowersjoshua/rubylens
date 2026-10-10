#!/usr/bin/env bash
# Navigation check against a real rust-analyzer (editors/vscode/test/lsp-e2e.js): clicks every
# sampled identifier in the Ruby view and requires 100% of resolved jumps, references and
# outline symbols to land on the right Ruby token.
#
#   scripts/lsp-check.sh            (RUST_ANALYZER=<path> to override; rust-src recommended)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
RA="${RUST_ANALYZER:-$(command -v rust-analyzer || true)}"
[ -x "$RA" ] || { echo "lsp-check needs rust-analyzer: rustup component add rust-analyzer rust-src (or set RUST_ANALYZER)" >&2; exit 1; }
RUBYLENS="$ROOT/target/release/rubylens"
# Always rebuild: testing a stale binary after editing src/ would report a false OK.
cargo build --release --quiet --manifest-path "$ROOT/Cargo.toml"
E2E="$ROOT/editors/vscode/test/lsp-e2e.js"
run() { echo "── $1"; shift; node "$E2E" --ra "$RA" --rubylens "$RUBYLENS" --min 100 "$@" | grep -E "clicked|landed correctly|^     |References|Outline|OK:|FAIL"; }

cargo fetch --quiet --manifest-path "$ROOT/tools/capture/workspace/Cargo.toml"
run "demo crate"            --project "$ROOT/examples/demo" --file src/shapes.rs
run "app with dependencies" --project "$ROOT/tools/capture/workspace" --file src/main.rs --file src/shapes.rs
run "rubylens itself"         --project "$ROOT" --file src/emit.rs --file src/lib.rs --file src/out.rs --file src/main.rs --limit 500
