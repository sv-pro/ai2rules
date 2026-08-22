# D31 — Ship as infrastructure (plugin/sidecar), not a standalone agent; lead with the Claude Code Governance Pack


**Date:** 2026-06-27.

- **Decision:** ai2rules is delivered as a governance **engine that always wraps a host
  the user already runs** — never as our own standalone agent/CLI product. The model is
  **OPA / seccomp / Envoy for agent actions**: standalone *in form* (its own crate /
  binary / hook set), plugin *in role* (zero value alone; it governs another system).
  One kernel is projected onto several surfaces — a **Claude Code Governance Pack**
  (hooks plugin, the **lead / v1**), a **Safe MCP Proxy** (sidecar), the **`harness gate`
  binary** + **`world-kernel` crate** (sidecar / library for embedders). User segments,
  packages, and "install → get" walkthroughs live in [`docs/USE-CASES.md`](../docs/USE-CASES.md).
- **Cut:** the earlier "custom CLI agent on a Claude Code basis" ambition is retired **as
  a product**. The `cli-harness` CLI and the E9 TUI remain a dev / demo / reference
  harness, not the shipped artifact.
- **Why:** the moat is the *border*, not the agent. Shipping our own agent (a) contradicts
  the host-neutral thesis (D24: the gate ABI exists precisely to sit *under* hosts),
  (b) forces competition on our weakest surface — model + host UX, vs. Anthropic / OpenAI
  / Hermes — while diluting the only differentiator (governance), and (c) the plugin form
  rides existing distribution (Claude Code's users, the MCP ecosystem) and is *already
  built and dogfooded*. Adoption path: free individual wedge (the Pack) → team/org
  policy-as-code + audit/replay (the **OPA-for-agents** revenue story) → embedders.
- **Alternatives (rejected):**
  - *Standalone governed agent* — biggest surface, weakest differentiation, contradicts
    host-neutrality.
  - *MCP-proxy-only* (the old Safe-MCP scope) — too narrow as the *lead*: an MCP proxy
    can't see a host's **native** tools (`Bash`/`Edit`/`Write`/`Read`/`WebFetch`), the
    highest-leverage governance gap; it ships as surface #2, not #1.
  - *Library-only* — too high-friction for the adoption wedge; ships as surface #3 for
    embedders.
- **Consequence for the plan:** `PLAN.md` gains a "Delivery model & packaging" section
  sequencing existing epics as products (CC Pack first); supporting layers (knowledge /
  intent / substrate) ship later as optional sidecars behind spine contracts, never as a
  v1 prerequisite.
- **Related:** D24 (host-neutral gate ABI), D30 (umbrella rename), `docs/THESIS.md` §4/§7,
  `docs/RESEARCH-BACKLOG.md` R1 (the cross-host super-harness is a *later* surface of this
  same engine, not v1).
