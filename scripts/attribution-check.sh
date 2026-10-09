#!/usr/bin/env bash
# Fails if a commit credits an AI tool. Project rule: commits and PRs carry no AI attribution
# (no Co-Authored-By trailers, "Generated with ..." lines or session links), and commits are
# authored and committed by the repository owner, never under an AI tool's identity.
#
#   scripts/attribution-check.sh <message-file>      check one message and the identity about to
#                                                    commit it (used by .githooks/commit-msg)
#   scripts/attribution-check.sh --range A..B        check every commit in a range (used by CI)
set -euo pipefail

PATTERN='^[[:space:]]*co-authored-by:.*(claude|anthropic|copilot|chatgpt|openai|gemini|cursor|codex|devin|aider)|noreply@anthropic\.com|generated (with|by) .*(claude|copilot|chatgpt|cursor|codex|ai)|^[[:space:]]*claude-session:|🤖'

# Author/committer identities of AI tools (a person named Claude with a surname is fine).
IDENT_PATTERN='@anthropic\.com>|^claude <|(claude|copilot|devin|cursor|codex)[a-z-]*\[bot\]'

check_ident() { # $1 = label, $2 = "Name <email>"
  if printf '%s\n' "$2" | grep -qiE "$IDENT_PATTERN"; then
    echo "✗ $1 is \"$2\", an AI tool's identity, which this project doesn't allow." >&2
    echo "  Commit as the repository owner: git config user.name / user.email (see AGENTS.md → Commits)." >&2
    return 1
  fi
}

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
    label="commit $(git log -1 --format='%h %s' "$sha")"
    git log -1 --format=%B "$sha" | check "$label" || status=1
    check_ident "$label: author" "$(git log -1 --format='%an <%ae>' "$sha")" || status=1
    check_ident "$label: committer" "$(git log -1 --format='%cn <%ce>' "$sha")" || status=1
  done
  exit $status
fi

# Drop the comment lines git adds to the editor template before checking.
status=0
grep -v '^#' "${1:?usage: attribution-check.sh <message-file> | --range A..B}" | check "commit message" || status=1
# The identity this commit is being made with (git exports --author and env overrides to hooks).
for who in AUTHOR COMMITTER; do
  ident="$(git var "GIT_${who}_IDENT" 2>/dev/null | sed -E 's/ [0-9]+ [-+][0-9]{4}$//')" || continue
  check_ident "this commit's $(echo "$who" | tr '[:upper:]' '[:lower:]')" "$ident" || status=1
done
exit $status
