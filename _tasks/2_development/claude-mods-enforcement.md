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

## 2026-10-05 — setup recipe, precise runtime blocker

Prepared genuine Linux file-based managed-policy image and managed-only runner;
see `docs/benchmarks/claude-mods/MANAGED.md`. Original project runner/evidence
unchanged. Checked Python syntax, JSON, synthetic oracle gates and whitespace.
No Docker/Podman is available; namespace creation returned Operation not
permitted. Image build and both live rows remain NOT RUN. Need a disposable
Linux amd64 Docker host with build-time registry access, not an Enterprise
tenant or API credentials. Next: execute only the two managed cases with the
documented commands and retain source/tier/decision/effect evidence. No guard.

## 2026-10-24 — main integration and current prerequisites

Integrated main eefdfe1 on an isolated branch. The sole conflict was README;
kept both D83 instruction/memory protection and the bounded mod finding.
Docker client 29.8.0 is present, but desktop-linux Server is null: the engine
pipe is absent and no Docker process is running. WSL resolves the same client.
The original Docker-absent note is historical, not the current blocker.
All four pinned npm meta/platform package versions remain available.

Added eight offline oracle tests; all pass. These validate DENY_NO_EFFECT,
DENY_WITH_EFFECT and INCOMPLETE classification only. They are not host evidence.
MANAGED.md now gives daemon preflight, the offline command, and a completion
checklist. Build and both managed rows remain NOT RUN; no system settings,
production runtime, demo checkout or deferred PRs were modified. The project
runner and its recorded results remain unchanged. Next: start/provide a Linux
amd64 Docker daemon, build the recipe, run only the two managed cases, inspect
and preserve sanitized source/tier/deny/effect evidence. Keep #99 / AI2-41 open
and PR #100 draft; no merge authorized.

## 2026-10-24 — managed live comparison complete

Started Docker Desktop as requested; built the pinned Linux amd64 recipe.
Ran mod off/on without network, mounts, credentials or capabilities, as uid1000.
Both: genuine root-owned system hook emits deny; exact marker absent.
sec-default/prepend loaded, override/user explicitly admitted in mod-on.
Initial run was INCOMPLETE solely because the classifier caught background
plugin-refresh diagnostics. Preserved that report; added an exact-notice
classification and regression test, rebuilt and reran both in a new container.
Final: DENY_NO_EFFECT twice, exit0, no evidence gaps; nine offline tests pass.

Artifacts: docs/benchmarks/claude-mods/results/managed-2.1.288.json and
results/managed-2.1.288/ (full normalized debug/hook logs, policy fixtures,
provenance, npm lockfile, initial report and human interpretation).
Original project probe/results, runtime policy and hook shim unchanged.
No merge, production guard, demo change or deferred PR change. #99 / AI2-41
remain open for review and the next targeted coverage/doctor/docs decision.
Do not rerun completed project proof or infer arbitrary-mod isolation.
