//! # provider-adapters
//!
//! Stateless parsers that normalize each model provider's tool-call dialect
//! (Anthropic / OpenAI / Gemini / MCP) into the neutral `ToolCall`. No policy
//! lives here — normalize early, gate once.
//!
//! Status: skeleton (E0). Implementation tracked in PLAN.md **E5**.
