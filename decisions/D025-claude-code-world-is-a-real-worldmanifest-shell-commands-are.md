# D25 — Claude Code world is a real WorldManifest; shell commands are adapter-classified into distinct actions

- **Epic:** E13.8 (extends D19/D24) · **Status:** Accepted
- **Decision:** Express the Claude Code host world as a real `WorldManifest`
  (`.claude/cc-world.yaml`), compiled by the real compiler and governed per call via
  `harness gate` — replacing the bespoke `cc-world.json` schema. Claude Code's
  native tools map onto manifest actions, most 1:1. Because the kernel decides at
  **action granularity** and must not parse shell syntax, the host adapter
  **classifies `Bash` by command shape into three distinct actions**: `Bash`
  (Process), `Bash_network` (egress patterns curl/wget/nc/ssh/… → side_effect
  Network), and `Bash_destructive` (rm -rf/sudo/mkfs/… → `approval_required`). The
  manifest declares each action's policy; the adapter only chooses which action a
  given command *is*. Verified end-to-end: tainted `WebFetch`/`Bash_network` → DENY
  (`taint_invariant`), `Bash_destructive` → ASK, clean reads → ALLOW, unknown tool →
  ABSENT — all by the real kernel.
- **Deferred (path-based read-taint):** `cc-world.json` also tainted the session on
  *reading* an untrusted path (`repos/`, `untrusted/`). The v1 gate escalates
  post-call taint by **side-effect class** (Network/External/Memory), not by
  read-path, so this heuristic is **not yet preserved**. The faithful fix is either
  (a) escalate by the call's `source_channel` trust (the adapter tags an untrusted
  read with an untrusted channel) or (b) an untrusted-read-roots manifest field —
  both design-level, recorded here as the open follow-up per *decisions-outrank-code*
  rather than patched ad hoc in the adapter.
- **Alternatives:** (a) **command-pattern rules in the manifest/kernel** (the kernel
  regex-matches shell commands) — rejected: puts shell-syntax parsing into the pure
  kernel, and patterns are host-specific; the adapter is the right place for
  host-shape normalization. (b) **mark `Bash` as `Network` wholesale** — rejected:
  over-broad, every `ls`/`cat` would be treated as egress and blocked under taint.
  (c) **keep `cc-world.json` + the Python reimplementation** — rejected by D24
  (drift / two sources of truth). (d) **one synthetic `Bash` action with arguments
  inspected by the kernel** — same shell-parsing-in-kernel objection as (a).
- **Why:** a real manifest makes the Claude Code world the *same* compiled artifact
  the harness runs on (one source of truth, D24), and action-level classification
  keeps the kernel pure while still catching the high-leverage cases (egress under
  taint, destructive commands). The boundary is honest: *what a command is*
  (host-syntactic) is the adapter's job; *what an action may do* (policy) is the
  manifest's.
- **Known limit:** classification fidelity is bounded by the adapter's pattern set
  (a crafted command can evade the egress patterns) and `PreToolUse` can't rewrite
  args — the same host-fidelity ceiling as D19. The manifest is the floor, not a
  complete sandbox; the E13.7 container + egress proxy is the enforcement backstop.
