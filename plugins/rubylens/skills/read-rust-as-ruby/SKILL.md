---
name: read-rust-as-ruby
description: Explain Rust code to someone who thinks in Ruby by quoting RubyLens's generated Ruby view of it, with each snippet tied to its Rust lines, instead of translating by hand. Use when the user wants Rust shown or explained as Ruby, in Ruby terms or for a Rubyist, or is a Ruby developer trying to understand a Rust codebase, crate or function.
license: MIT
compatibility: Needs the rubylens CLI on PATH (see the setup-rubylens skill). The excerpt helper needs python3.
---

# Read Rust as Ruby

RubyLens turns a Rust file into a read-only Ruby projection: the same structure and names, in Ruby syntax, with Rust's types kept in comments. The projection is generated deterministically, so it matches what the user sees if they open the file with the RubyLens editor extension. Your job is to show them that projection and explain it, not to write your own.

## Rules

1. **Quote, never translate.** Every Ruby snippet you show is RubyLens output, unchanged. You may cut lines and mark the cut with a `# …` line, nothing else. A hand-written Ruby version drifts from the real code and invents behaviour; if the view can't show something, explain it in words.
2. **Cite the Rust.** Give each snippet's Rust location as `path:start-end` (the helper prints it), so the user can open the real code.
3. **Say what the Ruby hides when it matters.** Ownership and borrowing, error handling, exhaustive `match`, numeric casts and types all vanish or move into comments (see the table). Flag them when the question depends on them.
4. **Names that look like Ruby still mean Rust.** RubyLens renames a few methods to their Ruby equivalents, such as `len` to `size`. Every other method keeps its Rust name and its Rust behaviour, even where Ruby has a method of the same name. Rust's `lines` drops the line endings. `split` on an empty string gives `[""]`. `parse` returns a `Result`, and the type it parses to comes from context. Check the Rust before explaining such a method in Ruby terms.
5. **Rust that RubyLens kept as Rust is Rust.** Comment lines like `# thread_local! {` and inline `rust!(%q{…})` mean RubyLens had no honest Ruby for it. Explain those as Rust.
6. **It's a reading aid, not a port.** The view doesn't run, and it isn't advice on how to write the Ruby equivalent.

## Steps

1. Check RubyLens is installed: `command -v rubylens`. If it isn't, use the setup-rubylens skill if you have it. Otherwise, point the user to https://github.com/stowersjoshua/rubylens#install.
2. Find the Rust lines you're explaining: read the file, or search for the item (`fn total_area`).
3. Get the Ruby for those lines:

   ```sh
   python3 <this skill's folder>/scripts/excerpt.py src/shapes.rs 52-54
   ```

   It works from any folder. The first line, `== src/shapes.rs:52-54  ->  .rubylens/src/shapes.rb:67-70`, gives the Rust lines the excerpt really covers. It can be wider than you asked, because it widens to whole statements or items. Cite those lines. A second line, such as `== in: module Shapes > class Canvas  # impl Canvas`, names what encloses the excerpt. The Ruby follows.

   Without python3, run `rubylens gen src/shapes.rs` for the whole view and find the item by name: each function's `# fn …` comment repeats its Rust signature.
4. For an overview of a file, read `rubylens gen <file.rs>` yourself and quote only the parts you discuss.
5. If an `impl` shows as `class Shape  # impl Shape` but `Shape` is an enum or trait defined in another file, run `rubylens build` once at the repository root and try again. RubyLens reads one file at a time and learns crate-wide names from that build.
6. For a dependency, run the same commands on its source. `cargo metadata --format-version 1` lists each package's `manifest_path`, and the crate's `src/` sits beside it.

`rubylens` caches views in a `.rubylens/` folder at the repository root (or the crate's root, for a dependency). The folder ignores itself in git, and nothing leaves the machine.

## Reading the view

| In the view | Rust | What the Ruby hides |
|---|---|---|
| `# fn add(&mut self, shape: Shape, tag: Option<&str>) -> &mut Self` | the function's signature | Parameter and return types, and `&`/`&mut` borrows, exist only in this comment |
| `attr_accessor :shapes  # Vec<Shape>` | a struct field | its type |
| `module Shape  # enum`, `Circle = Data.define(:radius) { include Shape }  # { radius: f64 }` | an enum and its variants | the set of variants is closed |
| `case self`, `in Shape::Circle(radius:)` | `match` | Rust refuses to compile a `match` that misses a variant |
| `def area = raise NotImplementedError  # required` | a trait method without a default | every implementing type must define it, checked at compile time |
| `module Shape  # impl Area for Shape`, `include Area` | `impl Area for Shape` | |
| `class Canvas  # impl Canvas` | `impl Canvas`: methods on the type | |
| `Canvas[name: name.to_s, shapes: [], tags: {}]` | a struct literal `Canvas { … }` | `Canvas.new` is a call to the type's own `fn new`, not this |
| `@shapes.map { \|s\| s.area }.sum` | `self.shapes.iter().map(\|s\| s.area()).sum()` | `.iter()`, `.clone()`, `.into()`, `&` and `*` are dropped: whether data is borrowed, moved or copied |
| `.try!` | `?` | on an error (or `None`), the function returns it right there |
| `(w.parse \|\| 0.0)` | `w.parse().unwrap_or(0.0)` | `\|\|` is a fallback for a failed parse or a missing value, not Ruby truthiness |
| `Std::Fs.read_to_string(path)` | `std::fs::read_to_string(path)` | Rust's standard library, not a Ruby class: module paths become constants |
| `text.lines.enumerate.each do \|i, line\|` | `for (i, line) in text.lines().enumerate()` | `enumerate` pairs each item with its index, index first |
| `next` | `continue` | |
| `Ok[port]`, `Err["unknown shape #{other.inspect}"]` | `Ok(…)` / `Err(…)`: a `Result` | callers must handle both |
| `if (t = tag)` | `if let Some(t) = tag` | `Option`: `None` is `nil`, `Some(x)` is just `x` |
| `.map_err { \|e\| ConfigError::Missing[e.to_s] }` | a method on `Result` | `map`, `and_then` and friends on `Option`/`Result` look like `Enumerable` calls but aren't |
| `.to_i` | `as usize` (and other `as` casts) | casts can truncate or wrap silently |
| `COUNTER = 0  # static u32` | a `static` | |

For anything not covered here, see [RubyLens's translation spec](https://github.com/stowersjoshua/rubylens/blob/master/docs/spec/translation.md).

## Answer shape

~~~markdown
`Canvas#total_area` (`src/shapes.rs:52-54`) adds up every shape's area:

```ruby
def total_area
  @shapes.map { |s| s.area }.sum
end
```

The view hides one detail: Rust's `.iter()` borrows the shapes, so nothing is copied.
~~~

Lead with what the code does, show the quoted Ruby, then name what the Ruby hides only if it matters to the question. You can't tell whether the user has the RubyLens extension, so put it conditionally: with it, `Ctrl+Alt+R` (`Cmd+Alt+R` on macOS) opens the same view beside the Rust, with go-to-definition.
