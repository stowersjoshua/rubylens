#!/usr/bin/env bash
# The one command to run before committing. Runs every step, then reports.
#
#   scripts/check.sh          fast checks (about a minute)
#   scripts/check.sh --full   + corpus regression (needs ruby) + real rust-analyzer navigation
#
# What each step protects is described in docs/testing.md.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
FULL=0
[ "${1:-}" = "--full" ] && FULL=1

results=()
LOG="$(mktemp -d)"
trap 'rm -rf "$LOG"' EXIT
step() { # step <name> <command...>
  local name="$1"; shift
  printf '%-34s' "$name"
  if "$@" >"$LOG/$name.log" 2>&1; then
    echo "✓"; results+=("ok")
  else
    echo "✗"; results+=("fail")
    sed 's/^/    │ /' "$LOG/$name.log" | tail -40
  fi
}

# ---------------------------------------------------------------- repository rules
tracked_junk() {
  local junk
  junk="$(git ls-files | grep -E '(^|/)(target|node_modules|\.rubylens)/' || true)"
  [ -z "$junk" ] || { echo "build output or caches are tracked:"; echo "$junk"; return 1; }
}
vsix_in_sync() {
  # The committed .vsix is what users install; it must contain the current extension source.
  python3 - <<'PY'
import glob, os, sys, zipfile
z = zipfile.ZipFile("editors/vscode/rubylens-0.1.0.vsix")
shipped = set(z.namelist())
source = ["package.json"] + sorted(os.path.basename(p) for p in glob.glob("editors/vscode/*.js"))
bad = [f for f in source
       if "extension/" + f not in shipped or z.read("extension/" + f) != open("editors/vscode/" + f, "rb").read()]
if bad:
    sys.exit("editors/vscode/rubylens-0.1.0.vsix is stale for: " + ", ".join(bad)
             + "\nrebuild: (cd editors/vscode && npx @vscode/vsce package --allow-missing-repository --skip-license -o rubylens-0.1.0.vsix)")
PY
}
demo_view_fresh() {
  ./target/release/rubylens gen examples/demo/src/shapes.rs --root examples/demo --no-cache | diff -u examples/demo/shapes.rb - \
    || { echo "examples/demo/shapes.rb is stale: ./target/release/rubylens gen examples/demo/src/shapes.rs --root examples/demo --no-cache > examples/demo/shapes.rb"; return 1; }
}
markdown_links() {
  python3 - <<'PY'
import os, re, subprocess, sys
bad = []
for md in subprocess.run(["git", "ls-files", "*.md"], capture_output=True, text=True).stdout.split():
    text = re.sub(r"```.*?```", "", open(md, encoding="utf-8").read(), flags=re.S)  # skip code blocks
    text = re.sub(r"`[^`\n]*`", "", text)  # and inline code
    for target in re.findall(r"\]\(([^)\s]+)\)", text):
        if re.match(r"^[a-z]+:|^#", target):
            continue
        path = os.path.normpath(os.path.join(os.path.dirname(md), target.split("#")[0]))
        if not os.path.exists(path):
            bad.append(f"{md}: {target}")
if bad:
    sys.exit("broken relative links:\n  " + "\n  ".join(bad))
PY
}
unpushed_attribution() {
  local range="${RUBYLENS_CHECK_RANGE:-}"
  if [ -z "$range" ]; then
    git rev-parse -q --verify origin/master >/dev/null || { echo "no origin/master; skipped"; return 0; }
    range="origin/master..HEAD"
  fi
  scripts/attribution-check.sh --range "$range"
}
hooks_enabled() {
  [ "$(git config core.hooksPath)" = ".githooks" ] && return 0
  [ -n "${CI:-}" ] && return 0
  echo "git hooks aren't enabled; run scripts/setup.sh"; return 1
}
js_deps() { [ -d node_modules ] || npm install --silent --no-audit --no-fund; }

# ---------------------------------------------------------------- steps
step "build"                         cargo build --release --quiet
step "rustfmt"                       cargo fmt --check
step "clippy"                        cargo clippy --all-targets --quiet -- -D warnings
step "tests (cargo test)"            cargo test --quiet
step "generation scales linearly"   scripts/scaling-check.py
step "eslint"                        sh -c 'npm install --silent --no-audit --no-fund >/dev/null 2>&1 || true; npx eslint .'
step "extension tests (mock vscode)" node editors/vscode/test/run.js
step "attribution guards"            scripts/guard-selftest.sh
step "no tracked build output"       tracked_junk
step ".vsix matches source"          vsix_in_sync
step "demo view up to date"          demo_view_fresh
step "markdown links"                markdown_links
step "no attribution in commits"     unpushed_attribution
step "git hooks enabled"             hooks_enabled
if [ $FULL = 1 ]; then
  step "corpus (66 crates)"          scripts/corpus-check.py --show 10
  step "navigation (rust-analyzer)"  scripts/lsp-check.sh
fi

fails=0
for r in "${results[@]}"; do [ "$r" = fail ] && fails=$((fails + 1)); done
echo
if [ $fails = 0 ]; then
  echo "all ${#results[@]} checks passed"
  [ $FULL = 1 ] || echo "(changed src/emit.rs, src/out.rs or editors/vscode/srcmap.js? run scripts/check.sh --full)"
else
  echo "$fails of ${#results[@]} checks failed"
  exit 1
fi
