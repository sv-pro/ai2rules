# D32 — Govern GitHub Copilot (and JetBrains) via the MCP surface, not a native hook; lead artifact = a shaped JIRA-MCP capability surface


**Date:** 2026-06-28.

- **Context:** goal is an **internal demo at the maintainer's workplace** on the hosts
  colleagues actually use — most on **GitHub Copilot** (JetBrains for backend devs, VS Code
  for frontend), a few on **Claude Code**.
- **Decision:** govern Copilot (VS Code + JetBrains) and Claude Code for the demo through
  the **MCP surface** via the **Safe MCP Proxy** — capability projection (**ABSENT**),
  scoped-capability **arg-locking**, descriptor-drift, audit. One proxy fronts the
  **Atlassian Remote MCP Server**; **one manifest governs all three hosts**. Lead artifact:
  *give Copilot scoped JIRA access (read + comment on a specific project), every destructive
  JIRA tool ABSENT* — "I can give Copilot JIRA access and not worry about an accidental
  destructive action."
- **Why:** Copilot exposes **no stable third-party per-call gate** over its native tools
  (unlike Claude Code's `PreToolUse`); the **MCP surface is exactly where Copilot *is*
  governable**, and it's **host-agnostic** (the same proxy config serves VS Code, JetBrains,
  and Claude Code). It also plays to our strongest primitive — capability projection /
  ABSENT (D27) — and needs nothing from a vendor roadmap.
- **Reuse (big de-risk):** `repos/safe-mcp-proxy` already ships an **Atlassian passthrough**
  (`atlassian/`: `passthrough.py`, `ManifestPolicyEngine` with `arg_rules`/data-flow,
  `CapabilityFilter`), an **MCP server mode** (`mcp_server --upstream …`),
  `manifests/atlassian_mvp.yaml` (real Atlassian MCP tool names; destructive tools already
  ABSENT; `project_key` arg-locked), an Atlassian demo, and an audit dashboard. So the demo
  is **wire + author-manifest + validate-against-real-JIRA**, not build.
- **Accepted split:** use the **existing Python `safe-mcp-proxy`** for the demo now;
  **Rust productization** of the proxy through the real kernel / gate ABI (**E13.4**) is a
  follow-up, not a demo blocker. (Note: `safe-mcp-proxy` is a reference repo under `repos/`
  — never `git add`ed; the demo's own artifacts — manifest, host configs, runbook — live in
  this repo under `docs/demos/`.)
- **Alternatives (rejected):**
  - *A VS Code / JetBrains extension intercepting Copilot's native file/terminal tools* —
    per-host, fragile, no stable public gate API, deep effort; deferred (a possible later
    surface, not for this demo).
  - *Wait for a Copilot-native governance hook* — not available; won't gate the demo on a
    vendor.
  - *Sandbox / egress-floor only (E8 / D21)* — strong for the exfil story but doesn't
    deliver the **shaped capability surface** the audience asked for; kept as the substrate
    complement, not the lead.
- **Consequence:** `PLAN.md` gains **E16** as the **top near-term priority**, ahead of the
  longer-tail epics.
- **Related:** D24 (gate ABI), D27 (ABSENT vs policy middleware), D31 (delivery model — this
  brings the "Safe MCP Proxy" surface forward), E7 / E13.4, `repos/safe-mcp-proxy`,
  `repos/mcp-tool-projection`.
