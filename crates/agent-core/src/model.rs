//! The model client (E5.5).
//!
//! `ModelClient` is the seam a real LLM plugs into; E5 ships only a deterministic
//! `ScriptedModel` (canned turns) so the loop runs fully offline. A turn is
//! either an Anthropic `tool_use` block to propose (exercising the real adapter
//! path) or a final text answer. Note a `ModelTurn` can carry a tool call but
//! never an `ExecutionSpec` — the model can only *propose* (invariant 4).

use std::collections::VecDeque;

use serde_json::Value;

use crate::context::TurnContext;

#[derive(Debug, Clone)]
pub enum ModelTurn {
    /// An Anthropic `tool_use` block proposing a tool call.
    ToolUse(Value),
    /// A final natural-language answer; ends the session.
    Final(String),
}

pub trait ModelClient {
    /// Produce the next turn given the packed context (perceptions + tools).
    fn next(&mut self, ctx: &TurnContext) -> ModelTurn;
}

/// A deterministic model: replays a fixed list of turns. When exhausted it ends
/// the session with a final note.
pub struct ScriptedModel {
    steps: VecDeque<ModelTurn>,
}

impl ScriptedModel {
    pub fn new(steps: impl IntoIterator<Item = ModelTurn>) -> Self {
        Self {
            steps: steps.into_iter().collect(),
        }
    }
}

impl ModelClient for ScriptedModel {
    fn next(&mut self, _ctx: &TurnContext) -> ModelTurn {
        self.steps
            .pop_front()
            .unwrap_or_else(|| ModelTurn::Final("(script complete)".to_string()))
    }
}
