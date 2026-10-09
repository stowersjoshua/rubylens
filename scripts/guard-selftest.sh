#!/usr/bin/env bash
# Self-test for the attribution guards: the git commit-msg check and the Claude Code hook.
# Fixtures live in scripts/fixtures/ (kept out of command lines so the live hook doesn't fire).
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
F="$ROOT/scripts/fixtures"
export CLAUDE_PROJECT_DIR="$ROOT"
fail=0
expect() { # expect <wanted-exit> <label> <command...>
  local want="$1" label="$2"; shift 2
  "$@" >/dev/null 2>&1; local got=$?
  if [ "$got" = "$want" ]; then echo "  ✓ $label"; else echo "  ✗ $label (exit $got, wanted $want)"; fail=1; fi
}
expect 0 "commit-msg: clean message accepted"        "$ROOT/scripts/attribution-check.sh" "$F/commit-msg-clean.txt"
expect 1 "commit-msg: attributed message rejected"   "$ROOT/scripts/attribution-check.sh" "$F/commit-msg-attributed.txt"
expect 0 "hook: clean commit allowed"                 sh -c "'$ROOT/.claude/hooks/guard-attribution.sh' < '$F/hook-commit-clean.json'"
expect 2 "hook: attributed commit blocked"            sh -c "'$ROOT/.claude/hooks/guard-attribution.sh' < '$F/hook-commit-attributed.json'"
expect 2 "hook: attributed PR body blocked"           sh -c "'$ROOT/.claude/hooks/guard-attribution.sh' < '$F/hook-pr-attributed.json'"
exit $fail
