# Governed Claude Code (containerized SUT)

A throwaway **Claude Code instance in a container** that runs this repo's PreToolUse
governance under an OS-level isolation boundary. Refreshed for the **Rust hook**
(D37: `harness cc-hook` + `.claude/cc-world.yaml`; the old Python `world-gate.py` /
`cc-world.json` path is retired to `.claude/hooks/superseded/`). PLAN.md **E13 / E8**,
two jobs:

1. **Separation** — run the agent *under test* (and the harness's dogfooding config:
   hooks, subagents, projected world) **out of your host dev session**, so an
   experiment that restricts tools, empties `settings.json`, or triggers taint can't
   brick the Claude you're driving.
2. **Enforcement floor (E8)** — the in-agent hook *decides* "no egress when tainted";
   the container *physically enforces* it — network policy, write confinement,
   non-root, dropped caps. Decisions + physics = defense in depth.

> **Rule of thumb:** try any governance change that could lock the agent out of its
> own tools **here**, never on your host session. A bricked SUT is just `docker rm`.

## Build & run

```bash
docker build -t governed-claude -f docker/Dockerfile .   # multi-stage: builds `harness`, installs Claude Code

./docker/run.sh                                    # offline shell (NET=none)
NET=bridge ANTHROPIC_API_KEY=sk-... ./docker/run.sh claude   # live, hook-governed
```

The image is **self-contained**: the Rust `harness` binary is built in a `rust:1-bookworm`
stage and installed at `/usr/local/bin/harness`, so the PreToolUse hook works with no host
build required. Your repo (incl. `.claude/`) is mounted at `/workspace`; auth is
`$ANTHROPIC_API_KEY` at runtime, never baked in.

Offline proof (NET=none, model API unreachable):

```bash
bash docs/demos/replace-permissions/demo.sh   # grant + taint floor + ABSENT via harness cc-hook
```

## Replace-mode experiment — empty settings.json, govern by the manifest

`MODE=replace` overlays a dedicated [`sut/settings.replace.json`](sut/settings.replace.json)
over the project's `settings.json` (**container-only**; your host file is untouched — this is
literally "separate settings under dev vs runtime"). Native permissions are **empty**; the
whole policy is the `WorldManifest`, enforced by `harness cc-hook --grant --enforce-absent`:

```bash
MODE=replace NET=bridge ANTHROPIC_API_KEY=sk-... ./docker/run.sh claude
```

Inside, ALLOW verdicts are **granted** (no prompt), anything the manifest doesn't declare is
**ABSENT → deny**, and a tainted egress is still **denied**. `--enforce-absent` *will* lock the
agent out of undeclared tools — that's the experiment, and it's safe because the container is
disposable.

## Network = the egress floor (E8)

| `NET=` | Behavior | Use for |
|---|---|---|
| `none` (default) | all network blocked | offline hook/replay testing (model API unreachable) |
| `bridge` | full network | live agent governed **only** by the hook — no OS floor |
| *allowlist proxy* | only the model API reachable | a **live but contained** agent — the real E8 |

The third row is [`compose.yaml`](compose.yaml) + [`egress-proxy/`](egress-proxy/): the agent
runs on an `internal` (no-gateway) network; its only egress is a tinyproxy that allowlists
`anthropic.com` (CONNECT :443 only). A compromised agent can reach the model but cannot
exfiltrate — bypassing the proxy env gets *no route* at all.

```bash
# dogfood governance behind the egress floor:
ANTHROPIC_API_KEY=sk-... docker compose -f docker/compose.yaml run --rm agent claude
# replace-mode experiment behind the egress floor:
ANTHROPIC_API_KEY=sk-... docker compose -f docker/compose.yaml run --rm agent-replace claude
```

Edit the allowlist in [`egress-proxy/filter`](egress-proxy/filter) (one host-regex per line).

## Shared taint store (cross-instance)

The taint sidecar lives in `.claude/state` under the mounted workspace, so containers
mounting the **same repo** already share taint. For instances that don't share a workspace,
mount a shared volume over it — `-v sut-taint:/workspace/.claude/state` (chown to uid 1000
first). See `DECISIONS.md` D20/D21.

## Notes & limits

- **Auth** via `$ANTHROPIC_API_KEY` at runtime; never in the image. `NET=none` blocks it.
- **Workspace** is mounted read-write so the agent can edit code; add `:ro` for a
  look-but-don't-touch demo. With `--rm`, anything outside `/workspace` is ephemeral.
- **Optional hardening:** `--read-only --tmpfs /home/node` for an immutable root.
- The bridge between **E13** (manifest-driven governance) and **E8** (OS sandbox): the
  container is where the harness's *declared* constraints become *enforced*.
