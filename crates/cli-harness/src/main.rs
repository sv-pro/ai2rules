use agent_core::{
    default_executor, run, ApprovalPolicy, ModelClient, ModelTurn, SessionConfig, TurnContext,
};
use clap::Parser;
use compiler::{compile, compile_default, loader::load_yaml};
use harness_types::{CompiledWorld, EffectMode, ExecutionMode, Provenance, ToolCall};
use provider_adapters::anthropic::tool_use_block;
use serde_json::Value;
use std::path::PathBuf;
use trace_store::{ApprovalStore, TraceStore};
use world_kernel::ExecEnv;

/// CLI Agent Harness
#[derive(Parser, Debug)]
#[command(author, version, about = "CLI Agent Harness (E9)", long_about = None)]
struct Cli {
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
    };

    println!("CLI Agent Harness initialized.");
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
            println!("\n▶ Action: {}", entry.action);
            println!("  Verdict: {}", entry.verdict);
            println!("  Result:  {} [taint: {:?}]", entry.result, entry.taint);
        }),
    );

    println!("\nSession ended. Final text: {:?}", outcome.final_text);
    println!("{} records appended to trace.", outcome.records);
}
