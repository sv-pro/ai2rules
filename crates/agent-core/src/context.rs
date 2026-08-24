//! Context packing and the projected tool surface (E5.4).
//!
//! The model is shown only the tools the compiled world *projects* — dangerous
//! actions that exist in the ontology but aren't projected are never offered. Raw
//! payloads never enter context; the model sees typed [`Perception`]s.

use harness_types::{ActionName, CompiledWorld, Descriptor, Perception};
use provider_adapters::anthropic;
use serde_json::Value;

/// What the model sees this turn: prior perceptions and the tool surface.
#[derive(Debug, Clone)]
pub struct TurnContext {
    pub perceptions: Vec<Perception>,
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
pub fn pack(world: &CompiledWorld, perceptions: Vec<Perception>) -> TurnContext {
    let surface = tool_surface(world);
    let tools = anthropic::tool_definitions(&surface);
    TurnContext { perceptions, tools }
}

#[cfg(test)]
mod tests {
    use super::*;
    use compiler::compile_default;

    #[test]
    fn surface_is_only_projected_actions() {
        let world = compile_default();
        let surface = tool_surface(&world);
        // Every entry is projected, and the count matches the projected set.
        assert!(surface.iter().all(|(a, _)| world.is_projected(a)));
        assert_eq!(surface.len(), world.projected_actions().count());
        // The default world projects its base actions — read_workspace is offered.
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
    }
}
