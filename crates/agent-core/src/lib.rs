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

pub mod arg_provenance;
pub mod context;
pub mod intent;
pub mod model;
pub mod orchestrator;

pub use context::{pack, pack_with_feedback, tool_surface, ToolFeedback, TurnContext};
pub use intent::{classify, Mapping};
pub use model::{ModelClient, ModelTurn, ScriptedModel};
pub use orchestrator::{run, ApprovalPolicy, SessionConfig, SessionOutcome, TranscriptEntry};

use executor::{
    CommandHandler, Executor, ExecutorBuilder, McpHandler, McpTransport, PatchHandler, ReadHandler,
    WebFetcher, WebHandler,
};
use harness_types::{ActionName, CompiledWorld};

/// An executor wired with the default world's local handlers and the descriptor
/// hashes they must match — the executable surface for the loop. A scoped
/// capability's spec carries the *scoped* action name + hash, so each is
/// registered alongside its base.
pub fn default_executor(world: &CompiledWorld) -> Executor {
    register_local(Executor::builder(), world).build()
}

/// Like [`default_executor`], but also wires the external channels (MCP, web)
/// through the given transports (E7). Mock transports keep this offline.
pub fn executor_with_transports(
    world: &CompiledWorld,
    mcp: Box<dyn McpTransport>,
    web: Box<dyn WebFetcher>,
) -> Executor {
    let hash = |a: &str| {
        world
            .descriptor_hash(&ActionName::new(a))
            .cloned()
            .unwrap_or_default()
    };
    register_local(Executor::builder(), world)
        .register(
            ActionName::new("call_known_mcp_tool"),
            hash("call_known_mcp_tool"),
            Box::new(McpHandler::new(mcp)),
        )
        .register(
            ActionName::new("fetch_web"),
            hash("fetch_web"),
            Box::new(WebHandler::new(web)),
        )
        .build()
}

