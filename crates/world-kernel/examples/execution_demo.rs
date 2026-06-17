//! Real-execution demo — the full round-trip with actual side effects, confined
//! to a throwaway sandbox directory.
//!
//! Run:
//!
//! ```text
//! cargo run -p world-kernel --example execution_demo
//! ```
//!
//! Where `kernel_demo` shows the *verdicts* (and runs allowed actions in
//! SIMULATE), this one wires `decide -> build_execution_spec(Execute) ->
//! executor.run` and lets allowed actions really happen: it reads a real file,
//! writes one, runs a real command — then shows the boundary *refusing* a write
//! that escapes the sandbox, a stale (drifted) descriptor, and a command that
//! overruns its timeout.
//!
//! Safety: all writes are pinned to a `tempfile` sandbox and commands are
//! offline (`echo`, `sleep`). Note that `NetworkPolicy::Disabled` is carried in
//! the spec but not yet OS-enforced — true isolation is E8.

use compiler::{compile, compile_default, default_cli_world};
use executor::{CommandHandler, ExecError, ExecOutput, Executor, PatchHandler, ReadHandler};
use harness_types::{
    ActionName, CallId, CompiledWorld, ContentHash, Decision, DescriptorHash, EffectMode,
    ExecutionMode, NetworkPolicy, Provenance, Provider, SessionId, SourceChannel, Taint,
    TaintContext, ToolCall, TraceId,
};
use serde_json::{json, Value};
use world_kernel::{
    build_execution_spec, decide, BudgetUsage, EvalContext, ExecEnv, KernelOutcome,
};

fn main() {
    let sandbox = tempfile::tempdir().expect("create sandbox");
    let root = sandbox.path();
    let notes = root.join("notes.txt");
    std::fs::write(&notes, "hello from the sandbox\n").expect("seed file");

    let world = compile_default();
    let executor = build_executor(&world);
    let env = ExecEnv {
        cwd: root.to_path_buf(),
        writable_roots: vec![root.to_path_buf()],
        readable_roots: vec![root.to_path_buf()],
        env_allowlist: vec!["PATH".to_string()],
        network: NetworkPolicy::Disabled,
        default_timeout_ms: 5_000,
    };

    println!();
    println!("  CLI Agent Harness — real-execution demo");
    println!("  sandbox: {}", root.display());
    println!("  Allowed actions really run here; writes can't escape this dir.\n");

    println!("  ── allowed actions (executed for real) ──\n");

    run(
        &world,
        &executor,
        &env,
        "Read a real file",
        "read_workspace",
        json!({ "path": notes.to_str().unwrap() }),
    );

    let created = root.join("created.txt");
    run(
        &world,
        &executor,
        &env,
        "Write a file via apply_patch",
        "apply_patch",
        json!({ "path": created.to_str().unwrap(), "contents": "written by the agent\n" }),
    );
    println!(
        "    proof    : {} now exists = {}\n",
        created.display(),
        created.exists()
    );

    run(
        &world,
        &executor,
        &env,
        "Run a real command",
        "run_command",
        json!({ "command": "echo", "args": ["governed!"] }),
    );

    println!("  ── refusals at the boundary (no LLM involved) ──\n");

    run(
        &world,
        &executor,
        &env,
        "Write outside the sandbox (escape attempt)",
        "apply_patch",
        json!({ "path": "/tmp/escape.txt", "contents": "pwned" }),
    );

    drift_refusal(&world, &env, &notes);
    timeout_refusal(&env);
}

/// Decide, lower an ALLOW to a spec, and execute it for real.
fn run(
    world: &CompiledWorld,
    executor: &Executor,
    env: &ExecEnv,
    title: &str,
    action: &str,
    args: Value,
) {
    println!("• {title}");
    println!("    proposes : {action}({})", compact(&args));

    let outcome = decide(world, &call(action, args), user_prompt(), &clean_ctx());
    match outcome {
        KernelOutcome::Evaluated {
            intent,
            disposition,
        } if disposition.decision == Decision::Allow => {
            match build_execution_spec(world, &intent, EffectMode::Execute, env, TraceId::new("x"))
            {
                Ok(spec) => match executor.run(&spec) {
                    Ok(result) => println!(
                        "    EXECUTE  : {}  [taint: {:?}]",
                        describe(&result.value),
                        result.taint
                    ),
                    Err(e) => println!("    REFUSED  : {e}"),
                },
                Err(e) => println!("    spec err : {e}"),
            }
        }
        other => println!("    verdict  : {:?} — not executed", other.decision()),
    }
    println!();
}

