# 0001. The generator is syntax-only; rust-analyzer provides all semantics

Status: Accepted
Date: 2026-10-09

## Context

Producing a readable view needs only syntax. Navigation needs semantics: name resolution, macro expansion, trait dispatch, type inference, and dependency sources. rustc and rust-analyzer already solve those, at great cost and with constant upkeep as Rust evolves.

## Decision

`src/emit.rs` works from `syn`'s syntax tree only. Every semantic question (definition, hover, references, outline, type) is answered at request time by mapping the Ruby position to Rust and asking rust-analyzer through the editor (`editors/vscode/extension.js`).

The generator may use cheap, local, syntactic heuristics. One example is the crate-wide `symbols.json` that records whether a name is an enum, struct or trait. But it must never try to resolve names or types itself.

## Consequences

- Generation is fast (milliseconds per file), works on stable Rust, works on code that doesn't compile, and needs no build.
- Navigation is exactly as good as rust-analyzer, including into dependencies and std.
- Some rendering choices can't use types. For example, `.map` on an `Option` looks like `Enumerable#map`. That's an accepted, documented limitation.
- The position map is load-bearing. Every emitted identifier must carry its span ([0004](0004-source-map-format.md)).

## Alternatives considered

- **Generate from rustc MIR via Charon.** This gives full semantics, but needs nightly and a successful build, loses surface structure (macros expanded, desugared control flow), and doesn't work on broken code.
- **Use rust-analyzer's crates (`ra_ap_*`) inside the generator.** This is heavy and version-coupled, and duplicates the rust-analyzer the editor already runs.
- **Ruby LSP over generated stubs.** It would answer Ruby questions about fake code. Wrong by construction.
