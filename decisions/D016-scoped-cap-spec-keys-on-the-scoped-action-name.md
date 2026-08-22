# D16 — Scoped-cap spec keys on the scoped action name

- **Epic:** E7 · **Status:** Accepted
- **Decision:** `build_execution_spec` keeps the spec's `action` = the scoped
  capability's name (e.g. `run_tests`) and carries the scoped cap's descriptor
  hash; the executor registers each scoped cap under its own name mapped to the
  base action's handler kind.
- **Alternatives:** Rewrite the spec's action to the base action (`run_command`).
- **Why:** The descriptor hash that drift-checks (invariant 11) is the scoped
  cap's; keying on the scoped name keeps the spec, the registered hash, and the
  audit trail consistent — rewriting to the base would mismatch the hash.
