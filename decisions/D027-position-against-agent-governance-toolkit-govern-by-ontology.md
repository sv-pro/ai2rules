# D27 — Position against Agent Governance Toolkit: govern by ontology + taint + process boundary, not by policy middleware

- **Status:** Accepted (positioning) · **See:** [`docs/THIRD-PARTY-ADOPTION.md`](../docs/THIRD-PARTY-ADOPTION.md) (A), [`docs/THESIS.md`](../docs/THESIS.md) §8
- **Decision:** Treat Microsoft's **Agent Governance Toolkit (AGT)** as the dominant
  *prior art* for the Action/Capability layers and position explicitly against it —
  neither adopt it nor ignore it. The differentiation is **mechanism, not goal**: AGT
  states our headline almost verbatim ("incapable of misbehaving," not "ask the agent
  to behave") but enforces via **in-process policy middleware** — a
  `default_action: allow` engine evaluating deny rules, with the policy engine and the
  agent sharing **one process boundary** (AGT's own SECURITY note). That is governance
  by *policy decision*. The border governs by *structure*: the dangerous capability is
  **`ABSENT`** (it does not exist in the compiled world, not denied by a rule a model
  can argue with), taint is **monotonic and provenanced**, and the policy layer
  **owns no handler callables** (the process-boundary primitive). Record AGT's MCP
  Security Gateway (tool-poisoning / descriptor drift) as a parallel to
  `safe-mcp-proxy`'s descriptor-drift primitive, and its OWASP-Agentic-Top-10 +
  PromptDefense corpus as **Flywheel discovery input**.
- **Alternatives:** (a) **adopt AGT as the policy layer** — rejected: different stack,
  and a same-process, default-allow rule engine is precisely the LLM-arguable surface
  the border removes; (b) **ignore it** — rejected: it is the most credible same-pitch
  project (Microsoft, MIT, 992 conformance tests), so silence cedes the comparison
  that *is* our contribution; (c) **reframe our positioning to avoid the overlap** —
  rejected: the overlap is the leverage — "deny-rule vs absent capability" only lands
  against a concrete incumbent.
- **Why:** the contrast (policy-decision vs ontology + taint + boundary) is the
  sharpest statement of the thesis and is only legible against the strongest existing
  system. Their conformance-test + RFC-2119-spec discipline is also a *method* worth
  borrowing for our own invariants.
- **Caveat:** AGT ships a package literally named **Agent Hypervisor** — distinct from
  our source `repos/agent-hypervisor` (a different artifact). Disambiguate in any
  public writing.
