---
name: change-translation
description: Use when changing or adding how a Rust construct appears in the Ruby view (src/emit.rs) — new syntax support, a different rendering, a new method/macro rename, or fixing invalid or misleading Ruby output. Covers golden-test-first workflow, mapping rules, spec updates and the full regression run.
---

# Change how Rust renders as Ruby

The translation contract is `docs/spec/translation.md`. Its executable form is the golden cases in `tests/golden/`. Both change together with `src/emit.rs`, or `cargo test` fails.

## 1. Decide whether it should change at all

Check `docs/intent.md`. In order: navigation never lies, then faithful over pretty, then read like Ruby. If the "better" rendering could mislead about what the Rust does, keep it verbatim instead (decision 0002). If it needs type or name resolution, it can't be done in the generator (decision 0001); say so rather than guessing from names.

## 2. Write the golden case first

- Pick the area file in `tests/golden/` (structs_and_impls, enums_and_match, traits, iterators_and_closures, errors_and_options, strings_and_macros, control_flow, macro_items, fn_local_items, ruby_names, modules), or create `tests/golden/<new_area>.rs`.
- Add the smallest *plausible* Rust that shows the construct, including an edge case. It's parsed, never compiled.
- See the current output:
  ```sh
  cargo build --release && ./target/release/rbview gen tests/golden/<area>.rs --no-cache
  ```

## 3. Change the emitter

- **Is it a pure name change?** Add a row to `METHOD_RENAMES`, `MACRO_RENAMES`, `KEYWORD_RENAMES` or `DROPPED_METHODS` at the top of `src/emit.rs`. That's the whole change.
- Otherwise edit the relevant function: `expr`, `method_call`, `call`, `pat`, `macro_call` / `macro_stmt`, `item`, `fn_def`, `struct_def` or `enum_def`.
- **Mapping rules.** Read `docs/spec/source-map.md` § "Writing emitter code":
  - Every Rust-derived token goes through `span_tok(text, span)`, `node_tok(text, node)` or `src_tokens(node)`. Only Ruby-only text goes through `self.out.push`.
  - Map keywords to their own token (`x.return_token.span`), not the whole node.
  - Never map the same Rust token twice: the code line wins (`src_tokens_except`).
- If the construct can appear in a receiver position, check `needs_parens` / `expr_recv`.

## 4. Update snapshots and review them

```sh
UPDATE_GOLDEN=1 cargo test --test golden
git diff tests/golden
```

Read every changed line. Unrelated diffs mean you changed more than you meant to. Then run `cargo test --test golden` (no env var). It checks bounds, identity, coverage, `ruby -c`, and that the spec cites every case.

## 5. Update the spec

Edit the matching row in `docs/spec/translation.md`, or add one, and cite the case as `[golden/<area>](../../tests/golden/<area>.rs)`. For a known limitation that yields invalid Ruby, add the marker `// rbview-golden: ruby-invalid (reason)` to the case and list it under "Known limitations".

## 6. Run the full regression

```sh
scripts/check.sh --full
```

- **corpus**: no crashes, valid Ruby ≥ 97.7%, map mismatches ≤ 0.01% across 2,789 real files. A drop means your rule misfires on real code; `--show 20` lists examples.
- **navigation**: 100% of resolved jumps land correctly against a real rust-analyzer.
- **demo view stale?** Regenerate `examples/demo/shapes.rb` (the check prints the command).

If you improved the corpus numbers, raise the thresholds in `scripts/corpus-check.py` and the README's testing numbers.

## 7. Summarize

In your summary, include the reviewed golden diff excerpt and the `check.sh` summary line.
