# AGENTS.md

Canonical guidance for AI coding assistants (Claude Code, Codex, Google
Antigravity, …) and humans working in this repository — the **CLI Agent
Harness**. This is the single source of truth for project conventions;
per-assistant files just point here (see [Per-assistant setup](#per-assistant-setup)).

Orient with `README.md` (overview), `docs/harness-architecture.md` (canonical
design), and `PLAN.md` (epic-level execution plan; the task source of truth).

## Conventions

- **Keep `README.md` current on every commit.** If a change affects project
  status, capabilities, crate layout, build/run instructions, or test counts,
  update the README's Status / layout / Build & test sections in the *same*
  commit. Keep `PLAN.md` epic checkboxes in sync too.
- **Don't commit the reference repos.** `repos/` holds `agent-hypervisor/`,
  `safe-mcp-proxy/`, and `mcp-tool-projection/` — separate git repositories kept
  only as references, not Cargo workspace members. Never `git add repos/`; stage
  harness paths explicitly.
- **Record architectural decisions in `DECISIONS.md`.** When a choice closes off
  a real alternative, append a `D<n>` entry (decision + alternatives + why) so it
  can be revisited later.

## Build & test

The local crate cache supports offline builds; prefer `--offline`.

```bash
cargo build --workspace --offline
cargo test  --workspace --offline
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --offline -- -D warnings
```

CI runs fmt-check, `clippy -D warnings`, build, and test on every push/PR.

## Architecture invariants (don't regress)

- The kernel (`world-kernel`) is pure: no I/O, no LLM, no mutable shared state.
- `IntentIR` is sealed — only `IRBuilder::build` can construct one.
- Taint is monotonic; `CompiledWorld` is immutable after construction.
- Only `ExecutionSpec` crosses into the executor.

## Per-assistant setup

These instructions are shared by reference, not by copy — keep the content here
and let each tool point at it:

- **Codex** and **Google Antigravity** read this `AGENTS.md` at the repo root
  natively; no extra file is needed. (Antigravity also supports workspace rules
  under `.agents/rules/` and global rules at `~/.gemini/GEMINI.md` if you want
  machine- or user-scoped additions.)
- **Claude Code** reads `CLAUDE.md`, which imports this file via `@AGENTS.md`.

When updating project conventions, edit **this file**; the per-assistant pointers
should not accumulate their own copies.
