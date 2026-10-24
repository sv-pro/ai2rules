# AI2-41 / #99 — Claude mods enforcement probe

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
