# Genuine managed-hook setup (AI2-41 / #99)

## Observed result — 2026-10-24

**Both managed cases completed: DENY_NO_EFFECT.** Claude Code **2.1.288** and
released harness **0.6.0**, Linux amd64, `acceptEdits`, real host/adapter/kernel,
scripted loopback Messages API. No real model inference or credentials.

| Case | Managed source | Loaded modules | Live adapter | Marker | Observation |
| --- | --- | --- | --- | --- | --- |
| Mod off | Root-owned system file | sec-default / prepend | deny | Absent | DENY_NO_EFFECT |
| Mod on | Same system file | sec-default / prepend; override / user | deny | Absent | DENY_NO_EFFECT |

The mod-on debug log explicitly records `plugin.register` judged by
`cc-plugin-sec-default: admitted`. The block is therefore not explained by
refusing the test plugin. Both hosts exited 0 after two scripted exchanges;
only the managed source registered the recording shim, project settings were
empty, and both live hook events contained the exact proposed Write.

[Machine-readable report](results/managed-2.1.288.json),
[human report and evidence inventory](results/managed-2.1.288/README.md), and
[provenance](results/managed-2.1.288/provenance.json) retain the evidence.
Docker Desktop was started at the user's request; Docker Engine 29.8.0 built
and ran the recipe. Runtime used uid 1000, no mounts, `--network none`, all
capabilities dropped and `no-new-privileges`. System policy and code were
root-owned and not writable by that user. No host managed policy was changed.

The first pair also denied both writes, but the original log classifier marked
both INCOMPLETE because it treated a background plugin-refresh notice as an
admission failure. That original report is retained. The analyzer now separates
only that exact known notice into `background_diagnostics`; unknown refusals,
disables and load failures still produce INCOMPLETE. Nine offline tests pass.
The image was rebuilt and **both cases were rerun in a new container**, producing
the final report above. Policy, hook shim, original fixture and binary hashes
were unchanged. The earlier daemon/namespace blockers are historical.

The bounded managed comparison is complete. PR #100 and both trackers remain
open for review and the follow-up decision; no merge or guard implementation.
The earlier project proof remains unchanged and was not rerun.

## Supported loading path

Anthropic's [managed settings documentation](https://code.claude.com/docs/en/managed-settings)
specifies `/etc/claude-code/managed-settings.json` for Linux file-based policy,
including self-built images. It is read at startup. `--settings` and project
settings do not become managed policy. Remote policy is not fetched when
`ANTHROPIC_BASE_URL` points to a non-Anthropic endpoint; this fixture uses only
loopback and a fresh config directory. The clean Linux image has no other policy.

The image copies [managed-settings.json](managed-settings.json) into that exact
system path as root, then runs the probe as non-root `node`. The system policy
alone registers `/opt/probe/managed_hook.py`. That recording shim calls the same
real `cc-hook`; it adds no policy logic. Both cases use the original `run_case`
fixture with project hook registration disabled and verified empty project JSON.
The original `probe.py` and its project/control `main()` are not changed or run.

