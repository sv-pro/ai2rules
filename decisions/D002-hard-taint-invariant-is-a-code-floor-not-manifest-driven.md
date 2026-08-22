# D2 — Hard taint invariant is a code floor, not manifest-driven

- **Epic:** E2 · **Status:** Accepted
- **Decision:** Enforce the taint × side-effect floor in code (`invariants.rs`),
  run before manifest policy; the manifest's `transition_policies` layer
  *additional* taint policy on top in disposition.
- **Alternatives:** Drive the floor purely from manifest `taint_rules`.
- **Why:** A manifest must never be able to *weaken* the floor. The default
  world's rules coincide with it — harmless overlap; the floor holds even if a
  manifest omits them.
