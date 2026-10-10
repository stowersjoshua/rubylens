# 0010. The project is called RubyLens

Status: Accepted
Date: 2026-10-10

## Context

The project started as `rbview`: short, but it says little to someone who hasn't used it, and "view" doesn't say that the view is a reading aid over code that stays Rust.

## Decision

The project is **RubyLens**: you read Rust through a Ruby lens. A lens changes how you see something, not the thing itself, which is the promise in [intent.md](../intent.md): the Rust stays the truth, and navigation always lands on it.

| Where | Name |
|---|---|
| prose, titles, the editor's command palette | RubyLens |
| crate, binary, commands | `rubylens` (`rubylens gen`, `rubylens build`, `rubylens serve`) |
| cache folder | `.rubylens/` |
| VS Code: extension, settings, commands, view scheme | `rubylens`, `rubylens.executable`, `rubylens.open`, `rubylens:/path/x.rs.rb` |
| environment variables | `RUBYLENS` (scripts' binary), `RUBYLENS_BUILD_ID`, `RUBYLENS_CHECK_RANGE` |

## Consequences

- Everything was renamed at once; nothing answers to `rbview` any more. Old `.rbview/` folders are ignored (they gitignore themselves) and safe to delete. An installed `rbview` binary and `rbview` extension should be uninstalled; the `rbview.executable` setting becomes `rubylens.executable`.
- The view's first lines now say `# RubyLens: …`, so every golden snapshot changed by that line only.

## Alternatives considered

- **Keep `rbview`.** Cheapest, but undescriptive.
- **Interlinear, RustGloss, RustCrib** (reading-aid metaphors) and **Ruby-Tinted, Ruby Loupe** (other lenses). RubyLens says what it does most plainly.
