---
name: verify-changes
description: Use before committing or declaring any change to rbview done, or when a check in scripts/check.sh fails and you need to know what it means and how to fix it properly (without weakening the check).
---

# Verify a change

## Pick the checks

| Changed | Run |
|---|---|
| docs only | `scripts/check.sh` (markdown links, spec ↔ golden linkage) |
| anything else | `scripts/check.sh` |
| `src/emit.rs`, `src/out.rs`, `src/lib.rs`, `editors/vscode/srcmap.js` | `scripts/check.sh --full` |
| how the demo looks | also `tools/capture/capture.sh` (see the `update-extension` skill) |
| a check itself (`scripts/check.sh`, `tests/golden.rs`, corpus or guard scripts) | commit, then `scripts/check-the-checks.sh`, which tests the last commit |

First run in a fresh clone? `scripts/setup.sh` enables the git hooks and installs lint deps.

## Read failures correctly

| Failing step | Meaning | Proper fix |
|---|---|---|
| rustfmt | formatting drift | `cargo fmt` |
| clippy | lint | fix the code; add an `#[allow]` only with a comment saying why |
| golden snapshots differ | output changed | if intended: `UPDATE_GOLDEN=1 cargo test --test golden` and review `git diff tests/golden`; if not: you broke something |
| map: wrong identifier | a token maps to a different Rust identifier | an intended rename goes in the tables in `src/emit.rs`; otherwise see the `debug-navigation` skill |
| map: coverage below 95% | Rust identifiers became unreachable | emit them with `span_tok`/`src_tokens`; never lower `MIN_COVERAGE` |
| not valid Ruby | a rendering broke Ruby syntax | fix it; mark the case `rbview-golden: ruby-invalid (reason)` only for genuine Ruby limitations (decision 0002) |
| spec linkage | a golden case is uncited, or a citation is missing | cite or add it in `docs/spec/translation.md` |
| attribution guards / commits | a commit message credits an AI tool | `git commit --amend` (or rebase) to remove the lines |
| .vsix matches source | the extension source changed without a rebuild | rebuild (the step prints the command) |
| demo view up to date | output changed | regenerate `examples/demo/shapes.rb` (the step prints the command) |
| markdown links | a relative link points nowhere | fix the path |
| corpus | rendering regressed on real code | `scripts/corpus-check.py --show 20` for examples; fix the rule |
| navigation | jumps land wrong against real rust-analyzer | `debug-navigation` skill |

## Never

- **Lower a threshold** (`MIN_COVERAGE`, corpus limits, `--min 100`) to make a change pass, unless the PR argues the trade explicitly.
- **Update golden snapshots** without reading the diff.
- **Skip `--full`** after changing the emitter or position lookup.
- **Report "checks pass"** without running them. Quote the summary line.
