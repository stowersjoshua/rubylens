# 0006. The view is deterministic; no generated prose in the core path

Status: Accepted
Date: 2026-10-09

## Context

A language model could produce more idiomatic Ruby for a method body, or explain it in prose. That's appealing for readability. But model output isn't reproducible, can be wrong in ways that look right, and has no token-level mapping to the Rust source.

## Decision

The view rbview generates is a pure function of the Rust source, the rbview version and the crate-wide symbol kinds it looked up (recorded in its map, [0008](0008-views-record-their-symbol-kinds.md)): same input, same bytes. No network calls and no model calls happen in `src/`.

A future "explain this method" feature may exist only as a **separate, clearly labelled layer**, such as a hover or side panel. It must be cached by content hash and never replace, edit or interleave with the mapped view.

## Consequences

- Golden tests and the `.rbview/` cache stay valid.
- Every claim the view makes can be traced to a Rust token.
- Readability improvements come from better deterministic rules, recorded in the spec with golden tests.

## Alternatives considered

- **Model-written views.** Rejected: unverifiable, unmappable and non-deterministic.
