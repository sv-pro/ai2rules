# CLAUDE.md

Project instructions live in **[`AGENTS.md`](AGENTS.md)** — the single source of
truth shared across AI coding assistants (Claude Code, Codex, Antigravity). It is
imported below so Claude Code picks it up automatically; edit `AGENTS.md`, not
this file.

@AGENTS.md

<!-- uh:begin agent-claude-code sha=9308f6da24b5 -->
## For Claude Code (generated)

Your roles here:

- **radar** (fallback #1): Find new threats; each finding goes to Linear triage (task files live in the private hq repo). Write only: `scripts/**`
- **engine** (first choice): Write the failing test, implement the defense, keep the suite green. Write only: `crates/**`, `tests/**`
- **megaphone** (first choice): Turn a shipped defense into a demo (.tape) and a post. Write only: `docs/**`, `blog/**`
- **critic** (fallback #1): Correcting review of a handed-off artifact; fixes in place, serialized, never concurrent with the owner. Write only: `crates/**`, `docs/**`, `blog/**`

Your entry points for project procedures:

- correcting-review: `/review-blog`

Your branches start with `claude/`.

- Cloud and mobile sessions see only what is committed to the repo.
<!-- uh:end agent-claude-code -->
