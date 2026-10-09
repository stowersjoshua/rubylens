# Performance

rbview is in the reader's way twice: when a view opens (one `rbview gen` per file, including every file a jump or a reference list lands in) and when `rbview build` warms a repo's cache. Both should feel instant. This page says how fast things are, how to measure them, and the rules that keep them fast.

## Where the time goes

`rbview bench` runs the generator in-process on every `.rs` file under a path and reports each phase. On the 66-crate corpus (2,789 files, 48.5 MB), best of 3, one thread:

| Phase | Before (b0a52b9) | Now |
|---|---|---|
| parse (`syn::parse_file`) | 4.26 s | 2.78 s |
| emit (`Gen::file`) | 3.89 s | 1.54 s |
| map JSON | 0.27 s | 0.20 s |
| **total** | **8.46 s** (5.7 MB/s) | **4.55 s** (10.7 MB/s) |
| per file p50 / p99 / max | 0.76 / 32 / 274 ms | 0.36 / 19 / 125 ms |

Parsing is now most of the cost, and it's syn's. What changed in our part:

| Change | Why it was slow |
|---|---|
| Block ranges read off the syntax tree (`src/range.rs`) | `tok_range` re-tokenized every item, statement and arm at every nesting level: O(size × depth) |
| `tok_range` looks up only its first and last token | it looked up the position of every token (two binary searches each) to take a min/max |
| ASCII fast paths in `src/out.rs` | the source was stored as one `Vec<char>` per line, and every mapped token scanned its line to count UTF-16 columns |
| one position lookup per token in `src_tokens` | each token's start and end were looked up twice |
| mimalloc, thin LTO, one codegen unit ([0007](decisions/0007-allocator-and-release-profile.md)) | the system allocator was ~30% of all instructions; syn and proc-macro2 couldn't be inlined into rbview |

## Measuring

```sh
cargo build --release
./target/release/rbview bench <dir-or-files>... [--repeat N] [--top K] [--digest FILE]
```

- `--repeat N` keeps each file's best of N runs; use 3 or more when comparing.
- `--top K` lists the K slowest files with µs/KB, a quick way to spot superlinear cases.
- `--digest FILE` writes a hash of each file's view and map. **A performance change must not change output:**

```sh
git stash && cargo build --release && ./target/release/rbview bench <corpus> --digest /tmp/before.txt
git stash pop && cargo build --release && ./target/release/rbview bench <corpus> --digest /tmp/after.txt
diff /tmp/before.txt /tmp/after.txt     # must be empty
```

The corpus is the one `scripts/corpus-check.py` fetches (`tests/corpus/`). Wall-clock numbers vary by a few percent between runs; for small changes, compare instruction counts instead:

```sh
valgrind --tool=callgrind --callgrind-out-file=cg.out ./target/release/rbview bench <files> --top 0
callgrind_annotate --inclusive=yes cg.out | head -60
```

## Rules

- **Don't re-tokenize big nodes.** `node.to_token_stream()` allocates a copy of the whole subtree. Fine for a type or a path; not for an item, a statement or anything you do per nesting level. For a node's source range use `node_range` (`src/range.rs`), which reads the first and last token off the tree.
- **Look up positions once.** `Span::start()` and `Span::end()` are binary searches in proc-macro2's line table. Read them once per token and pass the `LineColumn`s on.
- **Keep per-token work O(token).** Nothing that scans a whole line, file or output buffer per token.
- **Same output.** Prove it with `--digest` against the previous commit, on the whole corpus.
