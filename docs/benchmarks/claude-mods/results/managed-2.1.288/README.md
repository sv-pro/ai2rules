# Managed file-policy comparison — 2026-10-24

[Runbook](../../MANAGED.md) · [final machine report](../managed-2.1.288.json)

| Case | Host exit / exchanges | Live cc-hook | Override admission | Marker | Result |
| --- | --- | --- | --- | --- | --- |
| managed-mod-off | 0 / 2 | deny | Absent as intended | Absent | DENY_NO_EFFECT |
| managed-mod-on | 0 / 2 | deny | Explicitly admitted, user tier | Absent | DENY_NO_EFFECT |

Observed: Claude Code 2.1.288, released harness 0.6.0, acceptEdits. The genuine
`/etc/claude-code/managed-settings.json` was root-owned (0644), immutable to
uid1000, and the sole hook registration. Both project settings are `{}`.
sec-default automatically loaded in prepend; no admission lock was set.
The mod-on log records `plugin.register ... judged by cc-plugin-sec-default:
admitted`, followed by native Write resolved to the managed PreToolUse denial.
No `prior=deny` override callback was observed; the managed block can terminate
before tool.check. Both manifests are byte-identical. Both hosts sent the denied
tool result back to the scripted provider before completing.

Interpretation: this native Write denial survived the admitted user-tier mod,
consistent with the documented final managed-hook block. The earlier project
hook result remains an override-with-effect finding. This is not a guarantee
about direct mod filesystem/process access, rewritten calls, policy or state
integrity, remote Team/Enterprise policy, other modes/builds, or sandbox escape.
The world/state are writable fixtures. No prompt-injection or persistence claim.

## Retained evidence

- `../managed-2.1.288.json`: full final case reports with binary/policy hashes,
  versions, live hook events, kernel/adapter responses, CLI output, provider
  exchanges, loaded tiers and marker outcomes.
- `managed-mod-{off,on}-debug.txt`: full debug logs with only the generated
  `/evidence/managed-*` root replaced by `<RUN_DIR>`.
- `managed-mod-{off,on}-hook.jsonl`: full live shim records, same normalization.
- `managed-mod-{off,on}-project-settings.json` and `-world.yaml`: actual fixtures.
- `provenance.json`: source hashes, Docker server, resolved base-image digest,
  image layers, container isolation/exit state, raw/retained log and archive hashes.
- `package-lock.json`: exact npm package resolutions and integrity checksums.
- `initial-incomplete-report.json`: unmodified initial report, exit1. It also
  records deny/no marker twice, but is retained as INCOMPLETE rather than
  retroactively relabeled as a successful run.

No host config, credentials, Docker socket or bind mounts entered either
container. Build used registry access; execution used `--network none`, dropped
ALL capabilities, no-new-privileges, and the non-root node user. Model API was
scripted loopback only. CLI cost/usage fields are synthetic estimates, not billing.
Full raw archives and original image/container inspection remain locally under
the run's temporary evidence directory; provenance records their hashes.
Base tag and apt packages are not lock-pinned: hashes identify this observation,
but a future rebuild may differ. The tested harness is released 0.6.0, not main.

## Initial classifier correction and rerun

The initial analyzer matched every `disabled by` line. Both logs contained:
`Plugin autoupdate: command-source refresh disabled by
tengu_plugin_command_source_refresh (still excluding command-sourced plugins
from the regular pass)`. This is about refresh, not the inline plugin's admission.
The final runner retains that exact notice separately in `background_diagnostics`;
all other matching lines still trigger INCOMPLETE. A regression test includes
refusal, policy disable, mixed-case load failure and an altered notice. All nine
oracle tests pass. Both cases were rerun from fresh state after rebuilding the
image; the final runner exits0 with no evidence gaps. Binary/policy/manifest
hashes match the initial pair, and original probe.py/hook/image recipe are unchanged.

Other retained diagnostics were reviewed: mod-off's fresh-config ENOENT is
immediately followed by successful atomic config creation; missing optional
managed agents/commands/output-styles directories are expected in this minimal
image; the unsupported auto-mode warning does not change the recorded acceptEdits
mode. No mod refusal/load failure is present, and mod-on admission is explicit.

The bounded experiment is complete. Review PR #100 and decide on a focused
coverage/doctor/docs follow-up; publication and production guard changes are
separate work. No merge was performed.
