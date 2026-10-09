# Translation spec

How each Rust construct appears in the Ruby view. This is the contract. The golden cases in `tests/golden/` are its executable form, and `cargo test` fails if a case isn't cited here or a citation points nowhere.

The rules follow [docs/intent.md](../intent.md): navigation never lies, be faithful rather than executable, and read like Ruby.

**Every rule obeys the mapping invariants** in [source-map.md](source-map.md). Each Rust-derived token in the view maps back to its Rust token, including tokens inside comments and verbatim regions.

## File and module structure

| Rust | View | Golden |
|---|---|---|
| file `src/a/b.rs` | wrapped in `module A::B` (crate roots, `examples/`, `tests/`: no wrapper) | [golden/modules](../../tests/golden/modules.rs) |
| `mod name { … }` | `module Name … end` | [golden/modules](../../tests/golden/modules.rs) |
| `mod name;` | `# mod name  (separate file — follow with go-to-definition)` | |
| `use a::b::{C, D as E};` | `# use a::b::{C, D as E}` (each name mapped) | [golden/modules](../../tests/golden/modules.rs) |
| `const X: T = v;` / `static X: T = v;` | `X = v  # T` / `X = v  # static T` | [golden/modules](../../tests/golden/modules.rs) |
| `type A = B<C>;` | `A = B  # type alias: B<C>` | [golden/modules](../../tests/golden/modules.rs) |
| free `fn` in a module | `def self.name` (module function) | [golden/modules](../../tests/golden/modules.rs) |
| items declared inside a `fn` | listed in place as `# local struct X — shown after this method`, emitted right after the method with `# (declared inside fn f)` | [golden/fn_local_items](../../tests/golden/fn_local_items.rs) |
| doc comments `///` | `# …` comments | [golden/structs_and_impls](../../tests/golden/structs_and_impls.rs) |
| other attributes | `# #[derive(Debug)]` (tokens mapped) | [golden/structs_and_impls](../../tests/golden/structs_and_impls.rs) |

## Types

| Rust | View | Golden |
|---|---|---|
| `struct P { x: f64 }` | `class P` + `attr_accessor :x  # f64` | [golden/structs_and_impls](../../tests/golden/structs_and_impls.rs) |
| `struct M(f64);` | `class M` + `# tuple struct: self[0]: f64`; `self.0` → `self[0]` | [golden/structs_and_impls](../../tests/golden/structs_and_impls.rs) |
| `struct Marker;` | `class Marker; end` | [golden/structs_and_impls](../../tests/golden/structs_and_impls.rs) |
| `enum E { A { x: T }, B(T), C }` | `module E  # enum` with `A = Data.define(:x) { include E }  # { x: T }`, `B = Data.define(:_0) …`, `C = Data.define …` | [golden/enums_and_match](../../tests/golden/enums_and_match.rs) |
| `trait T { fn f(&self); fn g(&self) {…} }` | `module T  # trait`; required: `def f = raise NotImplementedError  # required`; default methods have bodies | [golden/traits](../../tests/golden/traits.rs) |
| associated `type Out;` / `const K: T = v;` | `# associated type Out` / `K = v  # T` | [golden/traits](../../tests/golden/traits.rs) |
| `impl P { … }` | reopens `class P  # impl P` (`module` when `P` is an enum or trait, via `symbols.json`) | [golden/structs_and_impls](../../tests/golden/structs_and_impls.rs) |
| `impl T for P { … }` | `class P  # impl T for P` + `include T` | [golden/traits](../../tests/golden/traits.rs) |
| generics and `where` clauses | kept in comments: `class P  # <T: Clone> where …` | [golden/traits](../../tests/golden/traits.rs) |
| lowercase type names (`timeval`, `str`) | Ruby constants: `Timeval`, `Str` | [golden/ruby_names](../../tests/golden/ruby_names.rs) |

## Functions

