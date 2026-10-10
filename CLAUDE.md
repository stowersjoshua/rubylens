@AGENTS.md

## Claude Code specifics

- **Skills** in `.claude/skills/` cover the common workflows:
  - `change-translation`: change or add how a Rust construct renders.
  - `debug-navigation`: a jump, hover or reference lands in the wrong place.
  - `update-extension`: change the VS Code extension, rebuild the `.vsix`, refresh media.
  - `improve-performance`: make RubyLens faster without changing output, and add a guard for what you fixed.
  - `verify-changes`: pick and interpret the right checks before committing.
- **Shared settings** (`.claude/settings.json`) turn off commit and PR attribution, and a PreToolUse hook (`.claude/hooks/guard-attribution.sh`) blocks `git commit` and `gh pr` commands that contain attribution. If the hook blocks a command that isn't really a commit, for example one that greps for attribution text, move the text into a file instead of working around the hook.
- Before your first commit, check `git config user.name` and `user.email`. Agent environments often set an AI identity globally; set the repo-local identity to the repository owner's instead (ask if you don't know it). The commit-msg hook rejects AI identities.
- Run `scripts/check.sh` yourself before reporting a change as done, and quote its summary line. Don't claim checks pass without running them.
- For anything that changes `src/emit.rs`, show the reviewed `git diff tests/golden` excerpt in your summary, so the human can see what readers will see differently.
