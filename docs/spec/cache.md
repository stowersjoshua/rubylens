# Cache spec (`.rbview/`)

Rationale: [decision 0003](../decisions/0003-rbview-cache.md).

## Root

The cache lives in `<root>/.rbview/`, where `<root>` is the first match of:

1. an ancestor directory that already contains `.rbview/`
2. the git repository root
3. the outermost ancestor containing `Cargo.toml`

`--root DIR` overrides this. The VS Code extension passes the workspace folder.

## Layout

| Path | Contents |
|---|---|
| `.gitignore` | `*`, so the folder is never committed |
| `_prelude.rb` | stub definitions for Rust-isms that survive translation (`Ok`, `Err`, `try!`, `unwrap`, `enumerate`, `rust!`) — from `src/prelude.rb` |
| `symbols.json` | `{ "kinds": { "TypeName": "struct" \| "enum" \| "trait" } }`, written by `rbview build` and read by every `gen` |
| `<path>.rb` + `<path>.rb.map.json` | the view and [map](source-map.md) of `<root>/<path>.rs` |
| `ext/<crate>-<version>/<path>.rb` (+ map) | views of files outside the root. Registry crates are keyed by crate and version, since their sources never change. Other files are keyed by absolute path |

## Keys and invalidation

- Each map stores `hash` = first 16 bytes of SHA-256 over `GEN_VERSION`, `RBVIEW_BUILD_ID` and the source text.
- `RBVIEW_BUILD_ID` is a fingerprint of every file in `src/`, computed by `build.rs`. **Any change to the generator invalidates every cached view automatically.** No manual bump is needed.
- On `gen`, a stored view is reused only if its `hash` matches. Otherwise the view is regenerated and both files are rewritten.
- `--no-cache` always regenerates (and still writes).

## Commands

| Command | Effect |
|---|---|
| `rbview gen <file.rs> [--root DIR] [--json] [--no-cache]` | one view. Prints Ruby, or JSON with `--json` |
| `rbview build [DIR] [--force]` | pass 1 writes `symbols.json` for every `.rs` under `DIR`; pass 2 generates all views in parallel. `--force` ignores hashes |

## Rules

- Deleting `.rbview/` must always be safe.
- Nothing in `.rbview/` may be read as an input to generation except `symbols.json`.
- Never commit it. `scripts/check.sh` fails if any `.rbview/`, `target/` or `node_modules/` path is tracked.
