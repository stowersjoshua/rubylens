# 0007. mimalloc and a link-time-optimized release build

Status: Accepted
Date: 2026-10-09

## Context

Generating a view is mostly syn parsing and re-tokenizing syntax nodes, which allocates and frees millions of small objects. Profiling the 66-crate corpus with callgrind put the system allocator (glibc malloc/free) at about 30% of all instructions. The parser and the emitter live in different crates (syn, proc-macro2, RubyLens), so the compiler can't inline across them without link-time optimization.

Measured with `rubylens bench` on the corpus (2,789 files, 48.5 MB, best of 3, same output by digest):

| Build | Total | Incremental release build |
|---|---|---|
| default release profile, system allocator | 7.9 s | 6 s |
| + `lto = "thin"`, `codegen-units = 1` | 7.1 s | 17 s |
| + mimalloc | 5.4 s | 17 s |

`lto = "fat"` measured the same as thin and builds slower. Thin LTO with the default 16 codegen units gave about half the gain.

## Decision

- The binary uses [mimalloc](https://crates.io/crates/mimalloc) as its global allocator, set in `src/main.rs` only. The library (`rubylens::generate`, the golden tests) leaves the allocator to its caller, as libraries should.
- `[profile.release]` sets `lto = "thin"` and `codegen-units = 1`. `cargo install --path .` uses it, so users get the fast build without extra flags.

## Consequences

- About 30% less time per view, for every user, with no change to output.
- mimalloc scans the environment for its options when the process starts. With a typical environment that's lost in the noise; with a very large one (23 KB) it adds about 0.5 ms to every `rubylens gen`, about a fifth of a small-file cache hit.
- Building RubyLens needs a C compiler for mimalloc. Every platform that builds Rust crates with C code already has one.
- An incremental release build takes about 17 s instead of 6 s. `scripts/check.sh` builds release, so each check run pays this once.
- The allocator choice can't change output (decision [0006](0006-deterministic-core.md)); the digest comparison in [docs/performance.md](../performance.md) proves it for each change.

## Alternatives considered

- **System allocator.** Simplest, but allocation was the single largest cost.
- **jemalloc.** Comparable speed, but it doesn't build on Windows MSVC.
- **Fewer allocations in syn.** syn's AST and proc-macro2's token trees are outside our control; we removed the allocations RubyLens itself caused instead (see [docs/performance.md](../performance.md)).
- **A separate `dist` profile.** Keeps contributor builds fast, but users installing with plain `cargo install` would silently get the slow build.
