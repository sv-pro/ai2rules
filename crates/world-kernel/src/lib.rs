//! # world-kernel
//!
//! The deterministic governance kernel: sealed `IntentIR`, `IRBuilder`, and (in
//! later epics) invariants, policy disposition, taint, and budgets. No I/O, no
//! LLM, no mutable shared state — decisions are a pure function of
//! `(intent, context, world)`.

pub mod intent;

pub use intent::{IRBuilder, IntentIR};

#[cfg(test)]
mod tests {
    use super::*;
    use harness_types::*;

    fn world_with(action: &str, projected: bool) -> CompiledWorld {
        let mut parts = CompiledWorldParts {
            world_id: WorldId::new("test"),
            ..Default::default()
        };
        let name = ActionName::new(action);
        parts.ontology.insert(name.clone());
        if projected {
            parts.projected.insert(name.clone());
        }
        parts.action_types.insert(name.clone(), ActionType::Read);
        parts.side_effects.insert(name, SideEffectClass::Read);
        CompiledWorld::new(parts)
    }

    fn call(action: &str) -> ToolCall {
        ToolCall {
            action_name: ActionName::new(action),
            arguments: serde_json::json!({}),
            provider: Provider::CliNative,
            call_id: CallId::new("c1"),
            source_perceptions: vec![],
            session_id: SessionId::new("s1"),
        }
    }

    fn prov() -> Provenance {
        Provenance::from_channel(
            SourceChannel::UserPrompt,
            SessionId::new("s1"),
            ContentHash::new("h"),
        )
    }

    #[test]
    fn build_succeeds_for_projected_action() {
        let world = world_with("read_workspace", true);
        let builder = IRBuilder::new(&world);
        let ir = builder
            .build(&call("read_workspace"), prov(), &TaintContext::clean())
            .expect("should build");
        assert_eq!(ir.action().as_str(), "read_workspace");
        assert_eq!(ir.taint(), Taint::Clean);
    }

    #[test]
    fn unknown_action_is_unknown_to_ontology() {
        let world = world_with("read_workspace", true);
        let builder = IRBuilder::new(&world);
        let err = builder
            .build(&call("send_email"), prov(), &TaintContext::clean())
            .unwrap_err();
        assert!(matches!(err, BuildError::UnknownToOntology { .. }));
    }

    #[test]
    fn known_but_unprojected_action_is_absent() {
        let world = world_with("read_workspace", false);
        let builder = IRBuilder::new(&world);
        let err = builder
            .build(&call("read_workspace"), prov(), &TaintContext::clean())
            .unwrap_err();
        assert!(matches!(err, BuildError::Absent { .. }));
    }

    #[test]
    fn taint_is_carried_from_context() {
        let world = world_with("read_workspace", true);
        let builder = IRBuilder::new(&world);
        let ir = builder
            .build(
                &call("read_workspace"),
                prov(),
                &TaintContext::from_taint(Taint::Tainted),
            )
            .expect("should build");
        assert_eq!(ir.taint(), Taint::Tainted);
    }
}
