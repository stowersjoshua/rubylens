# Intent

## Why rbview exists

More and more of the code that Ruby (and Python, and JavaScript) developers depend on is being rewritten in Rust: language tooling, parsers, the native core of popular gems. For someone who thinks in Ruby, those parts become black boxes. They can be read, but slowly, because Rust's syntax and its ownership machinery stand between the reader and what the code *does*.

rbview opens those boxes for reading. It shows a Rust file as Ruby that a Rubyist can skim. And it keeps the IDE working: go to definition, hover and references behave as they do in Rust, because rust-analyzer still answers every question.

The user is a developer **reading** unfamiliar Rust to understand it: to debug through a dependency, review a change, or learn how something works. They aren't writing Rust in rbview, and they never run the Ruby.

## Principles

These decide trade-offs. When two of them conflict, the earlier one wins.

1. **Navigation never lies.** Every identifier the view shows must lead to exactly the Rust token it came from, and every jump must land on the right token. A view that reads well but navigates wrongly is worse than one that reads awkwardly but navigates correctly. The golden tests and `scripts/lsp-check.sh` enforce this at 100%.

2. **Faithful, not executable.** The Ruby is a *projection* of the Rust, not a port. It must never suggest behaviour the Rust doesn't have. When a construct has no honest Ruby equivalent, show the Rust verbatim (mapped, as comments or `rust!(%q{…})`) rather than inventing a misleading translation.

3. **Syntax here, semantics in rust-analyzer.** The generator only parses (`syn`); it never resolves names, types or traits. Anything semantic, such as where a method is defined, a value's type, or which impl a call dispatches to, comes from rust-analyzer at the moment the reader asks. That keeps the generator small, fast and per-file, and keeps rbview correct wherever rust-analyzer is.

4. **Read like Ruby.** Within those limits, prefer what a Ruby developer expects: `case/in` for `match`, blocks for closures, `Enumerable` names for iterator methods, `@field` for `self.field`, and `nil` for `None`. Erase what only matters to the Rust compiler (`&`, `*`, lifetimes, `.clone()`, `.iter()`). Keep what explains the code (signatures, types, generics) as comments.

5. **Lazy and cached.** Only the files the reader opens are generated, and results are cached in `.rbview/` keyed by both the source and the generator. Big repos and dependency trees cost nothing until they're read.

6. **Deterministic.** The same source and the same rbview give the same view, byte for byte. Golden tests and the cache both depend on it.

## Non-goals

- **Running the Ruby.** The view isn't meant to execute, and no work should go into making it executable at the expense of the principles above.
- **Editing through the view.** It's read-only. Changes are made in the Rust.
- **Rust → Ruby migration.** rbview isn't a transpiler for porting code.
- **Ruby-side semantics.** Ruby language servers aren't consulted. Navigation always goes through rust-analyzer.
- **Machine-generated prose in the core path.** The view is produced deterministically from syntax. An optional, clearly separated "explain this method" layer could exist one day, but it must never replace or alter the deterministic, mapped view. See [decision 0006](decisions/0006-deterministic-core.md).

## What success looks like

A Ruby developer opens a Rust file they've never seen, presses one key, and reads it at their normal speed. They hover a call and see its real Rust signature. They follow it into a dependency they didn't know they had, then press the key again to see the real Rust. They understand what the code does, and they never notice the tool, because nothing it showed them was wrong.
