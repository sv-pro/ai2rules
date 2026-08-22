# D20 — Cross-agent taint rides Claude Code's shared in-process session id

- **Epic:** E13 · **Status:** Accepted (empirical)
- **Decision:** Do not build explicit parent↔subagent taint propagation for
  in-process subagents. An experiment (instrumented `world-gate.py` debug log +
  a spawned subagent) showed Claude Code assigns **one shared `session_id` to the
  whole in-process agent tree** (subagents are distinguished by `agent_id` /
  `agent_type`, not a new session). Since taint is keyed by `session_id`, child
  and parent already read/write the *same* sidecar — propagation is automatic and
  conservative (a subagent touching untrusted data taints the whole tree). Add a
  `SubagentStop` hook (`taint-notify.py`) that (a) surfaces taint to user+model
  when a subagent finishes (observability — the floor isn't silent), and (b)
  unions a child's taint into a *distinct* parent session if the host ever exposes
  a parent link.
- **Alternatives:** (1) Build per-agent taint stores + explicit propagation —
  rejected: redundant in-process, and it presumed a gap the experiment disproved.
  (2) Ignore subagents — rejected: a fail-open laundering gap (the intra-run
  ZombieAgent) *if* the shared-session assumption were ever false.
- **Why:** verify the host's real semantics instead of assuming them; lean on the
  shared session where it holds, name/enforce the invariant ourselves where it
  doesn't.
- **Known limit:** agents that run **isolated** (separate worktree / background /
  remote) get a distinct `session_id` *and* a distinct `.claude/state`, so the
  shared-sidecar propagation no longer applies and a local hook can't reach the
  child's state. Out of scope for the local-sidecar approach (the real fix is the
  in-data taint of the in-process kernel, or a shared taint store).
