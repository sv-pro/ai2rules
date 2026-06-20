# Governed Claude Code (containerized SUT)

A throwaway **Claude Code instance in a container** that runs this repo's
PreToolUse governance (`world-gate.py`) under an OS-level isolation boundary.
Two jobs (PLAN.md **E13 / E8**):

1. **Separation** — keep the agent *under test* (and the harness's dogfooding
   config: hooks, subagents, projected world) out of your host dev session, so
   restricting tools or triggering taint can't brick the session you work in.
2. **Enforcement floor (E8)** — the in-agent hook *decides* "no egress when
   tainted"; the container *physically enforces* it via network policy, write
   confinement, non-root, and dropped capabilities. Decisions + physics =
   defense in depth.

## Build & run

```bash
docker build -t governed-claude -f docker/Dockerfile .

./docker/run.sh                       # offline shell (NET=none)
NET=bridge ANTHROPIC_API_KEY=sk-... ./docker/run.sh claude   # live, hook-governed
```

Inside the container, everything is mounted at `/workspace`, so the offline
proofs run as-is:

```bash
bash .claude/hooks/test-gate.sh          # ABSENT / taint floor / ASK
bash .claude/hooks/demo-cross-agent.sh   # subagent taints shared session -> parent egress denied
```

## Network = the egress floor (E8)

| `NET=` | Behavior | Use for |
|---|---|---|
| `none` (default) | all network blocked | offline hook/replay testing (model API unreachable) |
| `bridge` | full network | live agent governed **only** by the hook — no OS floor |
| *allowlist proxy* | only the model API reachable | a **live but contained** agent — the real E8 |

The third row is the goal and is not wired up here yet. The shape: a second
container running an egress proxy (squid/tinyproxy) that allows only
`api.anthropic.com`, with the agent container joined to it and
`HTTPS_PROXY`/`HTTP_PROXY` pointed at it (a `compose.yaml` is the natural home).
Then even a compromised agent can reach the model but cannot exfiltrate — the OS
enforcing what the taint floor merely decides.

## Shared taint store (cross-instance)

The taint sidecar lives in `.claude/state` under the mounted workspace, so any
containers mounting the **same repo** already share taint — a tainted agent in one
taints the floor for another. The local sidecar's only limitation is its
*locality*: instances that *don't* share a workspace don't share `.claude/state`.
For that case, mount a shared volume over it —
`-v sut-taint:/workspace/.claude/state` (chown the volume to uid 1000 first) — the
"shared store" replacement for the local file (see `DECISIONS.md` D20/D21).

## Notes & limits

- **Auth** is passed via `$ANTHROPIC_API_KEY` at runtime; it is never baked into
  the image. `NET=none` blocks it by design (offline only).
- **Workspace** is mounted read-write so the agent can edit code; add `:ro` for a
  look-but-don't-touch demo. Host safety doesn't depend on it — with `--rm`,
  anything written outside `/workspace` and the state volume is ephemeral.
- **Optional hardening:** `--read-only --tmpfs /home/agent` for an immutable root.
- This is the bridge between **E13** (manifest-driven governance) and **E8**
  (OS-level sandbox backstop): the container is where the harness's *declared*
  network/write constraints become *enforced*.