| Rust | View | Golden |
|---|---|---|
| `fn f(&self, a: T) -> R` | `# fn f(&self, a: T) -> R` (signature, tokens mapped except the name and params) then `def f(a)` | [golden/structs_and_impls](../../tests/golden/structs_and_impls.rs) |
| associated fn (no `self`) | `def self.f` | [golden/structs_and_impls](../../tests/golden/structs_and_impls.rs) |
| destructuring parameter `(a, b): (T, U)` | `def f(arg0)` + `arg0 => [a, b]` at the top of the body | |
| `_` parameter | `_` | |
| `self.field` | `@field` | [golden/structs_and_impls](../../tests/golden/structs_and_impls.rs) |
| `Self` | the impl's type name | [golden/structs_and_impls](../../tests/golden/structs_and_impls.rs) |
| `P { x, y }` (struct literal) | `P[x: x, y: y]`, which is distinct from a `P.new` call; `..base` → `**base.to_h` | [golden/structs_and_impls](../../tests/golden/structs_and_impls.rs) |
| `P(1)` (tuple-struct/variant constructor) | `P[1]` | [golden/errors_and_options](../../tests/golden/errors_and_options.rs) |
| `a::b::f(x)` / `T::f(x)` | `A::B.f(x)` / `T.f(x)` | [golden/modules](../../tests/golden/modules.rs) |
| closure as last argument | block: `{ \|x\| … }` or `do \|x\| … end` | [golden/iterators_and_closures](../../tests/golden/iterators_and_closures.rs) |
| closure elsewhere | lambda: `->(x) { … }` | [golden/iterators_and_closures](../../tests/golden/iterators_and_closures.rs) |
| Ruby keyword as a local or parameter (`end`, `next`) | suffixed `end_`; as a field or method name it stays `end` | [golden/ruby_names](../../tests/golden/ruby_names.rs) |
| raw identifier `r#type` | `type` | [golden/ruby_names](../../tests/golden/ruby_names.rs) |

## Expressions and control flow

| Rust | View | Golden |
|---|---|---|
| `match x { … }` | `case x` / `in pattern` / `end`; guards → `in p if cond` | [golden/enums_and_match](../../tests/golden/enums_and_match.rs) |
| struct / tuple / unit variant patterns | `in E::A(x:)`, `in E::B[v]`, `in E::C`; `..` → `**` | [golden/enums_and_match](../../tests/golden/enums_and_match.rs) |
| `Some(x)` / `None` | `x` (with `unless x.nil?` in match arms) / `nil` | [golden/enums_and_match](../../tests/golden/enums_and_match.rs) |
| `if let Some(x) = e` | `if (x = e)` | [golden/enums_and_match](../../tests/golden/enums_and_match.rs) |
| `if let P = e` | `if e in P` | |
| `let Some(x) = e else { … };` | `x = e` + `if x.nil?  # let-else` | [golden/enums_and_match](../../tests/golden/enums_and_match.rs) |
| `matches!(e, P)` | `(e in P)` | [golden/enums_and_match](../../tests/golden/enums_and_match.rs) |
| `let (a, b) = e;` / `let P { … } = e;` | `a, b = e` / `e => P(…)` | [golden/ruby_names](../../tests/golden/ruby_names.rs) |
| `for x in it { … }` | `it.each do \|x\| … end` | [golden/control_flow](../../tests/golden/control_flow.rs) |
| `while let Some(x) = e` / `loop` / `while c` | `while (x = e)` / `loop do` / `while c` | [golden/control_flow](../../tests/golden/control_flow.rs) |
| `break` / `continue` / labels | `break` / `next` (from `KEYWORD_RENAMES`); labels kept as comments | [golden/control_flow](../../tests/golden/control_flow.rs) |
| `a..b` / `a..=b` / `x[a..]` | `(a...b)` / `(a..b)` / `x[(a...)]`; a full `x[..]` is just `x` | [golden/control_flow](../../tests/golden/control_flow.rs) |
| `x as f64` / `x as usize` | `x.to_f` / `x.to_i` | [golden/enums_and_match](../../tests/golden/enums_and_match.rs) |
| `&x`, `*x`, `&mut x` | `x` | [golden/enums_and_match](../../tests/golden/enums_and_match.rs) |
| `*target = v` where target isn't assignable in Ruby | `target.replace(v)`; compound `*t += v` → `t.add!(v)` | |
| `expr?` | `expr.try!`, mapped to the `?` ("unwrap, or return the error") | [golden/errors_and_options](../../tests/golden/errors_and_options.rs) |
| `Ok(v)` / `Err(e)` | `Ok[v]` / `Err[e]` (`Data` classes in the prelude) | [golden/errors_and_options](../../tests/golden/errors_and_options.rs) |
| `*map.entry(k).or_insert(v) += n` | `map[k] = (map[k] \|\| v) + n` (`[` maps to `entry`, `\|\|` to `or_insert`) | [golden/errors_and_options](../../tests/golden/errors_and_options.rs) |
| `x.unwrap_or(d)` / `x.is_some()` / `x.is_none()` / `x.get(k)` | `(x \|\| d)` / `!x.nil?` / `x.nil?` / `x[k]` | [golden/errors_and_options](../../tests/golden/errors_and_options.rs) |
| `a.cmp(&b)` / `partial_cmp` | `(a <=> b)` | |

