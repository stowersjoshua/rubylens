# 0003. Cache views in a self-ignoring `.rubylens/` folder, keyed by source and generator

Status: Accepted, amended by [0008](0008-views-record-their-symbol-kinds.md)
Date: 2026-10-09

## Context

Views are generated lazily, but repeated opens, crate-wide symbol lookups and dependency sources benefit from caching. The cache must never go stale silently, never end up in commits, and be safe to delete.

## Decision

- A hidden `.rubylens/` folder at the repo root, like `.git` or `.idea`, holds views, maps, `symbols.json`, `_prelude.rb`, and dependency views under `ext/<crate>-<version>/`.
- It contains its own `.gitignore` (`*`), so it's never committed, without editing the repo's `.gitignore`.
- Every entry is keyed by a hash of the source **and** of the generator itself. `build.rs` fingerprints `src/` into `RUBYLENS_BUILD_ID`. Editing a file or changing RubyLens invalidates exactly what changed. No manual version bumps are needed.

The full layout and rules are in [docs/spec/cache.md](../spec/cache.md).

## Consequences

- Deleting `.rubylens/` is always safe.
- Dependency views are shared per repo, not across repos. A global cache could come later.
- Anything that changes output must change something under `src/`, so the fingerprint changes. Keep generator logic in `src/`.

## Alternatives considered

- **No cache.** Generation is fast, but crate-wide symbol kinds and repeated dependency views made caching worthwhile.
- **A global cache in `~/.cache`.** Less discoverable, and it complicates cleanup. Revisit if dependency views become a cost.
- **A manual `GEN_VERSION` bump.** Agents and humans forget it, so the build fingerprint replaces it. `GEN_VERSION` remains only for map format changes.
