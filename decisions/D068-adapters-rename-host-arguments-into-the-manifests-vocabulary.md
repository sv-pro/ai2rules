# D68 — Adapters rename host arguments into the manifest's vocabulary; they do not add to it


**Date:** 2026-08-14. **Closes finding #23/#27.**

- **The hole.** `alias_neutral_args` mapped Antigravity's PascalCase keys onto the neutral ones
  by *adding* the neutral key and keeping the host spelling, on the reasoning that an audit
  record might reference the original. Object schemas are closed by default (an undeclared
  argument is rejected), so the adapter was injecting a second key the manifest author never
  wrote and could not anticipate. Any action with a `schema` therefore returned
  `schema_violation` on that host no matter how well-formed the call was — schemas and
  cross-host portability were mutually exclusive, and nothing said so.
- **The decision.** Rename. The host's spelling is a transport detail; the call, expressed in
  the manifest's vocabulary, has one name for one argument. The kernel never needed the host
  copy: the classifier reads `arguments[arg]` and the resolved path travels out-of-band in
  `GateRequest.path`, so nothing downstream was consuming the original.
- **What deliberately did not change.** A host argument the manifest does not declare still
  fails a closed schema. That is the security property, not collateral damage: an undeclared
  argument is input the kernel was never asked to judge, and quietly dropping it would mean
  deciding on a call different from the one that executes. `additionalProperties: true` is the
  explicit way to say you accept unjudged extras. Both halves are pinned by tests.
- **Alternatives rejected.**
  - *Teach the kernel which host keys to ignore.* Puts Antigravity's vocabulary inside the pure
    kernel, which is the thing the adapter layer exists to prevent.
  - *Strip undeclared arguments in the adapter so closed schemas always pass.* Makes the kernel
    judge a call that differs from the one the host runs — a governance gap dressed as
    convenience, and strictly worse than a refusal.
  - *Default schemas to `additionalProperties: true`.* Inverts a fail-closed default across
    every world to fix one host's spelling.
  - *Document it and move on.* That was the state being fixed; the constraint was undocumented
    folklore, discoverable only by hitting `schema_violation` on exactly one host.
- **Residual worth knowing.** The neutral vocabulary has three path spellings (`path`,
  `file_path`, `notebook_path`), so a schema written against one does not match an adapter that
  normalises to another. Narrowing that is a separate change to the neutral vocabulary itself.
- **Related:** D36, D48, D61; `crates/cli-harness/src/agy_hook.rs`,
  `crates/world-kernel/src/schema.rs`.
