# Decision records

Short records of choices that shape rbview, so nobody (human or agent) has to rediscover why things are the way they are, or quietly undoes them.

| # | Decision | Status |
|---|---|---|
| [0001](0001-syntax-only-generator.md) | The generator is syntax-only; rust-analyzer provides all semantics | Accepted |
| [0002](0002-verbatim-over-mistranslation.md) | Prefer verbatim Rust over a misleading translation | Accepted |
| [0003](0003-rbview-cache.md) | Cache views in a self-ignoring `.rbview/` folder, keyed by source and generator | Accepted |
| [0004](0004-source-map-format.md) | Token-level source maps in UTF-16 columns; the code line wins | Accepted |
| [0005](0005-single-source-tables.md) | Renames and erasures live in tables in `src/emit.rs` | Accepted |
| [0006](0006-deterministic-core.md) | The view is deterministic; no generated prose in the core path | Accepted |

## When to write one

Write a record when a change:

- contradicts a principle in [docs/intent.md](../intent.md) or an existing record, or
- picks between real alternatives that a later contributor might reasonably revisit, or
- adds a dependency, a new output format, or a new place state is stored.

Don't write one for routine translation rules. Those belong in [docs/spec/translation.md](../spec/translation.md) with a golden test.

## Template

Copy this into `NNNN-short-title.md`, add a row to the table above, and keep it under a page.

```markdown
# NNNN. Title

Status: Proposed | Accepted | Superseded by NNNN
Date: YYYY-MM-DD

## Context
What forces are at play? What problem does this solve?

## Decision
What we do, stated plainly.

## Consequences
What gets easier, what gets harder, what we must keep doing because of this.

## Alternatives considered
What else was weighed, and why it lost.
```
