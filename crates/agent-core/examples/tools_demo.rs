//! Tools demo (E7) — scoped capabilities, MCP, and the web channel, offline.
//!
//! Run:
//!
//! ```text
//! cargo run -p agent-core --example tools_demo
//! ```
//!
//! Part A shows scoped-capability narrowing at the kernel: `run_tests` proposed
//! with a malicious `command` lowers to argv `["pytest"]` (locked args stripped,
//! literal injected — invariant 12). Part B drives the loop (EXECUTE, mock
//! transports): an MCP call returns a tainted result, after which a web fetch is
//! DENIED because the context is now tainted (invariant 7), and a scoped repo
//! read drops an undeclared arg.

use agent_core::{executor_with_transports, run, ModelTurn, ScriptedModel, SessionConfig};
use compiler::compile_default;
use executor::{MockMcpTransport, MockWebFetcher};
use harness_types::{
    ActionName, CallId, ContentHash, EffectMode, Operation, Provenance, Provider, SessionId,
    SourceChannel, TaintContext, ToolCall, TraceId,
};
use provider_adapters::anthropic::tool_use_block;
use serde_json::json;
use trace_store::{ApprovalStore, TraceStore};
use world_kernel::{build_execution_spec, ExecEnv, IRBuilder};

fn main() {
    let world = compile_default();

    println!();
    println!("  ai2rules — tools demo (scoped caps · MCP · web)\n");

    // ── Part A: scoped-capability narrowing (invariant 12) ──
    println!("  A) run_tests proposed with a malicious command + extra arg:");
    let call = ToolCall {
        action_name: ActionName::new("run_tests"),
        arguments: json!({ "command": "rm -rf /", "path": "/etc/passwd" }),
        provider: Provider::CliNative,
        call_id: CallId::new("a"),
        source_perceptions: vec![],
        session_id: SessionId::new("demo"),
    };
    let prov = Provenance::from_channel(
        SourceChannel::UserPrompt,
        SessionId::new("demo"),
        ContentHash::new("h"),
    );
    let intent = IRBuilder::new(&world)
        .build(&call, prov, &TaintContext::clean())
        .expect("representable");
    let spec = build_execution_spec(
        &world,
        &intent,
        EffectMode::Simulate,
        &ExecEnv::default(),
        TraceId::new("a"),
    )
    .expect("spec");
    match spec.operation() {
        Operation::Argv(argv) => println!("     → lowered argv: {argv:?}  (locked + stripped)\n"),
        other => println!("     → {other:?}\n"),
    }

    // ── Part B: MCP + web through the loop (EXECUTE, mock transports) ──
    let sandbox = tempfile::tempdir().expect("sandbox");
    std::fs::write(sandbox.path().join("notes.txt"), "repo notes\n").unwrap();
    let trace = TraceStore::open(sandbox.path().join("trace.jsonl"));
    let mut store = ApprovalStore::open(sandbox.path().join("approvals.jsonl")).unwrap();

    let mcp = MockMcpTransport::new().with("docs", "search", json!({ "answer": "use OAuth" }));
    let web = MockWebFetcher::new().with("https://docs.example/guide", "<html>guide</html>");
    let executor = executor_with_transports(&world, Box::new(mcp), Box::new(web));

    let env = ExecEnv {
        cwd: sandbox.path().to_path_buf(),
        readable_roots: vec![sandbox.path().to_path_buf()],
        ..Default::default()
    };
    let config = SessionConfig {
        effect_mode: EffectMode::Execute,
        ..SessionConfig::default()
    };

    let notes = sandbox.path().join("notes.txt");
    let mut model = ScriptedModel::new([
        ModelTurn::ToolUse(tool_use_block(
            "t1",
            "call_known_mcp_tool",
            json!({ "query": "auth" }),
        )),
        ModelTurn::ToolUse(tool_use_block(
            "t2",
            "fetch_web",
            json!({ "url": "https://docs.example/guide" }),
        )),
        ModelTurn::ToolUse(tool_use_block(
            "t3",
            "read_repo_file",
            json!({ "path": notes.to_str().unwrap(), "evil": "drop" }),
        )),
        ModelTurn::Final("done".into()),
    ]);

    println!("  B) loop (mock MCP/web, EXECUTE):");
    let outcome = run(
        &world, &env, &executor, &trace, &mut store, &mut model, &config, None,
    );
    for step in &outcome.transcript {
        println!(
            "     {:<20} {:<22} {}  [taint: {:?}]",
            step.action, step.verdict, step.result, step.taint
        );
    }
    println!("\n     MCP result is tainted → the later web fetch is DENIED (invariant 7);");
    println!("     read_repo_file's undeclared `evil` arg was stripped (invariant 12).");
}
