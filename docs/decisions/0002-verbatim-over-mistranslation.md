# 0002. Prefer verbatim Rust over a misleading translation

Status: Accepted
Date: 2026-10-09

## Context

Many Rust constructs have no faithful Ruby equivalent: DSL macros (`tokio::select!`, `quote!`), `macro_rules!` bodies, `pub macro`, unusual patterns. There's a constant temptation to render *something* Ruby-shaped for them, which reads nicely but can misstate what the code does.

## Decision

When there's no honest translation, keep the Rust as written:

- **On its own line:** verbatim comment lines, mapped character-for-character (`Out::verbatim`).
- **Inline:** `rust!(%q{…})`, with its contents mapped exactly when the source is a single line.

Verbatim regions must still navigate: a jump into or out of them lands on the exact token.

Valid Ruby syntax is a goal, but a lower one. A view may be syntactically invalid Ruby when the only "fix" would mislead. The known case is bindings inside or-patterns (`A(x) | B(x)`), which Ruby forbids. Such golden cases carry a `// rubylens-golden: ruby-invalid (reason)` marker.

## Consequences

- Readers occasionally see Rust in the Ruby view. That's honest, and it navigates.
- The corpus check holds valid Ruby at ≥ 97.7% rather than 100%.
- Improving coverage means finding *faithful* renderings, not more aggressive ones.

## Alternatives considered

- **Always emit valid Ruby**, approximating where needed. Rejected: it optimizes for a parser nobody runs at the expense of the reader's understanding.
- **Drop untranslatable code.** Rejected: hiding code is the worst kind of misleading.
