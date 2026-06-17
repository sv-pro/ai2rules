//! # world-kernel
//!
//! The deterministic governance kernel. It runs in two phases (architecture §6):
//!
//! - **Representability** — [`IRBuilder::build`] turns a neutral `ToolCall` into
//!   a sealed [`IntentIR`] or a typed [`harness_types::BuildError`]. A built
//!   `IntentIR` is representable by construction.
//! - **Disposition** — [`disposition::evaluate`] turns a built intent into a
//!   [`harness_types::Disposition`] (`ALLOW + EffectMode`, `ASK`, `DENY`,
//!   `REPLAN`).
//!
//! [`decide`] is the single entry point that runs both and returns a
//! [`KernelOutcome`]. The kernel is pure: no I/O, no LLM, no mutable shared
//! state — every decision is a function of `(intent, context, world)`.

pub mod disposition;
pub mod intent;
pub mod invariants;
pub mod schema;
pub mod taint;

pub use disposition::{evaluate, BudgetUsage, EvalContext};
pub use intent::{IRBuilder, IntentIR};

use harness_types::{
    ActionName, BuildError, CompiledWorld, Decision, Disposition, Provenance, ToolCall,
};

/// The kernel's verdict for one proposed call — the single deterministic result
/// the orchestrator (E5) and CLI (E9) consume.
#[derive(Debug, Clone, PartialEq)]
pub enum KernelOutcome {
    /// The action is unknown to the compiled ontology. Kept as its own variant,
    /// distinct from `Absent`, so callers honor acceptance invariant 3.
    UnknownToOntology { action: ActionName },
    /// A known action that could not be sealed. The originating `BuildError` is
    /// mapped to a present-action `Decision` (with `rule`) for rendering and
    /// retained for the trace layer.
    NotRepresentable {
        decision: Decision,
        rule: String,
        error: BuildError,
    },
    /// The intent was built and evaluated; carries the full disposition.
    Evaluated(Disposition),
}

impl KernelOutcome {
    /// The effective policy decision. `UnknownToOntology` collapses to `Absent`
    /// for rendering; the variant itself stays distinct for invariant 3.
    pub fn decision(&self) -> Decision {
        match self {
            KernelOutcome::UnknownToOntology { .. } => Decision::Absent,
            KernelOutcome::NotRepresentable { decision, .. } => *decision,
            KernelOutcome::Evaluated(d) => d.decision,
        }
    }
}

/// Run the full kernel pass: build (representability) then evaluate
/// (disposition). Pure and deterministic in `(world, call, provenance, ctx)`.
pub fn decide(
    world: &CompiledWorld,
    call: &ToolCall,
    provenance: Provenance,
    ctx: &EvalContext,
) -> KernelOutcome {
    let builder = IRBuilder::new(world);
    match builder.build(call, provenance, &ctx.taint) {
        Ok(intent) => KernelOutcome::Evaluated(evaluate(world, &intent, ctx)),
        Err(BuildError::UnknownToOntology { action }) => {
            KernelOutcome::UnknownToOntology { action }
        }
        Err(error) => {
            let (decision, rule) = classify(&error);
            KernelOutcome::NotRepresentable {
                decision,
                rule: rule.to_string(),
                error,
            }
        }
    }
}

