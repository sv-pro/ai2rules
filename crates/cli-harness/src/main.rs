use agent_core::{
    default_executor, run, ApprovalPolicy, ModelClient, ModelTurn, SessionConfig, TranscriptEntry,
    TurnContext,
};
use clap::Parser;
use compiler::{compile, compile_default, loader::load_yaml};
use harness_preview::{gate, GateRequest};
use harness_types::{CompiledWorld, Decision, EffectMode, ExecutionMode, Provenance, ToolCall};
use provider_adapters::anthropic::tool_use_block;
use serde_json::Value;
use std::io::Read;
use std::path::{Path, PathBuf};
use trace_store::{ApprovalStore, TraceStore};
use world_kernel::ExecEnv;

mod cc_hook;
mod mcp_gateway;
mod mock_jira;
mod serve;

/// ai2rules
#[derive(Parser, Debug)]
#[command(author, version, about = "ai2rules", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Run in simulate mode (no side effects)
    #[arg(long)]
    simulate: bool,

    /// Run in background mode (collapses ASK to DENY)
    #[arg(long)]
    background: bool,

    /// Path to world manifest (YAML)
    #[arg(long)]
    world: Option<PathBuf>,
}

#[derive(clap::Subcommand, Debug)]
enum Command {
    /// Launch the World Authoring Tool: a local browser editor for world
    /// manifests, backed by the real compiler + kernel (E11).
    Serve {
        /// Port to bind on 127.0.0.1
        #[arg(long, default_value_t = 8787)]
        port: u16,
    },
    /// Govern one proposed tool call: read a `GateRequest` JSON on stdin and
    /// write the `GateResponse` verdict on stdout — the host-neutral gate ABI
    /// (D24). A host adapter wraps this. See `docs/harness-gate-abi.md`.
    Gate {
        /// Path to the world manifest (YAML/JSON) to govern against.
        #[arg(long)]
        world: PathBuf,
    },
    /// Run a self-contained mock JIRA MCP server on stdio — the demo upstream the
    /// gateway governs (DECISIONS D33 / E16.A).
    MockJira,
    /// Claude Code PreToolUse adapter, in Rust (D33 / E16.C): read a PreToolUse
    /// event on stdin, govern it with the kernel in-process, and emit a deny/ask
    /// decision. Additive (never auto-allows); fail-open. Replaces the Python
    /// `world-gate-adapter.py` — this is the "deep" half governing native tools.
    CcHook {
        /// Path to the world manifest (YAML/JSON) that governs this session.
        #[arg(long)]
        world: PathBuf,
        /// Directory for the per-session taint sidecar.
        #[arg(long, default_value = ".claude/state")]
        state: PathBuf,
        /// Execution mode threaded into every gate call. The kernel itself
        /// collapses ASK→DENY in background (invariant 10).
        #[arg(long, default_value = "interactive", value_parser = ["interactive", "background"])]
        mode: String,
        /// Enforce ABSENT: deny tools the world does not declare (reason
        /// prefixed "ABSENT: "). Default is passthrough — additive dogfooding,
        /// since a PreToolUse hook cannot remove native tools and denying
        /// everything outside the manifest would brick the host.
        #[arg(long)]
        enforce_absent: bool,
    },
    /// Front a real upstream MCP server with the kernel: shape its `tools/list`
    /// (ABSENT) and gate every `tools/call`, forwarding only ALLOW (D33 / E16.B).
    McpGateway {
        /// Path to the world manifest (YAML/JSON) that shapes the surface.
        #[arg(long)]
        world: PathBuf,
        /// Provenance source channel = the proposer's trust (cli|workspace_file|web|…).
        #[arg(long, default_value = "cli")]
        source: String,
        /// Initial carried session taint floor: `clean` (default) | `tainted`.
        #[arg(long, default_value = "clean")]
        taint: String,
        /// Execution mode threaded into every gate call. The kernel itself
        /// collapses ASK→DENY in background (invariant 10).
        #[arg(long, default_value = "interactive", value_parser = ["interactive", "background"])]
        mode: String,
        /// Optional append-only JSONL audit log path.
        #[arg(long)]
        audit: Option<PathBuf>,
        /// Upstream MCP server command (pass after `--`), e.g. `-- harness mock-jira`.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
        upstream: Vec<String>,
    },
}

struct InteractiveModel;

