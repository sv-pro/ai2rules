//! Trace & replay demo — the M1-closing story.
//!
//! Run:
//!
//! ```text
//! cargo run -p trace-store --example trace_demo
//! ```
//!
//! It records a handful of kernel decisions to an append-only trace (with
//! secrets redacted before they touch disk), replays them against the same world
//! to prove they reproduce exactly, then replays against a changed manifest to
//! show policy drift detected as an explicit diff.

use compiler::{compile, compile_default, default_cli_world};
use harness_types::{
    ActionName, ActionType, CallId, ContentHash, ExecutionMode, Provenance, Provider, SessionId,
    SourceChannel, Taint, TaintContext, ToolCall, TraceId, TrustLevel,
};
use serde_json::{json, Value};
use trace_store::{drift_report, record_decision, replay, TraceStore};
use world_kernel::{decide, BudgetUsage, EvalContext};

fn main() {
    let world = compile_default();
    let dir = tempfile::tempdir().expect("sandbox");
    let path = dir.path().join("trace.jsonl");
    let store = TraceStore::open(&path);

    println!();
    println!("  CLI Agent Harness — trace & replay demo");
    println!("  trace: {}\n", path.display());

    // (action, args, channel, taint) — one of each verdict, and a secret to redact.
    let scenarios: Vec<(&str, Value, SourceChannel, Taint)> = vec![
        (
            "read_workspace",
            json!({ "path": "src/lib.rs", "env": { "GITHUB_TOKEN": "ghp_LEAKED_SECRET" } }),
            SourceChannel::UserPrompt,
            Taint::Clean,
        ),
        (
            "write_workspace",
            json!({}),
            SourceChannel::Web,
            Taint::Tainted,
        ),
        (
            "fetch_web",
            json!({ "url": "https://x" }),
            SourceChannel::UserPrompt,
            Taint::Tainted,
        ),
        (
            "start_pty",
            json!({}),
            SourceChannel::UserPrompt,
            Taint::Clean,
        ),
        (
            "send_email",
            json!({}),
            SourceChannel::UserPrompt,
            Taint::Clean,
        ),
    ];

    println!("  1) record decisions");
    for (seq, (action, args, channel, taint)) in scenarios.iter().enumerate() {
        let call = ToolCall {
            action_name: ActionName::new(*action),
            arguments: args.clone(),
            provider: Provider::CliNative,
            call_id: CallId::new("demo"),
            source_perceptions: vec![],
            session_id: SessionId::new("trace-demo"),
        };
        let provenance = Provenance::from_channel(
            *channel,
            SessionId::new("trace-demo"),
            ContentHash::new("h"),
        );
        let ctx = EvalContext {
            taint: TaintContext::from_taint(*taint),
            mode: ExecutionMode::Interactive,
            usage: BudgetUsage::default(),
        };
        let outcome = decide(&world, &call, provenance.clone(), &ctx);
        println!("       seq {seq}: {action:<16} -> {:?}", outcome.decision());
        let record = record_decision(
            &world,
            TraceId::new("demo"),
            seq as u64,
            &call,
            &provenance,
            &ctx,
            &outcome,
        );
        store.append(&record).expect("append");
    }

    println!("\n  2) secrets are redacted before disk");
    let raw = std::fs::read_to_string(&path).unwrap();
    let first = raw.lines().next().unwrap_or_default();
    println!("       first record on disk:\n       {first}");
    println!(
        "       contains the secret token? {}",
        raw.contains("ghp_LEAKED_SECRET")
    );

    println!("\n  3) replay against the same world (determinism)");
    let records = TraceStore::read(&path).unwrap();
    let report = replay(&records, &world);
    println!(
        "       reproduced {}/{} decisions — drift: {}",
        report.matched,
        report.total,
        report.mismatches.len()
    );

    println!("\n  4) replay against a changed manifest (Trusted loses Read)");
    let mut manifest = default_cli_world();
    for grant in &mut manifest.capabilities {
        if grant.trust == TrustLevel::Trusted {
            grant.actions.retain(|a| *a != ActionType::Read);
        }
    }
    let drifted = compile(&manifest).expect("compile drifted world");
    let drift = drift_report(&records, &drifted);
    println!("       {} decision(s) now differ:", drift.mismatches.len());
    for m in &drift.mismatches {
        println!(
            "         seq {}: {:<16} {:?} -> {:?}",
            m.seq,
            m.action.as_str(),
            m.recorded.decision,
            m.recomputed.decision
        );
    }
    println!();
}
