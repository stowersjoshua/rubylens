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
| `symbols.json` | `{ "kinds": { "TypeName": "struct" \| "enum" \| "trait" } }`, sorted by name: every type defined by a file in `index.json`. Written by `rbview build`, read by `gen` when it generates or validates a view |
| `index.json` | `{ "files": { "<path>": { "hash", "defs": { "TypeName": kind } } } }`: what each file defined at its last build, keyed by its source hash. Written and read by `rbview build` only, so unchanged files aren't parsed again to build `symbols.json` |
| `<path>.rb` + `<path>.rb.map.json` | the view and [map](source-map.md) of `<root>/<path>.rs` |
| `ext/<crate>-<version>/<path>.rb` (+ map) | views of files outside the root. Registry crates are keyed by crate and version, since their sources never change. Other files are keyed by absolute path |

## Keys and invalidation

- Each map starts with three fields ([source-map.md](source-map.md)):
  - `version`: the map format (`GEN_VERSION`).
  - `hash`: the first 16 bytes of SHA-256 over `GEN_VERSION`, `RBVIEW_BUILD_ID` and the source text. `RBVIEW_BUILD_ID` is a fingerprint of every file in `src/`, computed by `build.rs`. **Any change to the generator invalidates every cached view automatically.** No manual bump is needed.
  - `deps`: every crate-wide kind the view relied on, as `{ "TypeName": kind or null }`. Today that's the self type of each `impl` not defined in the file itself (`class` or `module`).
- A stored view is reused only if its `version` and `hash` match **and** every `deps` entry is still what `symbols.json` says. Otherwise the view is regenerated and both files are rewritten. So a view never goes stale when another file adds, changes or removes a type it used.
- Reuse never needs `symbols.json` unless the view has `deps`, and never needs more of the map than its first fields. `rbview build` reads only the start of each map.
- `--no-cache` always regenerates (and still writes).
- Writes go to a temporary file that is renamed into place, the view before its map; readers read the map first. Concurrent `gen` and `build` runs on the same files (the editor runs several at once) never see half a file.

## Commands

| Command | Effect |
|---|---|
| `rbview gen <file.rs> [--root DIR] [--json] [--no-cache]` | one view. Prints Ruby, or JSON with `--json` |
| `rbview build [DIR] [--force]` | every `.rs` under `DIR`, in parallel, biggest files first. Pass 1 brings each view up to date against the `symbols.json` of the last build and collects the types each file defines (from `index.json` when the file is unchanged), parsing each file at most once. Then `index.json` and `symbols.json` are written. Pass 2 regenerates only the views that relied on a kind that just changed. `--force` regenerates everything. Entries for files outside `DIR` are kept, so building a subfolder doesn't forget the rest of the repo |

## Rules

- Deleting `.rbview/` must always be safe.
- Nothing in `.rbview/` may be read as an input to generation except `symbols.json`. `index.json` feeds only `symbols.json`, and only through entries whose hash matches the current source.
- The same sources and the same rbview give the same `.rbview/`, byte for byte (`tests/cache.rs`).
- Never commit it. `scripts/check.sh` fails if any `.rbview/`, `target/` or `node_modules/` path is tracked.