## Methods (tables in `src/emit.rs`)

Name changes are rows in tables, never ad-hoc code ([decision 0005](../decisions/0005-single-source-tables.md)):

| Table | Examples |
|---|---|
| `METHOD_RENAMES` | `len` → `size`, `is_empty` → `empty?`, `filter` → `select`, `fold` → `inject`, `contains` → `include?`, `to_lowercase` → `downcase` |
| `DROPPED_METHODS` | `iter`, `into_iter`, `clone`, `cloned`, `as_ref`, `as_str`, `into`, `to_owned`… (zero-argument only) |
| `MACRO_RENAMES` | `println!` → `puts`, `eprintln!` → `warn`, `panic!` → `raise`, `assert_eq!` → `assert`, `dbg!` → `p` |
| `KEYWORD_RENAMES` | `continue` → `next`, `None` → `nil` |

Contextual renderings that aren't name swaps:

- `collect::<HashMap<…>>()` → `.to_h`, `collect::<HashSet<…>>()` → `.to_set` and `collect::<String>()` → `.join`, each mapped over the method and its turbofish. A bare `collect()` is dropped.
- `String::new()` → `""`, `Vec::new()` → `[]`, `HashMap::new()` → `{}`.
- `Box::new(x)`, `Rc::new(x)` and `String::from(x)` → `x`.

Covered by [golden/iterators_and_closures](../../tests/golden/iterators_and_closures.rs) and [golden/errors_and_options](../../tests/golden/errors_and_options.rs).

## Literals and macros

| Rust | View | Golden |
|---|---|---|
| `"text"` / `'c'` / `b"raw"` | `"text"` (with `#{` escaped) / `"c"` / `"raw".b` | [golden/strings_and_macros](../../tests/golden/strings_and_macros.rs) |
| `b'0'` in expressions / patterns | `"0".ord` / `48`, with the Rust pattern as a trailing comment | [golden/strings_and_macros](../../tests/golden/strings_and_macros.rs) |
| integer and float suffixes (`10u32`, `1.5f64`) | dropped | |
| `format!("{} {x:?} {:.2}", a, b)` | `"#{a} #{x.inspect} #{format('%.2f', b)}"`, with the opening quote mapped to `format!` | [golden/strings_and_macros](../../tests/golden/strings_and_macros.rs) |
| `write!(f, …)` / `writeln!` | `f << "…"` | [golden/strings_and_macros](../../tests/golden/strings_and_macros.rs) |
| `vec![a, b]` / `vec![x; n]` | `[a, b]` / `Array.new(n, x)` | [golden/strings_and_macros](../../tests/golden/strings_and_macros.rs) |
| `assert!(c, "msg {}", x)` | `assert c, "msg #{x}"` | [golden/strings_and_macros](../../tests/golden/strings_and_macros.rs) |
| `todo!()` / `unimplemented!()` | `raise NotImplementedError` | [golden/strings_and_macros](../../tests/golden/strings_and_macros.rs) |
| macro whose body is items (`cfg_x! { fn a() {} }`), also inside `impl` | the items, translated, between `# cfg_x! {` and `# }` | [golden/macro_items](../../tests/golden/macro_items.rs) |
| `macro_rules!` definitions | verbatim comment lines, mapped character-for-character | [golden/macro_items](../../tests/golden/macro_items.rs) |
| other macro on its own line (DSLs like `select!`) | verbatim comment lines, mapped | [golden/macro_items](../../tests/golden/macro_items.rs) |
| other macro whose arguments parse as expressions | `name!(args…)` | |
| anything else untranslatable inline | `rust!(%q{…})`, exact and mapped when the source is one line ([decision 0002](../decisions/0002-verbatim-over-mistranslation.md)) | |

## Known limitations

| Rust | View | Golden |
|---|---|---|
| bindings inside or-patterns: `A(x) \| B(x)` | rendered as written; **not valid Ruby**, because Ruby forbids bindings in alternative patterns | [golden/or_pattern_bindings](../../tests/golden/or_pattern_bindings.rs) |
| methods on `Option`/`Result` that share iterator names (`map`, `and_then`) | look like `Enumerable` calls; the generator has no types ([decision 0001](../decisions/0001-syntax-only-generator.md)) | |

## Changing this spec

Follow the `change-translation` skill (`.claude/skills/change-translation/SKILL.md`). In short:

1. Write or extend a golden case first.
2. Change `src/emit.rs`.
3. Run `UPDATE_GOLDEN=1 cargo test --test golden` and review `git diff tests/golden` like code.
4. Update the row here.
5. Run `scripts/check.sh --full`.
