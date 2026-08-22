# D37 — Claude Code live-hook cutover to `harness cc-hook` via in-place bootstrap shims


**Date:** 2026-07-12. **Executes the cutover D26 deferred**; supersedes the live Python
engine (E13.2/D29 interim).

- **Decision:** the live host's PreToolUse governance now runs the **real Rust kernel**:
  `settings.json` points at `.claude/hooks/world-gate.sh`, a bootstrap shim (now hardened by
  D46 to locate `harness` only from an explicit absolute override or installer-owned absolute
  path; fail-open exit 0 if absent; else `exec harness cc-hook --world .claude/cc-world.yaml
  --state .claude/state`). `world-gate.py` was **replaced in content, in place**, with the same
  shim in Python. The Python engine (`world-gate.py` original, `_gatelib.py`,
  `world-gate-adapter.py`, `cc-world.json`, its tests and demos) is archived under
  `.claude/hooks/superseded/` with a README. `taint-notify.py` stays (observability, not
  policy; degrades gracefully without `_gatelib`).
- **The in-place-shim rule:** hook configs may be **snapshotted at session start** — if
  the configured hook *file* disappears mid-session, `python3` exits 2 and every
  subsequent tool call is blocked, unrecoverably (a session was lost exactly this way:
  `git mv world-gate.py superseded/` before editing `settings.json`). Therefore a live
  hook file is never moved or deleted; it is emptied into a shim, and only *new* wiring
  changes paths.
- **What the cutover consciously drops** (recorded, not hidden): **trust pins (D29)** —
  no typed `trust_pins` field exists in the compiled `WorldManifest` yet, so operator
  attestations are not honored until it lands; **path-based read-taint** — reading
  `repos/` no longer taints (taint enters via Network/External/Memory outputs, the v1
  gate policy); the archived `demo-injection-egress.sh` depended on it.
- **Alternatives rejected:** keep the Python engine as the live gate (two sources of
  truth — the state D24/D33 exist to end); cut over by moving files + editing
  `settings.json` (the session-bricking trap above); wait for trust-pins/path-taint
  parity first (indefinite delay for features the kernel will gain as typed manifest
  fields — D26 already validated the adapter path).
- **Related:** D24, D26, D29 (open follow-up), D34, D36, `docs/one-kernel-many-hosts.md`,
  `.claude/hooks/superseded/README.md`.
