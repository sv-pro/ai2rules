# Current fix: AI2-57 / PR #129 (2026-10-25)

User authorizes fixing both review findings, reviewing, checking CI/conflicts,
and merging if good. Worktree pr129-review, branch codex/pr129-review.
Shared hostkit now joins native components (root/prefix preserved) and checks
missing/.. before canonicalization. Added actual hook+kernel verdict checks,
Windows path forms and drive prefixes, plus Unix symlink/.. compatibility.
Focused Windows and Linux tests, full native Windows workspace tests, fmt and
clippy passed. Independent candidate review finishing; fresh CI still required.
No merge yet. Stand remains clean at810c1a6;
PR150 and other deferred PRs untouched. Next: finish review/checks, push existing
PR branch, fresh CI, resolve review threads, merge pinned head, sync Linear.

Earlier review and inherited handoff follow.

# Current review: AI2-57 / PR #129 (2026-10-25)

Reviewed remote head 44c0ec86787e9d5e4886525a36e51d153234dfb4 in isolated
branch codex/pr129-review. User lifted the Oct 27 hold and authorized merge only
if review passes. No merge: two native Windows regression tests fail.
P1: root-relative backslash path loses its root and inherits project ReadWrite;
actual hook_request + gate returns ALLOW versus DENY for the real target.
P2: absolute missing/.. canonicalizes before the missing-component guard.
Tests are uncommitted at the end of hostkit.rs, module pr129_review_regressions.
Run: cargo test -p cli-harness --bin harness --offline --locked pr129_review_regressions -- --nocapture
No external write executed; only temp fixture directories and decision calls.
Inline review: https://github.com/sv-pro/ai2rules/pull/129#pullrequestreview-5643163428
All 15 original GitHub checks pass; no conflicts; D88 collision already fixed.
Linear AI2-57 moved In Progress with findings. No production changes or pushes.
Demo checkout started clean at 810c1a6; untouched. PR150 and others untouched.
Next: fix Windows rooted/prefixed handling and check original absolute components
before canonicalization; add regression coverage, rerun tests/CI and re-review.

Earlier inherited main handoff follows (not evidence about PR129).

# Current handoff — PR #100 managed comparison

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

Earlier entries below are historical and superseded where they say NOT RUN.

# AI2-41 / #99 — Claude mods enforcement probe

## 2026-10-24 — conflict repair and refreshed preflight

Worktree: pr100-managed-policy; local branch codex/ai2-41-pr100-managed-policy.
Integrating main eefdfe1; only README conflicted. Resolution preserves main's
D83 memory protection and this PR's bounded project-mod finding. No PR merge
is authorized; demo and deferred PRs #129/#148/#149 are untouched.

Docker client 29.8.0 now exists, desktop-linux context selected, but Server is
null and dockerDesktopLinuxEngine pipe is absent. No Docker process was found.
No service was started, host managed policy changed, or live case attempted.
Pinned Claude 2.1.288 / harness 0.6.0 npm metadata is available. Build and both
managed cases remain NOT RUN. Next: validate the oracle offline, publish the
conflict resolution and record the exact remaining environment/evidence gates.
## 2026-10-05 — managed setup prepared, runtime prerequisite remains

Added `docs/benchmarks/claude-mods/MANAGED.md`, `managed.Dockerfile`,
`managed-settings.json`, `managed_hook.py`, and `managed_probe.py`. Linux's
supported genuine source is `/etc/claude-code/managed-settings.json`; the image
installs it as root and runs the two-case comparison as non-root, without
credentials or external network at runtime. No project proof rerun or guard.
Original `probe.py` and project evidence are byte-unchanged.

Validation: Python syntax, policy JSON, synthetic positive/negative oracle gates
and diff whitespace. These are setup checks, not a managed-host execution.
Image build and both managed rows are **NOT RUN**. Docker/Podman are absent;
namespace preflight returned `Operation not permitted` on `/proc/self/uid_map`.
No system policy was written and that restriction was not worked around.

Exact next prerequisite: a disposable Linux amd64 Docker host with build-time
base-image/Debian/npm access. Run the build/create/start/copy commands in
MANAGED.md; inspect actual source, hook records, tiers/sec-default and marker
effects; commit sanitized runtime evidence. The recipe records pinned target
versions, but no new version or tier has been observed here. Keep AI2-41 High /
In Progress and #99 open; acceptance remains unchecked. Do not repeat project
proof or start a guard. PR #100 remains draft.

## Previous completed slice

2026-10-03, Codex. Branch: `codex/claude-mods-enforcement-probe`.

Completed: real Claude Code 2.1.288 + released ai2rules 0.6.0 probe, twice in
fresh temporary roots. Project PreToolUse emits deny; a narrowly matched
user-tier tool.check mod sees deny and returns allow; the native Write creates
the marker. Control writes; no-mod project denial prevents the write.

Evidence and reproduction: `docs/benchmarks/claude-mods/README.md`, `probe.py`,
`results/project-2.1.288.json`. Final run passed all effect and decision checks.
The model API is scripted/loopback; the host, hooks, kernel and effect are real.
No production policy or adapter behavior was changed. README/PLAN qualify scope.

Next commit-sized step: reproduce genuine managed PreToolUse with mod off/on
in a disposable container/VM, preserving the same call and policy. Record
loaded sec-default and actual scope. Managed cases are NOT RUN; no isolated
managed-policy test setup was provisioned in this slice. Do not relabel
project/CLI settings as managed. Do not start a guard implementation.

Done for next step: both managed cases have pinned configuration, live adapter
response and marker-effect evidence; report/trackers updated. Full issue stays
open while those rows are missing. Consult linked PR and both trackers before
continuing. No live prompt-injection or persistence claim is established.
Offline oracle: all 8 tests passed. Pinned npm meta/platform versions all available. MANAGED.md refreshed with Docker Server prerequisite and exact remaining evidence checklist. No runtime experiment or image build attempted because engine is unavailable. Next: syntax/JSON/diff checks, commit integration, push PR #100, fresh CI and tracker sync.
FINAL 2026-10-24: pushed 32725813edd92e6141b89af75625c8d46966cb2e to existing PR #100. 10/10 CI checks passed (40105171659), CLEAN/MERGEABLE. PR remains draft, #99 and AI2-41 open; all three updated with current prerequisites. Managed build/mod-off/mod-on remain NOT RUN: Docker client present, Linux daemon unavailable. Stand clean main f70af23 untouched. This final local handoff note is intentionally uncommitted; cloud continuity is in PR/trackers.
