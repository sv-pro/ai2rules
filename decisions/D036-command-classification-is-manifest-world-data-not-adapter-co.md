# D36 — Command classification is manifest/world data, not adapter code


**Date:** 2026-07-12. **Extends D25** (which placed classification in the adapter).

- **Context:** D25 let each host adapter classify `Bash` by command shape into
  `Bash`/`Bash_network`/`Bash_destructive`. By E17 the same pattern lists + word-boundary
  matcher existed **three times** — Rust (`cc_hook.rs`), TypeScript (`ai2rules-gate.ts`),
  Python (`world-gate.py`) — the exact reimplementation-drift class D24 exists to end
  (one had already drifted once: the word-boundary fix had to be ported to all copies).
- **Decision:** classification is **world data**. The manifest gains `command_classes`
  (`action` + `arg` (default `command`) + ordered `classes: [{to, patterns}]`), compiled
  into `CompiledWorld`; `gate()` resolves the **effective action** first
  (`classify_command`: first class whose any pattern matches at a left word boundary) and
  returns it as the new `GateResponse.action` field (a backward-compatible v1 addition,
  used in the approval token and the adapters' taint-cause notes). Adapters send the
  **raw host tool name**. `skip_serializing_if` keeps pre-D36 manifest hashes stable;
  `validate()` rejects classifiers naming undeclared actions or empty patterns. The D25
  golden vectors moved into `harness-preview` gate tests; a conformance test pins the
  pattern lists byte-identical across the three host manifests.
- **Alternatives rejected:** (a) **per-adapter regex copies** (status quo) — three
  drifting engines; (b) **a generated shared list** (codegen from one source into each
  language) — sync tooling for what is simply *data the kernel already compiles*;
  (c) **host-specific exceptions** (let a host override classes locally) — reintroduces
  per-host policy, the thing adapters must never own.
- **Why this does not violate "no shell parsing in the kernel" (D25 alt (a)):** the
  kernel still parses nothing — it substring-matches operator-declared patterns from the
  compiled world, the same class of data-driven check as `arg_constraints`. What a
  command *is* remains manifest-declared (design-time, auditable), not adapter-coded.
- **Related:** D24, D25, D34, `docs/one-kernel-many-hosts.md`, `tests/one_kernel.rs`.
