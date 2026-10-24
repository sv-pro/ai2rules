# Genuine managed-hook setup (AI2-41 / #99)

## Status — 2026-10-24

Setup recipe prepared; **image build and both live cases NOT RUN here**.
The current Windows host has Docker client **29.8.0**, with the `desktop-linux`
context selected, but `docker version` reports `Server: null` and the
`dockerDesktopLinuxEngine` named pipe is absent. No Docker process was running.
The WSL Ubuntu 24.04 environment is x86_64 and resolves the same Desktop client;
it does not provide a separate running engine. Podman was not found.
No service was started and no host managed policy was modified in this check.

This supersedes the earlier authoring environment's "Docker/Podman absent"
blocker. The 2026-10-05 namespace failure remains historical evidence; it was
not retried or worked around. The immediate prerequisite is a **running Linux
container daemon**, not installing the Docker CLI again.

The npm registry returned the pinned meta and linux-x64 packages for Claude
Code **2.1.288** and ai2rules-harness **0.6.0**. This was a metadata-only check:
no packages were installed, and image/base-layer/Debian downloads or container
startup have not been validated. Keep the pins; do not silently replace either
version with latest to make the experiment run.
**Exact prerequisite:** a disposable Linux amd64 container host with a working
Docker daemon (or a separately provisioned Linux VM), permission to build/run
the image, and build-time access to the base image, Debian and npm registries.
No Anthropic login, Enterprise tenant, real API key or paid inference is needed
for this file-policy/loopback experiment. A Team/Enterprise server-managed policy
would be a different delivery mechanism, not required by this recipe.

| Case | Runtime managed source | Loaded tier/sec-default | Adapter | Marker |
| --- | --- | --- | --- | --- |
| Managed / mod off | NOT RUN | NOT RUN | NOT RUN | NOT RUN |
| Managed / mod on | NOT RUN | NOT RUN | NOT RUN | NOT RUN |

Pinned targets (not newly observed versions): Claude Code **2.1.288**, released
harness **0.6.0**, `acceptEdits`. Keep AI2-41 and #99 open. The earlier project
proof and its checked-in evidence are unchanged.

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
managed hook block precedes mods and is final. That is the expected result,
**not an observed result**. In particular, `tool.check` may never run after the
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

Eight tests exercise complete denial, denial with an exact effect, missing or
wrong load/source/decision evidence, wrong permission mode or target, changed
policy, and unexpected marker content. All passed on 2026-10-24. These synthetic
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
docker cp "$probe_container:/evidence/." "$probe_output/"
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

## Remaining completion checklist

- [ ] Working Linux amd64 daemon and successful image build; retain image metadata
  and npm lockfile. Build-time base-image/Debian/npm access still needs a real build.
- [ ] Run `managed-mod-off` and `managed-mod-on` in the same disposable image with
  `--network none`, fresh per-case state, the pinned versions and no host mounts.
- [ ] Inspect raw hook/debug logs and the report: real root-owned system policy,
  empty project settings, exact Write arguments, adapter deny, `acceptEdits`,
  sec-default/prepend and (mod-on) override/user admission, no refusal/load errors.
- [ ] Retain actual marker absence or exact content. Expected is DENY_NO_EFFECT
  twice; DENY_WITH_EFFECT is a finding, not success, and INCOMPLETE needs more evidence.
- [ ] Inspect and commit sanitized versioned managed evidence under `results/`,
  then update #99, AI2-41, README, PLAN and handoff with the observed result.

No managed evidence file exists yet. No Enterprise tenant, real API credential,
paid model call, new production guard, or project-proof rerun is a prerequisite
for this bounded comparison. Both task trackers remain open and PR #100 draft.
