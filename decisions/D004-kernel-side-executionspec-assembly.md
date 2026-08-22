# D4 — Kernel-side `ExecutionSpec` assembly

- **Epic:** E3 · **Status:** Accepted
- **Decision:** `world-kernel::build_execution_spec` mints the spec from a sealed
  `IntentIR`; `KernelOutcome::Evaluated` carries the intent so an `ALLOW` can be
  lowered. Runtime config arrives via `ExecEnv` (kernel stays pure).
- **Alternatives:** Build the spec in a separate orchestrator step.
- **Why:** The kernel is the sole producer of the only object that crosses the
  boundary (architecture §6); the `executor` keeps **no** dependency on the
  kernel and evaluates no policy.
