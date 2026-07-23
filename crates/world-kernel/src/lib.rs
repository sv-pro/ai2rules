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
pub mod spec;
pub mod taint;

pub use disposition::{evaluate, BudgetUsage, EvalContext};
pub use intent::{IRBuilder, IntentIR};
pub use spec::{build_execution_spec, ExecEnv, SpecError};

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
    /// The intent was built and evaluated. Carries the sealed `IntentIR` (so an
    /// `ALLOW` can be lowered to an `ExecutionSpec` via [`build_execution_spec`])
    /// alongside the disposition.
    Evaluated {
        intent: IntentIR,
        disposition: Disposition,
    },
}

impl KernelOutcome {
    /// The effective policy decision. `UnknownToOntology` collapses to `Absent`
    /// for rendering; the variant itself stays distinct for invariant 3.
    pub fn decision(&self) -> Decision {
        match self {
            KernelOutcome::UnknownToOntology { .. } => Decision::Absent,
            KernelOutcome::NotRepresentable { decision, .. } => *decision,
            KernelOutcome::Evaluated { disposition, .. } => disposition.decision,
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
        Ok(intent) => {
            let disposition = evaluate(world, &intent, ctx);
            KernelOutcome::Evaluated {
                intent,
                disposition,
            }
        }
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

    fn valid_default_args(action: &str) -> Value {
        match action {
            "read_workspace" => json!({ "path": "Cargo.toml" }),
            "write_workspace" => json!({ "path": "out.txt", "content": "x" }),
            "apply_patch" => json!({ "path": "out.txt", "contents": "x" }),
            "run_command" => json!({ "command": "echo ok" }),
            "fetch_web" => json!({ "url": "https://docs.example/guide" }),
            "call_mcp_tool" => json!({ "query": "guide" }),
            "update_memory" => json!({ "key": "k", "value": "v" }),
            _ => json!({}),
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
            &default_call("read_workspace", valid_default_args("read_workspace")),
            from_channel(SourceChannel::UserPrompt),
            &EvalContext::interactive_clean(),
        );
        match outcome {
            KernelOutcome::Evaluated { disposition, .. } => {
                assert_eq!(disposition.decision, Decision::Allow);
                assert_eq!(disposition.effect_mode, Some(EffectMode::Execute));
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
                &default_call(action, valid_default_args(action)),
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
    fn approval_required_in_background_denies() {
        // Invariant 10: nobody to ask in BACKGROUND → fail closed.
        let world = compile_default();
        let ctx = EvalContext {
            taint: TaintContext::clean(),
            mode: ExecutionMode::Background,
            usage: BudgetUsage::default(),
            approval_granted: false,
        };
        let outcome = decide(
            &world,
            &default_call("start_pty", json!({})),
            from_channel(SourceChannel::UserPrompt),
            &ctx,
        );
        assert_eq!(outcome.decision(), Decision::Deny);
    }

    #[test]
    fn granted_approval_allows() {
        // Invariant 9 (resume): a prior approval lets the action through.
        let world = compile_default();
        let ctx = EvalContext::interactive_clean().with_approval(true);
        let outcome = decide(
            &world,
            &default_call("start_pty", json!({})),
            from_channel(SourceChannel::UserPrompt),
            &ctx,
        );
        assert_eq!(outcome.decision(), Decision::Allow);
    }

    // --- PACT L2 end-to-end through decide() over the real default world -------

    /// `fetch_web` declares `url: Target` in the default world, so a fetch whose
    /// *url* is clean is allowed even when the ambient session is tainted — the
    /// Theorem-3 false positive (poisoned_knowledge_demo session-2/step-3),
    /// recovered by the wired kernel.
    #[test]
    fn l2_recovers_clean_url_in_tainted_session() {
        let world = compile_default();
        let ctx = EvalContext {
            taint: TaintContext::from_taint(Taint::Tainted)
                .with_arg_taint([("url".to_string(), Taint::Clean)]),
            mode: ExecutionMode::Interactive,
            usage: BudgetUsage::default(),
            approval_granted: false,
        };
        let outcome = decide(
            &world,
            &default_call("fetch_web", json!({ "url": "https://docs.example/guide" })),
            from_channel(SourceChannel::UserPrompt),
            &ctx,
        );
        assert_eq!(outcome.decision(), Decision::Allow);
    }

    /// The security property is preserved: when the url itself is tainted-derived,
    /// the same action is still denied. No false negative.
    #[test]
    fn l2_still_blocks_tainted_url() {
        let world = compile_default();
        let ctx = EvalContext {
            taint: TaintContext::from_taint(Taint::Tainted)
                .with_arg_taint([("url".to_string(), Taint::Tainted)]),
            mode: ExecutionMode::Interactive,
            usage: BudgetUsage::default(),
            approval_granted: false,
        };
        let outcome = decide(
            &world,
            &default_call(
                "fetch_web",
                json!({ "url": "http://attacker.evil/collect" }),
            ),
            from_channel(SourceChannel::UserPrompt),
            &ctx,
        );
        assert_eq!(outcome.decision(), Decision::Deny);
    }

    /// An externally-effectful action with *no* roles (e.g. `update_memory`) keeps
    /// the conservative ambient floor — L2 changes nothing it was not opted into.
    #[test]
    fn action_without_roles_keeps_ambient_floor() {
        let world = compile_default();
        let ctx = EvalContext {
            taint: TaintContext::from_taint(Taint::Tainted)
                .with_arg_taint([("value".to_string(), Taint::Clean)]),
            mode: ExecutionMode::Interactive,
            usage: BudgetUsage::default(),
            approval_granted: false,
        };
        let outcome = decide(
            &world,
            &default_call("update_memory", json!({ "key": "k", "value": "v" })),
            from_channel(SourceChannel::UserPrompt),
            &ctx,
        );
        // No arg_roles on update_memory → ambient Tainted floor → denied, as before.
        assert_eq!(outcome.decision(), Decision::Deny);
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
            &default_call("run_command", valid_default_args("run_command")),
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
                &default_call("read_workspace", valid_default_args("read_workspace")),
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
                            approval_granted: false,
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

    // --- End-to-end: decide -> build_execution_spec -> executor.run ------------

    /// An executor wired with the default world's local handlers and their
    /// descriptor hashes, so specs built from the same world never drift.
    fn executor_for(world: &CompiledWorld) -> executor::Executor {
        use executor::{CommandHandler, Executor, PatchHandler, ReadHandler};
        let hash = |a: &str| {
            world
                .descriptor_hash(&ActionName::new(a))
                .cloned()
                .unwrap_or_default()
        };
        Executor::builder()
            .register(
                ActionName::new("read_workspace"),
                hash("read_workspace"),
                Box::new(ReadHandler),
            )
            .register(
                ActionName::new("apply_patch"),
                hash("apply_patch"),
                Box::new(PatchHandler),
            )
            .register(
                ActionName::new("run_command"),
                hash("run_command"),
                Box::new(CommandHandler),
            )
            .build()
    }

    #[test]
    fn end_to_end_execute_reads_a_real_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("hello.txt");
        std::fs::write(&file, "round-trip").unwrap();

        let world = compile_default();
        let call = default_call("read_workspace", json!({ "path": file.to_str().unwrap() }));
        let outcome = decide(
            &world,
            &call,
            from_channel(SourceChannel::UserPrompt),
            &EvalContext::interactive_clean(),
        );
        let KernelOutcome::Evaluated {
            intent,
            disposition,
        } = outcome
        else {
            panic!("expected Evaluated");
        };
        assert_eq!(disposition.decision, Decision::Allow);

        let env = ExecEnv {
            readable_roots: vec![dir.path().to_path_buf()],
            ..Default::default()
        };
        let spec = build_execution_spec(
            &world,
            &intent,
            EffectMode::Execute,
            &env,
            TraceId::new("rt"),
        )
        .expect("spec");
        let result = executor_for(&world).run(&spec).expect("execute");
        assert_eq!(
            result.value,
            executor::ExecOutput::FileContents("round-trip".to_string())
        );
    }

    #[test]
    fn end_to_end_simulate_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("out.txt");

        let world = compile_default();
        let call = default_call(
            "apply_patch",
            json!({ "path": target.to_str().unwrap(), "contents": "x" }),
        );
        let outcome = decide(
            &world,
            &call,
            from_channel(SourceChannel::UserPrompt),
            &EvalContext::interactive_clean(),
        );
        let KernelOutcome::Evaluated {
            intent,
            disposition,
        } = outcome
        else {
            panic!("expected Evaluated");
        };
        assert_eq!(disposition.decision, Decision::Allow);

        let env = ExecEnv {
            writable_roots: vec![dir.path().to_path_buf()],
            ..Default::default()
        };
        let spec = build_execution_spec(
            &world,
            &intent,
            EffectMode::Simulate,
            &env,
            TraceId::new("rt"),
        )
        .expect("spec");
        let result = executor_for(&world).run(&spec).expect("simulate");
        assert!(matches!(result.value, executor::ExecOutput::Simulated(_)));
        assert!(!target.exists(), "simulate must not write to disk");
    }
}