impl ModelClient for InteractiveModel {
    fn next(&mut self, ctx: &TurnContext) -> ModelTurn {
        let mut available_actions: Vec<String> = vec![];
        if let Some(tools_arr) = ctx.tools.as_array() {
            for tool in tools_arr {
                if let Some(name) = tool.get("name").and_then(|n| n.as_str()) {
                    available_actions.push(name.to_string());
                }
            }
        }
        available_actions.sort();
        available_actions.insert(0, "quit".to_string());

        println!("\n[Model] Select an action to propose:");
        let action = match inquire::Select::new("Action:", available_actions).prompt() {
            Ok(s) if s == "quit" => return ModelTurn::Final("User quit".into()),
            Ok(s) => s,
            Err(_) => return ModelTurn::Final("Interrupted".into()),
        };

        // Extract and print the schema for the chosen action to guide the user
        if let Some(tools_arr) = ctx.tools.as_array() {
            if let Some(tool) = tools_arr
                .iter()
                .find(|t| t.get("name").and_then(|n| n.as_str()) == Some(&action))
            {
                if let Some(schema) = tool.get("input_schema") {
                    println!(
                        "  Expected schema: {}",
                        serde_json::to_string(schema).unwrap_or_default()
                    );
                }
            }
        }

        let args_str = match inquire::Text::new("Arguments (JSON):")
            .with_default("{}")
            .prompt()
        {
            Ok(s) => s,
            Err(_) => return ModelTurn::Final("Interrupted".into()),
        };

        let args: Value = serde_json::from_str(&args_str).unwrap_or_else(|_| {
            println!("Invalid JSON. Defaulting to empty object {{}}.");
            serde_json::json!({})
        });
        ModelTurn::ToolUse(tool_use_block("req-1", &action, args))
    }
}

fn ask_approval(call: &ToolCall, _world: &CompiledWorld, _provenance: &Provenance) -> bool {
    println!("\n[APPROVAL REQUIRED]");
    println!("Action: {}", call.action_name);
    println!(
        "Arguments: {}",
        serde_json::to_string_pretty(&call.arguments).unwrap_or_default()
    );

    inquire::Confirm::new("Approve this action?")
        .with_default(false)
        .prompt()
        .unwrap_or(false)
}

fn main() {
    let cli = Cli::parse();

    if let Some(Command::Serve { port }) = cli.command {
        if let Err(e) = serve::run(port) {
            eprintln!("authoring server error: {e}");
            std::process::exit(1);
        }
        return;
    }

    if let Some(Command::Gate { world }) = &cli.command {
        std::process::exit(run_gate(world));
    }

    if let Some(Command::MockJira) = &cli.command {
        std::process::exit(mock_jira::run());
    }

    if let Some(Command::CcHook {
        world,
        state,
        mode,
        enforce_absent,
    }) = &cli.command
    {
        std::process::exit(cc_hook::run(world, state, mode, *enforce_absent));
    }

    if let Some(Command::McpGateway {
        world,
        source,
        taint,
        mode,
        audit,
        upstream,
    }) = &cli.command
    {
        let tainted = taint == "tainted";
        std::process::exit(mcp_gateway::run(
            world,
            upstream,
            source,
            tainted,
            mode,
            audit.as_deref(),
        ));
    }

    let world = if let Some(path) = cli.world {
        let content = std::fs::read_to_string(path).expect("failed to read world file");
        let manifest = load_yaml(&content).expect("failed to parse manifest YAML");
        compile(&manifest).expect("failed to compile world")
    } else {
        compile_default()
    };

    let executor = default_executor(&world);
    // Use a tempdir for sandbox storage in this CLI runner (simulating a session workspace).
    // In a real deployed tool, we'd persist this in `.agents/`.
    let sandbox = tempfile::tempdir().expect("sandbox");
    let trace = TraceStore::open(sandbox.path().join("trace.jsonl"));
    let mut store = ApprovalStore::open(sandbox.path().join("approvals.jsonl")).expect("store");

    let mut model = InteractiveModel;

    let config = SessionConfig {
        effect_mode: if cli.simulate {
            EffectMode::Simulate
        } else {
            EffectMode::Execute
        },
        mode: if cli.background {
            ExecutionMode::Background
        } else {
            ExecutionMode::Interactive
        },
        approval: ApprovalPolicy::Interactive(ask_approval),
        max_steps: 100, // Let the interactive model handle stopping
        ..SessionConfig::default()
    };

    println!("ai2rules initialized.");
    println!("World: {}", world.world_id().as_str());
    println!("Mode: {:?} | Effect: {:?}", config.mode, config.effect_mode);

    let outcome = run(
        &world,
        &ExecEnv::default(),
        &executor,
        &trace,
        &mut store,
        &mut model,
        &config,
        Some(&mut |entry| {
            render_entry(entry);
        }),
    );

    println!("\nSession ended. Final text: {:?}", outcome.final_text);
    println!("{} records appended to trace.", outcome.records);
}

