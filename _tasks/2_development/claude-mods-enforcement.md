# AI2-41 / #99 — Claude mods enforcement authority

Canonical scope: https://github.com/sv-pro/ai2rules/issues/99
Linear: https://linear.app/ai2rules/issue/AI2-41/

## 2026-10-03 — first executable evidence

Project-hook denial override reproduced on Claude Code 2.1.288 + npm harness
0.6.0. Real cc-hook emits deny, mod logs prior=deny then allows, and native Write
creates the temporary marker. Control and no-mod baseline pass. Repeated in a
fresh root with explicit effect/decision assertions; all pass.

Artifacts: `docs/benchmarks/claude-mods/{README.md,probe.py}` and
`docs/benchmarks/claude-mods/results/project-2.1.288.json`.
See branch HANDOFF.md. No real model inference or production changes.

Remaining: genuine managed-hook mod off/on comparison in an isolated test
environment. NOT RUN; keep both issues open. Next decision follows that result;
do not build a second policy engine or claim prompt-injection persistence.
