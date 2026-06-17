//! # provider-adapters
//!
//! Stateless parsers that normalize a model provider's tool-call dialect into the
//! neutral `ToolCall`, and format neutral results back into the provider's shape.
//! No policy lives here — normalize early, gate once (architecture §4).
//!
//! E5 ships the Anthropic adapter ([`anthropic`]); OpenAI / Gemini / MCP share
//! the same neutral contract and are added later (E5.6).

pub mod anthropic;

use harness_types::CallId;

/// The neutral result a tool call produced, ready for a provider adapter to
/// format back to the model (E5.1). The orchestrator fills this from a kernel
/// verdict or an execution result; keeping it here lets `provider-adapters`
/// stay a leaf (no `world-kernel` / `executor` dependency).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolOutcome {
    pub call_id: CallId,
    pub content: String,
    /// True when the call was denied/absent/errored rather than executed.
    pub is_error: bool,
}

/// Why a provider block could not be normalized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterError {
    MissingField(&'static str),
    BadType {
        field: &'static str,
        expected: &'static str,
    },
}

impl std::fmt::Display for AdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdapterError::MissingField(field) => write!(f, "missing field `{field}`"),
            AdapterError::BadType { field, expected } => {
                write!(f, "field `{field}` must be {expected}")
            }
        }
    }
}

impl std::error::Error for AdapterError {}
