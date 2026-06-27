//! Approvals demo (E6) — human-in-the-loop and fail-closed background.
//!
//! Run:
//!
//! ```text
//! cargo run -p agent-core --example approvals_demo
//! ```
//!
//! The same `start_pty` proposal (flagged `approval_required`) under two modes:
//! INTERACTIVE with auto-approve resumes it to ALLOW (token minted → approved →
//! executed); BACKGROUND fails closed to DENY with no token minted. Then a note
//! on drift — an approval is bound to the exact call and voids if it changes.

use agent_core::{default_executor, run, ApprovalPolicy, ModelTurn, ScriptedModel, SessionConfig};
use compiler::compile_default;
use harness_types::ExecutionMode;
use provider_adapters::anthropic::tool_use_block;
use serde_json::json;
use trace_store::{ApprovalStore, TraceStore};
use world_kernel::ExecEnv;

fn main() {
    println!();
    println!("  ai2rules — approvals demo");
    println!("  Action: start_pty (approval_required in the default world)\n");

    run_once(
        "1) INTERACTIVE + auto-approve",
        ExecutionMode::Interactive,
        ApprovalPolicy::AutoApprove,
    );
    run_once(
        "2) BACKGROUND (no human → fail closed)",
        ExecutionMode::Background,
        ApprovalPolicy::AutoApprove, // never reached — BACKGROUND collapses ASK to DENY
    );

    println!("  Binding: an approval is pinned to action + params + world +");
    println!("  descriptor hash + provenance + effect mode — any drift voids reuse.");
}

fn run_once(label: &str, mode: ExecutionMode, approval: ApprovalPolicy) {
    let world = compile_default();
    let executor = default_executor(&world);
    let dir = tempfile::tempdir().expect("sandbox");
    let trace = TraceStore::open(dir.path().join("trace.jsonl"));
    let mut store = ApprovalStore::open(dir.path().join("approvals.jsonl")).expect("store");

    let mut model = ScriptedModel::new([
        ModelTurn::ToolUse(tool_use_block("t1", "start_pty", json!({}))),
        ModelTurn::Final("done".into()),
    ]);
    let config = SessionConfig {
        mode,
        approval,
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

    println!("  {label}");
    for step in &outcome.transcript {
        println!("     {} -> {}", step.action, step.verdict);
    }
    println!("     ({} decision(s) recorded)\n", outcome.records);
}
