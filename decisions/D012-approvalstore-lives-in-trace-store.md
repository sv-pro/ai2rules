# D12 — ApprovalStore lives in `trace-store`

- **Epic:** E6 · **Status:** Accepted
- **Decision:** The durable approval store is a module in `trace-store`
  (append-only JSONL transitions, folded on load), reusing its serde/JSONL/io
  patterns and `compiler::sha256_hex` for the params-binding hash.
- **Alternatives:** A new `approval-store` crate; or in `agent-core`.
- **Why:** `trace-store` is already the durable-persistence home and carries the
  needed deps; a new crate would re-establish the same dependencies for one
  module. (Trade-off: approvals are operational state, not audit — colocated for
  pragmatism, separable later if it grows.)
