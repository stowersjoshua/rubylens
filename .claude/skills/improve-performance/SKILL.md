---
name: improve-performance
description: Use when making rbview faster (generator, cache, build, or the extension's position lookups), when a change might slow it down, or when scripts/scaling-check.py or a cache test fails. Covers measuring, proving output is unchanged, the known traps, and which numbers and docs to update.
---

# Improve performance

Read [docs/performance.md](../../../docs/performance.md) first: it has the current numbers, the tools and the rules. Speed never buys a different view: a performance change produces byte-identical output, or it's a translation change and goes through `change-translation`.

## 1. Measure before changing anything

```sh
scripts/bench-corpus.sh --repeat 3 --top 10 --digest /tmp/before.txt     # the pinned 66-crate corpus
./target/release/rbview bench <files or dirs> --repeat 3 --top 10          # anything else
```

`--top` lists the slowest files with µs/KB: a file far above the others is usually a superlinear path.

For anything under ~10% wall clock, compare instruction counts, which don't vary between runs:

```sh
valgrind --tool=callgrind --callgrind-out-file=/tmp/cg ./target/release/rbview bench <a few files> --top 0
callgrind_annotate --inclusive=yes /tmp/cg | head -60     # where the time goes, callers included
callgrind_annotate /tmp/cg | head -40                     # self cost: allocation, memcpy, hashing
```

For `rbview gen` latency (what the editor waits for), time whole processes against a built cache (`rbview build <repo>` first), cache hit and `--no-cache`, median of 15 or more.

## 2. Change, then prove the output is identical

```sh
scripts/bench-corpus.sh --repeat 3 --top 0 --digest /tmp/after.txt
diff /tmp/before.txt /tmp/after.txt        # must be empty
```

If you changed `src/range.rs`, also run the debug binary over the corpus: debug builds check every range against the slow path.

```sh
cargo build && RBVIEW=target/debug/rbview scripts/bench-corpus.sh --top 0
```

## 3. Known traps

| Trap | Instead |
|---|---|
| `node.to_token_stream()` on an item, statement or block, especially per nesting level | `node_range` (`src/range.rs`) for ranges; re-tokenize only small nodes (types, paths) |
| `Span::start()`/`end()` called repeatedly for the same token | look up once, pass the `LineColumn` |
| scanning a line, the output or a list per token | an index built once (`SrcLine`, `innermostIndex` in `srcmap.js`) |
| reading something from outside the file in the emitter without `Gen::kind_of` | record it as a dep, or cached views go stale silently |
| writing cache files with `fs::write` | `write_atomic`; editors read views while other `gen` runs write them |
| a new per-process setup cost in `rbview gen` (loading a table, scanning the cache) | make it lazy; most runs are cache hits that need nothing |
| parsing a file twice in `rbview build` | `refresh` returns what the file defines from the same parse |

## 4. Check and record

```sh
scripts/scaling-check.py            # 8x the input must take < 20x the time
scripts/check.sh --full
```

- If a number in [docs/performance.md](../../../docs/performance.md) or the README speed table moved noticeably, update it, and say what machine you measured on if it's not comparable.
- A new kind of slowness deserves a guard: a case in `scripts/scaling-check.py`, a test in `tests/cache.rs`, or a lookup in `editors/vscode/test/run.js`. Then add a mutation to `scripts/check-the-checks.sh` that proves the guard fires.
- New dependency or build-profile change: write a decision record (see [0007](../../../docs/decisions/0007-allocator-and-release-profile.md)).
