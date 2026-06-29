//! The model loop (E5.5) with approvals (E6): propose → adapt → decide →
//! (approve?) → execute → perceive.
//!
//! The model proposes an Anthropic `tool_use` block; the adapter normalizes it;
//! the kernel decides under the session's `ExecutionMode`. An `ALLOW` runs; an
//! `ASK` (interactive only — `BACKGROUND` already collapses to `DENY`) mints a
//! durable approval token, resolves it via the session's `ApprovalPolicy`, and on
//! approval **re-decides** with the grant → `ALLOW` → execute. Every decision is
//! recorded to the trace. The loop never builds an `ExecutionSpec` from anything
//! but a sealed, allowed intent (invariant 4).

use executor::{ExecOutput, Executor};
use harness_types::{
    ApprovalToken, ApprovalTokenId, CompiledWorld, ContentHash, Decision, EffectMode,
    ExecutionMode, PayloadRef, Perception, PerceptionId, PerceptionKind, Provenance,
    RedactionPolicy, SessionId, SourceChannel, Taint, TaintContext, TraceId, TrustLevel,
};
use provider_adapters::{anthropic, ToolOutcome};
use trace_store::{params_hash, record_decision, ApprovalStore, TraceStore};
use world_kernel::{
    build_execution_spec, decide, BudgetUsage, EvalContext, ExecEnv, IntentIR, KernelOutcome,
};

use crate::context;
use crate::model::{ModelClient, ModelTurn};

/// How an interactive `ASK` is resolved when the loop has no live human.
//
// No `PartialEq`/`Eq`: the `Interactive` variant holds a `fn` pointer, whose
// address is not guaranteed unique (clippy::unpredictable_function_pointer_comparisons),
// and nothing compares policies for equality anyway.
#[derive(Debug, Clone, Copy)]
pub enum ApprovalPolicy {
    /// Leave the token pending and surface `ASK` (a real CLI would block here).
    Manual,
    AutoApprove,
    AutoReject,
    /// Pause and ask the human via a callback.
    Interactive(
        fn(
            &harness_types::ToolCall,
            &harness_types::CompiledWorld,
            &harness_types::Provenance,
        ) -> bool,
    ),
}

/// One recorded step of the session, for display/inspection.
#[derive(Debug, Clone)]
pub struct TranscriptEntry {
    pub action: String,
    pub verdict: String,
    pub decision: Option<Decision>,
    pub rule: Option<String>,
    pub effect_mode: Option<EffectMode>,
    pub result: String,
    pub taint: Taint,
}

#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Effect mode for allowed actions — `Simulate` for safe demos.
    pub effect_mode: EffectMode,
    /// Whether a human is available to approve (`ASK` fails closed in background).
    pub mode: ExecutionMode,
    /// How an interactive `ASK` is resolved.
    pub approval: ApprovalPolicy,
    pub max_steps: u64,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            effect_mode: EffectMode::Simulate,
            mode: ExecutionMode::Interactive,
            approval: ApprovalPolicy::Manual,
            max_steps: 16,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionOutcome {
    pub transcript: Vec<TranscriptEntry>,
    pub final_text: Option<String>,
    /// Number of decision records appended to the trace.
    pub records: usize,
}

