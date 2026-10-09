# rbview for VS Code

Open any Rust file as a generated, read-only **Ruby view**. Navigation is answered by rust-analyzer, so it's precise:

- **Go to Definition / Type Definition / Implementation**: lands in the Ruby view of the target file, including dependency sources in `~/.cargo/registry`
- **Hover**: shows rust-analyzer's real Rust signature and docs
- **Find References**, **document highlights**, **Outline**

## Use

- Click the Ruby icon in a Rust editor's title bar, or press `Ctrl+Alt+R` (`Cmd+Alt+R` on macOS). The Ruby view opens beside the Rust file at the same spot.
- In a Ruby view, `Ctrl+Alt+R` (or the title-bar button) jumps back to the Rust source.
- **rbview: Pre-generate Ruby Views for Workspace** warms the `.rbview/` cache for the whole repo.

## Requirements

- The `rbview` binary on your `PATH`, or set `rbview.executable`.
- The rust-analyzer extension.