/// The `harness gate` subcommand: compile `world_path`, read one `GateRequest`
/// JSON on stdin, run the pure kernel gate, and write the `GateResponse` on
/// stdout. Returns the process exit code (`docs/harness-gate-abi.md` §5):
/// `0` = evaluated — the verdict (incl. `DENY`/`ASK`) is on stdout; `2` = a
/// malformed request or an unreadable/uncompilable manifest; `1` = internal
/// error. The verdict is never encoded in the exit code (D24): mapping it to a
/// host's decision shape is the adapter's job.
fn run_gate(world_path: &Path) -> i32 {
    let content = match std::fs::read_to_string(world_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("gate: cannot read world {}: {e}", world_path.display());
            return 2;
        }
    };
    let manifest = match load_yaml(&content) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("gate: cannot parse world {}: {e}", world_path.display());
            return 2;
        }
    };
    let world = match compile(&manifest) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("gate: cannot compile world {}: {e}", world_path.display());
            return 2;
        }
    };

    let mut buf = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut buf) {
        eprintln!("gate: cannot read stdin: {e}");
        return 1;
    }
    let req: GateRequest = match serde_json::from_str(&buf) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("gate: malformed GateRequest: {e}");
            return 2;
        }
    };

    let response = gate(&world, &req);
    match serde_json::to_string(&response) {
        Ok(json) => {
            println!("{json}");
            0
        }
        Err(e) => {
            eprintln!("gate: cannot serialize response: {e}");
            1
        }
    }
}

fn render_entry(entry: &TranscriptEntry) {
    println!("\n▶ Action: {}", entry.action);
    if let Some(decision) = entry.decision {
        println!("  Decision: {}", decision_label(decision));
        if let Some(rule) = entry.rule.as_deref() {
            println!("  Rule:     {rule}");
        }
        if let Some(effect) = entry.effect_mode {
            println!("  Effect:   {effect:?}");
        }
        if entry.verdict != decision_name(decision) {
            println!("  Flow:     {}", entry.verdict);
        }
        println!("  Feedback: {}", feedback(decision, entry.rule.as_deref()));
    } else {
        println!("  Verdict:  {}", entry.verdict);
    }
    println!("  Result:   {} [taint: {:?}]", entry.result, entry.taint);
}

fn decision_name(decision: Decision) -> &'static str {
    match decision {
        Decision::Absent => "ABSENT",
        Decision::Allow => "ALLOW",
        Decision::Deny => "DENY",
        Decision::Ask => "ASK",
        Decision::Replan => "REPLAN",
    }
}

fn decision_label(decision: Decision) -> String {
    let code = match decision {
        Decision::Allow => "32;1",
        Decision::Deny => "31;1",
        Decision::Ask => "33;1",
        Decision::Absent => "35;1",
        Decision::Replan => "36;1",
    };
    format!("\x1b[{code}m{}\x1b[0m", decision_name(decision))
}

fn feedback(decision: Decision, rule: Option<&str>) -> &'static str {
    match (decision, rule.unwrap_or("")) {
        (Decision::Absent, "unknown_to_ontology") => {
            "action is not in the ontology; no intent or executor path exists"
        }
        (Decision::Absent, "absent") => "action exists but is not projected into this world",
        (Decision::Absent, "capability") => {
            "current trust/capability context cannot see this action"
        }
        (Decision::Absent, _) => "action is unavailable in this world/context",
        (Decision::Deny, "background_denies_ask") => {
            "approval-required action failed closed in background mode"
        }
        (Decision::Deny, "taint_invariant") => "tainted input cannot cross this effect boundary",
        (Decision::Deny, _) => "policy blocked a visible action",
        (Decision::Ask, "approval_required") => "human approval is required before execution",
        (Decision::Ask, "approval_rejected") => "approval was rejected; action did not execute",
        (Decision::Ask, _) => "approval flow paused execution",
        (Decision::Replan, _) => "budget or scope exceeded; propose a smaller step",
        (Decision::Allow, _) => "execution spec crossed the boundary",
    }
}
