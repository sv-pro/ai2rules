# D50 — Remote ingress taints by *backing*, not by declared side effect


**Date:** 2026-08-01. Closes security finding **#13**
(`sv-pro/agentic-execution-governance#13`, P1). Extends D24 (gate ABI) and the invariant-7
taint floor.

- **The bug.** `harness mcp-gateway` forwarded an upstream read result to the model without
  the session becoming tainted, so the *next* external write sailed through the taint floor.
  The PoC is two calls: `jira_get_issue` then `jira_add_comment` — both audit rows showed
  `taint_in:false` and the comment was forwarded to the upstream server.
- **Root cause was in the manifests, and the kernel let them lie.** Post-call taint was
  computed purely from the declared `side_effect` class, and all three JIRA demo worlds
  declared their *remote MCP read tools* as `action_type: Read, side_effect: Read` — the same
  class as reading a local file. `side_effect_taint(Read) = Clean`, so a fetch across a
  process boundary from a server we do not control introduced no taint. The one action
  declared honestly (`jira_add_comment`, `!McpServer` backing, `External`) tainted correctly,
  which is why the tainted-session tests passed and hid this.
- **Decision.** An action whose descriptor carries a `BackingIdentity::McpServer` taints its
  output on ALLOW, **regardless of the declared side-effect class**, joined monotonically with
  the carried taint. Keyed on the *backing* — what the action actually talks to — so a manifest
  cannot describe a remote fetch as a clean local read, by accident or by an author's
  optimism. The three demo worlds are also corrected to declare their real backing.
- **Alternatives rejected:**
  - *Escalate `session_taint` in the gateway after any successful upstream call.* The smallest
    diff, and wrong: it puts taint algebra in an adapter, which is exactly the drift D34/D36/D48
    exist to prevent, and it would fix only the MCP gateway while `cc-hook`, `agy-hook` and the
    OpenCode plugin kept the same blind spot. **The adapter needed no change at all** — it
    already escalates from the gate's post-call taint; the kernel was under-tainting.
  - *Fix the manifests only (declare the reads `side_effect: External`).* Fixes these three
    worlds and leaves the trap armed for the next author. It also changes reachability: the
    demo's transition policies deny `Tainted → External`, so reads would stop working in a
    tainted session — a behavior change the finding does not ask for.
  - *Key on `action_type: Mcp`.* Rejected because `action_type` feeds the capability matrix
    (`can_perform(trust, action_type)`): reclassifying the reads would have silently removed
    them from `Untrusted`/`Derived` actors, changing projection as a side effect of a taint fix.
- **Three tests asserted the vulnerable behavior** and had to be corrected — the gateway e2e
  (`clean_session_allows_read_and_comment_but_destructive_is_absent`), the Atlassian-skin e2e,
  and the cross-host conformance case `clean_jira_read_is_allowed` (`expect: taint: clean`).
  Worth stating plainly: the suite was green *because* it encoded the bug, across all entry
  points at once. The renamed cases now say what the kernel does.
- **Demo narrative improves.** The scorecard's taint-floor beat no longer needs
  `TAINT=tainted` to simulate an untrusted context: in an ordinary clean session, reading a
  JIRA issue is itself the untrusted ingress, and the write that follows is denied.
- **Related:** D24, D34, D36, D48, D49; `docs/harness-gate-abi.md`,
  `docs/demos/jira-copilot/`, finding #13.
