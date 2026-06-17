//! Provider-neutral tool-call proposal (architecture §5 `ToolCall`).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ids::{ActionName, CallId, PerceptionId, SessionId};

/// The model provider whose dialect a call was parsed from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Provider {
    Anthropic,
    OpenAi,
    Gemini,
    Mcp,
    CliNative,
}

/// A normalized, provider-independent proposal from the model. Adapters produce
/// this; the kernel consumes it. No policy logic lives on this type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub action_name: ActionName,
    pub arguments: Value,
    pub provider: Provider,
    pub call_id: CallId,
    pub source_perceptions: Vec<PerceptionId>,
    pub session_id: SessionId,
}
