# 0008. Cached views record the symbol kinds they relied on

Status: Accepted (amends [0003](0003-rbview-cache.md))
Date: 2026-10-09

## Context

A view depends on more than its own source. `impl Shape` renders as `class Shape` or `module Shape` depending on whether `Shape`, defined in another file, is a struct or an enum. That knowledge comes from `symbols.json`. The cache key covered only the source and the generator, so after `Shape` changed kind, or appeared, or was deleted, every view that used it kept the old rendering until its own source changed. Decision 0003 requires that the cache never go stale silently.

There were performance problems too:

- Every `rbview gen` parsed all of `symbols.json`, about 15,000 names in the test corpus. That was 8 of the 10 ms a cache hit took.
- `rbview build` parsed every file in a single thread just to collect type names, even when nothing had changed: 3.4 s on the corpus to rebuild nothing.
- `symbols.json` only ever grew, and on a name defined twice, which definition won depended on directory order.

## Decision

- Each map records `deps`: every name the view looked up in the crate-wide kinds, and what it found (including "not found"). A cached view is reused only when its source hash matches and every dep still holds. `version`, `hash` and `deps` are the map's first fields, so a check reads only the start of the file.
- `rbview gen` reads `symbols.json` only when it has to: to generate, or to check a view that has deps. Views without deps never touch it.
- `rbview build` keeps `index.json`: the types each file defined at its last build, keyed by source hash. Pass 1 brings views up to date against the last build's kinds and collects definitions, parsing each changed file once for both. Pass 2 regenerates only the views whose recorded deps disagree with the new kinds. `symbols.json` is rebuilt from the index on every build, so deleted types disappear. A name defined twice resolves in path order, and the file is sorted by name, so it's deterministic.
- Both passes run on every core, biggest files first.
- Cache files are written to a temporary name and renamed, so parallel `gen` runs from the editor never read half a file.

## Consequences

- A view is regenerated exactly when its source, the generator, or a symbol kind it used changes.
- A cache hit takes about 3 ms instead of 10, and a no-op `rbview build` of the corpus takes 0.12 s instead of 3.4 s.
- The map format changed (`GEN_VERSION` 2). Editors ignore `deps`.
- Anything new the emitter learns from outside the file must go through `Gen::kind_of` (or something like it that records a dep). Otherwise the cache can't see the dependency.

## Alternatives considered

- **Hash all of `symbols.json` into every key.** Correct, but adding one type anywhere would regenerate every view in the repo.
- **Keep per-file definitions in each map.** That avoids `index.json`, but it puts build bookkeeping in the file the editor loads for every view.
- **A stamp of `symbols.json` in each map, to skip the dep check when nothing changed.** It would save about 1 ms per hit, but it adds a second validity rule that has to stay in sync with the first.
