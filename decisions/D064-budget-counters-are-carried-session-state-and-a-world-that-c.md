# D64 — Budget counters are carried session state, and a world that counts calls requires them


**Date:** 2026-08-14. **Closes the Codex-scan finding "runtime budget counters are reset at gate
and agent decision boundaries".**

- **The hole.** `Budget` and `budget_exceeded` were complete and correct; nothing ever supplied
  them a non-zero `BudgetUsage`. `gate()` and the agent orchestrator both constructed
  `BudgetUsage::default()` per decision, so every call was evaluated as the session's first.
  `max_commands_per_task`, `max_network_calls`, `max_file_writes` and
  `max_tokens_per_session` were decorative in every manifest that declared them — including
  all three live ones.
- **The decision: budgets are carried exactly like taint.** The kernel is pure and each hook
  invocation is a separate process, so counters travel in `GateContext.usage`, come back
  charged in `GateResponseContext.usage`, and the adapter persists them in a `usage-<session>`
  sidecar beside the taint marker. The mcp-gateway is one long-lived process and keeps them in
  memory; the orchestrator owns a live session and threads a `&mut BudgetUsage` next to its
  existing `&mut Taint`.
- **Charging lives beside the limit.** `charge()` sits immediately above `budget_exceeded()` in
  `disposition.rs` and mirrors its categories. A limit whose charge lands in a different bucket
  is a limit that never binds, and the only defence against that drift is adjacency plus a test.
  Only an ALLOW that will actually run is charged — a refusal must not push a session toward
  its limit.
- **Omission fails closed, scoped to worlds that count.** `missing_usage` mirrors
  `missing_path`: a control a thin adapter can disable by leaving a field out is not a control.
  It fires only when the manifest declares a *counted* limit; `command_timeout_ms` bounds one
  execution and is enforced by the executor, so it does not oblige a caller to carry counters.
  Unreadable or corrupt counters fail closed for the same reason, and unpersistable ones refuse
  the call they would have charged (D59's rule, applied to budgets).
- **Breaking for the wire ABI, deliberately, without a version bump.** Requiring `usage` under a
  counted budget is the fourth fail-closed hardening of v1, after `taint`, `source_channel` and
  `path`. Bumping `v` would let a caller pin v1 and keep running ungoverned, which is the
  condition being removed.
- **Alternatives rejected.**
  - *Default missing usage to zero.* Preserves compatibility and preserves the finding: any
    caller that omits the field gets an unlimited session, silently.
  - *Accumulate counters inside the kernel.* Would make `evaluate` stateful and destroy the
    property the whole design rests on — a pure, replayable decision function.
  - *Track usage in the trace store and read it back at decision time.* Gives the kernel I/O by
    another route, and makes a verdict depend on a store that may be absent, remote, or slow.
  - *Charge on the ASK, not on the execution.* An approval the human declines would consume
    budget, so repeatedly declining would exhaust the session.
- **Known limit, unchanged by this.** An over-budget verdict is `REPLAN`, and neither Claude
  Code nor Antigravity has a "propose a smaller step" channel, so the adapters fall through to
  the host's own permission flow rather than denying. In replace mode that means the human is
  prompted rather than silently allowed. Tokens are still never charged at the gate — only a
  caller that has seen a model response can count them.
- **Related:** D24 (the gate ABI), D59 (persist-or-refuse), D46/D61 (`missing_path`, the same
  shape); `crates/world-kernel/src/disposition.rs`, `docs/harness-gate-abi.md` §3/§4/§6.
