# D3 — Minimal, no-dependency schema validation

- **Epic:** E2 · **Status:** Accepted
- **Decision:** Hand-rolled argument validation (required keys, declared-property
  types, `enum`/`const`) in `world-kernel/schema.rs`.
- **Alternatives:** Pull in a JSON Schema crate.
- **Why:** Keeps the lean offline dependency set; the default world carries no
  schemas yet. Full Draft validation deferred as later hardening.
