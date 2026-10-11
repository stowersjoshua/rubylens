# 0011. Agent skills quote the view; they never translate

Status: Accepted
Date: 2026-10-11

## Context
More and more people read unfamiliar code with a coding agent beside them. Asked to explain Rust "as Ruby", an agent writes its own translation. That translation drifts from the code, smooths over what Ruby can't say, and sometimes invents behaviour: exactly what [0002](0002-verbatim-over-mistranslation.md) rules out for RubyLens itself. RubyLens already produces a deterministic projection ([0006](0006-deterministic-core.md)) that matches what the editor shows, with a map back to every Rust line. Agents just don't know to use it.

## Decision
RubyLens ships a plugin, `plugins/rubylens/`, listed by a marketplace file at the repository root (`.claude-plugin/marketplace.json`), with two skills:

- **read-rust-as-ruby**: quote RubyLens output unchanged, cite the Rust lines, say what the Ruby hides when it matters, and explain Rust that RubyLens kept as Rust as Rust. A small helper (`scripts/excerpt.py`) uses the view's block map to print the Ruby for given Rust lines.
- **setup-rubylens**: install, update and troubleshoot the CLI and the extension.

The skills use only the [Agent Skills](https://agentskills.io) standard's frontmatter fields, so agents other than Claude Code can load them. They are separate from the contributor skills in `.claude/skills/`. The crate, the extension and the plugin share one version.

## Consequences
- What the skills cite becomes a contract with other people's agents: the CLI's commands and flags, the view's header lines, the block map format, the extension's command titles, settings and keys, and the translation examples they quote. `scripts/plugin-check.py` fails when any of these drift, so a change to RubyLens and its skills land together.
- Every Ruby snippet in the skills must be real output, found in the golden snapshots or the demo view.
- The helper reads map version 2. Bumping `GEN_VERSION` means updating it (the check fails until then).
- Users of the helper need python3. Without it, the skill falls back to reading the whole view.

## Alternatives considered
- **No skills.** Agents keep translating by hand, and readers get a confident, unfaithful Ruby.
- **A `--lines` flag on `rubylens gen` instead of the helper.** No python3 needed, but it adds CLI surface that no editor uses. Worth revisiting if users without python3 ask for it.
- **An MCP server.** More moving parts for the same three commands; the CLI is already the interface.
- **Have RubyLens write explanations.** Generated prose has no place in the deterministic core ([0006](0006-deterministic-core.md)); explaining is the agent's job, quoting is RubyLens's.
