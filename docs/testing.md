# Testing

## Which checks to run

| You changed | Run |
|---|---|
| anything | `scripts/check.sh` (fast, about a minute) |
| `src/emit.rs`, `src/out.rs`, `src/lib.rs` | `scripts/check.sh --full` |
| `editors/vscode/srcmap.js` or `extension.js` | `scripts/check.sh --full`, rebuild the `.vsix`, and regenerate media if the UI changed |
| anything for speed | `scripts/bench-corpus.sh --digest` before and after (output must be identical), then `scripts/check.sh --full` ([performance](performance.md)) |
| how the demo looks | `tools/capture/capture.sh`, then review every still |

`scripts/check.sh` runs every step and then reports. Setup is one command: `scripts/setup.sh`.

## What each check protects

| Check | Protects | Where |
|---|---|---|
| rustfmt, clippy `-D warnings`, ESLint | style and common mistakes | `rustfmt.toml`, `eslint.config.mjs` |
| **golden snapshots** | the [translation spec](spec/translation.md): every rule's output, byte for byte | `tests/golden/*.rs` → `*.rb` |
| **map invariants** | bounds, identity and coverage of the [source map](spec/source-map.md) | `tests/golden.rs` |
| **Ruby syntax** | views parse with `ruby -c`, except cases marked `rubylens-golden: ruby-invalid` | `tests/golden.rs` |
| **spec linkage** | every golden case is specified, and every spec citation exists | `tests/golden.rs` ↔ `docs/spec/translation.md` |
| **cache** | views are reused exactly when still right (source, generator and symbol kinds unchanged), builds are deterministic, parallel readers never see half a file | `tests/cache.rs` |
| **server** | `rubylens serve` answers what `rubylens gen --json` prints, matches parallel responses by id, follows `symbols.json`, survives bad requests, exits on EOF, idle or a replaced binary | `tests/serve.rs` |
| extension (mock) | providers map positions both ways, references across files share one request, lookups stay cheap in huge views, views still open when the server dies or doesn't exist | `editors/vscode/test/run.js` |
| **generation scales linearly** | 8x the input takes < 20x the time, for the shapes that have been superlinear before (deep nesting, long lines, long matches, …) | `scripts/scaling-check.py` |
| attribution guards | the commit-msg hook and Claude Code hook still block AI attribution | `scripts/guard-selftest.sh` |
| repo rules | no tracked build output, `.vsix` matches source, demo view fresh, markdown links resolve, no attribution in unpushed commits | `scripts/check.sh` |
| **corpus** (`--full`) | 2,789 files from 66 pinned crates: no crashes, ≥ 97.7% valid Ruby, ≤ 0.01% identifier mismatches, ≥ 98.8% of Rust identifiers reachable | `scripts/corpus-check.py`, `tests/corpus/` |
| **navigation** (`--full`) | against a real rust-analyzer, 100% of resolved definitions, references and outline symbols land on the right Ruby token | `scripts/lsp-check.sh` |
| media capture | the real extension in real VS Code | `tools/capture/` |

## Golden tests

```sh
cargo test --test golden                      # check
UPDATE_GOLDEN=1 cargo test --test golden      # rewrite snapshots after an intended change
git diff tests/golden                         # review the snapshot diff like code
```

A golden case is a small, plausible Rust file that is parsed but never compiled. Name it after the area it covers. Add the marker comment `// rubylens-golden: ruby-invalid (reason)` only for a known limitation; the test fails if such a case becomes valid, so remove the marker when you fix it.

## Thresholds

The corpus thresholds in `scripts/corpus-check.py` sit just below the measured baseline. **Raise them when you improve things. Never lower them to make a change pass** unless the PR explains why the trade is right ([docs/intent.md](intent.md) principles first). The same goes for `MIN_COVERAGE` in `tests/golden.rs`, `--min 100` in `scripts/lsp-check.sh`, `MAX_RATIO` in `scripts/scaling-check.py` and the lookup-scaling limit in `editors/vscode/test/run.js`.

## Checking the checks

`scripts/check-the-checks.sh` injects realistic mistakes into a throwaway clone and requires each to be caught by the check that claims it. Examples: an unmapped token, a rename hidden in code, an uncited golden case, a stale `.vsix`, an attributed commit or an AI author identity, a tracked cache folder, a broken link, identifiers that silently stop mapping, a stale cached view, half-written cache files, and quadratic generation or lookups. It tests the last commit, so commit first. **Run it after changing any check**, and add a case when you add a check.

## Requirements

| Tool | Needed for |
|---|---|
| Rust stable with rustfmt and clippy | everything |
| Node 20+, npm | ESLint, extension tests |
| Ruby 3.x | `ruby -c` checks (golden tests skip them without Ruby; the corpus check requires it) |
| `rustup component add rust-analyzer rust-src` | `scripts/lsp-check.sh` |
| code-server, ffmpeg, a Chromium | `tools/capture/` only |
