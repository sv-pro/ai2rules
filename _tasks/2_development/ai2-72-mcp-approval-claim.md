# MCP approvals: claim before upstream effects

**Owner:** Codex. **Status:** fix prepared; PR remains open, no merge authorized.
**Linear:** [AI2-72](https://linear.app/ai2rules/issue/AI2-72/mcp-approvals-durable-pre-effect-claim-and-safe-ambiguous-outcomes-pr).
**GitHub:** [PR #17](https://github.com/sv-pro/ai2rules/pull/17).
**Branch:** `codex/ai2-72-pr17-approval-claim` → PR branch `approval-capable-hosts`.

## Review finding

The previous gateway called the upstream before marking consent executed. An
effect followed by a disconnect, or an ignored persistence failure, left the
grant reusable. Main's D73/D78 already supplies durable atomic consumption.

## 2026-10-24 — implementation and review

- Integrated main `0504335` in an isolated worktree, retaining its store, kernel
  and scoped-forwarding contracts. Demo checkout was not used for changes.
- Consume before RPC, reevaluate policy, and use the common forwarding path.
  Unknown results keep Consumed; no retry can reuse that authorization.
- Explicit launcher principal, upstream/request/effective-call binding,
  five-minute expiry, and operator list/approve/reject workflow.
- Eight subprocess regression cases plus a native gate test. A negative control
  removing consumption reproduced Approved after effect + disconnect; the fixed
  path requires Consumed. Independent investigator and fresh candidate reviewer
  checked the boundary; candidate review found no actionable findings.
- D89 / E13.4d replace the old PR's collisions with occupied D71 / E18 numbering.

Await CI and maintainer review. Do not merge or update the demo binary as part of
this task. Windows approval storage remains the separate deferred #129 task;
the new store integration tests run on Unix with Python 3.

Local validation: complete workspace tests and `clippy -D warnings` passed on
WSL/Linux. Two existing shell fixtures needed temporary CRLF-to-LF conversion
for Bash; their original bytes were restored. Governance benchmark passed over
both transports and its committed reports are unchanged.

CI follow-up: the first WASM job detected CRLF embedded from the Windows checkout
while CI builds from LF. Rebuilt with the single embedded default_world.yaml
normalized temporarily, checked freshness, then restored its exact original bytes.
No compiler logic or manifest content changed.
