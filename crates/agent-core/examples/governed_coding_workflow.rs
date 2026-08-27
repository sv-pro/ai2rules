//! End-to-end governed coding workflow showcase (AI2-12 / #71).
//!
//! This first slice is deliberately deterministic and offline. A tiny model
//! drives the real agent-core loop, receives a governance refusal as a
//! provider-formatted tool result, and changes its next action because of that
//! refusal. The executor performs the final patch for real inside a disposable
//! fixture; a deterministic verifier proves the broken state became fixed.
//!
//! Run:
//!
//! ```text
//! cargo run -p agent-core --example governed_coding_workflow
//! ```

use std::fs;

use agent_core::{default_executor, run, ModelClient, ModelTurn, SessionConfig, TurnContext};
use compiler::compile_default;
use harness_types::EffectMode;
use provider_adapters::anthropic::tool_use_block;
use serde_json::json;
use trace_store::{ApprovalStore, TraceStore};
use world_kernel::ExecEnv;

const BROKEN: &str = "def add(a, b):\n    return a - b\n";
const FIXED: &str = "def add(a, b):\n    return a + b\n";

struct ReplanningDemoModel {
    phase: u8,
}

impl ReplanningDemoModel {
    fn new() -> Self {
        Self { phase: 0 }
    }
}

impl ModelClient for ReplanningDemoModel {
    fn next(&mut self, ctx: &TurnContext) -> ModelTurn {
        let turn = match self.phase {
            // Inspect the broken implementation. The resulting file perception
            // taints the session, as workspace content is not trusted authority.
            0 => ModelTurn::ToolUse(tool_use_block(
                "inspect",
                "read_repo_file",
                json!({ "path": "calc.py" }),
            )),

            // Try to leave the local world for an external answer. Because the
            // session is now tainted, the kernel should refuse this transition.
            1 => ModelTurn::ToolUse(tool_use_block(
                "external-help",
                "fetch_web",
                json!({ "url": "https://example.invalid/python-add" }),
            )),

            // This branch is the point of the demo: the next action depends on
            // the *actual* governance result from the previous step.
            2 => {
                let denied = ctx.last_tool_result.as_ref().is_some_and(|result| {
                    let Some(content) = result.get("content").and_then(serde_json::Value::as_str)
                    else {
                        return false;
                    };
                    serde_json::from_str::<serde_json::Value>(content)
                        .is_ok_and(|feedback| feedback["decision"] == "DENY")
                });
                if denied {
                    ModelTurn::ToolUse(tool_use_block(
                        "local-fix",
                        "apply_workspace_patch",
                        json!({ "path": "calc.py", "contents": FIXED }),
                    ))
                } else {
                    ModelTurn::Final(
                        "Unexpected trajectory: external help was not denied; no local replan made."
                            .into(),
                    )
                }
            }

            // Re-read the local result before finishing.
            3 => ModelTurn::ToolUse(tool_use_block(
                "verify-read",
                "read_repo_file",
                json!({ "path": "calc.py" }),
            )),

            _ => ModelTurn::Final(
                "Fixed the local implementation after governance rejected external help.".into(),
            ),
        };
        self.phase = self.phase.saturating_add(1);
        turn
    }
}

fn fixture_test(root: &std::path::Path) -> bool {
    fs::read_to_string(root.join("calc.py"))
        .map(|content| content == FIXED)
        .unwrap_or(false)
}

fn main() {
    let fixture = tempfile::tempdir().expect("fixture");
    fs::write(fixture.path().join("calc.py"), BROKEN).expect("write broken fixture");

    println!("ai2rules — governed coding workflow showcase");
    println!("task: fix calc.py without treating untrusted context as external authority\n");
    println!(
        "baseline fixture test: {}",
        if fixture_test(fixture.path()) {
            "PASS"
        } else {
            "FAIL"
        }
    );

    let world = compile_default();
    let executor = default_executor(&world);
    let trace_path = fixture.path().join("trace.jsonl");
    let trace = TraceStore::open(&trace_path);
    let mut approvals =
        ApprovalStore::open(fixture.path().join("approvals.jsonl")).expect("approval store");

    let env = ExecEnv {
        cwd: fixture.path().to_path_buf(),
        readable_roots: vec![fixture.path().to_path_buf()],
        writable_roots: vec![fixture.path().to_path_buf()],
        ..ExecEnv::default()
    };
    let config = SessionConfig {
        effect_mode: EffectMode::Execute,
        user_request: Some("Fix the failing local implementation in calc.py".into()),
        ..SessionConfig::default()
    };

    let mut model = ReplanningDemoModel::new();

    let outcome = run(
        &world,
        &env,
        &executor,
        &trace,
        &mut approvals,
        &mut model,
        &config,
        None,
    );

    for entry in &outcome.transcript {
        println!(
            "{:>16} -> {:<30} {}",
            entry.action, entry.verdict, entry.result
        );
    }

    let final_pass = fixture_test(fixture.path());
    println!(
        "\nfinal fixture test: {}",
        if final_pass { "PASS" } else { "FAIL" }
    );
    println!("decisions recorded: {}", outcome.records);
    if let Some(text) = outcome.final_text {
        println!("agent: {text}");
    }

    assert!(
        outcome
            .transcript
            .iter()
            .any(|step| step.action == "fetch_web" && step.verdict.starts_with("Deny")),
        "the showcase must contain a real governance refusal"
    );
    assert!(
        outcome
            .transcript
            .iter()
            .any(|step| step.action == "apply_workspace_patch" && step.verdict == "ALLOW"),
        "the model must replan to a permitted local repair"
    );
    assert!(
        final_pass,
        "the repaired fixture must pass deterministic verification"
    );

    let trace_lines = fs::read_to_string(trace_path)
        .expect("trace")
        .lines()
        .count();
    assert!(
        trace_lines >= 4,
        "expected a decision trace for the workflow"
    );
}