/// Register every locally-backed action (base + its scoped capabilities) under
/// its own descriptor hash, mapping to the base action's handler kind.
fn register_local(builder: ExecutorBuilder, world: &CompiledWorld) -> ExecutorBuilder {
    let hash = |a: &str| {
        world
            .descriptor_hash(&ActionName::new(a))
            .cloned()
            .unwrap_or_default()
    };
    let mut b = builder
        .register(
            ActionName::new("read_workspace"),
            hash("read_workspace"),
            Box::new(ReadHandler),
        )
        .register(
            ActionName::new("read_repo_file"),
            hash("read_repo_file"),
            Box::new(ReadHandler),
        )
        .register(
            ActionName::new("apply_patch"),
            hash("apply_patch"),
            Box::new(PatchHandler),
        )
        .register(
            ActionName::new("apply_workspace_patch"),
            hash("apply_workspace_patch"),
            Box::new(PatchHandler),
        );
    for cmd in [
        "run_command",
        "run_tests",
        "git_status",
        "git_diff",
        "git_commit",
    ] {
        // Explicit unconfined acknowledgment (D47): no OS sandbox (E8) exists yet
        // to enforce a subprocess's network/filesystem policy, so command Execute
        // runs with host authority. Fail-closed is the handler default; this is
        // the one place that opts in, and it should become operator-configurable
        // (and retire once E8 provides a real sandbox posture).
        b = b.register(
            ActionName::new(cmd),
            hash(cmd),
            Box::new(CommandHandler::unconfined()),
        );
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    use compiler::compile_default;
    use executor::{MockMcpTransport, MockWebFetcher};
    use harness_types::{Decision, EffectMode, ExecutionMode, Taint};
    use provider_adapters::anthropic::tool_use_block;
    use serde_json::json;
    use trace_store::{ApprovalStore, TraceStore};
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
        let dir = tempfile::tempdir().unwrap();
        let world = compile_default();
        let executor = default_executor(&world);
        let trace = TraceStore::open(dir.path().join("trace.jsonl"));
        let mut store = ApprovalStore::open(dir.path().join("approvals.jsonl")).unwrap();
        let mut model = scripted();
        run(
            &world,
            &ExecEnv::default(),
            &executor,
            &trace,
            &mut store,
            &mut model,
            &SessionConfig::default(),
            None,
        )
    }

    #[test]
    fn loop_runs_the_full_verdict_range() {
        let outcome = run_session();
        assert_eq!(outcome.final_text.as_deref(), Some("done"));
        assert_eq!(outcome.records, 4);
        assert_eq!(outcome.transcript.len(), 4);

        let read = &outcome.transcript[0];
        assert_eq!(read.action, "read_workspace");
        assert_eq!(read.verdict, "ALLOW");
        assert_eq!(read.taint, Taint::Tainted);

        assert_eq!(outcome.transcript[1].action, "fetch_web");
        assert!(outcome.transcript[1].verdict.starts_with("Deny"));
        assert_eq!(outcome.transcript[1].decision, Some(Decision::Deny));
        assert_eq!(
            outcome.transcript[1].rule.as_deref(),
            Some("taint_invariant")
        );

        assert!(outcome.transcript[2]
            .verdict
            .contains("UNKNOWN_TO_ONTOLOGY"));
        assert_eq!(outcome.transcript[2].decision, Some(Decision::Absent));
        assert_eq!(
            outcome.transcript[2].rule.as_deref(),
            Some("unknown_to_ontology")
        );

        assert!(outcome.transcript[3].verdict.starts_with("ASK"));
        assert_eq!(outcome.transcript[3].decision, Some(Decision::Ask));
        assert_eq!(
            outcome.transcript[3].rule.as_deref(),
            Some("approval_required")
        );
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

    fn run_one(action: &str, args: serde_json::Value, config: &SessionConfig) -> SessionOutcome {
        let dir = tempfile::tempdir().unwrap();
        let world = compile_default();
        let executor = default_executor(&world);
        let trace = TraceStore::open(dir.path().join("t.jsonl"));
        let mut store = ApprovalStore::open(dir.path().join("a.jsonl")).unwrap();
        let mut model =
            ScriptedModel::new([ModelTurn::ToolUse(tool_use_block("t1", action, args))]);
        run(
            &world,
            &ExecEnv::default(),
            &executor,
            &trace,
            &mut store,
            &mut model,
            config,
            None,
        )
    }

    #[test]
    fn clean_read_alone_is_allowed() {
        let outcome = run_one(
            "read_workspace",
            json!({ "path": "x" }),
            &SessionConfig::default(),
        );
        assert_eq!(outcome.transcript[0].verdict, "ALLOW");
        assert_eq!(outcome.transcript[0].taint, Taint::Tainted);
        assert_eq!(outcome.transcript[0].decision, Some(Decision::Allow));
        assert_eq!(
            outcome.transcript[0].effect_mode,
            Some(EffectMode::Simulate)
        );
    }

    #[test]
    fn auto_approve_resumes_pty_to_allow() {
        let config = SessionConfig {
            approval: ApprovalPolicy::AutoApprove,
            ..SessionConfig::default()
        };
        let outcome = run_one("start_pty", json!({}), &config);
        assert_eq!(outcome.transcript[0].verdict, "ASK → APPROVED → ALLOW");
        assert_eq!(outcome.transcript[0].decision, Some(Decision::Allow));
        assert_eq!(
            outcome.transcript[0].effect_mode,
            Some(EffectMode::Simulate)
        );
        assert_eq!(outcome.records, 2);
    }

    #[test]
    fn authorization_binds_the_kernel_effective_action() {
        let dir = tempfile::tempdir().unwrap();
        let world = compile_default();
        let executor = default_executor(&world);
        let trace = TraceStore::open(dir.path().join("t.jsonl"));
        let approval_path = dir.path().join("a.jsonl");
        let mut store = ApprovalStore::open(&approval_path).unwrap();
        let mut model = ScriptedModel::new([ModelTurn::ToolUse(tool_use_block(
            "t1",
            "run_command",
            json!({"command": "rm -rf ./generated"}),
        ))]);
        let config = SessionConfig {
            approval: ApprovalPolicy::AutoApprove,
            ..SessionConfig::default()
        };

        let outcome = run(
            &world,
            &ExecEnv::default(),
            &executor,
            &trace,
            &mut store,
            &mut model,
            &config,
            None,
        );
        assert_eq!(outcome.transcript[0].verdict, "ASK → APPROVED → ALLOW");
        let evidence = std::fs::read_to_string(approval_path).unwrap();
        assert!(evidence.contains("\"action\":\"run_command_destructive\""));
    }

    #[test]
    fn background_denies_pty() {
        let config = SessionConfig {
            mode: ExecutionMode::Background,
            approval: ApprovalPolicy::AutoApprove,
            ..SessionConfig::default()
        };
        let outcome = run_one("start_pty", json!({}), &config);
        assert!(outcome.transcript[0].verdict.starts_with("Deny"));
        assert_eq!(outcome.transcript[0].decision, Some(Decision::Deny));
        assert_eq!(
            outcome.transcript[0].rule.as_deref(),
            Some("background_denies_ask")
        );
        assert_eq!(outcome.records, 1);
        let _ = Decision::Deny;
    }

    #[test]
    fn mcp_result_taints_then_web_is_denied() {
        let world = compile_default();
        let dir = tempfile::tempdir().unwrap();
        let trace = TraceStore::open(dir.path().join("t.jsonl"));
        let mut store = ApprovalStore::open(dir.path().join("a.jsonl")).unwrap();
        let mcp = MockMcpTransport::new().with("docs", "search", json!({ "answer": "x" }));
        let web = MockWebFetcher::new().with("https://x", "body");
        let executor = executor_with_transports(&world, Box::new(mcp), Box::new(web));
        let mut model = ScriptedModel::new([
            ModelTurn::ToolUse(tool_use_block(
                "t1",
                "call_known_mcp_tool",
                json!({ "query": "q" }),
            )),
            ModelTurn::ToolUse(tool_use_block(
                "t2",
                "fetch_web",
                json!({ "url": "https://x" }),
            )),
            ModelTurn::Final("done".into()),
        ]);
        let config = SessionConfig {
            effect_mode: EffectMode::Execute,
            ..SessionConfig::default()
        };
        let outcome = run(
            &world,
            &ExecEnv::default(),
            &executor,
            &trace,
            &mut store,
            &mut model,
            &config,
            None,
        );
        assert_eq!(outcome.transcript[0].verdict, "ALLOW");
        assert_eq!(outcome.transcript[0].taint, Taint::Tainted);
        assert!(outcome.transcript[1].verdict.starts_with("Deny"));
    }

    #[test]
    fn l2_producer_recovers_user_supplied_url_in_tainted_session() {
        const GUIDE: &str = "https://docs.example/guide";
        let env = ExecEnv {
            network: harness_types::NetworkPolicy::AllowHosts(vec!["docs.example".into()]),
            ..ExecEnv::default()
        };
        let world = compile_default();
        let dir = tempfile::tempdir().unwrap();
        let trace = TraceStore::open(dir.path().join("t.jsonl"));
        let mut store = ApprovalStore::open(dir.path().join("a.jsonl")).unwrap();
        let mcp = MockMcpTransport::new().with("docs", "search", json!({ "answer": GUIDE }));
        let web = MockWebFetcher::new().with(GUIDE, "guide");
        let executor = executor_with_transports(&world, Box::new(mcp), Box::new(web));
        let mut model = ScriptedModel::new([
            ModelTurn::ToolUse(tool_use_block(
                "t1",
                "call_known_mcp_tool",
                json!({ "query": "q" }),
            )),
            ModelTurn::ToolUse(tool_use_block(
                "t2",
                "fetch_web",
                json!({ "url": GUIDE }),
            )),
            ModelTurn::Final("done".into()),
        ]);
        let config = SessionConfig {
            effect_mode: EffectMode::Execute,
            user_request: Some(format!("Please read {GUIDE}")),
            ..SessionConfig::default()
        };
        let outcome = run(
            &world,
            &env,
            &executor,
            &trace,
            &mut store,
            &mut model,
            &config,
            None,
        );
        assert_eq!(outcome.transcript[0].decision, Some(Decision::Allow));
        assert_eq!(outcome.transcript[1].decision, Some(Decision::Allow));
    }
}
