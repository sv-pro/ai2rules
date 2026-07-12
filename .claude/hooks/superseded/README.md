# Superseded Python governance engine (archived at the D37 cutover)

On 2026-07-12 the live Claude Code hook was cut over from the Python
reimplementation to the **real Rust kernel**: `world-gate.py` became a ~15-line
bootstrap shim that `exec`s `harness cc-hook --world .claude/cc-world.yaml`
(and `world-gate.sh` is the canonical `settings.json` wiring for new sessions).
See DECISIONS **D37** and `docs/one-kernel-many-hosts.md`.

These files are the pre-cutover artifacts, kept verbatim for reference. They are
**not wired anywhere** and must not be re-enabled — the kernel is the single
source of governance now (D24/D36).

| File | What it was | Why superseded |
|---|---|---|
| `world-gate.py` | The E13.2 PreToolUse gate: a Python port of ABSENT-for-native, the taint floor, ASK, plus the D29 trust-pin ledger. | Governance now lives in the compiled `cc-world.yaml` + `world-kernel`; the live file is a bootstrap shim with no policy. |
| `_gatelib.py` | Shared taint-ledger / trust-pins logic (D29) used by `world-gate.py` and `taint-notify.py`. | Trust pins are **dropped at cutover** (see below); `taint-notify.py` degrades to existence-based taint without it. |
| `world-gate-adapter.py` | The D26 shim that shelled out to `harness gate` — the proof-of-concept for the cutover. | Replaced by `harness cc-hook` (in-process `gate()`, D34). |
| `cc-world.json` | The bespoke pre-D25 world schema (`projected_tools` / `egress` / `ask` / `taint_sources` / `trust_pins`). | Replaced by the real `WorldManifest` `.claude/cc-world.yaml` compiled by the real compiler. |
| `test-gate.sh` | Contract tests for the Python gate (incl. trust-pin §4). | The contract now lives in `crates/cli-harness/tests/{cc_hook,one_kernel}.rs`. |
| `test-gate-adapter.sh` | Contract tests for the D26 adapter shim. | Same. |
| `demo-injection-egress.sh` / `.tape` | The E13.5 injection→egress demo. Depended on **path-based read-taint** (`taint_sources.read_paths`), which the v1 kernel gate does not implement. | Superseded by `scripts/demo-one-kernel-many-hosts.sh`; taint now enters via network/MCP/external outputs. Path read-taint is a recorded follow-up (D25/D29). |
| `demo-cross-agent.sh` | The D20 cross-agent taint walkthrough. | Session-taint propagation is unchanged (shared sidecar), but the gate behind it is now the kernel; the script's Python-hook assertions no longer apply. |

## What the cutover consciously drops

- **Trust pins (D29):** the compiled `WorldManifest` has no `trust_pins` field
  yet, so operator attestations are not honored until a typed field lands in the
  kernel. Until then a tainted session stays tainted.
- **Path-based read-taint:** reading `repos/` / `untrusted/` no longer taints;
  taint enters via Network/External/Memory outputs (the v1 gate's side-effect
  policy). The design follow-up is recorded in D25/D29.

## Why `world-gate.py` was replaced in place (never moved)

Hook configs may be **snapshotted at session start**: if the file a running
session's PreToolUse points at disappears, `python3` exits 2 and **every**
subsequent tool call is blocked — unrecoverable from inside the session. So the
cutover keeps the path alive as a shim and changes only `settings.json` for
future sessions (D37).