The [mod administration contract](https://code.claude.com/docs/en/plugins/mods/admin)
and [built-in sec-default description](https://github.com/anthropics/claude-code/blob/main/mods/sec-default/README.md)
describe automatic prepend seating on machines with managed settings.
Leave `prependPlugins` unset so this tests the default. Also leave
`allowManagedHooksOnly`, `allowManagedModsOnly`, `disableAllHooks` and
`disableSideloadFlags` unset: denying the test mod admission would answer a
different question. The only policy key in this fixture is `hooks`.

The [event contract](https://code.claude.com/docs/en/plugins/mods/events) says a
managed hook block precedes mods and is final. The recorded cases agree with
that contract; rewritten-call behavior was not tested. In particular,
`tool.check` may never run after the
managed block: absence of `prior=deny` is not by itself a failure. Mod admission
and its user tier must still be visible in the mod-on debug log.

Managed-settings, mod administration and event-order documentation rechecked 2026-10-24. These are current documentation, not an immutable
specification for 2.1.288. Record actual binary hashes and loaded tiers; do not
infer a historical binary's behavior from a newer sec-default README.

## Build and run only the two managed cases

First start a Linux container engine (Docker Desktop's Linux engine on the
reviewed Windows host, or a separate disposable Linux amd64 host). Then verify:

```bash
docker version
docker info --format '{{.OSType}}/{{.Architecture}}'
```

Require a populated **Server** section and Linux/amd64 (also reported as
Linux/x86_64). A client version alone is not a successful preflight. The commands
below are Bash syntax; on Windows run them in a WSL terminal integrated with the
started Docker Desktop engine. Keep the production/demo checkout out of mounts.

Before spending time on the image, the oracle can be checked offline on any
Python 3 host, without Docker or a system policy file:

```bash
python3 -m unittest discover -s docs/benchmarks/claude-mods -p test_managed_probe.py -v
```

Nine tests exercise complete denial, denial with an exact effect, missing or
wrong load/source/decision evidence, wrong permission mode or target, changed
policy, unexpected marker content, and background versus admission diagnostics.
All passed on 2026-10-24. These synthetic
checks validate classification only; they do not satisfy either live row.

From the repository root on the disposable container host:

```bash
docker build --platform linux/amd64 \
  -f docs/benchmarks/claude-mods/managed.Dockerfile \
  -t ai2rules-managed-probe:2.1.288 docs/benchmarks/claude-mods
probe_output=$(mktemp -d /tmp/ai2rules-managed-evidence-XXXXXX)
probe_container=$(docker create --platform linux/amd64 --network none \
  --cap-drop ALL --security-opt no-new-privileges \
  ai2rules-managed-probe:2.1.288)
docker image inspect ai2rules-managed-probe:2.1.288 > "$probe_output/image.json"
docker start -a "$probe_container"
# Preserve evidence even if the runner exits nonzero (do not use set -e here).
docker inspect "$probe_container" > "$probe_output/container.json"
docker cp "$probe_container:/evidence/." - > "$probe_output/raw-evidence.tar"
# Preserve Linux symlinks inside the archive; direct extraction on Windows
# may fail without symlink privileges. Read regular files from the tar instead.
docker cp "$probe_container:/opt/probe-tools/package-lock.json" "$probe_output/package-lock.json"
docker rm "$probe_container"
printf '%s\n' "$probe_output"
```

Build requires network; execution has only loopback. No bind mounts, real config,
Docker socket or credentials enter the container. The root-owned system policy
and recording code cannot be edited by the runtime user. The WorldManifest,
state and exact marker remain disposable writable fixtures, as in the original
probe; this is not a test of their integrity against arbitrary hostile code.
The base image tag and OS packages are not byte-pinned; keep `image.json`, npm's
lockfile and the reported binary hashes if exact replay is required.

`managed_probe.py` imports `run_case` twice; it does not call project `main()`.
It retains the loopback model, only native Write, exact-path/content mod,
`--setting-sources project`, empty strict MCP config, fresh config per case and
no session persistence. Managed settings are discovered independently of the
project settings source selection. No `--settings`, safe mode or bypass mode
is used. Both cases share byte-identical policy/manifest; their fresh absolute
marker paths differ only by case directory, like the earlier comparison.

## Evidence and completion gate

`managed-report.json` includes policy path/contents/hash/ownership, binary
versions/hashes, CLI permission mode, full live hook event/adapter responses,
module load lines with tiers, kernel verdict, manifest hash, scripted provider
exchanges and actual marker presence/content. Raw debug logs and CLI evidence
remain in `managed-*/managed-mod-{off,on}/`; the report replaces the run path.
Policy-source attribution combines the sole system registration, empty project
settings, isolated config and execution of the uniquely named managed shim.
For manual source confirmation, `/status` should identify
`Enterprise managed settings (file)` in the same isolated configuration; never
substitute a file's mere presence for live hook execution.

The oracle requires successful host completion, two API exchanges, live adapter
denial, sec-default loaded in prepend, and (mod-on) override loaded in user tier.
It records all hook invocations rather than assuming managed dispatch has the
project case's count. Missing load evidence is **INCOMPLETE**, not a pass. Check
raw logs for refusals/errors even if a preceding line says a module loaded.
Any exact marker after a verified deny is **DENY_WITH_EFFECT**, a finding that
also exits nonzero. Expected is **DENY_NO_EFFECT** in both rows.

Before closing acceptance, inspect the report and raw logs, retain a sanitized
versioned report under `results/`, and update both trackers. If loading or source
attribution cannot be established, preserve the evidence and record the precise
remaining gap. Do not rerun the project proof or begin a guard in this step.

## Completion checklist

- [x] Working Linux amd64 daemon and successful image build; image/base metadata
  and npm lockfile retained.
- [x] Both cases ran in the same image, fresh state, no network or host mounts.
- [x] Raw logs confirm system-policy dispatch, exact Write, adapter deny,
  acceptEdits, sec-default/prepend, and admitted override/user in the on case.
- [x] Both exact marker paths absent; complete DENY_NO_EFFECT evidence.
- [x] Sanitized versioned evidence retained; README, PLAN and handoff updated.

Next: review the evidence in PR #100 and choose a targeted coverage/doctor/docs
follow-up. Project-hook denial is overridable in the recorded case; this managed
native-Write case retained denial. Do not generalize to arbitrary mod fs/process
access, policy/state integrity, rewritten calls, other versions, remote policy,
other modes, or a sandbox boundary. Publication and production guard work remain
separate decisions. GitHub #99 / AI2-41 remain open pending review.
