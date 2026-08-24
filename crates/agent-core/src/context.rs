//! Context packing and the projected tool surface (E5.4).
//!
//! The model is shown only the tools the compiled world *projects* — dangerous
//! actions that exist in the ontology but aren't projected are never offered. Raw
//! execution payloads never enter context directly; the model sees typed
//! [`Perception`]s plus explicit governance feedback for non-executed calls.

use harness_types::{ActionName, CompiledWorld, Decision, Descriptor, Perception};
use provider_adapters::anthropic;
use serde_json::Value;

/// Deterministic policy/execution feedback for a proposed tool call that did not
/// produce a normal executor perception (DENY, ABSENT, pending ASK, spec/exec
/// failure, and similar outcomes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolFeedback {
    pub action: String,
    pub verdict: String,
    pub decision: Option<Decision>,
    pub rule: Option<String>,
    pub is_error: bool,
}

/// What the model sees this turn: prior perceptions, governance feedback, and
/// the currently projected tool surface.
#[derive(Debug, Clone)]
pub struct TurnContext {
    pub perceptions: Vec<Perception>,
    pub feedback: Vec<ToolFeedback>,
    /// Anthropic-format tool definitions for exactly the projected actions.
    pub tools: Value,
}

/// The projected tool surface: each projected action paired with its descriptor.
/// Only projected actions appear — `is_projected` gates membership.
pub fn tool_surface(world: &CompiledWorld) -> Vec<(ActionName, &Descriptor)> {
    world
        .projected_actions()
        .filter_map(|action| world.descriptor(action).map(|d| (action.clone(), d)))
        .collect()
}

/// Pack typed perceptions plus the projected surface into a turn context.
/// Existing callers that do not need explicit governance feedback keep using
/// this convenience entry point.
pub fn pack(world: &CompiledWorld, perceptions: Vec<Perception>) -> TurnContext {
    pack_with_feedback(world, perceptions, Vec::new())
}

/// Pack perceptions, deterministic governance feedback, and the projected tool
/// surface. The orchestrator uses this form so a model can genuinely react to a
/// refusal rather than merely having that refusal printed in a transcript.
pub fn pack_with_feedback(
    world: &CompiledWorld,
    perceptions: Vec<Perception>,
    feedback: Vec<ToolFeedback>,
) -> TurnContext {
    let surface = tool_surface(world);
    let tools = anthropic::tool_definitions(&surface);
    TurnContext {
        perceptions,
        feedback,
        tools,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use compiler::compile_default;

    #[test]
    fn surface_is_only_projected_actions() {
        let world = compile_default();
        let surface = tool_surface(&world);
        assert!(surface.iter().all(|(a, _)| world.is_projected(a)));
        assert_eq!(surface.len(), world.projected_actions().count());
        assert!(surface.iter().any(|(a, _)| a.as_str() == "read_workspace"));
    }

    #[test]
    fn pack_exposes_tools_as_anthropic_defs() {
        let world = compile_default();
        let ctx = pack(&world, Vec::new());
        let tools = ctx.tools.as_array().expect("tools is an array");
        assert_eq!(tools.len(), tool_surface(&world).len());
        assert!(tools
            .iter()
            .all(|t| t.get("name").is_some() && t.get("input_schema").is_some()));
        assert!(ctx.feedback.is_empty());
    }

    #[test]
    fn pack_with_feedback_preserves_governance_result() {
        let world = compile_default();
        let expected = ToolFeedback {
            action: "fetch_web".into(),
            verdict: "Deny (taint_invariant)".into(),
            decision: Some(Decision::Deny),
            rule: Some("taint_invariant".into()),
            is_error: true,
        };
        let ctx = pack_with_feedback(&world, Vec::new(), vec![expected.clone()]);
        assert_eq!(ctx.feedback, vec![expected]);
    }
}
