//! The model loop (E5.5): propose → adapt → decide → execute → perceive.
//!
//! One deterministic pass per step. The model proposes an Anthropic `tool_use`
//! block; the adapter normalizes it to a neutral `ToolCall`; the kernel decides;
//! the decision is recorded to the trace; on `ALLOW` the spec is built and run
//! through the executor (in the configured effect mode), its tainted result
//! perceived and fed back; any non-allow verdict is fed back as a structured,
//! erroring `ToolOutcome`. The loop never builds an `ExecutionSpec` from anything
//! but a sealed, allowed intent (invariant 4).

use executor::{ExecOutput, Executor};
use harness_types::{
    ContentHash, Decision, EffectMode, ExecutionMode, PayloadRef, Perception, PerceptionId,
    PerceptionKind, Provenance, RedactionPolicy, SessionId, SourceChannel, Taint, TaintContext,
    TraceId, TrustLevel,
};
use provider_adapters::{anthropic, ToolOutcome};
use trace_store::{record_decision, TraceStore};
use world_kernel::{
    build_execution_spec, decide, BudgetUsage, EvalContext, ExecEnv, KernelOutcome,
};

use crate::context;
use crate::model::{ModelClient, ModelTurn};

/// One recorded step of the session, for display/inspection.
#[derive(Debug, Clone)]
pub struct TranscriptEntry {
    pub action: String,
    pub verdict: String,
    pub result: String,
    pub taint: Taint,
}

#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Effect mode for allowed actions — `Simulate` for safe demos.
    pub effect_mode: EffectMode,
    pub max_steps: u64,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            effect_mode: EffectMode::Simulate,
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
/// steps. Pure of policy — every verdict comes from the kernel.
pub fn run(
    world: &harness_types::CompiledWorld,
    env: &ExecEnv,
    executor: &Executor,
    trace: &TraceStore,
    model: &mut dyn ModelClient,
    config: &SessionConfig,
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

    for seq in 0..config.max_steps {
        let ctx = context::pack(world, perceptions.clone());
        match model.next(&ctx) {
            ModelTurn::Final(text) => {
                return SessionOutcome {
                    transcript,
                    final_text: Some(text),
                    records,
                };
            }
            ModelTurn::ToolUse(block) => {
                let call = match anthropic::tool_use_to_call(&block, session.clone()) {
                    Ok(call) => call,
                    Err(e) => {
                        transcript.push(TranscriptEntry {
                            action: "<malformed>".to_string(),
                            verdict: format!("adapter error: {e}"),
                            result: String::new(),
                            taint: Taint::Clean,
                        });
                        continue;
                    }
                };

                let eval = EvalContext {
                    taint: TaintContext::from_taint(taint),
                    mode: ExecutionMode::Interactive,
                    usage: BudgetUsage::default(),
                };
                let outcome = decide(world, &call, provenance.clone(), &eval);
                let record = record_decision(
                    world,
                    TraceId::new("agent"),
                    seq,
                    &call,
                    &provenance,
                    &eval,
                    &outcome,
                );
                let _ = trace.append(&record);
                records += 1;

                let action = call.action_name.as_str().to_string();
                match outcome {
                    KernelOutcome::Evaluated {
                        intent,
                        disposition,
                    } if disposition.decision == Decision::Allow => {
                        let result = match build_execution_spec(
                            world,
                            &intent,
                            config.effect_mode,
                            env,
                            TraceId::new("agent"),
                        ) {
                            Ok(spec) => match executor.run(&spec) {
                                Ok(tv) => {
                                    let content = describe_output(&tv.value);
                                    taint = taint.join(tv.taint);
                                    perceptions.push(result_perception(
                                        &action, &content, tv.taint, &session,
                                    ));
                                    TranscriptEntry {
                                        action,
                                        verdict: "ALLOW".to_string(),
                                        result: content,
                                        taint: tv.taint,
                                    }
                                }
                                Err(e) => TranscriptEntry {
                                    action,
                                    verdict: "ALLOW".to_string(),
                                    result: format!("exec error: {e}"),
                                    taint: Taint::Clean,
                                },
                            },
                            Err(e) => TranscriptEntry {
                                action,
                                verdict: "ALLOW".to_string(),
                                result: format!("spec error: {e}"),
                                taint: Taint::Clean,
                            },
                        };
                        transcript.push(result);
                    }
                    other => {
                        let verdict = verdict_label(&other);
                        // What a real model would receive back as a tool_result.
                        let _feedback = anthropic::format_tool_result(&ToolOutcome {
                            call_id: call.call_id.clone(),
                            content: verdict.clone(),
                            is_error: true,
                        });
                        transcript.push(TranscriptEntry {
                            action,
                            verdict,
                            result: "(not executed)".to_string(),
                            taint: Taint::Clean,
                        });
                    }
                }
            }
        }
    }

    SessionOutcome {
        transcript,
        final_text: None,
        records,
    }
}

fn describe_output(output: &ExecOutput) -> String {
    match output {
        ExecOutput::Simulated(note) => format!("(simulated) {note}"),
        ExecOutput::FileContents(c) => format!("read {} bytes", c.len()),
        ExecOutput::CommandResult { exit_code, .. } => format!("exit {exit_code}"),
        ExecOutput::PatchApplied { path } => format!("wrote {}", path.display()),
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
