# Performance

rbview is in the reader's way twice: when a view opens (one `rbview gen` per file, including every file a jump or a reference list lands in) and when `rbview build` warms a repo's cache. Both should feel instant. This page says how fast things are, how to measure them, and the rules that keep them fast.

## Where the time goes

`rbview bench` runs the generator in-process on every `.rs` file under a path and reports each phase. On the 66-crate corpus (2,789 files, 48.5 MB), best of 5, one thread, both measured in the same session:

| Phase | Before (b0a52b9) | Now |
|---|---|---|
| parse (`syn::parse_file`) | 4.16 s | 2.86 s |
| emit (`Gen::file`) | 3.85 s | 1.60 s |
| map JSON | 0.26 s | 0.21 s |
| **total** | **8.31 s** (5.8 MB/s) | **4.70 s** (10.3 MB/s) |
| per file p50 / p99 / max | 0.73 / 33 / 239 ms | 0.35 / 21 / 142 ms |

Parsing is now most of the cost, and it's syn's (the allocator and LTO are what sped it up). What changed in our part:

| Change | Why it was slow |
|---|---|
| Block ranges read off the syntax tree (`src/range.rs`) | `tok_range` re-tokenized every item, statement and arm at every nesting level: O(size × depth) |
| `tok_range` looks up only its first and last token | it looked up the position of every token (two binary searches each) to take a min/max |
| Per-line tables in `src/out.rs`: O(1) column conversions (ASCII lines need none) | the source was stored as one `Vec<char>` per line, and every mapped token scanned its line to count UTF-16 columns: O(n²) on long lines |
| One position lookup per token in `src_tokens` | each token's start and end were looked up twice |
| Indentation written in chunks | one 2-byte write per level per line, which added up in deeply nested code |
| A macro body that reads as call arguments is parsed once | it was parsed to check, then again to render |
| mimalloc, thin LTO, one codegen unit ([0007](decisions/0007-allocator-and-release-profile.md)) | the system allocator was ~30% of all instructions; syn and proc-macro2 couldn't be inlined into rbview |

## Opening a view and building a repo

What the editor waits for is a whole `rbview gen` process. Medians on a 2-core machine, with a built cache (`hit`) or `--no-cache` (`miss`):

| | Before (b0a52b9) | Now |
|---|---|---|
| tokio `sync/oneshot.rs` (1,600 lines): hit / miss | 2.4 / 12.7 ms | 2.6 / 8.1 ms |
| corpus, anyhow `lib.rs`: hit / miss | 9.9 / 14.4 ms | 3.5 / 10.1 ms |
| corpus, syn `expr.rs` (142 KB): hit / miss | 11.9 / 57 ms | 5.7 / 27 ms |
| corpus, typenum `generated.rs` (629 KB): hit / miss | 24 / 363 ms | 12.5 / 177 ms |
| tokio `rbview build` (555 files): empty cache / nothing changed | 1.07 / 0.47 s | 0.36 / 0.016 s |
| corpus `rbview build` (2,789 files): empty cache / nothing changed | 13.4 / 5.2 s | 4.7 / 0.11 s |

A small-file hit is mostly process start (about 2 ms), so it didn't move. Hits in the corpus did, because its `symbols.json` has 15,000 names. mimalloc reads its options from the environment when the process starts: unnoticeable with a typical environment (the numbers above use one of about 2 KB), about 0.5 ms with a 23 KB one.

What changed ([0008](decisions/0008-views-record-their-symbol-kinds.md)):

- A hit read and parsed all of `symbols.json` (15,000 names here), twice over with SipHash. Now it's read only when a view's map has `deps`, in one pass into an FNV-hashed table.
- `build` parsed every file twice, the first time in one thread just to collect type names. Now each file is parsed at most once (pass 1 brings the view up to date and collects its types from the same parse), unchanged files aren't parsed at all, and both passes use every core, biggest files first.
- `build` read every cached view and map in full to compare a hash. Now it reads the first few KB of the map.
- Worker threads drop proc-macro2's per-thread line tables after each file. Otherwise they grow with every file the thread parses.

## Measuring

```sh
cargo build --release
./target/release/rbview bench <dir-or-files>... [--repeat N] [--top K] [--digest FILE]
scripts/bench-corpus.sh [--repeat N] [--top K] [--digest FILE]     # the same, over the pinned corpus
```

- `--repeat N` keeps each file's best of N runs; use 3 or more when comparing.
- `--top K` lists the K slowest files with µs/KB, a quick way to spot superlinear cases.
- `--digest FILE` writes a hash of each file's view and map. **A performance change must not change output:**

```sh
git stash && scripts/bench-corpus.sh --digest /tmp/before.txt
git stash pop && scripts/bench-corpus.sh --digest /tmp/after.txt
diff /tmp/before.txt /tmp/after.txt     # must be empty
```

Wall-clock numbers vary by several percent between runs (more on a shared machine); for small changes, compare instruction counts instead:

```sh
valgrind --tool=callgrind --callgrind-out-file=cg.out ./target/release/rbview bench <files> --top 0
callgrind_annotate --inclusive=yes cg.out | head -60
```

## Rules

- **Don't re-tokenize big nodes.** `node.to_token_stream()` allocates a copy of the whole subtree. Fine for a type or a path; not for an item, a statement or anything you do per nesting level. For a node's source range use `node_range` (`src/range.rs`), which reads the first and last token off the tree.
- **Look up positions once.** `Span::start()` and `Span::end()` are binary searches in proc-macro2's line table. Read them once per token and pass the `LineColumn`s on.
- **Keep per-token work O(token).** Nothing that scans a whole line, file or output buffer per token.
- **Same output.** Prove it with `--digest` against the previous commit, on the whole corpus.

## Guards

| Guard | Catches | Where |
|---|---|---|
| scaling check | generation that grows faster than its input: 8x the input must take < 20x the time | `scripts/scaling-check.py` (in `scripts/check.sh`) |
| range self-check | `node_range` disagreeing with re-tokenizing, in every debug build | `src/range.rs` (golden tests; the whole corpus: `cargo build && RBVIEW=target/debug/rbview scripts/bench-corpus.sh`) |
| cache tests | a view reused when it's stale, or regenerated when it isn't; nondeterministic builds; half-written files under parallel `gen` | `tests/cache.rs` |
| lookup scaling | position lookups that scan a whole view | `editors/vscode/test/run.js` |
| check-the-checks | each guard above still fires on the mistake it's for | `scripts/check-the-checks.sh` |

The `improve-performance` skill (`.claude/skills/`) walks through a performance change step by step.
