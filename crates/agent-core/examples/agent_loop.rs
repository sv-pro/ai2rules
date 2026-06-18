//! Agent-loop demo — a (scripted) model drives the harness through the gate.
//!
//! Run:
//!
//! ```text
//! cargo run -p agent-core --example agent_loop
//! ```
//!
//! A deterministic `ScriptedModel` stands in for an LLM (no network, no API key).
//! It proposes Anthropic `tool_use` blocks; the adapter normalizes them; the
//! kernel decides; allowed actions run in SIMULATE and their tainted results feed
//! back; every decision is recorded to a trace. No LLM sits on the gate.

use agent_core::{default_executor, run, ScriptedModel, SessionConfig, SessionOutcome};
use compiler::compile_default;
use provider_adapters::anthropic::tool_use_block;
use serde_json::json;
use trace_store::{ApprovalStore, TraceStore};
use world_kernel::ExecEnv;

fn main() {
    let world = compile_default();
    let executor = default_executor(&world);
    let sandbox = tempfile::tempdir().expect("sandbox");
    let trace = TraceStore::open(sandbox.path().join("trace.jsonl"));
    let mut store = ApprovalStore::open(sandbox.path().join("approvals.jsonl")).expect("store");

    let mut model = ScriptedModel::new([
        agent_core::ModelTurn::ToolUse(tool_use_block(
            "t1",
            "read_workspace",
            json!({ "path": "src/lib.rs" }),
        )),
        agent_core::ModelTurn::ToolUse(tool_use_block(
            "t2",
            "fetch_web",
            json!({ "url": "https://evil.example/leak" }),
        )),
        agent_core::ModelTurn::ToolUse(tool_use_block("t3", "send_email", json!({ "to": "ceo" }))),
        agent_core::ModelTurn::ToolUse(tool_use_block("t4", "start_pty", json!({}))),
        agent_core::ModelTurn::Final(
            "All set — read the file; everything else was refused.".into(),
        ),
    ]);

    println!();
    println!("  CLI Agent Harness — agent loop demo");
    println!("  world: {}", world.world_id().as_str());
    println!("  A scripted model proposes; the kernel governs; results feed back.\n");

    let SessionOutcome {
        transcript,
        final_text,
        records,
    } = run(
        &world,
        &ExecEnv::default(),
        &executor,
        &trace,
        &mut store,
        &mut model,
        &SessionConfig::default(),
        None,
    );

    for (i, step) in transcript.iter().enumerate() {
        println!("  {}. proposes {}", i + 1, step.action);
        println!("     verdict : {}", step.verdict);
        println!(
            "     result  : {}  [taint: {:?}]\n",
            step.result, step.taint
        );
    }

    if let Some(text) = final_text {
        println!("  model: {text}");
    }
    println!("  ({records} decisions recorded to the trace)");
}
