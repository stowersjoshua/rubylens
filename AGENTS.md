# AGENTS.md

Guidance for anyone, human or AI agent, changing this repository. Read it before your first change. It's short on purpose; follow the links when a task touches that area.

## What this is

rbview shows Rust source as a read-only Ruby projection, so Rubyists can read Rust. Every identifier in the view maps back to its Rust token, and navigation (definition, hover, references) is answered by rust-analyzer. **Read [docs/intent.md](docs/intent.md) first.** Its principles decide every trade-off below.

## Repository map

| Path | What |
|---|---|
| `src/emit.rs` | Rust AST (`syn`) → Ruby text + position map. Most changes land here. The rename tables are at the top |
| `src/out.rs` | output buffer: UTF-16 position tracking, segments, verbatim lines |
| `src/range.rs` | source ranges of items, statements and arms read off the syntax tree (for the block map) |
| `src/lib.rs` / `src/main.rs` | `generate()`, the `.rbview/` cache, symbols / the CLI (`gen`, `build`, `bench`) |
| `src/bench.rs` | `rbview bench`: in-process phase timings and output digests ([performance](docs/performance.md)) |
| `editors/vscode/` | VS Code extension: `extension.js` (providers), `srcmap.js` (position lookup), prebuilt `.vsix`, tests |
| `tests/golden/` | golden cases (`*.rs` → `*.rb`): the executable translation spec |
| `tests/corpus/` | the pinned 66-crate regression corpus (fetched, never built) |
| `docs/` | [intent](docs/intent.md), [specs](docs/spec/), [decision records](docs/decisions/), [testing](docs/testing.md), [performance](docs/performance.md) |
| `scripts/` | `check.sh` (run before committing), `setup.sh`, corpus/LSP checks, attribution guard |
| `tools/capture/` | regenerates `docs/media/` screenshots and the GIF in real VS Code |
| `.claude/` | shared Claude Code settings, hooks and skills |

## Non-negotiables

1. **Navigation never lies.** Every Rust-derived token goes through `span_tok`, `node_tok`, `src_tokens` or `Out::verbatim`, never `push`. Jumps must land on the right token at 100% (`scripts/lsp-check.sh`).
2. **The generator is syntax-only.** No name, type or trait resolution in `src/`; rust-analyzer answers those ([0001](docs/decisions/0001-syntax-only-generator.md)).
3. **Faithful over pretty.** If there's no honest Ruby rendering, keep the Rust verbatim and mapped. Never invent behaviour ([0002](docs/decisions/0002-verbatim-over-mistranslation.md)).
4. **Renames live in tables.** `METHOD_RENAMES`, `MACRO_RENAMES`, `KEYWORD_RENAMES` and `DROPPED_METHODS` in `src/emit.rs`, never ad-hoc `match` arms ([0005](docs/decisions/0005-single-source-tables.md)).
5. **Deterministic output.** No network calls, model calls, randomness or time in `src/` ([0006](docs/decisions/0006-deterministic-core.md)).
6. **Spec, golden test and code change together.** A translation change without a golden case and a spec row fails `cargo test`.
7. **Never lower a threshold to make a change pass.** Coverage, corpus and navigation thresholds only go up, unless a PR argues the trade explicitly ([docs/testing.md](docs/testing.md#thresholds)).
8. **No AI attribution in commits or PRs.** No `Co-Authored-By` trailers for AI tools, no "Generated with …" lines, no session links. Hooks enforce this (see [Commits and PRs](#commits-and-prs)).

## Making a change

1. Find the governing spec: [translation](docs/spec/translation.md), [source map](docs/spec/source-map.md), [cache](docs/spec/cache.md) or [extension](docs/spec/extension.md). If your change contradicts it or a decision record, update those first, or stop and ask.
2. For translation changes, **write or extend a golden case first** (`tests/golden/<area>.rs`), then change `src/emit.rs`.
3. Run `UPDATE_GOLDEN=1 cargo test --test golden`, then **review `git diff tests/golden` line by line**. An unexplained diff is a bug.
4. Run `scripts/check.sh`. If you touched `src/emit.rs`, `src/out.rs` or `editors/vscode/srcmap.js`, run `scripts/check.sh --full`.
5. Update docs that state what you changed: the spec row, README tables or numbers, and a decision record if you made a real choice.

Skills in `.claude/skills/` walk through the common cases step by step: `change-translation`, `debug-navigation`, `update-extension` and `verify-changes`. They're plain markdown, so any agent can follow them.

## Commands

```sh
scripts/setup.sh                                  # once: git hooks, lint deps, build
scripts/check.sh                                  # before every commit
scripts/check.sh --full                           # + 66-crate corpus + real rust-analyzer navigation
UPDATE_GOLDEN=1 cargo test --test golden          # rewrite golden snapshots (then review the diff)
scripts/check-the-checks.sh                       # after changing any check: proves each still catches its mistake
cargo build --release && ./target/release/rbview gen path/to/file.rs   # print one view
./target/release/rbview gen path/to/file.rs --json                     # view + map
```

## Commits and PRs

- **Branch:** `master` is the default and only long-lived branch. Never create or push `main`.
- **Messages:** imperative subject under about 72 characters, and a body that explains *why*. Write as the repository owner. No AI attribution of any kind.
- **Guards** (all run by `scripts/check.sh`):
  - `.githooks/commit-msg` rejects attribution in any commit. Run `scripts/setup.sh` to enable it.
  - `.claude/settings.json` turns off Claude Code's attribution, and its PreToolUse hook blocks `git commit` and `gh pr` commands containing attribution.
  - `scripts/attribution-check.sh --range` checks every commit in a CI range.
- **Don't commit** `target/`, `node_modules/` or `.rbview/` (enforced), or scratch files.
- **The `.vsix`** (`editors/vscode/rbview-0.1.0.vsix`) is what users install. Rebuild it whenever `extension.js`, `srcmap.js` or the extension's `package.json` change (enforced).
- **PRs** fill in `.github/pull_request_template.md`.

## Gotchas

- **`push` versus `span_tok`.** Text written with `push` is unmapped. Use it only for Ruby keywords, punctuation and comment markers.
- **Duplicated tokens.** If a Rust token appears twice in the view (e.g. the signature comment and `def`), only the code occurrence may be mapped (`src_tokens_except`). Otherwise jumps land in comments.
- **Keyword spans.** Map keywords to their own token (`return_token`, `question_token`), not the whole expression; multi-line spans degrade to one character.
- **The cache invalidates itself** when anything in `src/` changes (`build.rs` fingerprint). Don't add manual version bumps. `GEN_VERSION` is only for map format changes.
- **The demo view** `examples/demo/shapes.rb` must be regenerated after output changes (enforced).
- **Formatting:** rustfmt uses `max_width = 120`. Run `cargo fmt` rather than hand-wrapping.
- **The live Claude Code hook** blocks any Bash command that contains both `git commit` and attribution text, even a test. Keep such text in fixture files (see `scripts/fixtures/`).
- **No VS Code or rust-analyzer download?** `scripts/lsp-check.sh` accepts `RUST_ANALYZER=…`, and `tools/capture/README.md` explains the stand-in client.

## When unsure

Prefer the reading that best serves [docs/intent.md](docs/intent.md). If two principles conflict, the earlier one wins. If a change would alter a principle or a decision record, propose it, as a PR description or a new record in `docs/decisions/`, instead of making it silently.
