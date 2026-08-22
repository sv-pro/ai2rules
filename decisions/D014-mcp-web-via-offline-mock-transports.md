# D14 — MCP/web via offline mock transports

- **Epic:** E7 · **Status:** Accepted
- **Decision:** MCP and web go through pluggable `McpTransport` / `WebFetcher`
  traits with deterministic mock impls; MCP dispatch and web fetch flow through
  the same IntentIR/descriptor/provenance gate and the executor's drift check,
  with no network or async.
- **Alternatives:** Real stdio/HTTP MCP transport + real web client (reqwest) now.
- **Why:** Keeps CI offline and deterministic, matching the kernel and the E5
  model client (D9). Real transports are a later, feature-gated add.
