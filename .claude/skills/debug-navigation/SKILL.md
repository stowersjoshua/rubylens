---
name: debug-navigation
description: Use when go-to-definition, hover, references or outline in a Ruby view lands on the wrong token, does nothing, or lands in a comment — or when scripts/lsp-check.sh or the golden map tests report misses. Covers reproducing with the LSP harness, reading the position map, the usual root causes and how to lock in the fix.
---

# Debug a navigation miss

Navigation is two lookups around rust-analyzer: Ruby position → Rust (`rbToRs`), and the target Rust range → Ruby (`rsRangeToRb`). Both are in `editors/vscode/srcmap.js` and use the map from `src/emit.rs`. A miss is almost always a map problem, not a rust-analyzer problem.

## 1. Reproduce headlessly

```sh
cargo build --release
node editors/vscode/test/lsp-e2e.js --ra "$(command -v rust-analyzer)" \
  --project <crate dir> --file <src/file.rs> --verbose --limit 2000
```

It clicks every identifier in the view and prints each miss with the Ruby line it came from, the Rust target, the expected text and the Ruby line it landed on. Add `--all-features` if the code is behind cargo features (otherwise rust-analyzer resolves nothing there).

## 2. Look at the map around the spot

```sh
./target/release/rubylens gen <file.rs> --json --no-cache > /tmp/v.json
python3 - <<'PY'
import json
d = json.load(open('/tmp/v.json')); rb = d['ruby'].split('\n')
RB_LINE = 41  # 0-based Ruby line of interest
for s in d['map']['segs']:
    if s[0] == RB_LINE:
        print(s, repr(rb[s[0]][s[1]:s[1]+s[2]]))
PY
```

Segments are `[rb_line, rb_col, rb_len, rs_line, rs_col, rs_len]`: 0-based, UTF-16 columns. ASCII slicing is fine for most code.

## 3. Usual root causes

| Symptom | Cause | Fix |
|---|---|---|
| Lands in a `#` comment instead of on code | the same Rust token is mapped twice (signature comment and `def`) | pass that token's position to `src_tokens_except` for the comment copy |
| Lands one or more columns off | mapped text doesn't match the source text, e.g. a literal written with delimiters or collapsed whitespace | map only the exact source text (`src_exact`) and push delimiters unmapped |
| Lands at the start of a long expression | keyword mapped to a whole multi-line node (`rs_len = 1`) | map the keyword's own token (`return_token`, `question_token`) |
| Nothing happens (jump to a module or file) | target is a whole file, or inside a long doc comment with no segments | `rsToRb` falls back to the nearest code; check that the fallback still applies |
| Clicked word unmapped | emitted with `self.out.push` | emit with `span_tok` / `node_tok` |
| Wrong token after a rename | rename hard-coded in emitter code | move it to the tables in `src/emit.rs` |
| Many misses inside `cfg_*! { … }` or DSL macros | macro body flattened onto one line | translate item bodies (`macro_stmt`) or use `Out::verbatim` |

## 4. Lock it in

1. Reduce the failing code to a few lines in the matching `tests/golden/<area>.rs`.
2. Confirm `cargo test --test golden` fails on the identity or coverage invariant, if it's a map bug, before fixing.
3. Fix, then `UPDATE_GOLDEN=1 cargo test --test golden` and review `git diff tests/golden`.
4. `scripts/check.sh --full`: navigation must stay at 100%.
