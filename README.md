# rbview: read Rust as Ruby

`rbview` turns Rust source into a **read-only Ruby projection**. The editor extension lets you browse it with normal IDE navigation (go to definition, hover, references, outline), all answered by rust-analyzer. The Ruby isn't meant to run. It's there so you can read what a Rust black box is doing in a language you think in.

```rust
pub fn total_area(&self) -> f64 {
    self.shapes.iter().map(|s| s.area()).sum()
}

pub fn add(&mut self, shape: Shape, tag: Option<&str>) -> &mut Self {
    if let Some(t) = tag {
        *self.tags.entry(t.to_string()).or_insert(0) += 1;
    }
    self.shapes.push(shape);
    self
}
```

```ruby
# fn total_area(&self) -> f64
def total_area
  @shapes.map { |s| s.area }.sum
end

# fn add(&mut self, shape: Shape, tag: Option<&str>) -> &mut Self
def add(shape, tag)
  if (t = tag)
    @tags[t.to_s] = (@tags[t.to_s] || 0) + 1
  end
  @shapes.push(shape)
  self
end
```

See `examples/demo/src/shapes.rs` and its generated view in `examples/demo/shapes.rb`.

## Install

```sh
cargo install --path .                                  # puts `rbview` on your PATH
code --install-extension editors/vscode/rbview-0.1.0.vsix
```

Requires the rust-analyzer VS Code extension.

## Use

- **VS Code:** in any Rust file, click the Ruby icon in the editor title bar or press `Ctrl+Alt+R`. The Ruby view opens beside it at the same spot, and the same key jumps back.
- **CLI:**
  - `rbview gen src/foo.rs` prints one view.
  - `rbview build` pre-generates every `.rs` file in the repo.

## How it works

```
foo.rs ──syn──▶ Ruby text + position map ──▶ rbview:/…/foo.rs.rb (virtual doc)
                                                   │ go-to-def at Ruby (l,c)
                                                   ▼
                     map back to Rust (l,c) ─▶ rust-analyzer ─▶ target .rs location
                                                   │
                     map forward into the target's Ruby view ◀──┘
```

The generator only does syntax. All semantics (name resolution, trait dispatch, macros, dependencies) come from the live rust-analyzer. That's why it can be fast, lazy and per-file.

### The `.rbview/` folder

Like `.git` or `.idea`, a hidden folder at the repo root caches everything:

| Path | What |
|---|---|
| `.gitignore` | ignores the whole folder, so nothing is ever committed |
| `_prelude.rb` | stub definitions for Rust-isms (`Ok`, `Err`, `try!`, `unwrap`, `enumerate`, …) |
| `symbols.json` | crate-wide type kinds, so `impl Shape` knows `Shape` is an enum (a Ruby `module`) |
| `src/**.rb`, `*.rb.map.json` | generated views and their position maps, mirroring the repo |
| `ext/<crate>-<version>/**` | views of dependency sources (registry crates are immutable, so they stay cached) |

Every entry is keyed by a hash of the source **and of the generator itself**, so editing a file or upgrading rbview invalidates exactly what changed. Deleting the folder is always safe.

Speed, measured on tokio (555 files):

| | |
|---|---|
| Cold `rbview build` | 1.3 s |
| Warm `rbview build` | 0.6 s |
| Single 1,600-line file, uncached | 19 ms |
| Single 1,600-line file, cached | 3 ms |

## Translation cheatsheet

