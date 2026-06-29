//! `harness cc-hook` — the Claude Code **PreToolUse adapter, in Rust** (DECISIONS
//! D33 / E16.C). Replaces the Python `world-gate-adapter.py`: it reads a PreToolUse
//! event on stdin, maps it onto a `GateRequest`, runs the real kernel via `gate()`
//! **in-process** (no subprocess), persists monotonic per-session taint in a
//! sidecar, and emits a PreToolUse decision.
//!
//! - **Additive:** it only ever `deny`/`ask`. ALLOW / ABSENT / REPLAN fall through
//!   to Claude Code's normal permission flow (the hook never auto-allows).
//! - **Fail-open:** any error → exit 0. A broken hook must never brick a session.
//!
//! This is the "deep" half of the governability demo: it governs Claude Code's
//! **native** tools (Bash / Edit / Write / Read / WebFetch) — the layer Copilot
//! does not expose.

use compiler::{compile, loader::load_yaml};
use harness_preview::{gate, GateContext, GateRequest, ABI_VERSION};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::Path;

// Host-syntactic Bash classification (D25) — patterns, not policy; the policy for
// each resulting action lives in the world manifest.
const EGRESS: [&str; 8] = [
    "curl ", "wget ", "nc ", "ncat ", "telnet ", "ssh ", "scp ", "sftp ",
];
const DESTRUCTIVE: [&str; 6] = ["rm -rf", "rm -fr", "sudo ", "mkfs", "dd if=", ":(){"];

/// Map a host tool + input onto a manifest action name. The only place a host quirk
/// (Bash being one tool with many effects) is normalized.
fn classify(tool: &str, ti: &Value) -> String {
    if tool != "Bash" {
        return tool.to_string();
    }
    let cmd = ti.get("command").and_then(|c| c.as_str()).unwrap_or("");
    if EGRESS.iter().any(|p| cmd.contains(p)) {
        return "Bash_network".to_string();
    }
    if DESTRUCTIVE.iter().any(|p| cmd.contains(p)) {
        return "Bash_destructive".to_string();
    }
    "Bash".to_string()
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "_.-".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Emit a PreToolUse decision (only used for deny/ask) and exit 0.
fn emit(decision: &str, reason: &str) -> ! {
    println!(
        "{}",
        json!({"hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": decision,
            "permissionDecisionReason": reason,
        }})
    );
    std::process::exit(0);
}

pub fn run(world_path: &Path, state_dir: &Path) -> i32 {
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        return 0; // fail-open
    }
    let ev: Value = match serde_json::from_str(&input) {
        Ok(v) => v,
        Err(_) => return 0, // fail-open
    };

    let tool = ev.get("tool_name").and_then(|t| t.as_str()).unwrap_or("");
    let ti = ev.get("tool_input").cloned().unwrap_or_else(|| json!({}));
    let ti = if ti.is_object() { ti } else { json!({}) };
    let sid = ev
        .get("session_id")
        .and_then(|s| s.as_str())
        .unwrap_or("default")
        .to_string();

    let taint_file = state_dir.join(format!("taint-{}", sanitize(&sid)));
    let tainted = taint_file.exists();

    // Compile the world and decide, in-process.
    let world = match std::fs::read_to_string(world_path)
        .ok()
        .and_then(|c| load_yaml(&c).ok())
        .and_then(|m| compile(&m).ok())
    {
        Some(w) => w,
        None => return 0, // fail-open
    };

    let action = classify(tool, &ti);
    let req = GateRequest {
        v: ABI_VERSION,
        tool: action.clone(),
        arguments: ti,
        context: GateContext {
            session_id: sid,
            mode: Some("interactive".to_string()),
            taint: tainted.then(|| "tainted".to_string()),
            source_channel: None,
            approval_token: None,
        },
    };
    let res = gate(&world, &req);

    // Persist the kernel-computed monotonic taint for the next call.
    if res.context.taint == "tainted" && !tainted {
        let _ = std::fs::create_dir_all(state_dir);
        if let Ok(mut f) = std::fs::File::create(&taint_file) {
            let _ = writeln!(f, "tainted by {tool} ({action})");
        }
    }

    match res.decision.as_str() {
        "DENY" => emit("deny", &res.reason),
        "ASK" => emit("ask", &res.reason),
        _ => 0, // passthrough — ALLOW / ABSENT / REPLAN
    }
}
