//! `harness demo` — a bounded, effect-free first proof over the real compiled world.
//!
//! The command intentionally does not execute host tools. It exercises the same pure
//! host-neutral gate used by live adapters, records every request/response pair, and
//! immediately replays the requests to prove deterministic decisions. That makes it
//! safe to run after `harness init` with no credentials, network, or live model.

use crate::hostkit;
use harness_preview::{gate, GateContext, GateRequest, GateResponse, GateUsage, ABI_VERSION};
use harness_types::CompiledWorld;
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const DEFAULT_WORLD: &str = ".claude/cc-world.yaml";

struct DemoCase {
    name: &'static str,
    request: GateRequest,
    expected: &'static str,
}

/// Run the first-proof demo. Returns a process exit code.
pub fn run(world_override: Option<&Path>) -> i32 {
    let world_path = world_override
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_WORLD));

    if !world_path.exists() {
        eprintln!(
            "demo: {} not found; run `harness init` in this project first",
            world_path.display()
        );
        return 2;
    }

    let world = match hostkit::load_compiled_world(&world_path) {
        Ok(world) => world,
        Err(e) => {
            eprintln!("demo: {e}");
            return 2;
        }
    };

    let cwd = match std::env::current_dir().and_then(fs::canonicalize) {
        Ok(path) => path,
        Err(e) => {
            eprintln!("demo: cannot resolve current project directory: {e}");
            return 1;
        }
    };
    let local_target = match hostkit::canonicalize_action_path(&cwd.join("ai2rules-demo-local.txt")) {
        Some(path) => hostkit::path_to_string(path),
        None => {
            eprintln!("demo: cannot resolve a safe workspace-local test path");
            return 1;
        }
    };
    let outside_target = match outside_target(&cwd) {
        Some(path) => path,
        None => {
            eprintln!("demo: cannot resolve a safe out-of-root test path");
            return 1;
        }
    };

    let cases = vec![
        DemoCase {
            name: "workspace-local write",
            request: request(
                "Write",
                json!({"file_path": "ai2rules-demo-local.txt", "content": "not executed"}),
                Some(local_target),
                "interactive",
                "clean",
            ),
            expected: "ALLOW",
        },
        DemoCase {
            name: "out-of-root write (background fail-closed)",
            request: request(
                "Write",
                json!({"file_path": outside_target, "content": "not executed"}),
                Some(outside_target.clone()),
                "background",
                "clean",
            ),
            expected: "DENY",
        },
        DemoCase {
            name: "tainted input -> network effect",
            request: request(
                "WebFetch",
                json!({"url": "https://example.invalid/ai2rules-demo"}),
                None,
                "interactive",
                "tainted",
            ),
            expected: "DENY",
        },
        DemoCase {
            name: "approval-required destructive command (interactive)",
            request: request(
                "Bash",
                json!({"command": "rm -rf ./ai2rules-demo-never-run"}),
                None,
                "interactive",
                "clean",
            ),
            expected: "ASK",
        },
        DemoCase {
            name: "approval-required destructive command (background)",
            request: request(
                "Bash",
                json!({"command": "rm -rf ./ai2rules-demo-never-run"}),
                None,
                "background",
                "clean",
            ),
            expected: "DENY",
        },
    ];

    let trace_path = match prepare_trace_path() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("demo: cannot create trace: {e}");
            return 1;
        }
    };

    println!("ai2rules first proof");
    println!("World: {}", world.world_id().as_str());
    println!("Effect mode: decision-only (no tool is executed)");
    println!("Trace: {}", trace_path.display());

    let mut observed = Vec::with_capacity(cases.len());
    for (index, case) in cases.into_iter().enumerate() {
        let response = gate(&world, &case.request);
        if response.decision != case.expected {
            eprintln!(
                "demo: scenario `{}` expected {} but kernel returned {} ({:?})",
                case.name, case.expected, response.decision, response.rule
            );
            return 1;
        }
        if let Err(e) = append_trace(&trace_path, index + 1, case.name, &case.request, &response) {
            eprintln!("demo: cannot append trace: {e}");
            return 1;
        }
        print_result(index + 1, case.name, &response, &trace_path);
        observed.push((case.request, response));
    }

    let matched = observed
        .iter()
        .filter(|(request, recorded)| equivalent(recorded, &gate(&world, request)))
        .count();
    if matched != observed.len() {
        eprintln!(
            "demo: replay mismatch: {matched}/{} decisions reproduced",
            observed.len()
        );
        return 1;
    }

    println!("\nReplay: {matched}/{} decisions reproduced", observed.len());
    println!("Next: harness doctor");
    0
}

fn request(
    tool: &str,
    arguments: Value,
    path: Option<String>,
    mode: &str,
    taint: &str,
) -> GateRequest {
    GateRequest {
        v: ABI_VERSION,
        tool: tool.to_string(),
        arguments,
        path,
        context: GateContext {
            session_id: "first-proof".to_string(),
            mode: Some(mode.to_string()),
            taint: Some(taint.to_string()),
            source_channel: Some("user_prompt".to_string()),
            approval_token: None,
            usage: Some(GateUsage::default()),
        },
    }
}

fn outside_target(cwd: &Path) -> Option<String> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);

    if let Some(home) = home {
        if let Some(parent) = home.parent() {
            let candidate = parent.join(format!("ai2rules-demo-outside-{}", std::process::id()));
            if !candidate.starts_with(cwd) {
                if let Some(path) = hostkit::canonicalize_action_path(&candidate) {
                    return Some(hostkit::path_to_string(path));
                }
            }
        }
    }

    let parent = cwd.parent()?;
    let candidate = parent.join(format!("ai2rules-demo-outside-{}", std::process::id()));
    hostkit::canonicalize_action_path(&candidate).map(hostkit::path_to_string)
}

fn prepare_trace_path() -> std::io::Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!("ai2rules-demo-{}", std::process::id()));
    fs::create_dir_all(&dir)?;
    let path = dir.join("trace.jsonl");
    fs::write(&path, b"")?;
    Ok(path)
}

fn append_trace(
    path: &Path,
    seq: usize,
    scenario: &str,
    request: &GateRequest,
    response: &GateResponse,
) -> std::io::Result<()> {
    let record = json!({
        "seq": seq,
        "scenario": scenario,
        "request": {
            "v": request.v,
            "tool": request.tool,
            "arguments": request.arguments,
            "path": request.path,
            "context": {
                "session_id": request.context.session_id,
                "mode": request.context.mode,
                "taint": request.context.taint,
                "source_channel": request.context.source_channel,
                "usage": request.context.usage,
            }
        },
        "response": response,
    });
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, &record).map_err(std::io::Error::other)?;
    writeln!(file)?;
    Ok(())
}

fn print_result(index: usize, name: &str, response: &GateResponse, trace: &Path) {
    println!("\n{index}. {name}");
    println!("   verdict: {}", response.decision);
    println!("   rule: {}", response.rule.as_deref().unwrap_or("-"));
    println!("   reason: {}", response.reason);
    println!("   manifest: {}", response.manifest_hash);
    println!("   trace: {}", trace.display());
}

fn equivalent(a: &GateResponse, b: &GateResponse) -> bool {
    a.decision == b.decision
        && a.action == b.action
        && a.rule == b.rule
        && a.reason == b.reason
        && a.context.taint == b.context.taint
        && a.context.usage == b.context.usage
        && a.manifest_hash == b.manifest_hash
}
