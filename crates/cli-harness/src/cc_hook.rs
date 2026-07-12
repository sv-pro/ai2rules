//! `harness cc-hook` — the Claude Code **PreToolUse adapter, in Rust** (DECISIONS
//! D33 / D34 / D37, E16.C). It reads a PreToolUse event on stdin, maps it onto a
//! `GateRequest`, runs the real kernel via `gate()` **in-process** (no
//! subprocess), persists monotonic per-session taint in a sidecar, and emits a
//! PreToolUse decision.
//!
//! This is a **thin adapter**: it translates the host event shape, restores and
//! persists session taint, passes the execution mode, and maps the verdict via
//! the shared [`host_outcome`] layer. It holds **no governance logic** — no
//! policy, no taint algebra, no command classification. Bash commands are
//! classified by the *kernel* from the world's `command_classes` (D36); the
//! adapter sends the raw host tool name.
//!
//! - **Additive by default:** it only ever emits `deny`/`ask`; ALLOW / REPLAN
//!   fall through to Claude Code's normal permission flow (the hook never
//!   auto-allows). `ABSENT` passes through too unless `--enforce-absent`: a
//!   PreToolUse hook cannot remove native tools from the host's surface, and
//!   denying every tool outside the manifest would brick the host — so
//!   ABSENT-enforcement is an explicit opt-in.
//! - **Fail-open (documented strategy):** any PROCESS error — unreadable event,
//!   uncompilable world — exits 0 with no output. A broken hook must never brick
//!   a session. A process failure is never an outcome (see `host.rs`).

use compiler::{compile, loader::load_yaml};
use harness_preview::{
    gate, host_outcome, BlockKind, GateContext, GateRequest, HostOutcome, ABI_VERSION,
};
use harness_types::ActionName;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::Path;

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

/// Host-tool-name normalization — a *mapping*, not policy: use the exact host
/// tool name if the world's ontology declares it; else its lowercase form if
/// that is declared; else unchanged (the kernel will report it ABSENT).
fn normalize(world: &harness_types::CompiledWorld, tool: &str) -> String {
    if world.in_ontology(&ActionName::new(tool)) {
        return tool.to_string();
    }
    let lower = tool.to_lowercase();
    if world.in_ontology(&ActionName::new(&lower)) {
        return lower;
    }
    tool.to_string()
}

pub fn run(world_path: &Path, state_dir: &Path, mode: &str, enforce_absent: bool) -> i32 {
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

    // Compile the world and decide, in-process (D34).
    let world = match std::fs::read_to_string(world_path)
        .ok()
        .and_then(|c| load_yaml(&c).ok())
        .and_then(|m| compile(&m).ok())
    {
        Some(w) => w,
        None => return 0, // fail-open
    };

    let req = GateRequest {
        v: ABI_VERSION,
        tool: normalize(&world, tool),
        arguments: ti,
        context: GateContext {
            session_id: sid,
            mode: Some(mode.to_string()),
            taint: tainted.then(|| "tainted".to_string()),
            source_channel: None,
            approval_token: None,
        },
    };
    let res = gate(&world, &req);

    // Persist the kernel-computed monotonic taint for the next call. The note
    // records the host tool and the kernel's effective action (D36).
    if res.context.taint == "tainted" && !tainted {
        let _ = std::fs::create_dir_all(state_dir);
        if let Ok(mut f) = std::fs::File::create(&taint_file) {
            let _ = writeln!(f, "tainted by {tool} ({})", res.action);
        }
    }

    match host_outcome(&res) {
        HostOutcome::Proceed => 0, // passthrough — the host runs the tool
        HostOutcome::NeedsApproval { reason } => emit("ask", &reason),
        HostOutcome::Block {
            kind: BlockKind::Deny,
            reason,
        } => emit("deny", &reason),
        HostOutcome::Block {
            kind: BlockKind::Absent,
            reason,
        } => {
            if enforce_absent {
                emit("deny", &format!("ABSENT: {reason}"));
            }
            0 // additive dogfooding default: fall through to the host's flow
        }
        HostOutcome::Block {
            kind: BlockKind::Replan,
            reason: _,
        } => 0, // no host channel for "smaller step" — fall through
    }
}
