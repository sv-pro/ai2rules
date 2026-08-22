# D41 — Pure gate approval tokens are correlation ids, not grants


**Date:** 2026-07-23.

- **Context:** A security review found that the host-neutral `harness gate` ABI treated any
  non-empty `context.approval_token` in a `GateRequest` as `EvalContext.approval_granted = true`.
  The gate is deliberately pure and has no approval-store lookup, verifier callback, or trusted
  host identity. That made a request-controlled correlation field equivalent to a bearer grant for
  every approval-required action.
- **Decision:** Keep the v1 `context.approval_token` field for wire compatibility and keep returning
  an `approval.token` on `ASK`, but define both as correlation ids only. The pure gate ignores
  request-supplied approval tokens and never maps them to `approval_granted`. A trusted runtime
  that supports approval resumption must validate a durable approval-store binding outside the gate
  (action, params, world, descriptor, provenance, and effect mode) before setting
  `EvalContext.approval_granted` at its own kernel boundary.
- **Why:** This preserves the pure native/WASM/conformance gate contract while closing the forged
  approval-token path. Without a verifier, the only safe default is to fail closed and return `ASK`.
- **Alternatives rejected:** remove the field outright (unnecessary v1 wire break); add store I/O or
  a verifier callback to `harness-preview::gate` (breaks the pure shared gate used by native and
  WASM); continue treating the token as a grant (request-controlled approval bypass).
- **Related:** D24 (host-neutral gate ABI), D34 (in-process vs wire), D37 (live-hook cutover),
  E6 approval binding model.
