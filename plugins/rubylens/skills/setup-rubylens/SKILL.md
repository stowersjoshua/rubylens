---
name: setup-rubylens
description: Install, update or troubleshoot RubyLens, which shows Rust source as read-only Ruby. Covers the rubylens CLI, the VS Code extension and pre-generating a workspace's views. Use when the user asks to install, set up, update or fix RubyLens, or when another RubyLens skill finds the rubylens command missing.
license: MIT
compatibility: Needs a Rust toolchain (cargo) and network access to GitHub. The editor part needs VS Code.
---

# Set up RubyLens

RubyLens has two parts. The `rubylens` command generates the Ruby views. The VS Code extension shows them beside the Rust, with go-to-definition, hover and references answered by rust-analyzer. Agents only need the command; people reading code in VS Code want both.

Say what each step installs or changes before you run it. Install the editor part only if the user wants it.

## 1. The command

```sh
command -v rubylens || echo "not installed"
cargo --version
```

Without `cargo`, the user needs a Rust toolchain first (https://rustup.rs). Then install from the repository, which takes about a minute:

```sh
cargo install --locked --git https://github.com/stowersjoshua/rubylens rubylens
```

To update, run the same command with `--force`. The binary lands in `~/.cargo/bin`. If `rubylens` still isn't found, that folder isn't on `PATH`.

Running `rubylens` with no arguments prints its usage, which confirms the install.

## 2. The VS Code extension (optional)

It needs rust-analyzer, and it isn't on the Marketplace yet, so install its package from the repository:

```sh
code --install-extension rust-lang.rust-analyzer
curl -fsSLO https://raw.githubusercontent.com/stowersjoshua/rubylens/master/editors/vscode/rubylens-0.1.0.vsix
code --install-extension rubylens-0.1.0.vsix
```

Without the `code` command, the user can run **Extensions: Install from VSIX…** from VS Code's command palette and pick the downloaded file.

If VS Code can't find `rubylens` (for example, when it was started from a desktop launcher with a different `PATH`), set the `rubylens.executable` setting to the binary's full path, such as `~/.cargo/bin/rubylens`.

## 3. Pre-generate the workspace

At the repository root:

```sh
rubylens build
```

This generates a view for every `.rs` file and records which names are enums, traits and structs across the crate. Without it, a view of `impl Shape` in one file can't know that `Shape` is an enum defined in another. In VS Code, the **RubyLens: Pre-generate Ruby Views for Workspace** command does the same.

Views are cached in `.rubylens/` at the repository root. The folder ignores itself in git and is safe to delete.

## 4. Check it works

```sh
rubylens gen src/lib.rs | head -20
```

Use `src/main.rs` for a binary crate. The view's header includes `# RubyLens: read-only Ruby projection of Rust source.` In VS Code, open a `.rs` file and run **RubyLens: Open as Ruby** (`Ctrl+Alt+R`, or `Cmd+Alt+R` on macOS). The view opens beside the Rust file.

## When something's wrong

| Symptom | Fix |
|---|---|
| `rubylens: command not found` after installing | add `~/.cargo/bin` to `PATH` |
| The view reads `# RubyLens failed for …` and `spawn rubylens ENOENT` | VS Code can't find the binary: set `rubylens.executable` to its full path |
| Hover and go-to-definition do nothing in the Ruby view | rust-analyzer must be installed and done indexing; its status shows in VS Code's status bar |
| An `impl` of an enum or trait shows as `class` | run `rubylens build` at the repository root |
| The view starts with `# RubyLens: could not parse this file` | the Rust doesn't parse (often mid-edit); the view shows the Rust source until it does |
