# D75 — Discovery projection lives beside `gate`, in `harness-preview`


**Context.** D72 introduced discovery projection as the sibling wire ABI to `gate`:
the world owns which names exist, the host owns the schema. The implementation,
however, landed inside `cli-harness/src/project.rs` — a *wire skin* that also held
the projection logic. `gate` had already been split the other way (pure function in
`harness-preview`, stdin/stdout skin in `cli-harness`) precisely so that native,
WASM, and in-process Rust hosts could not answer differently.

The asymmetry became visible the moment a second caller appeared. The governance
benchmark (E18) needs to ask the projection question from Rust; with the logic in a
`[[bin]]`-only crate its only options were to spawn the CLI or to reimplement the
filter — and an adapter that reimplements a governance filter is exactly what the
one-kernel model exists to prevent.

**Decision.** Move the pure projection into `harness_preview::project`, beside
`gate` and `preview`. `cli-harness/src/project.rs` keeps the wire operation —
argument parsing, manifest loading, stdin/stdout — and delegates. The three
projection tests moved with the function.

**Alternatives.** (a) Leave it and let the benchmark spawn `harness project` for
every call: correct, but it makes an in-process Rust host a second-class citizen
and forbids a linked/wire parity check. (b) Give `cli-harness` a `[lib]` target and
depend on that: works, but it makes a terminal entrypoint into a library other
crates link, and puts governance logic in the crate whose job is host plumbing.
(c) Duplicate the filter in the benchmark: rejected outright — the drift would be
undetectable and the benchmark would be measuring its own copy.

**Consequence.** One projection implementation, reachable from the CLI, from
in-process Rust, and (when exported) from WASM. E18 exercises both transports on
every scenario and compares them step for step, so a future divergence is a test
failure rather than a discovery. No wire behaviour changed: `harness project`
answers byte-identically.
