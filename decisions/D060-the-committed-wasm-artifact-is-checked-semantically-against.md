# D60 — The committed WASM artifact is checked semantically against the kernel, not byte-for-byte


**Date:** 2026-08-12. **Prompted by the same review (finding #18).**

- **The drift.** `blog/public/vendor/harness-wasm/` is a build output kept in git so the
  playground can load it as a static asset. Nothing rebuilt it. Between 2026-06-22 and
  2026-08-12 it fell nine preview-affecting commits behind — the entire `roots` feature and
  D36 command classification among them — while reporting `version() == "0.0.1"` and every
  CI job stayed green. AGENTS.md had stated the no-drift invariant the whole time. An
  invariant nothing checks is a wish.
- **What the drift actually cost.** Less than it first appears, and the distinction matters:
  the WASM surface exports `preview`, `default_world`, and `version` — not `gate`. So the
  playground was never running an exploitable pre-fix gate; it was *misdescribing* the
  current kernel's decisions to anyone evaluating the project in a browser. A fidelity
  failure, not an exposed vulnerability. Worth fixing on both counts.
- **The decision.** `scripts/check-wasm-freshness.mjs` loads the committed artifact *and* a
  freshly built one and requires them to answer identically: same `version()`, same bundled
  `default_world()`, same `preview()` over a case set that deliberately includes a `roots`
  world and a `command_classes` world — the two features whose absence went unnoticed. The
  `wasm` CI job builds from source and runs it.
- **Alternatives rejected.**
  - *Byte-compare the committed artifact against a fresh build.* The obvious check, and it
    would go red every time `dtolnay/rust-toolchain@stable` or wasm-pack changed codegen.
    That is drift in the build, not in the kernel, and a check that cries wolf gets deleted.
  - *Compare only `version()`.* It would have caught this instance (0.0.1 vs 0.2.1) and
    nothing else: a rebuild within one version window is exactly when drift is hardest to
    see. The preview cases are what give the check teeth — verified by diffing the stale
    artifact against the current one on the `command_classes` case.
  - *Stop committing the artifact and build it in the blog pipeline.* Defensible, and a
    larger change than this finding warrants: it puts a Rust and wasm-pack toolchain in the
    blog's build path. Revisit if the blog ever needs the engine at more than one version.
  - *Install wasm-pack in CI via a third-party action.* This project reviews other people's
    supply chains; it should not add an unpinned action to a workflow to save ninety
    seconds. `cargo install wasm-pack --locked`.
- **Related:** D22 (one engine, no reimplementation), E14; `.github/workflows/ci.yml` (the
  `wasm` job), `scripts/check-wasm-freshness.mjs`.
