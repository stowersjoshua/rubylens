#!/usr/bin/env bash
# PreToolUse hook (Bash): block `git commit` / `gh pr create|edit` commands whose text credits
# an AI tool. Same rule and pattern as the git commit-msg hook (scripts/attribution-check.sh),
# applied before the command runs. Exit 2 blocks the call and shows stderr to the agent.
input="$(cat)"
cmd="$(printf '%s' "$input" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("tool_input",{}).get("command",""))' 2>/dev/null || printf '%s' "$input")"
case "$cmd" in
  *"git commit"*|*"gh pr create"*|*"gh pr edit"*) ;;
  *) exit 0 ;;
esac
err="$(mktemp)"
trap 'rm -f "$err"' EXIT
if ! printf '%s\n' "$cmd" | "$CLAUDE_PROJECT_DIR/scripts/attribution-check.sh" /dev/stdin 2>"$err"; then
  cat "$err" >&2
  exit 2
fi
exit 0
