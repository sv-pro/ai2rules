# D26 — Validate the gate adapter in the containerized SUT; don't rewrite the live host hook

- **Epic:** E13.8 (extends D21/D24/D25) · **Status:** Accepted
- **Decision:** Realize the D24 host adapter by **adding a new shim**
  (`.claude/hooks/world-gate-adapter.py`) that shells out to `harness gate --world
  .claude/cc-world.yaml`, and **validate it in the E13.7 container SUT** — *not* by
  editing the live `world-gate.py` that governs the host dev session. The live hook
  and its `settings.json` wiring stay untouched; cutting the live host over (and
  retiring `world-gate.py` + `cc-world.json`) is a separate, opt-in step. The shim is
  pure plumbing — Bash classification (D25), taint-sidecar restore/persist, the
  `harness gate` call, and decision→`PreToolUse` mapping (DENY→deny, ASK→ask, else
  passthrough; fail-open) — **no decision logic**.
- **Why:** (1) the live hook governs *this* session; rewriting it in place risks
  weakening/breaking our own governance for no gain, since the adapter is a new
  artifact provable in isolation. (2) The container is what E13.7/D21 exists for —
  disposable (a shim bug can't harm the host), **backstopped by the egress proxy**
  (so the v1 gate's deferred path-taint gap, D25, is covered by the network floor —
  defense in depth), and the realistic deployment target. (3) Neither environment
  loses protection during the migration: the container has the proxy floor; the live
  host keeps the full (path-taint-capable) Python hook until a deliberate cutover.
- **Alternatives:** (a) **edit the live `world-gate.py` in place** — rejected:
  self-governance risk, and nothing requires it. (b) **prove the shim by fixtures
  only, skip the container** — weaker: misses the real Claude Code integration and
  the proxy-backstop story (kept as the fast Tier-1 check, not the whole validation).
  (c) **cut the live host over immediately** — premature before the shim is proven
  and before path-taint parity (D25) is resolved.
- **Cost / open sub-choice:** the SUT image must ship the `harness` binary (today it
  ships only `python3`). Packaging — a Rust build stage in the Dockerfile vs a
  mounted host-built static/musl binary — is recorded when taken.
