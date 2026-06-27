//! # harness-types
//!
//! Language-neutral core contracts shared across the ai2rules harness (see
//! `docs/harness-architecture.md` §5). This crate is pure data: no I/O, no
//! policy logic, no LLM.
//!
//! `IntentIR` is intentionally **not** defined here — it is sealed inside
//! `world-kernel` so it can only be built by the kernel's `IRBuilder`. Keeping
//! the rest of the contracts here lets `executor`, `trace-store`, and the
//! adapters depend on the types without depending on the kernel.

pub mod action;
pub mod approval;
pub mod compiled;
pub mod decision;
pub mod descriptor;
pub mod error;
pub mod execution;
pub mod ids;
pub mod manifest;
pub mod perception;
pub mod provenance;
pub mod tool_call;

pub use action::*;
pub use approval::*;
pub use compiled::*;
pub use decision::*;
pub use descriptor::*;
pub use error::*;
pub use execution::*;
pub use ids::*;
pub use manifest::*;
pub use perception::*;
pub use provenance::*;
pub use tool_call::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taint_join_is_monotonic() {
        assert_eq!(Taint::Clean.join(Taint::Clean), Taint::Clean);
        assert_eq!(Taint::Clean.join(Taint::Tainted), Taint::Tainted);
        assert_eq!(Taint::Tainted.join(Taint::Clean), Taint::Tainted);
        assert_eq!(Taint::Tainted.join(Taint::Tainted), Taint::Tainted);
    }

    #[test]
    fn channel_defaults_match_architecture() {
        assert_eq!(SourceChannel::UserPrompt.default_taint(), Taint::Clean);
        assert_eq!(SourceChannel::Web.default_taint(), Taint::Tainted);
        assert_eq!(SourceChannel::McpOutput.default_taint(), Taint::Tainted);
        assert_eq!(
            SourceChannel::UserPrompt.default_trust(),
            TrustLevel::Trusted
        );
        assert_eq!(SourceChannel::Web.default_trust(), TrustLevel::Untrusted);
    }

    #[test]
    fn taint_context_joins_outputs() {
        let outputs = vec![
            TaintedValue::clean(1u32),
            TaintedValue::tainted(2u32),
            TaintedValue::clean(3u32),
        ];
        assert_eq!(TaintContext::from_outputs(&outputs).taint(), Taint::Tainted);
        let clean = vec![TaintedValue::clean(1u32)];
        assert_eq!(TaintContext::from_outputs(&clean).taint(), Taint::Clean);
    }

    #[test]
    fn compiled_world_is_read_only_after_construction() {
        let mut parts = CompiledWorldParts {
            world_id: WorldId::new("test"),
            ..Default::default()
        };
        parts.ontology.insert(ActionName::new("read_workspace"));
        parts.projected.insert(ActionName::new("read_workspace"));
        let world = CompiledWorld::new(parts);
        assert!(world.in_ontology(&ActionName::new("read_workspace")));
        assert!(world.is_projected(&ActionName::new("read_workspace")));
        assert!(!world.in_ontology(&ActionName::new("send_email")));
        assert_eq!(world.world_id().as_str(), "test");
    }

    #[test]
    fn approval_token_transitions_are_guarded() {
        let mut token = ApprovalToken::pending(
            ApprovalTokenId::new("t1"),
            ActionName::new("git_commit"),
            ContentHash::new("p"),
            WorldId::new("w"),
            DescriptorHash::new("d"),
            Provenance::from_channel(
                SourceChannel::UserPrompt,
                SessionId::new("s"),
                ContentHash::new("c"),
            ),
            EffectMode::Execute,
        );
        // Cannot execute before approval.
        assert!(token.mark_executed().is_err());
        assert!(token.approve().is_ok());
        assert!(token.mark_executed().is_ok());
        // Cannot reject after execution.
        assert!(token.reject().is_err());
    }
}
