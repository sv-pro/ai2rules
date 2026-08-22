# D9 — Offline `ModelClient` trait; defer a live HTTP client

- **Epic:** E5 · **Status:** Accepted
- **Decision:** `agent-core` defines a `ModelClient` trait + a deterministic
  `ScriptedModel`; the Anthropic piece is pure format translation. No network,
  no async, no API key.
- **Alternatives:** Add a real Anthropic HTTP client (reqwest + tokio) now.
- **Why:** Keeps CI offline and the loop deterministic, matching the kernel's
  posture. A live client is a later, feature-gated add.
