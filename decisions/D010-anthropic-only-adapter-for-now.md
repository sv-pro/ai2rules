# D10 — Anthropic-only adapter for now

- **Epic:** E5 · **Status:** Accepted
- **Decision:** Build only the Anthropic adapter (E5.1–E5.5); defer OpenAI/Gemini
  (E5.6).
- **Alternatives:** Build all three adapters now.
- **Why:** One adapter proves the single gate end-to-end; the others share the
  neutral `ToolCall` contract, so adding them later is mechanical.
