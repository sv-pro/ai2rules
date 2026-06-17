//! Deterministic replay (E4.3) and policy-drift reports (E4.4).
//!
//! Replay reconstructs each recorded decision's inputs, re-runs the kernel, and
//! compares the recomputed outcome with the recorded one. Against the *same*
//! compiled world every decision must reproduce (invariant 14). Against a
//! *different* world the mismatches are the explicit policy-drift diff.

use harness_types::{ActionName, CallId, CompiledWorld, Provider, TaintContext, ToolCall};
use world_kernel::{decide, BudgetUsage, EvalContext, KernelOutcome};

use crate::record::{DecisionRecord, OutcomeKind, OutcomeSummary, RecordPayload, TraceRecord};

/// One decision whose recomputed outcome differs from what was recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mismatch {
    pub seq: u64,
    pub action: ActionName,
    pub recorded: OutcomeSummary,
    pub recomputed: OutcomeSummary,
}

/// The result of replaying a trace against a world.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReplayReport {
    pub total: usize,
    pub matched: usize,
    pub mismatches: Vec<Mismatch>,
}

impl ReplayReport {
    /// True when every recorded decision reproduced (no drift).
    pub fn is_reproducible(&self) -> bool {
        self.mismatches.is_empty()
    }
}

/// Replay a trace against a compiled world. Same world ⇒ `matched == total`.
pub fn replay(records: &[TraceRecord], world: &CompiledWorld) -> ReplayReport {
    let mut report = ReplayReport::default();
    for record in records {
        let RecordPayload::Decision(decision) = &record.payload else {
            continue;
        };
        report.total += 1;
        let recomputed = recompute(world, decision);
        if recomputed == decision.outcome {
            report.matched += 1;
        } else {
            report.mismatches.push(Mismatch {
                seq: record.seq,
                action: decision.action.clone(),
                recorded: decision.outcome.clone(),
                recomputed,
            });
        }
    }
    report
}

/// Replay against a *new* world; the mismatches are the policy-drift diff. Same
/// computation as [`replay`], named for intent at the call site.
pub fn drift_report(records: &[TraceRecord], new_world: &CompiledWorld) -> ReplayReport {
    replay(records, new_world)
}

fn recompute(world: &CompiledWorld, record: &DecisionRecord) -> OutcomeSummary {
    let call = ToolCall {
        action_name: record.action.clone(),
        arguments: record.params.clone(),
        provider: Provider::CliNative,
        call_id: CallId::new("replay"),
        source_perceptions: Vec::new(),
        session_id: record.provenance.session_id.clone(),
    };
    let ctx = EvalContext {
        taint: TaintContext::from_taint(record.context.taint),
        mode: record.context.mode,
        usage: BudgetUsage {
            commands_run: record.context.commands_run,
            tokens_used: record.context.tokens_used,
            file_writes: record.context.file_writes,
            network_calls: record.context.network_calls,
        },
    };
    summarize(&decide(world, &call, record.provenance.clone(), &ctx))
}

/// Fold a `KernelOutcome` into the comparable `OutcomeSummary`. Shared by the
/// recorder (`lib::record_decision`) and replay so both speak the same shape.
pub(crate) fn summarize(outcome: &KernelOutcome) -> OutcomeSummary {
    use harness_types::Decision;
    match outcome {
        KernelOutcome::UnknownToOntology { .. } => OutcomeSummary {
            kind: OutcomeKind::UnknownToOntology,
            decision: Decision::Absent,
            rule: "unknown_to_ontology".to_string(),
            effect_mode: None,
            descriptor_hash: None,
        },
        KernelOutcome::NotRepresentable { decision, rule, .. } => OutcomeSummary {
            kind: OutcomeKind::NotRepresentable,
            decision: *decision,
            rule: rule.clone(),
            effect_mode: None,
            descriptor_hash: None,
        },
        KernelOutcome::Evaluated {
            intent,
            disposition,
        } => OutcomeSummary {
            kind: OutcomeKind::Evaluated,
            decision: disposition.decision,
            rule: disposition.rule.clone(),
            effect_mode: disposition.effect_mode,
            descriptor_hash: Some(intent.expected_descriptor_hash().clone()),
        },
    }
}
