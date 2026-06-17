//! # agent-core
//!
//! Provider-independent turn state and orchestration: packs typed `Perception`s
//! into context, exposes only the projected tool surface, and drives the loop
//! propose → adapt → kernel → execute → perceive. Depends on the kernel and the
//! edge crates; the dependency only ever flows inward to `harness-types`.
//!
//! Status: skeleton (E0). Implementation tracked in PLAN.md **E5**.
