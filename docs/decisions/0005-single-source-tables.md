# 0005. Renames and erasures live in tables in `src/emit.rs`

Status: Accepted
Date: 2026-10-09

## Context

The view renames some Rust names to their Ruby counterparts: methods (`len` → `size`, `fold` → `inject`), macros (`println!` → `puts`) and keywords (`continue` → `next`). It also erases some methods entirely (`.iter()`, `.clone()`). The tests must know which differences are intended, or they can't tell a rename from a mapping bug. Scattered `match` arms made that impossible.

## Decision

Every intended rename or erasure is a row in one of four public tables in `src/emit.rs`:

| Table | Holds |
|---|---|
| `METHOD_RENAMES` | Rust method → Ruby method |
| `MACRO_RENAMES` | Rust macro → Ruby word |
| `KEYWORD_RENAMES` | Rust keyword/literal → Ruby counterpart |
| `DROPPED_METHODS` | zero-argument methods erased from the view |

The emitter reads them, and so do the golden tests (`tests/golden.rs`) and the corpus check (`scripts/corpus-check.py`, which parses them from the source). Nothing else may encode a rename.

## Consequences

- Adding a rename is a one-line change, and the tests accept it automatically.
- A rename hidden in emitter code shows up as a mapping failure, which points to the table.
- Contextual renderings that aren't name swaps, such as `x.is_some()` → `!x.nil?` or `collect::<HashMap>` → `.to_h`, stay in code but must keep mapping to the Rust method token.

## Alternatives considered

- **Allowlists inside the tests.** These drift from the emitter, and drift is the whole problem.
