# CLAUDE.md

Project instructions live in **[`AGENTS.md`](AGENTS.md)** — the single source of
truth shared across AI coding assistants (Claude Code, Codex, Antigravity). It is
imported below so Claude Code picks it up automatically; edit `AGENTS.md`, not
this file.

@AGENTS.md

<!-- uh:begin agent-claude-code sha=69855f0f5c4a -->
## For Claude Code (generated)

Your roles here:

- **radar** (fallback #1): Find new threats and write one discovery file per finding. Write only: `_tasks/1_discovery/**`, `scripts/**`
- **engine** (first choice): Write the failing test, implement the defense, keep the suite green. Write only: `crates/**`, `tests/**`, `_tasks/2_development/**`
- **megaphone** (fallback #1): Turn a shipped defense into a demo (.tape) and a post. Write only: `docs/**`, `blog/**`, `_tasks/3_advocacy/**`
- **critic** (first choice): Correcting review of a handed-off artifact; fixes in place, serialized, never concurrent with the owner. Write only: `crates/**`, `docs/**`, `blog/**`

Your entry points for project procedures:

- correcting-review: `/review-blog`

Your branches start with `claude/`.

- Cloud and mobile sessions see only what is committed to the repo.
<!-- uh:end agent-claude-code -->
