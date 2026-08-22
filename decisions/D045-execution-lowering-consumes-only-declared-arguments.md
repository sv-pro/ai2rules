# D45 — Execution lowering consumes only declared arguments


**Date:** 2026-07-23.

- **Context:** A security review found that model-facing schemas validated one argument surface
  while `build_execution_spec` could later consume undeclared local-handler fields. `run_command`
  accepted a modeled `command` but lowered attacker-supplied `argv`; `apply_patch` exposed a
  modeled `patch` argument but lowered hidden `path` and `contents`.
- **Decision:** Object schemas with declared properties now reject undeclared arguments by default
  unless `additionalProperties: true` is explicit. Local-handler spec lowering also carries a
  descriptor/scoped-capability argument contract and refuses to read a field not explicitly declared
  there, so even permissive schemas cannot become execution-field backdoors. Scoped capabilities
  keep their existing narrowing behavior: unknown or locked actor inputs are stripped before schema
  validation and again before lowering. The default `apply_patch` descriptor now matches its real
  E3 handler: a full-file write with required `path` and `contents`.
- **Why:** The `ExecutionSpec` is the only object allowed to cross into the executor. Its contents
  must be derived from fields proven by the sealed descriptor or scoped capability, not from extra
  JSON keys a model can smuggle beside a benign modeled argument.
- **Alternatives rejected:** rely only on schema validation (misses explicit
  `additionalProperties: true` and direct lowering drift); keep silently ignoring scoped extras but
  allow base extras (the reviewed bypass); replace the full-file patch handler with unified-diff
  application in this patch (larger product change, offline diff library deferred since E3).
- **Related:** D9 (ExecutionSpec boundary), D24 (gate ABI), D36 (kernel-side classification),
  D43 (compiled manifest policy), E3 full-file patch handler.