/// Drive a model through the projected surface until it answers or runs out of
/// steps. Pure of policy — every verdict comes from the kernel; approvals are
/// durable state in `store`.
#[allow(clippy::too_many_arguments)]
pub fn run(
    world: &CompiledWorld,
    env: &ExecEnv,
    executor: &Executor,
    trace: &TraceStore,
    store: &mut ApprovalStore,
    model: &mut dyn ModelClient,
    config: &SessionConfig,
    mut observer: Option<&mut dyn FnMut(&TranscriptEntry)>,
) -> SessionOutcome {
    let session = SessionId::new("agent-session");
    // The agent proposes with the developer's (trusted) authority; containment
    // comes from taint, which accumulates from perceived results.
    let provenance = Provenance::from_channel(
        SourceChannel::UserPrompt,
        session.clone(),
        ContentHash::new("agent"),
    );

    let mut transcript = Vec::new();
    let mut perceptions: Vec<Perception> = Vec::new();
    let mut taint = Taint::Clean;
    let mut records = 0usize;

    for _ in 0..config.max_steps {
        let ctx = context::pack(world, perceptions.clone());
        let block = match model.next(&ctx) {
            ModelTurn::Final(text) => {
                return SessionOutcome {
                    transcript,
                    final_text: Some(text),
                    records,
                }
            }
            ModelTurn::ToolUse(block) => block,
        };

        let call = match anthropic::tool_use_to_call(&block, session.clone()) {
            Ok(call) => call,
            Err(e) => {
                transcript.push(TranscriptEntry {
                    action: "<malformed>".to_string(),
                    verdict: format!("adapter error: {e}"),
                    decision: None,
                    rule: None,
                    effect_mode: None,
                    result: String::new(),
                    taint: Taint::Clean,
                });
                continue;
            }
        };

        let action = call.action_name.as_str().to_string();
        let base = EvalContext {
            taint: TaintContext::from_taint(taint),
            mode: config.mode,
            usage: BudgetUsage::default(),
            approval_granted: false,
        };
        let outcome = decide(world, &call, provenance.clone(), &base);
        append(
            trace,
            world,
            &call,
            &provenance,
            &base,
            &outcome,
            &mut records,
        );

        let entry = match outcome {
            // Allowed outright → execute.
            KernelOutcome::Evaluated {
                intent,
                disposition,
            } if disposition.decision == Decision::Allow => run_allowed(
                world,
                env,
                executor,
                config.effect_mode,
                &intent,
                action,
                &mut perceptions,
                &mut taint,
                &session,
                "ALLOW",
                Decision::Allow,
                disposition.rule,
            ),

            // Needs approval (interactive) → mint, resolve, maybe resume.
            KernelOutcome::Evaluated { disposition, .. }
                if disposition.decision == Decision::Ask =>
            {
                resolve_approval(
                    world,
                    env,
                    executor,
                    trace,
                    store,
                    config,
                    &call,
                    &provenance,
                    base,
                    action,
                    &mut perceptions,
                    &mut taint,
                    &session,
                    &mut records,
                )
            }

            // Everything else is feedback the model would receive as an error.
            other => {
                let verdict = verdict_label(&other);
                let (decision, rule, effect_mode) = verdict_metadata(&other);
                let _feedback = anthropic::format_tool_result(&ToolOutcome {
                    call_id: call.call_id.clone(),
                    content: verdict.clone(),
                    is_error: true,
                });
                TranscriptEntry {
                    action,
                    verdict,
                    decision: Some(decision),
                    rule: Some(rule),
                    effect_mode,
                    result: "(not executed)".to_string(),
                    taint: Taint::Clean,
                }
            }
        };
        if let Some(ref mut obs) = observer {
            obs(&entry);
        }
        transcript.push(entry);
    }

    SessionOutcome {
        transcript,
        final_text: None,
        records,
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_approval(
    world: &CompiledWorld,
    env: &ExecEnv,
    executor: &Executor,
    trace: &TraceStore,
    store: &mut ApprovalStore,
    config: &SessionConfig,
    call: &harness_types::ToolCall,
    provenance: &Provenance,
    base: EvalContext,
    action: String,
    perceptions: &mut Vec<Perception>,
    taint: &mut Taint,
    session: &SessionId,
    records: &mut usize,
) -> TranscriptEntry {
    let descriptor_hash = world
        .descriptor_hash(&call.action_name)
        .cloned()
        .unwrap_or_default();
    let token = ApprovalToken::pending(
        ApprovalTokenId::new(format!("appr-{records}")),
        call.action_name.clone(),
        params_hash(&call.arguments),
        world.world_id().clone(),
        descriptor_hash.clone(),
        provenance.clone(),
        config.effect_mode,
    );
    let id = match store.mint(token) {
        Ok(id) => id,
        Err(e) => {
            return entry_with(
                action,
                format!("ASK (store error: {e})"),
                Decision::Ask,
                "approval_store_error",
                None,
            );
        }
    };

    match config.approval {
        ApprovalPolicy::Manual => entry_with(
            action,
            "ASK (pending approval)".to_string(),
            Decision::Ask,
            "approval_required",
            None,
        ),
        ApprovalPolicy::AutoReject => {
            let _ = store.reject(&id);
            entry_with(
                action,
                "ASK → REJECTED".to_string(),
                Decision::Ask,
                "approval_rejected",
                None,
            )
        }
        ApprovalPolicy::AutoApprove | ApprovalPolicy::Interactive(_) => {
            let approved = match config.approval {
                ApprovalPolicy::Interactive(cb) => cb(call, world, provenance),
                _ => true,
            };

            if !approved {
                let _ = store.reject(&id);
                return entry_with(
                    action,
                    "ASK → REJECTED (interactive)".to_string(),
                    Decision::Ask,
                    "approval_rejected",
                    None,
                );
            }

            let _ = store.approve(&id);
            let granted = store.is_granted(
                &call.action_name,
                &call.arguments,
                world.world_id(),
                &descriptor_hash,
                provenance,
                config.effect_mode,
            );
            let resumed_ctx = base.with_approval(granted);
            let resumed = decide(world, call, provenance.clone(), &resumed_ctx);
            append(
                trace,
                world,
                call,
                provenance,
                &resumed_ctx,
                &resumed,
                records,
            );

            match resumed {
                KernelOutcome::Evaluated {
                    intent,
                    disposition,
                } if disposition.decision == Decision::Allow => {
                    let mut e = run_allowed(
                        world,
                        env,
                        executor,
                        config.effect_mode,
                        &intent,
                        action,
                        perceptions,
                        taint,
                        session,
                        "ASK → APPROVED → ALLOW",
                        Decision::Allow,
                        disposition.rule,
                    );
                    let _ = store.mark_executed(&id);
                    e.verdict = "ASK → APPROVED → ALLOW".to_string();
                    e
                }
                other => {
                    let (decision, rule, effect_mode) = verdict_metadata(&other);
                    entry_with(
                        action,
                        format!("ASK → APPROVED but {}", verdict_label(&other)),
                        decision,
                        rule,
                        effect_mode,
                    )
                }
            }
        }
    }
}

fn append(
    trace: &TraceStore,
    world: &CompiledWorld,
    call: &harness_types::ToolCall,
    provenance: &Provenance,
    ctx: &EvalContext,
    outcome: &KernelOutcome,
    records: &mut usize,
) {
    let record = record_decision(
        world,
        TraceId::new("agent"),
        *records as u64,
        call,
        provenance,
        ctx,
        outcome,
    );
    let _ = trace.append(&record);
    *records += 1;
}

#[allow(clippy::too_many_arguments)]
fn run_allowed(
    world: &CompiledWorld,
    env: &ExecEnv,
    executor: &Executor,
    effect_mode: EffectMode,
    intent: &IntentIR,
    action: String,
    perceptions: &mut Vec<Perception>,
    taint: &mut Taint,
    session: &SessionId,
    verdict: &str,
    decision: Decision,
    rule: String,
) -> TranscriptEntry {
    match build_execution_spec(world, intent, effect_mode, env, TraceId::new("agent")) {
        Ok(spec) => match executor.run(&spec) {
            Ok(tv) => {
                let content = describe_output(&tv.value);
                *taint = taint.join(tv.taint);
                perceptions.push(result_perception(&action, &content, tv.taint, session));
                TranscriptEntry {
                    action,
                    verdict: verdict.to_string(),
                    decision: Some(decision),
                    rule: Some(rule),
                    effect_mode: Some(effect_mode),
                    result: content,
                    taint: tv.taint,
                }
            }
            Err(e) => entry_with(
                action,
                format!("{verdict}: exec error: {e}"),
                decision,
                "executor_error",
                Some(effect_mode),
            ),
        },
        Err(e) => entry_with(
            action,
            format!("{verdict}: spec error: {e}"),
            decision,
            "spec_error",
            Some(effect_mode),
        ),
    }
}

fn entry_with(
    action: String,
    verdict: String,
    decision: Decision,
    rule: impl Into<String>,
    effect_mode: Option<EffectMode>,
) -> TranscriptEntry {
    TranscriptEntry {
        action,
        verdict,
        decision: Some(decision),
        rule: Some(rule.into()),
        effect_mode,
        result: "(not executed)".to_string(),
        taint: Taint::Clean,
    }
}

fn describe_output(output: &ExecOutput) -> String {
    match output {
        ExecOutput::Simulated(note) => format!("(simulated) {note}"),
        ExecOutput::FileContents(c) => format!("read {} bytes", c.len()),
        ExecOutput::CommandResult { exit_code, .. } => format!("exit {exit_code}"),
        ExecOutput::PatchApplied { path } => format!("wrote {}", path.display()),
        ExecOutput::External { source, content } => format!("{source}: {} bytes", content.len()),
    }
}

fn verdict_label(outcome: &KernelOutcome) -> String {
    match outcome {
        KernelOutcome::UnknownToOntology { action } => {
            format!("UNKNOWN_TO_ONTOLOGY ({})", action.as_str())
        }
        KernelOutcome::NotRepresentable { decision, rule, .. } => format!("{decision:?} ({rule})"),
        KernelOutcome::Evaluated { disposition, .. } => {
            format!("{:?} ({})", disposition.decision, disposition.rule)
        }
    }
}

fn verdict_metadata(outcome: &KernelOutcome) -> (Decision, String, Option<EffectMode>) {
    match outcome {
        KernelOutcome::UnknownToOntology { .. } => {
            (Decision::Absent, "unknown_to_ontology".to_string(), None)
        }
        KernelOutcome::NotRepresentable { decision, rule, .. } => (*decision, rule.clone(), None),
        KernelOutcome::Evaluated { disposition, .. } => (
            disposition.decision,
            disposition.rule.clone(),
            disposition.effect_mode,
        ),
    }
}

/// Build a tainted result perception so execution output flows back into context
/// under its taint — keeping the loop honest about provenance (invariant 6/7).
fn result_perception(action: &str, content: &str, taint: Taint, session: &SessionId) -> Perception {
    Perception {
        id: PerceptionId::new(format!("result-{action}")),
        kind: PerceptionKind::CommandStdout,
        source_channel: SourceChannel::ShellOutput,
        trust_level: TrustLevel::SemiTrusted,
        taint,
        content_hash: ContentHash::new("result"),
        provenance: Provenance::from_channel(
            SourceChannel::ShellOutput,
            session.clone(),
            ContentHash::new("result"),
        ),
        payload_ref: PayloadRef(content.to_string()),
        redaction_policy: RedactionPolicy::None,
    }
}
