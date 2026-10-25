# AI2-25 / #87 — doctor fixtures (PR #95)

Status: review fixes implemented; local validation passed; CI pending. Partial acceptance matrix only.

## 2026-10-24 review correction

- Work is isolated on `codex/ai2-25-pr95-doctor-fixtures`; main was integrated from `c9c35a3` without conflicts.
- Both positive settings helpers now use the installer's `type: command`.
- A separate invalid-type fixture changes a real init registration to `external`. It reproduced false `ok/0` before the fix. Doctor now recognizes a shim only for command hooks and returns `broken/2`, `CC_SHIM_UNSUPPORTED`, with the unsupported entry preserved.
- Init and doctor share the same temporary HOME. The per-project switch test takes its literal path from the generated installer output, asserts it is outside the project, toggles it off/on/off, and checks the exact `project_modern` path. Global and obsolete switches stay absent.
- Focused Linux tests: 40 passed, 3 existing specification tests ignored, both normally and with inherited HARNESS_BIN/CLAUDE_PROJECT_DIR set to junk. fmt, workspace clippy and build passed. Full Linux workspace tests: 552 passed, 3 ignored. The Windows checkout required temporary LF normalization of two existing shell fixtures (.claude/hooks/world-gate.sh and scripts/install-governance.sh); original bytes restored after validation.

## Remaining #87 scope

Three ignored specification divergences remain: missing settings classification, obsolete project switch handling, and broken manifest priority over an enabled switch. Permission-error injection, unexpected inspector failure, legacy profiles, FIFO and oversize-input matrix coverage remain follow-up work. This PR does not close #87 or all of AI2-25.

No PR merge is authorized. Demo checkout and deferred PRs #129/#148/#149 are out of scope.
