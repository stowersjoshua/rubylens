#!/usr/bin/env bash
# Checks the checks: injects realistic mistakes into a throwaway clone of the current commit
# and requires each to be caught by the check that claims to catch it. Run it after changing
# scripts/check.sh, tests/golden.rs, scripts/corpus-check.py or the attribution guards.
#
#   scripts/check-the-checks.sh      (needs ruby; takes a few minutes)
set -uo pipefail
SRC="$(cd "$(dirname "$0")/.." && pwd)"
N="$(mktemp -d)/rbview"
trap 'rm -rf "$(dirname "$N")"' EXIT
git clone -q "$SRC" "$N" && cd "$N"
git config core.hooksPath .githooks
git config user.name "Test" && git config user.email "test@example.com"
[ -d "$SRC/node_modules" ] && ln -s "$SRC/node_modules" node_modules || npm install --silent --no-audit --no-fund
cargo build --release -q 2>/dev/null
pass=0; fail=0
caught() { # caught <label> <evidence-regex> <command...>  — the command must FAIL and print the evidence
  local label="$1" evidence="$2"; shift 2
  local log="$N/../check.log"
  if "$@" >"$log" 2>&1; then echo "  ✗ NOT caught: $label"; fail=$((fail+1))
  elif ! grep -qE "$evidence" "$log"; then echo "  ✗ failed for another reason: $label"; grep -E "✗|FAILED|error" "$log" | head -3 | sed 's/^/      /'; fail=$((fail+1))
  else echo "  ✓ caught: $label — $(grep -m1 -E "$evidence" "$log" | sed 's/^[[:space:]]*//' | cut -c1-100)"; pass=$((pass+1)); fi
  git reset -q --hard; git clean -qfd -e node_modules 2>/dev/null
}
mutate() { python3 - "$@" <<'PY'
import sys
path, old, new = sys.argv[1], sys.argv[2], sys.argv[3]
s = open(path).read()
assert old in s, f"mutation target not found in {path}: {old[:60]}"
open(path, "w").write(s.replace(old, new, 1))
PY
}

echo "1. a method name emitted unmapped (push instead of span_tok)"
mutate src/emit.rs 'self.span_tok(&rb, m.method.span());' 'self.out.push(&rb);'
caught "golden map coverage" "rust_identifiers_are_reachable.*FAILED" cargo test -q --test golden

echo "2. a rename hard-coded in emitter code instead of the tables"
mutate src/emit.rs 'self.span_tok(&rb, m.method.span());' 'self.span_tok(&rb.replace("size", "length"), m.method.span());'
caught "golden identity check" "mapped_identifiers_land.*FAILED" cargo test -q --test golden mapped_identifiers

echo "3. a golden case added without a spec row"
printf 'pub fn shiny() -> u8 { 1 }\n' > tests/golden/shiny_new_rule.rs
caught "spec linkage" "every_case_is_specified.*FAILED" cargo test -q --test golden every_case

echo "4. output changed but snapshots not reviewed/updated"
mutate src/emit.rs '("len", "size"),' '("len", "length"),'
caught "golden snapshots" "snapshots_match.*FAILED" cargo test -q --test golden snapshots_match

echo "5. signature comment re-maps the fn name (code line must win)"
mutate src/emit.rs 'self.src_tokens_except(sig, &skip);' 'self.src_tokens(sig);'
caught "extension test: jump lands on def, not comment" "FAIL: definition lands on" sh -c 'cargo build --release -q && node editors/vscode/test/run.js'

echo "6. extension source edited without rebuilding the .vsix"
printf '\n// tweak\n' >> editors/vscode/extension.js
caught ".vsix matches source" "^\\.vsix matches source +✗" scripts/check.sh

echo "7. a commit that credits an AI tool"
printf 'x\n' > note.txt && git add note.txt
caught "commit-msg hook" "contains AI attribution" git commit -q -F scripts/fixtures/commit-msg-attributed.txt

echo "8. a cache folder committed"
mkdir -p .rbview && printf 'x' > .rbview/leak.rb && git add -f .rbview/leak.rb
caught "no tracked build output" "^no tracked build output +✗" scripts/check.sh

echo "9. a doc link that points nowhere"
printf '\nSee [the plan](docs/missing-plan.md).\n' >> AGENTS.md && git add AGENTS.md
caught "markdown links" "^markdown links +✗" scripts/check.sh

echo "10. a lint error in the extension"
printf '\nvar unused = 1;\n' >> editors/vscode/srcmap.js
caught "eslint" "no-var" npx eslint .

echo "11. path calls (a::b::f) silently stop mapping — nothing maps wrongly, things just vanish"
mutate src/emit.rs 'self.span_tok(&n, last.ident.span());' 'self.out.push(&n);'
caught "corpus coverage" "^FAIL" scripts/corpus-check.py --show 0

echo
echo "$pass caught, $fail missed"
[ $fail = 0 ]
