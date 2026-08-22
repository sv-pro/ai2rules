# D46 — Governance shims never execute project-local harness binaries


**Date:** 2026-07-23.

- **Context:** A security review found that Claude Code and OpenCode bootstrap shims searched
  `$CLAUDE_PROJECT_DIR` or the OpenCode project directory for `target/release/harness` and
  `target/debug/harness` before falling back to an installed binary. A governed repository could
  check in or generate a fake `target/debug/harness`; the shim would execute it with the developer's
  privileges and accept its forged governance response.
- **Decision:** The governed project is untrusted for binary resolution. Installed Claude shims bake
  the installer-owned absolute `--bin-dir` path, and live shims resolve only an explicit absolute
  `HARNESS_BIN`/`AI2RULES_HARNESS` override or standard installer-owned absolute locations
  (`~/.local/bin/harness`, `/usr/local/bin/harness`, `/opt/ai2rules/bin/harness`). OpenCode follows
  the same rule with `AI2RULES_HARNESS` plus those installed locations. No live shim searches
  `target/` under the governed project or falls back to a relative executable.
- **Why:** The shim runs before the kernel can decide anything, so binary identity is the bootstrap
  trust root. A target project must not choose the program that enforces governance over itself.
- **Alternatives rejected:** keep project-local target lookup for developer convenience (the
  reviewed bypass); use `PATH` fallback in shims (can be influenced by project/session launch
  context and is not an installer-owned absolute path); require every user to set an env var
  manually (worse install ergonomics than baking the installer path).
- **Related:** D37 (live-hook cutover), D24 (gate ABI), `scripts/install-governance.sh`,
  `.claude/hooks/world-gate.{sh,py}`, `.opencode/plugin/ai2rules-gate.ts`.
