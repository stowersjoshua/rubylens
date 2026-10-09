#!/usr/bin/env bash
# Fails if a commit message credits an AI tool. Project rule: commits and PRs carry no
# AI attribution (no Co-Authored-By trailers, "Generated with ..." lines or session links).
#
#   scripts/attribution-check.sh <message-file>      check one message (used by .githooks/commit-msg)
#   scripts/attribution-check.sh --range A..B        check every commit in a range (used by CI)
set -euo pipefail

PATTERN='^[[:space:]]*co-authored-by:.*(claude|anthropic|copilot|chatgpt|openai|gemini|cursor|codex|devin|aider)|noreply@anthropic\.com|generated (with|by) .*(claude|copilot|chatgpt|cursor|codex|ai)|^[[:space:]]*claude-session:|🤖'

check() { # $1 = label, stdin = message
  local hits
  hits="$(grep -inE "$PATTERN" || true)"
  if [ -n "$hits" ]; then
    echo "✗ $1 contains AI attribution, which this project doesn't allow:" >&2
    echo "$hits" | sed 's/^/    /' >&2
    echo "  Remove those lines (see AGENTS.md → Commits)." >&2
    return 1
  fi
}

if [ "${1:-}" = "--range" ]; then
  status=0
  for sha in $(git rev-list "$2"); do
    git log -1 --format=%B "$sha" | check "commit $(git log -1 --format='%h %s' "$sha")" || status=1
  done
  exit $status
fi

# Drop the comment lines git adds to the editor template before checking.
grep -v '^#' "${1:?usage: attribution-check.sh <message-file> | --range A..B}" | check "commit message"
