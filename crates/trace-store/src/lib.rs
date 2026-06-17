//! # trace-store
//!
//! The harness's append-only audit trail and replay layer. It records every
//! kernel decision (with secrets redacted before they touch disk), reproduces
//! decisions deterministically (same trace + same world ⇒ same verdict), reports
//! policy drift (old trace + new world ⇒ the diff), and packages a trace with
//! its manifest into a self-contained replay bundle.
//!
//! It is a top-of-stack consumer: it depends on `world-kernel` (to re-run
//! `decide`) and `compiler` (to recompile a bundled world), but the kernel is
//! pure and depends on nothing here.

mod bundle;
mod record;
mod redact;
mod replay;
mod store;

pub use bundle::{export_bundle, import_bundle, replay_bundle, Bundle};
pub use record::{
    ContextSnapshot, DecisionRecord, ExecSummary, ExecutionRecord, OutcomeKind, OutcomeSummary,
    RecordPayload, TraceRecord,
};
pub use redact::redact;
pub use replay::{drift_report, replay, Mismatch, ReplayReport};
pub use store::TraceStore;

use harness_types::{CompiledWorld, Provenance, ToolCall, TraceId};
use world_kernel::{EvalContext, KernelOutcome};

use crate::replay::summarize;

/// Build a redacted, replayable record from a kernel decision. The arguments are
/// redacted with the world's patterns *before* the record exists, so a secret
/// can never reach the store.
pub fn record_decision(
    world: &CompiledWorld,
    trace_id: TraceId,
    seq: u64,
    call: &ToolCall,
    provenance: &Provenance,
    ctx: &EvalContext,
    outcome: &KernelOutcome,
) -> TraceRecord {
    let params = redact(&call.arguments, world.redaction_patterns());
    let context = ContextSnapshot {
        taint: ctx.taint.taint(),
        mode: ctx.mode,
        commands_run: ctx.usage.commands_run,
        tokens_used: ctx.usage.tokens_used,
        file_writes: ctx.usage.file_writes,
        network_calls: ctx.usage.network_calls,
    };
    TraceRecord {
        trace_id,
        session_id: call.session_id.clone(),
        world_id: world.world_id().clone(),
        manifest_hash: world.manifest_hash().clone(),
        seq,
        payload: RecordPayload::Decision(DecisionRecord {
            action: call.action_name.clone(),
            params,
            provenance: provenance.clone(),
            context,
            outcome: summarize(outcome),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use compiler::{compile, compile_default, default_cli_world};
    use harness_types::{
        ActionName, ActionType, CallId, ContentHash, Decision, Provenance, Provider, SessionId,
        SourceChannel, ToolCall, TrustLevel,
    };
    use serde_json::{json, Value};
    use world_kernel::{decide, EvalContext};

    fn call(action: &str, args: Value) -> ToolCall {
        ToolCall {
            action_name: ActionName::new(action),
            arguments: args,
            provider: Provider::CliNative,
            call_id: CallId::new("c"),
            source_perceptions: vec![],
            session_id: SessionId::new("s"),
        }
    }

    fn prov(channel: SourceChannel) -> Provenance {
        Provenance::from_channel(channel, SessionId::new("s"), ContentHash::new("h"))
    }

    /// Decide a call against `world` and record it.
    fn decide_and_record(
        world: &CompiledWorld,
        seq: u64,
        action: &str,
        args: Value,
        channel: SourceChannel,
    ) -> TraceRecord {
        let c = call(action, args);
        let p = prov(channel);
        let ctx = EvalContext::interactive_clean();
        let outcome = decide(world, &c, p.clone(), &ctx);
        record_decision(world, TraceId::new("t"), seq, &c, &p, &ctx, &outcome)
    }

    fn mixed_trace(world: &CompiledWorld) -> Vec<TraceRecord> {
        vec![
            decide_and_record(
                world,
                0,
                "read_workspace",
                json!({"path": "a"}),
                SourceChannel::UserPrompt,
            ),
            decide_and_record(world, 1, "write_workspace", json!({}), SourceChannel::Web), // ABSENT (capability)
            decide_and_record(world, 2, "start_pty", json!({}), SourceChannel::UserPrompt), // ASK
            decide_and_record(world, 3, "send_email", json!({}), SourceChannel::UserPrompt), // UNKNOWN
        ]
    }

    #[test]
    fn replay_against_same_world_reproduces_every_decision() {
        // Invariant 14.
        let world = compile_default();
        let records = mixed_trace(&world);
        let report = replay(&records, &world);
        assert_eq!(report.total, 4);
        assert_eq!(report.matched, 4);
        assert!(report.is_reproducible());
    }

    #[test]
    fn drift_report_flags_a_changed_verdict() {
        // Record against the default world, then replay against one where the
        // trusted actor lost the Read capability: the read flips Allow -> Absent.
        let world = compile_default();
        let records = mixed_trace(&world);

        let mut manifest = default_cli_world();
        for grant in &mut manifest.capabilities {
            if grant.trust == TrustLevel::Trusted {
                grant.actions.retain(|a| *a != ActionType::Read);
            }
        }
        let drifted = compile(&manifest).unwrap();

        let report = drift_report(&records, &drifted);
        assert!(!report.is_reproducible());
        let read_drift = report
            .mismatches
            .iter()
            .find(|m| m.action.as_str() == "read_workspace")
            .expect("read_workspace should have drifted");
        assert_eq!(read_drift.recorded.decision, Decision::Allow);
        assert_eq!(read_drift.recomputed.decision, Decision::Absent);
    }

    #[test]
    fn store_redacts_secrets_before_disk() {
        // Invariant 15: a secret in a call's args never reaches the file.
        let dir = tempfile::tempdir().unwrap();
        let world = compile_default();
        let record = decide_and_record(
            &world,
            0,
            "read_workspace",
            json!({"path": "src/lib.rs", "env": {"GITHUB_TOKEN": "ghp_LEAKED"}}),
            SourceChannel::UserPrompt,
        );
        let store = TraceStore::open(dir.path().join("trace.jsonl"));
        store.append(&record).unwrap();

        let bytes = std::fs::read_to_string(store.path()).unwrap();
        assert!(
            !bytes.contains("ghp_LEAKED"),
            "secret leaked to the trace file"
        );
        assert!(bytes.contains("[REDACTED]"));
    }

    #[test]
    fn bundle_round_trips_and_replays() {
        let dir = tempfile::tempdir().unwrap();
        let world = compile_default();
        let records = mixed_trace(&world);
        let bundle = Bundle::new(default_cli_world(), records);

        let path = dir.path().join("bundle.json");
        export_bundle(&path, &bundle).unwrap();
        let imported = import_bundle(&path).unwrap();
        assert_eq!(imported, bundle);

        let report = replay_bundle(&imported).unwrap();
        assert_eq!(report.matched, report.total);
        assert!(report.is_reproducible());
    }
}