/// Register a handler under a *stale* descriptor hash to mimic an upstream
/// rug-pull, then run a normally-allowed read: the boundary blocks it on drift.
fn drift_refusal(world: &CompiledWorld, env: &ExecEnv, notes: &std::path::Path) {
    println!("• Stale descriptor (rug-pull) blocked before the handler");
    let drifted = Executor::builder()
        .register(
            ActionName::new("read_workspace"),
            DescriptorHash::new("hash-from-yesterday"),
            Box::new(ReadHandler),
        )
        .build();

    let args = json!({ "path": notes.to_str().unwrap() });
    println!("    proposes : read_workspace({})", compact(&args));
    if let KernelOutcome::Evaluated { intent, .. } = decide(
        world,
        &call("read_workspace", args),
        user_prompt(),
        &clean_ctx(),
    ) {
        let spec =
            build_execution_spec(world, &intent, EffectMode::Execute, env, TraceId::new("d"))
                .expect("spec");
        match drifted.run(&spec) {
            Err(ExecError::DescriptorDrift { .. }) => {
                println!("    REFUSED  : descriptor drift — handler never ran")
            }
            other => println!("    unexpected: {other:?}"),
        }
    }
    println!();
}

/// A command that overruns the world's command timeout is killed.
fn timeout_refusal(env: &ExecEnv) {
    println!("• Command that overruns its timeout is killed");
    // A world whose command budget is a short 200 ms (descriptor hashes are
    // budget-independent, so the main executor still matches).
    let mut manifest = default_cli_world();
    manifest.budget.command_timeout_ms = Some(200);
    let fast = compile(&manifest).expect("compile fast world");
    let executor = build_executor(&fast);

    let args = json!({ "command": "sleep", "args": ["5"] });
    println!(
        "    proposes : run_command({})  [budget: 200ms]",
        compact(&args)
    );
    if let KernelOutcome::Evaluated { intent, .. } = decide(
        &fast,
        &call("run_command", args),
        user_prompt(),
        &clean_ctx(),
    ) {
        let spec =
            build_execution_spec(&fast, &intent, EffectMode::Execute, env, TraceId::new("t"))
                .expect("spec");
        match executor.run(&spec) {
            Err(ExecError::Timeout { timeout_ms }) => {
                println!("    REFUSED  : killed after {timeout_ms}ms")
            }
            other => println!("    unexpected: {other:?}"),
        }
    }
    println!();
}

fn build_executor(world: &CompiledWorld) -> Executor {
    let hash = |a: &str| {
        world
            .descriptor_hash(&ActionName::new(a))
            .cloned()
            .unwrap_or_default()
    };
    Executor::builder()
        .register(
            ActionName::new("read_workspace"),
            hash("read_workspace"),
            Box::new(ReadHandler),
        )
        .register(
            ActionName::new("apply_patch"),
            hash("apply_patch"),
            Box::new(PatchHandler),
        )
        .register(
            ActionName::new("run_command"),
            hash("run_command"),
            Box::new(CommandHandler),
        )
        .build()
}

fn call(action: &str, args: Value) -> ToolCall {
    ToolCall {
        action_name: ActionName::new(action),
        arguments: args,
        provider: Provider::CliNative,
        call_id: CallId::new("demo"),
        source_perceptions: vec![],
        session_id: SessionId::new("exec-demo"),
    }
}

fn user_prompt() -> Provenance {
    Provenance::from_channel(
        SourceChannel::UserPrompt,
        SessionId::new("exec-demo"),
        ContentHash::new("demo"),
    )
}

fn clean_ctx() -> EvalContext {
    EvalContext {
        taint: TaintContext::from_taint(Taint::Clean),
        mode: ExecutionMode::Interactive,
        usage: BudgetUsage::default(),
    }
}

fn describe(output: &ExecOutput) -> String {
    match output {
        ExecOutput::FileContents(c) => format!("read {} bytes: {:?}", c.len(), c.trim()),
        ExecOutput::CommandResult {
            exit_code, stdout, ..
        } => format!("exit {exit_code}, stdout {:?}", stdout.trim()),
        ExecOutput::PatchApplied { path } => format!("wrote {}", path.display()),
        ExecOutput::Simulated(note) => format!("(simulated) {note}"),
    }
}

fn compact(args: &Value) -> String {
    serde_json::to_string(args).unwrap_or_default()
}
