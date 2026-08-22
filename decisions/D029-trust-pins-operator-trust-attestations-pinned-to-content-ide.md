# D29 — `trust_pins`: operator trust attestations pinned to content identity; taint becomes a recomputed cause-ledger

- **Epic:** E13.8 / E2 (taint) · extends D25 · **Status:** Accepted (live-hook interim shipped; canonical kernel field pending) · **See:** [`docs/trust-pins.md`](../docs/trust-pins.md)
- **Decision:** Add **`trust_pins`** — operator attestations that a *specific read
  source is trusted*, each pinned to the source's **content identity** (`sha256` of
  the file bytes, or a reference repo's own `git_commit` + clean tree). At gate time
  a `Read` whose **live** content still matches a pin is classified **Trusted** and
  does **not** taint; any **drift** (bytes/commit change) or `expires` date revokes
  the pin and the read taints as normal. The per-session taint sidecar becomes a
  **ledger of causes**, and `tainted` is **recomputed every call** = *any recorded
  cause not covered by a valid pin*. Shipped in the live host hook: shared logic in
  `.claude/hooks/_gatelib.py` (used by both `world-gate.py` and `taint-notify.py`),
  `trust_pins` declared in `.claude/cc-world.json`. The **canonical home** is a
  `trust_pins` field in the real `WorldManifest` enforced by the pure `gate()`
  (kernel), to land with the D26 host cutover.
- **Why it is NOT a hole in invariant 6 (monotonic taint) or 7 (egress floor):** a
  pin re-classifies a source's trust **upstream of taint** — a pinned, content-
  verified read was *never* an untrusted-taint cause, so the recompute reflects
  *corrected provenance* (a human, design-time, auditable attestation), not a
  decrease of taint under fixed facts. The ledger **retains every cause** (audit),
  drift is **tamper-evident** (the descriptor-drift primitive from `safe-mcp-proxy`
  applied to reads), and the floor itself is untouched — an unpinned/tainted cause
  still `DENY`s egress. In the manifest's channel model it is exactly: a valid pin
  flips a read's `source_channel` from `workspace_files` (SemiTrusted, taint:true)
  to **Trusted (taint:false)**.
- **Binding correction (vs the initial "pin to HEAD" sketch):** bound to **content
  identity, not the harness repo's `HEAD`** — `repos/3p` is *not tracked in this
  repo* (`AGENTS.md`: never `git add repos/`), so this repo's HEAD says nothing about
  that content. Use the file's `sha256` (git-agnostic, per-file precise) or the
  **reference repo's own** HEAD commit + clean tree.
- **Resolves D25's deferred read-taint:** D25 option (a) was "tag an untrusted read
  with an untrusted `source_channel`"; `trust_pins` is the *exception* that re-tags a
  vouched read as Trusted. Implement the two together in the kernel port.
- **Alternatives:** (a) **delete the sidecar / reset taint** — rejected: unrecorded,
  blind re-taint, indistinguishable from a decrease-by-fiat; (b) **drop `repos/` from
  `taint_sources`** — rejected: blanket-trusts the whole tree *forever*, including
  future malicious edits, with no drift detection; (c) **drop `WebFetch` from
  `egress.tools`** — rejected: weakens invariant 7 itself; (d) **implement only in the
  kernel/manifest now** — the correct long-term home, but it does not govern the live
  session, so it cannot clear an in-flight tainted session (the operator's immediate
  need); recorded as the canonical follow-up; (e) **pin to the harness repo's HEAD** —
  rejected per the binding correction above.
- **Why ship the interim in the live hook:** the live `world-gate.py` is what governs
  this session; the pin/ledger is provable in isolation (`test-gate.sh` §4 + a
  throwaway-projdir simulation, both run green) and fails **open** on any helper/parse
  error, so it cannot brick a session. This mirrors the E13.2 "Python-first slice
  before the kernel ABI" pattern (D19→D24).
- **Known limits:** (1) the interim **grows the Python reimplementation D24 wants to
  retire** — accepted as interim; canonical logic is one `gate()` in the kernel.
  (2) a `sha256` pin is per-file; a `git_commit` pin trusts a whole clean tree at a
  commit (coarser, voided by any local edit). (3) editing the hook that governs *this*
  session is the **self-governance risk D26 flags** — done at operator direction, with
  fail-open preserved and out-of-band validation before reliance. (4) a pin is only as
  good as the operator's review of those bytes — it deliberately moves trust from *the
  model's runtime judgement* to *a human's design-time attestation*. That is the point.