/// Map a representability failure to a present-action `Decision` per the §7
/// "Default action outcomes" table. Note the nuance: a capability gap reads as
/// `ABSENT`, not `DENY`.
fn classify(error: &BuildError) -> (Decision, &'static str) {
    match error {
        BuildError::Absent { .. } => (Decision::Absent, "absent"),
        BuildError::CapabilityViolation { .. } => (Decision::Absent, "capability"),
        BuildError::SchemaViolation { .. } => (Decision::Deny, "schema_violation"),
        BuildError::DescriptorDrift { .. } => (Decision::Deny, "descriptor_drift"),
        BuildError::TaintViolation { .. } => (Decision::Deny, "taint_invariant"),
        BuildError::InvariantViolation { .. } => (Decision::Deny, "invariant_violation"),
        BuildError::ApprovalRequired { .. } => (Decision::Ask, "approval_required"),
        BuildError::BudgetExceeded { .. } => (Decision::Replan, "budget_exceeded"),
        // `decide` peels this off before calling `classify`.
        BuildError::UnknownToOntology { .. } => (Decision::Absent, "unknown_to_ontology"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use compiler::compile_default;
    use harness_types::*;
    use serde_json::{json, Value};

    // --- Hand-built minimal worlds (fast, no compiler) -------------------------

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
        // Grant the trusted actor the Read capability so the capability check
        // (added in E2.1) passes for these representability tests.
        parts
            .capability_matrix
            .entry(TrustLevel::Trusted)
            .or_default()
            .insert(ActionType::Read);
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

    // --- decide() facade over hand-built worlds --------------------------------

    #[test]
    fn decide_unknown_is_distinct_from_absent() {
        // Invariant 3: unknown-to-ontology and absence are different outcomes.
        let world = world_with("read_workspace", true);
        let unknown = decide(
            &world,
            &call("send_email"),
            prov(),
            &EvalContext::interactive_clean(),
        );
        assert!(matches!(unknown, KernelOutcome::UnknownToOntology { .. }));

        let absent_world = world_with("read_workspace", false);
        let absent = decide(
            &absent_world,
            &call("read_workspace"),
            prov(),
            &EvalContext::interactive_clean(),
        );
        assert!(matches!(
            absent,
            KernelOutcome::NotRepresentable {
                decision: Decision::Absent,
                ..
            }
        ));
        assert_ne!(
            std::mem::discriminant(&unknown),
            std::mem::discriminant(&absent)
        );
    }

    // --- decide() facade over the real default world ---------------------------

    fn default_call(action: &str, args: Value) -> ToolCall {
        ToolCall {
            action_name: ActionName::new(action),
            arguments: args,
            provider: Provider::CliNative,
            call_id: CallId::new("c1"),
            source_perceptions: vec![],
            session_id: SessionId::new("s1"),
        }
    }

    fn from_channel(ch: SourceChannel) -> Provenance {
        Provenance::from_channel(ch, SessionId::new("s1"), ContentHash::new("h"))
    }

    #[test]
    fn clean_read_is_allowed_and_executes() {
        let world = compile_default();
        let outcome = decide(
            &world,
            &default_call("read_workspace", json!({})),
            from_channel(SourceChannel::UserPrompt),
            &EvalContext::interactive_clean(),
        );
        match outcome {
            KernelOutcome::Evaluated(d) => {
                assert_eq!(d.decision, Decision::Allow);
                assert_eq!(d.effect_mode, Some(EffectMode::Execute));
            }
            other => panic!("expected Allow + Execute, got {other:?}"),
        }
    }

    #[test]
    fn capability_gap_reads_as_absent() {
        // An untrusted (web) actor may only Read; proposing a Write is ABSENT.
        let world = compile_default();
        let outcome = decide(
            &world,
            &default_call("write_workspace", json!({})),
            from_channel(SourceChannel::Web),
            &EvalContext::interactive_clean(),
        );
        assert_eq!(outcome.decision(), Decision::Absent);
        assert!(matches!(
            outcome,
            KernelOutcome::NotRepresentable {
                error: BuildError::CapabilityViolation { .. },
                ..
            }
        ));
    }

    #[test]
    fn tainted_value_cannot_reach_network() {
        // Invariant 7 floor: tainted context into an externally-effectful action
        // is not representable.
        let world = compile_default();
        for action in ["fetch_web", "call_mcp_tool", "update_memory"] {
            let outcome = decide(
                &world,
                &default_call(action, json!({})),
                from_channel(SourceChannel::UserPrompt),
                &EvalContext::interactive_clean()
                    .with_taint(TaintContext::from_taint(Taint::Tainted)),
            );
            assert_eq!(outcome.decision(), Decision::Deny, "{action} should DENY");
            assert!(
                matches!(
                    outcome,
                    KernelOutcome::NotRepresentable {
                        error: BuildError::TaintViolation { .. },
                        ..
                    }
                ),
                "{action} should be a taint violation"
            );
        }
    }

    #[test]
    fn approval_required_action_asks() {
        // start_pty is flagged approval_required in the default world.
        let world = compile_default();
        let outcome = decide(
            &world,
            &default_call("start_pty", json!({})),
            from_channel(SourceChannel::UserPrompt),
            &EvalContext::interactive_clean(),
        );
        assert_eq!(outcome.decision(), Decision::Ask);
    }

    #[test]
    fn over_command_budget_replans() {
        let world = compile_default();
        let max = world
            .budget()
            .max_commands_per_task
            .expect("default world sets a command budget");
        let ctx = EvalContext::interactive_clean().with_usage(BudgetUsage {
            commands_run: max,
            ..Default::default()
        });
        let outcome = decide(
            &world,
            &default_call("run_command", json!({})),
            from_channel(SourceChannel::UserPrompt),
            &ctx,
        );
        assert_eq!(outcome.decision(), Decision::Replan);
    }

    #[test]
    fn tainted_taint_survives_across_sessions() {
        // Invariant 6: a tainted context from another session is never lowered.
        let world = compile_default();
        let prior_session = Provenance::from_channel(
            SourceChannel::WorkspaceFile,
            SessionId::new("earlier-session"),
            ContentHash::new("h"),
        );
        let builder = IRBuilder::new(&world);
        let ir = builder
            .build(
                &default_call("read_workspace", json!({})),
                prior_session,
                &TaintContext::from_taint(Taint::Tainted),
            )
            .expect("read is representable even when tainted");
        assert_eq!(ir.taint(), Taint::Tainted);
    }

    #[test]
    fn decide_is_deterministic_over_matrix() {
        // Invariant 6 / §6 stability: same inputs ⇒ identical outcome.
        let world = compile_default();
        let actions = [
            "read_workspace",
            "write_workspace",
            "run_command",
            "fetch_web",
            "start_pty",
            "call_mcp_tool",
            "update_memory",
            "send_email", // unknown to ontology
        ];
        let channels = [
            SourceChannel::UserPrompt,
            SourceChannel::Web,
            SourceChannel::WorkspaceFile,
        ];
        let taints = [Taint::Clean, Taint::Tainted];
        let usages = [
            BudgetUsage::default(),
            BudgetUsage {
                commands_run: 9_999,
                network_calls: 9_999,
                file_writes: 9_999,
                tokens_used: 0,
            },
        ];

        for action in actions {
            for ch in channels {
                for taint in taints {
                    for usage in usages {
                        let ctx = EvalContext {
                            taint: TaintContext::from_taint(taint),
                            mode: ExecutionMode::Interactive,
                            usage,
                        };
                        let tc = default_call(action, json!({}));
                        let first = decide(&world, &tc, from_channel(ch), &ctx);
                        let second = decide(&world, &tc, from_channel(ch), &ctx);
                        assert_eq!(
                            first, second,
                            "decide must be deterministic for {action}/{ch:?}/{taint:?}"
                        );
                    }
                }
            }
        }
    }
}
