# D51 — The world owns the tool surface it publishes, not the upstream


**Date:** 2026-08-01. Closes security finding **#14**
(`sv-pro/agentic-execution-governance#14`, P1). Extends D33 (mcp-gateway), D45 (declared
arguments only), and D50.

- **The bug.** `harness mcp-gateway` filtered the upstream's `tools/list` **by name** and
  republished each surviving tool object verbatim — including its `inputSchema`. A malicious
  or drifted upstream could therefore advertise an extra, dangerous argument on a tool the
  world *does* allow (`jira_get_issue` carrying `deleteAll`), and the model would be offered
  it as a legitimate parameter of a legitimate tool. Name-level authorization; schema-level
  trust.
- **Half of the finding was already closed, and saying so matters.** The finding also reports
  that the raw argument is *forwarded* after an ALLOW. It no longer is: **D45** (sealed
  execution-spec argument fields) made an undeclared object property a `schema_violation` at
  the gate, so `deleteAll: true` is DENY before anything reaches the upstream. That is pinned
  here by test rather than assumed, with the poisoned mock echoing back whatever arrives to
  prove the call never landed.
- **Decision.** Each tool that survives projection is **re-issued from the world's
  descriptor**: `name` and `inputSchema` come from the compiled manifest, never from the
  upstream. A tool the world cannot describe is dropped rather than passed through
  (fail-closed, even though projection should already have excluded it). When an upstream
  schema differs from the world's, the gateway logs it — benign drift and attempted poisoning
  are indistinguishable at that seam, so it says so either way rather than choosing.
- **Deliberately *not* done: a second argument filter in the adapter.** The remediation text
  says "forward only sanitized, descriptor-validated parameters", which is satisfied by the
  kernel refusing undeclared arguments — what crosses is declared *by construction*. Adding an
  independent filter in the gateway would duplicate policy into an adapter (the D34/D36/D48
  drift) and create two schema interpretations that can disagree. A world that sets
  `additionalProperties: true` is making an explicit choice, and the gateway should not
  second-guess it.
- **Known residual: tool *descriptions* are still the upstream's.** The manifest has no field
  for one, and sending a bare name would leave the model unable to use the tool. So prose-level
  tool poisoning — malicious instructions in a description — remains open. It is a different
  vector from this finding (which is about arguments), and it is exactly what MCP `2026-07-28`
  means when it says annotations from an untrusted server should be treated as untrusted.
  Tracked separately rather than silently absorbed.
- **This is the line D49's transparency work does not cross.** `cbd9541` made the gateway
  deliberately *more* faithful to the upstream — forwarding `_meta`, preserving `tools/list`
  result siblings. That is right for **protocol metadata** and wrong for the **security
  surface**. Transparent about what the protocol says; authoritative about what the world
  allows.
- **Related:** D33, D45, D49, D50; `docs/one-kernel-many-hosts.md`, finding #14,
  `crates/cli-harness/tests/mcp_gateway_poisoned.rs`.
