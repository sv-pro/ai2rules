//! End-to-end governed coding workflow showcase (AI2-12 / #71).
//!
//! This first slice is deliberately deterministic and offline. A tiny model
//! drives the real agent-core loop, receives a governance refusal as a
//! provider-formatted tool result, and changes its next action because of that
//! refusal. The executor performs the final patch for real inside a fresh
//! fixture retained under `target/demo-artifacts/`; a deterministic verifier
//! proves the broken state became fixed and the trace replays without drift.
//!
//! Run:
//!
//! ```text
//! cargo run -p agent-core --example governed_coding_workflow
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use agent_core::{
    default_executor, run, tool_surface, ModelClient, ModelTurn, SessionConfig, TurnContext,
};
use compiler::compile_default;
use harness_types::EffectMode;
use provider_adapters::anthropic::tool_use_block;
use serde_json::json;
use trace_store::{replay, ApprovalStore, TraceStore};
use world_kernel::ExecEnv;

const BROKEN: &str = "def add(a, b):\n    return a - b\n";
const FIXED: &str = "def add(a, b):\n    return a + b\n";
const EXPECTED_DIFF: &str = "--- calc.py (before)\n+++ calc.py (after)\n@@ -1,2 +1,2 @@\n def add(a, b):\n-    return a - b\n+    return a + b\n";

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

fn fixture_test(root: &Path) -> bool {
    fs::read_to_string(root.join("calc.py"))
        .map(|content| content == FIXED)
        .unwrap_or(false)
}

fn fresh_fixture() -> PathBuf {
    let artifact_parent = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/demo-artifacts");
    fs::create_dir_all(&artifact_parent).expect("create demo artifact directory");
    let artifact_parent = fs::canonicalize(artifact_parent).expect("canonical artifact directory");
    tempfile::Builder::new()
        .prefix("governed-coding-workflow-")
        .tempdir_in(artifact_parent)
        .expect("create clean fixture")
        .keep()
}

fn main() {
    let fixture = fresh_fixture();
    fs::write(fixture.join("calc.py"), BROKEN).expect("write broken fixture");

    println!("ai2rules — governed coding workflow showcase");
    println!("task: fix calc.py without treating untrusted context as external authority\n");
    println!(
        "baseline fixture test: {}",
        if fixture_test(&fixture) {
            "PASS"
        } else {
            "FAIL"
        }
    );

    let world = compile_default();
    let projected_tools = tool_surface(&world)
        .into_iter()
        .map(|(action, _)| action.as_str().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    println!("projected tools: {projected_tools}\n");

    let executor = default_executor(&world);
    let trace_path = fixture.join("trace.jsonl");
    let trace = TraceStore::open(&trace_path);
    let mut approvals =
        ApprovalStore::open(fixture.join("approvals.jsonl")).expect("approval store");

    let env = ExecEnv {
        cwd: fixture.clone(),
        readable_roots: vec![fixture.clone()],
        writable_roots: vec![fixture.clone()],
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

    let final_pass = fixture_test(&fixture);
    fs::write(fixture.join("final.diff"), EXPECTED_DIFF).expect("write final diff");
    println!("\nfinal diff:\n{EXPECTED_DIFF}");
    println!(
        "final fixture test: {}",
        if final_pass { "PASS" } else { "FAIL" }
    );
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

    let records = TraceStore::read(&trace_path).expect("read trace artifact");
    let replay_report = replay(&records, &world);
    println!("trace artifact: {}", trace_path.display());
    println!(
        "replay: {}/{} decisions reproduced",
        replay_report.matched, replay_report.total
    );
    assert!(
        replay_report.total >= 4,
        "expected a complete decision trace for the workflow"
    );
    assert_eq!(
        replay_report.total, outcome.records,
        "every recorded workflow decision must enter replay"
    );
    assert!(
        replay_report.is_reproducible(),
        "the workflow decisions must replay without drift"
    );
}
