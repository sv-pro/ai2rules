//! # executor
//!
//! The execution boundary. Accepts only `ExecutionSpec`, runs it behind a hard
//! process boundary, applies the `EffectMode`, and returns a `TaintedValue`.
//! Evaluates no policy and holds no policy state.
//!
//! Status: skeleton (E0). Implementation tracked in PLAN.md **E3**.
