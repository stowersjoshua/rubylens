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
| Anything untranslated | kept verbatim as `rust!(%q{…})` |

## Testing

Tests run against 64 real crates (2,789 files: tokio, serde, syn, clap, regex, rayon, …):

- **No crashes.** 1 file fell back to showing raw source.
- **97.5% of views are syntactically valid Ruby** (`ruby -c`).
- **Position maps are accurate.** In 400 random files, all 195,834 identifier tokens map back to the right Rust token, apart from intentional renames like `len` → `size`.
- **Extension end-to-end test** (`node editors/vscode/test/run.js`): definition, hover, references, outline and round-trips, with a mock rust-analyzer.

## Known limitations

- **Lossy by design.** Ownership, borrowing, lifetimes and `Send`/`Sync` are erased, and they often explain *why* code is shaped the way it is. Use the view as a reading aid; trust the Rust.
- **Bindings inside or-patterns.** Patterns like `A(x) | B(x)` can't be expressed in Ruby, which forbids them. Those few views aren't valid Ruby.
- **Untested with live rust-analyzer.** The extension uses rust-analyzer only through VS Code's standard `execute*Provider` commands, but it has been tested against a mock, not a live server.
- **Macro bodies.** Calls like `quote!`, `ast_struct!` or `tokio::select!` are shown verbatim unless their arguments parse as expressions.
- **Partly tuned for Ruby readers.** A few method renames assume iterator semantics (`map`/`filter` on `Option` read as Enumerable).

## Ideas for next steps

- **Neovim / other editors.** The same pattern works as a tiny LSP proxy serving `rbview:` buffers.
- **Workspace symbol search** over `symbols.json`.
- **Optional "explain this method" pass.** An LLM could rewrite one method body more idiomatically, cached by hash, without touching the deterministic skeleton or its navigation.
