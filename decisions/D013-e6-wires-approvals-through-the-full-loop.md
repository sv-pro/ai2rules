# D13 — E6 wires approvals through the full loop

- **Epic:** E6 · **Status:** Accepted
- **Decision:** Beyond the kernel branch + store, wire approvals into the
  orchestrator: an `ApprovalPolicy` (`Manual`/`AutoApprove`/`AutoReject`) + an
  `ExecutionMode` on the session, with a demo showing `ASK → approve → resume →
  ALLOW` and `BACKGROUND → DENY`.
- **Alternatives:** Kernel + store only, deferring loop wiring/demo.
- **Why:** End-to-end wiring is what actually demonstrates invariants 9 and 10,
  and completes Milestone 2.
