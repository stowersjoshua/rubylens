# 0004. Token-level source maps in UTF-16 columns; the code line wins

Status: Accepted
Date: 2026-10-09

## Context

Navigation translates positions in both directions between the Ruby view and the Rust file, many times per user action. LSP positions are in UTF-16 code units. Identifiers are renamed (`len` → `size`, `end` → `end_`), reordered (items hoisted out of functions) and sometimes repeated, as when a name appears both in a signature comment and on the `def` line.

## Decision

- The map is a list of **segments**, one per emitted token: `[rb_line, rb_col, rb_len, rs_line, rs_col, rs_len]`. There are also coarse **blocks** per statement and item: `[rb_start, rb_end, rs_start, rs_end]`.
- Lines are 0-based. Columns and lengths are **UTF-16 code units**, so positions pass straight through to and from LSP.
- Lookup prefers the smallest segment containing the position, then the nearest segment on the line, then blocks, then the nearest mapped line.
- **The code line wins.** When the same Rust token appears twice in the view, such as a function name in the signature comment and in `def name`, only the code occurrence is mapped. The comment copy is plain text (`src_tokens_except`).

The precise format and invariants are in [docs/spec/source-map.md](../spec/source-map.md).

## Consequences

- Hover, definition, references and outline all reduce to two lookups.
- Golden tests can verify mapping mechanically: bounds, identifier identity and coverage.
- Emitter code must route every Rust-derived token through `span_tok`, `node_tok` or `src_tokens`, never through `push`.

## Alternatives considered

- **Line-only maps.** Too coarse: a line holds many identifiers.
- **Standard JavaScript source maps (VLQ).** Built for one-way, generated-to-source lookups with byte or UTF-16 ambiguity. Our needs are bidirectional and LSP-shaped.
