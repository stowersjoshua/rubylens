# Source map spec

Every view comes with a map that translates positions between the Ruby view and its Rust file. All navigation depends on it ([decision 0004](../decisions/0004-source-map-format.md)).

## Format

`rbview gen <file> --json` prints `{ "rbPath", "cached", "ruby", "map" }`. The cache stores the map as `<view>.rb.map.json`:

```json
{
  "version": 1,
  "hash": "9f2c…",
  "segs":   [[rb_line, rb_col, rb_len, rs_line, rs_col, rs_len], …],
  "blocks": [[rb_start, rb_end, rs_start, rs_end], …]
}
```

| Field | Meaning |
|---|---|
| `version` | map format version (`GEN_VERSION` in `src/lib.rs`); bump only when this format changes |
| `hash` | cache key: source + generator fingerprint ([cache.md](cache.md)) |
| `segs` | one per mapped token: a Ruby range and the Rust range it came from |
| `blocks` | one per item, statement and match arm: Ruby line range ↔ Rust line range, inclusive |

**Units.** Lines are 0-based. Columns and lengths are UTF-16 code units, which is LSP's unit, so positions pass to and from the editor unchanged.

A Rust range that spans several lines is recorded with `rs_len = 1`, which pins its first character. Prefer mapping individual tokens so this rarely happens.

## Invariants

Enforced by `tests/golden.rs` (all cases) and `scripts/corpus-check.py` (66 real crates):

1. **In bounds.** Every segment and block lies inside both texts. Lengths are positive.
2. **Identity.** A segment whose Ruby text is an identifier maps to the same identifier in Rust. The exceptions are intended renames listed in `METHOD_RENAMES`, `MACRO_RENAMES` or `KEYWORD_RENAMES`, and the normalizations: camelized module names, a `_` suffix on Ruby keywords, `r#`, `@field`, and `Self` → the type name.
3. **Coverage.** At least 95% of a file's Rust identifiers are reachable from the view. Keywords, lifetimes and things erased by design (`DROPPED_METHODS`, `Some` in `Some(x)`, `_`) don't count.
4. **The code line wins.** When a Rust token appears twice in the view, only the code occurrence is mapped. The usual case is a function name or parameter in the signature comment and on the `def` line. Comment copies are plain text (`src_tokens_except`).
5. **Determinism.** The same source and the same generator give an identical map.

And enforced against a live rust-analyzer by `scripts/lsp-check.sh`:

6. **Jumps land right.** For every resolved go-to-definition, reference and outline symbol, the Ruby text at the landing position is the identifier rust-analyzer pointed at: 100% required.

## Lookup (`editors/vscode/srcmap.js`)

**Ruby → Rust** (`rbToRs`):

1. Among segments on the line that contain the column, take the smallest. Keep the offset within the token, clamped to the Rust token's length.
2. Otherwise, the nearest segment on the line.
3. Otherwise, the innermost block containing the line, offset proportionally.

**Rust → Ruby** (`rsToRb`):

1. Among segments on the Rust line that contain the column, take the smallest.
2. Otherwise, the nearest segment on that line.
3. Otherwise, the innermost block.
4. Otherwise, the nearest mapped Rust line, preferring code below (e.g. a target inside a long doc comment). As a last resort, line 0.

**Ranges** (`rsRangeToRb`): an exact token keeps its Ruby length. Otherwise both ends are mapped, and the end is clamped so it never precedes the start.

## Writing emitter code that keeps the invariants

- Route every Rust-derived token through `span_tok`, `node_tok`, `src_tokens` or `Out::verbatim`, never through `push`. `push` is for Ruby-only text: keywords like `def`, `end` and `case`, punctuation and comments.
- Map keywords to their own token (`return_token`, `question_token`), not to the whole expression, or a multi-line expression degrades to one character.
- When the same token is rendered twice, decide which occurrence is the code line and pass the other's positions to `src_tokens_except`.