| Rust | Ruby view |
|---|---|
| `struct Foo { a: T }` | `class Foo` + `attr_accessor :a  # T` |
| `enum E { A { x }, B(u8), C }` | `module E` with `A = Data.define(:x) { include E }` … |
| `trait T` / `impl T for Foo` | `module T` / `class Foo; include T` |
| `fn new() -> Self` (no `self`) | `def self.new` |
| `Foo { a: 1 }` (struct literal) | `Foo[a: 1]` (distinct from a `Foo.new` call) |
| `match` / `if let` / `matches!` | `case/in`, `if x in Pat`, `(x in Pat)` |
| `Some(x)` / `None` | `x` / `nil` |
| `expr?` | `expr.try!` ("unwrap or return the error") |
| `.iter().filter(..).map(..).collect()` | `.select { }.map { }` |
| `.len()`, `.is_empty()`, `.fold(i, f)` | `.size`, `.empty?`, `.inject(i) { }` |
| `format!("{} is {x:?}", a)` | `"#{a} is #{x.inspect}"` |
| `self.field` | `@field` |
| `&x`, `*x`, `.clone()`, lifetimes, `mut` | dropped |
| Rust signature, generics, field types | kept as comments |
| Items declared inside a function | shown right after the method (Ruby can't nest them) |
| `cfg_rt! { pub fn x() {} }` and other macros wrapping items | the items are translated, between `# cfg_rt! {` and `# }` |
| `macro_rules!` definitions, `tokio::select!` and other DSLs | kept verbatim as comment lines, mapped character-for-character |
| Anything else untranslated inline | kept verbatim as `rust!(%q{…})` |

## Testing

### Against a real rust-analyzer

`editors/vscode/test/lsp-e2e.js` drives a live rust-analyzer over LSP and replays exactly what the extension does: click an identifier in the Ruby view, map it to Rust, ask rust-analyzer, map the answer into the target file's Ruby view, and check that the cursor lands on the identifier rust-analyzer meant.

| Project | Definitions landing correctly | Into deps / std | References | Outline |
|---|---|---|---|---|
| demo crate | 190 / 190 | 0 / 32 | 27 / 27 | 28 / 28 |
| app using regex, semver, serde_json, itertools, anyhow | 117 / 117 | 28 / 18 | 7 / 7 | 7 / 7 |
| rbview's own source (syn, serde, sha2) | 1,313 / 1,313 | 191 / 199 | 603 / 603 | 143 / 143 |
| tokio (4 core files, all features) | 1,596 / 1,596 | 51 / 287 | 304 / 304 | 291 / 291 |

Hover returned rust-analyzer content for 540 of 540 sampled tokens. Tested with rust-analyzer 2026-08-17 and Rust 1.97 standard-library sources.

```sh
node editors/vscode/test/lsp-e2e.js --ra $(which rust-analyzer) --project path/to/crate \
  --file src/lib.rs --file src/other.rs [--all-features] [--limit 400] [--verbose]
```

### Translation, across 64 real crates (2,789 files: tokio, serde, syn, clap, regex, rayon, …)

- **No crashes.** 1 file fell back to showing raw source.
- **97.8% of views are syntactically valid Ruby** (`ruby -c`).
- **Position maps are accurate.** In 400 random files, every identifier token maps back to the right Rust token, apart from intentional renames like `len` → `size`.

### Extension plumbing

`node editors/vscode/test/run.js` runs the extension's providers against a mock `vscode` module: definition, hover, references, outline and round-trips.

## Known limitations

- **Lossy by design.** Ownership, borrowing, lifetimes and `Send`/`Sync` are erased, and they often explain *why* code is shaped the way it is. Use the view as a reading aid; trust the Rust.
- **Bindings inside or-patterns.** Patterns like `A(x) | B(x)` can't be expressed in Ruby, which forbids them. Those few views aren't valid Ruby.
- **VS Code itself is untested.** The mapping and real rust-analyzer responses are tested headlessly. The thin layer that forwards requests through VS Code's `execute*Provider` commands has only been tested against a mock.
- **Macro bodies.** Macros that wrap items are translated. DSL-style macros like `tokio::select!` or `quote!` are shown as verbatim Rust, which still navigates precisely but isn't translated.
- **Feature-gated code.** Code behind cargo features that rust-analyzer considers disabled has no semantics to navigate. Enable the features in your rust-analyzer settings, as you would for Rust.
- **Partly tuned for Ruby readers.** A few method renames assume iterator semantics (`map`/`filter` on `Option` read as Enumerable).

## Ideas for next steps

- **Neovim / other editors.** The same pattern works as a tiny LSP proxy serving `rbview:` buffers.
- **Workspace symbol search** over `symbols.json`.
- **Optional "explain this method" pass.** An LLM could rewrite one method body more idiomatically, cached by hash, without touching the deterministic skeleton or its navigation.
