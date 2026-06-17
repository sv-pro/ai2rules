//! # agent-core
//!
//! Provider-independent orchestration: packs typed `Perception`s into context,
//! exposes only the projected tool surface, and drives the loop propose → adapt →
//! kernel → execute → perceive. Depends on the kernel and the edge crates; the
//! dependency only ever flows inward to `harness-types`.
//!
//! The model only ever *proposes* a `ToolCall`; the kernel is the sole producer
//! of an `ExecutionSpec`, so the model can never reach the executor directly
//! (invariant 4). See [`orchestrator::run`].

pub mod context;
pub mod intent;
pub mod model;
pub mod orchestrator;

pub use context::{pack, tool_surface, TurnContext};
pub use intent::{classify, Mapping};
pub use model::{ModelClient, ModelTurn, ScriptedModel};
pub use orchestrator::{run, SessionConfig, SessionOutcome, TranscriptEntry};

use executor::{CommandHandler, Executor, PatchHandler, ReadHandler};
use harness_types::{ActionName, CompiledWorld};

/// An executor wired with the default world's local handlers and the descriptor
/// hashes they must match — the executable surface for the loop.
pub fn default_executor(world: &CompiledWorld) -> Executor {
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

#[cfg(test)]
mod tests {
    use super::*;
    use compiler::compile_default;
    use harness_types::Taint;
    use provider_adapters::anthropic::tool_use_block;
    use serde_json::json;
    use trace_store::TraceStore;
    use world_kernel::ExecEnv;

    /// A scripted session: read (allow → taints), fetch_web (denied by the now
    /// tainted context), send_email (unknown), start_pty (ask), then a final.
    fn scripted() -> ScriptedModel {
        ScriptedModel::new([
            ModelTurn::ToolUse(tool_use_block(
                "t1",
                "read_workspace",
                json!({ "path": "src/lib.rs" }),
            )),
            ModelTurn::ToolUse(tool_use_block(
                "t2",
                "fetch_web",
                json!({ "url": "https://x" }),
            )),
            ModelTurn::ToolUse(tool_use_block("t3", "send_email", json!({}))),
            ModelTurn::ToolUse(tool_use_block("t4", "start_pty", json!({}))),
            ModelTurn::Final("done".to_string()),
        ])
    }

    fn run_session() -> SessionOutcome {
        // The tempdir drops at the end of this fn — fine, we assert on the
        // in-memory outcome, not the trace file on disk.
        let dir = tempfile::tempdir().unwrap();
        let world = compile_default();
        let executor = default_executor(&world);
        let trace = TraceStore::open(dir.path().join("trace.jsonl"));
        let mut model = scripted();
        run(
            &world,
            &ExecEnv::default(),
            &executor,
            &trace,
            &mut model,
            &SessionConfig::default(),
        )
    }

    #[test]
    fn loop_runs_the_full_verdict_range() {
        let outcome = run_session();
        assert_eq!(outcome.final_text.as_deref(), Some("done"));
        // Four tool calls were decided and recorded.
        assert_eq!(outcome.records, 4);
        assert_eq!(outcome.transcript.len(), 4);

        let read = &outcome.transcript[0];
        assert_eq!(read.action, "read_workspace");
        assert_eq!(read.verdict, "ALLOW");
        assert_eq!(read.taint, Taint::Tainted); // execution results are tainted

        // The read tainted the context, so the later web fetch is denied.
        assert_eq!(outcome.transcript[1].action, "fetch_web");
        assert!(outcome.transcript[1].verdict.starts_with("Deny"));

        // Unknown action is distinct (invariant 3).
        assert!(outcome.transcript[2]
            .verdict
            .contains("UNKNOWN_TO_ONTOLOGY"));

        // Approval-required action asks.
        assert!(outcome.transcript[3].verdict.starts_with("Ask"));
    }

    #[test]
    fn loop_is_deterministic() {
        let a = run_session();
        let b = run_session();
        let labels = |o: &SessionOutcome| {
            o.transcript
                .iter()
                .map(|e| (e.action.clone(), e.verdict.clone(), e.taint))
                .collect::<Vec<_>>()
        };
        assert_eq!(labels(&a), labels(&b));
        assert_eq!(a.final_text, b.final_text);
    }

    #[test]
    fn clean_read_alone_is_allowed() {
        let world = compile_default();
        let dir = tempfile::tempdir().unwrap();
        let trace = TraceStore::open(dir.path().join("t.jsonl"));
        let executor = default_executor(&world);
        let mut model = ScriptedModel::new([ModelTurn::ToolUse(tool_use_block(
            "t1",
            "read_workspace",
            json!({ "path": "x" }),
        ))]);
        let outcome = run(
            &world,
            &ExecEnv::default(),
            &executor,
            &trace,
            &mut model,
            &SessionConfig::default(),
        );
        assert_eq!(outcome.transcript[0].verdict, "ALLOW");
        assert_eq!(outcome.transcript[0].taint, Taint::Tainted);
    }
}
