# D1 — `harness-types` as the foundation crate

- **Epic:** E0 · **Status:** Accepted
- **Decision:** Put the language-neutral contracts in a dedicated `harness-types`
  crate that every other crate depends on inward; keep `IntentIR` in
  `world-kernel`.
- **Alternatives:** Define the contracts inside `world-kernel`.
- **Why:** Lets `executor`, `trace-store`, and the adapters depend on the
  contracts **without** depending on the kernel, while Rust's privacy still
  *seals* `IntentIR` (only `IRBuilder::build` can mint one).
