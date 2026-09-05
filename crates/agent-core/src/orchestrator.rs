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
    ActionName, ActionType, ApprovalToken, ApprovalTokenId, CompiledWorld, ContentHash, Decision,
    EffectMode, ExecutionMode, PayloadRef, Perception, PerceptionId, PerceptionKind, PrincipalId,
    Provenance, RedactionPolicy, SessionId, SideEffectClass, SourceChannel, Taint, TaintContext,
    TraceId, TrustLevel,
};
use provider_adapters::{anthropic, ToolOutcome};
use serde_json::json;
use trace_store::{
    effect_binding, effect_resource, record_decision, ApprovalStore, ConsumeOutcome, TraceStore,
};
use world_kernel::{
    build_execution_spec, charge, decide, BudgetUsage, EvalContext, ExecEnv, IntentIR,
    KernelOutcome,
};

use crate::arg_provenance;
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
    /// Principal authenticated by the trusted host, never inferred from model data.
    pub principal: PrincipalId,
    /// Host-assigned session identity, also bound through provenance.
    pub session_id: SessionId,
    /// Effect mode for allowed actions — `Simulate` for safe demos.
    pub effect_mode: EffectMode,
    /// Whether a human is available to approve (`ASK` fails closed in background).
    pub mode: ExecutionMode,
    /// How an interactive `ASK` is resolved.
    pub approval: ApprovalPolicy,
    /// Lifetime of one exact, single-use authorization instance.
    pub approval_ttl_ms: u64,
    pub max_steps: u64,
    /// The trusted user request seeding the clean provenance corpus (PACT L2).
    /// A value that appears verbatim here is provably clean-origin, so an
    /// authority-bearing argument the user themselves supplied is not blocked by
    /// ambient session taint. `None` → no clean seed → the per-argument producer
    /// only ever falls back to the ambient floor (fully conservative).
    pub user_request: Option<String>,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            principal: PrincipalId::new("user:local"),
            session_id: SessionId::new("agent-session"),
            effect_mode: EffectMode::Simulate,
            mode: ExecutionMode::Interactive,
            approval: ApprovalPolicy::Manual,
            approval_ttl_ms: 5 * 60 * 1_000,
            max_steps: 16,
            user_request: None,
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
    let session = config.session_id.clone();
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
    // Budget counters for the session (finding #16). The orchestrator owns a live
    // session, so unlike the pure gate it can hold these directly — but it was
    // passing a zeroed `BudgetUsage` into every decision, so no manifest limit was
    // ever reachable. Charged only on an allowed call, via the kernel's `charge`
    // so the cost and the limit cannot drift apart.
    let mut usage = BudgetUsage::default();
    let mut records = 0usize;
    // A `ModelClient` is stateful and the current model contract permits one
    // tool call per turn, so the provider feedback path is a single consumed
    // result slot rather than a second conversation history.
    let mut pending_outcome: Option<ToolOutcome> = None;
    // The trusted user request seeds the clean provenance corpus for the L2
    // producer. It is the one source we can assert is clean-origin by construction
    // (a Trusted, untainted channel — the developer's own words).
    let clean_sources: Vec<String> = config.user_request.iter().cloned().collect();

    for _ in 0..config.max_steps {
        let previous_outcome = pending_outcome.take();
        let ctx = context::pack(world, perceptions.clone(), previous_outcome.as_ref());
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

        let mut call = match anthropic::tool_use_to_call(&block, session.clone()) {
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

        // Normalize shell-shaped actions through the compiled world's classifier
        // before both decision and authorization. This is the same kernel-owned
        // classification used by the Gate ABI; the provider adapter stays free of
        // policy and a destructive command cannot retain the base action's identity.
        call.action_name = world.classify_command(&call.action_name, &call.arguments);

        let action = call.action_name.as_str().to_string();
        // PACT L2 producer: derive per-argument taint from the actual data flow —
        // which tainted output (or clean source) each argument's value came from —
        // so the kernel can judge authority-bearing arguments individually instead
        // of blocking on ambient session taint. Deterministic, no LLM.
        let arg_taint = arg_provenance::arg_taint(
            &call.arguments,
            &tainted_payloads(&perceptions),
            &clean_sources,
        );
        let base = EvalContext {
            taint: TaintContext::from_taint(taint).with_arg_taint(arg_taint),
            mode: config.mode,
            usage,
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
                &mut usage,
                &session,
                "ALLOW",
                Decision::Allow,
                disposition.rule,
            ),

            // Needs approval (interactive) → mint, resolve, maybe resume.
            KernelOutcome::Evaluated {
                intent,
                disposition,
            } if disposition.decision == Decision::Ask => resolve_approval(
                world,
                env,
                executor,
                trace,
                store,
                config,
                &call,
                intent.action(),
                &provenance,
                base,
                action,
                &mut perceptions,
                &mut taint,
                &mut usage,
                &session,
                &mut records,
            ),

            // Everything else is feedback the model would receive as an error.
            other => {
                let verdict = verdict_label(&other);
                let (decision, rule, effect_mode) = verdict_metadata(&other);
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
        pending_outcome = Some(ToolOutcome {
            call_id: call.call_id.clone(),
            content: model_feedback(&entry),
            is_error: entry.decision != Some(Decision::Allow) || entry.result == "(not executed)",
        });
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
    effective_action: &ActionName,
    provenance: &Provenance,
    base: EvalContext,
    action: String,
    perceptions: &mut Vec<Perception>,
    taint: &mut Taint,
    usage: &mut BudgetUsage,
    session: &SessionId,
    records: &mut usize,
) -> TranscriptEntry {
    let descriptor_hash = world
        .descriptor_hash(effective_action)
        .cloned()
        .unwrap_or_default();
    let binding = effect_binding(
        config.principal.clone(),
        effective_action.clone(),
        &call.arguments,
        effect_resource(&call.arguments),
        world.world_id().clone(),
        world.manifest_hash().clone(),
        descriptor_hash,
        provenance.clone(),
        config.effect_mode,
    );
    let now = now_unix_ms();
    let token = ApprovalToken::pending(
        ApprovalTokenId::new(format!(
            "auth-{now}-{}-{}",
            *records,
            &binding.canonical_effect_hash.as_str()[..12]
        )),
        binding.principal.clone(),
        binding.action.clone(),
        binding.params_hash.clone(),
        binding.canonical_effect_hash.clone(),
        binding.resource.clone(),
        binding.world_id.clone(),
        binding.manifest_hash.clone(),
        binding.descriptor_hash.clone(),
        binding.provenance.clone(),
        binding.effect_mode,
        now.saturating_add(config.approval_ttl_ms.max(1)),
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

            if let Err(e) = store.approve(&id) {
                return entry_with(
                    action,
                    format!("ASK (approval store error: {e})"),
                    Decision::Ask,
                    "authorization_store_error",
                    None,
                );
            }
            // Claim before the external effect. A crash after this point burns the
            // grant (at-most-once/fail-closed) rather than permitting replay.
            let consumed = match store.consume(&id, &binding, now_unix_ms()) {
                Ok(ConsumeOutcome::Consumed(_)) => true,
                Ok(ConsumeOutcome::Rejected(reason)) => {
                    return entry_with(
                        action,
                        format!("ASK (authorization rejected: {reason:?})"),
                        Decision::Ask,
                        "authorization_rejected",
                        None,
                    );
                }
                Err(e) => {
                    return entry_with(
                        action,
                        format!("ASK (authorization store error: {e})"),
                        Decision::Ask,
                        "authorization_store_error",
                        None,
                    );
                }
            };
            let resumed_ctx = base.with_approval(consumed);
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
                        usage,
                        session,
                        "ASK → APPROVED → ALLOW",
                        Decision::Allow,
                        disposition.rule,
                    );
                    if e.result != "(not executed)" {
                        let _ = store.mark_executed(&id);
                    }
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

fn now_unix_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

/// The payloads of every tainted perception so far — the tainted corpus the L2
/// producer matches argument values against.
fn tainted_payloads(perceptions: &[Perception]) -> Vec<String> {
    perceptions
        .iter()
        .filter(|p| p.taint.is_tainted())
        .map(|p| p.payload_ref.0.clone())
        .collect()
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
    usage: &mut BudgetUsage,
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
                // Charge the budget for a call that actually ran, beside the taint
                // join for the same reason: this is where session state advances.
                *usage = charge(
                    usage,
                    world
                        .action_type(intent.action())
                        .unwrap_or(ActionType::Read),
                    world
                        .side_effect(intent.action())
                        .unwrap_or(SideEffectClass::Read),
                );
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

/// Produce stable, structured model feedback while keeping the provider adapter
/// free of kernel types and policy. The outer provider block preserves the call
/// identity; this content preserves the canonical decision and its evidence.
fn model_feedback(entry: &TranscriptEntry) -> String {
    let result = if entry.result == "(not executed)" {
        "not_executed"
    } else {
        entry.result.as_str()
    };
    json!({
        "action": entry.action.as_str(),
        "decision": entry.decision.map(decision_name),
        "rule": entry.rule.as_deref(),
        "effect_mode": entry.effect_mode,
        "verdict": entry.verdict.as_str(),
        "result": result,
    })
    .to_string()
}

fn decision_name(decision: Decision) -> &'static str {
    match decision {
        Decision::Absent => "ABSENT",
        Decision::Allow => "ALLOW",
        Decision::Deny => "DENY",
        Decision::Ask => "ASK",
        Decision::Replan => "REPLAN",
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
