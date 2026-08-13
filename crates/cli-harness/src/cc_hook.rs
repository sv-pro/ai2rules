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
//! - **Additive by default:** it only emits `deny`/`ask`; ALLOW / REPLAN fall
//!   through to Claude Code's normal permission flow (the hook never
//!   auto-allows). With `--grant` (replace mode) ALLOW instead emits an explicit
//!   `allow` that *grants* — bypassing the host's Allow/Deny prompt — so the
//!   manifest becomes the authoritative allowlist, not an overlay. `ABSENT`
//!   passes through too unless `--enforce-absent`: a PreToolUse hook cannot
//!   remove native tools from the host's surface, and denying every tool outside
//!   the manifest would brick the host — so ABSENT-enforcement is an explicit
//!   opt-in.
//! - **Fail-open (documented strategy):** any PROCESS error — unreadable event,
//!   uncompilable world — exits 0 with no output. A broken hook must never brick
//!   a session. A process failure is never an outcome (see `host.rs`).

use crate::hostkit::{
    canonicalize_root_paths, normalize_tool, persist_taint, persist_usage, read_usage,
    resolve_action_path, sanitize,
};
use compiler::{compile, loader::load_yaml, resolve_root_paths};
use harness_preview::{
    gate, host_outcome, BlockKind, GateContext, GateRequest, HostOutcome, ABI_VERSION,
};
use serde_json::{json, Value};
use std::io::Read;
use std::path::Path;

/// Emit a PreToolUse decision (`deny`/`ask`, or `allow` in `--grant` mode) and exit 0.
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

pub fn run(
    world_path: &Path,
    state_dir: &Path,
    mode: &str,
    enforce_absent: bool,
    grant: bool,
) -> i32 {
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
    // Budget counters are session state, carried across calls exactly like taint
    // (finding #16). Unreadable counters are unenforceable ones, so a corrupt
    // sidecar fails closed rather than silently restarting the budget at zero.
    let usage_file = state_dir.join(format!("usage-{}", sanitize(&sid)));
    let carried_usage = read_usage(&usage_file);

    // The project base (for resolving `.`/relative roots + the action path) and $HOME
    // (for `~`), read at the I/O boundary so the compiler/kernel stay pure.
    let base = std::env::var("CLAUDE_PROJECT_DIR").ok().or_else(|| {
        std::env::current_dir()
            .ok()
            .map(|p| p.display().to_string())
    });
    let home = std::env::var("HOME").ok();

    // Compile the world and decide, in-process (D34). Roots paths are resolved to
    // absolute here (env-dependent) before the pure compile.
    let world = match std::fs::read_to_string(world_path)
        .ok()
        .and_then(|c| load_yaml(&c).ok())
        .and_then(|mut m| {
            if let Some(r) = &m.roots {
                let roots = resolve_root_paths(r, home.as_deref(), base.as_deref());
                m.roots = Some(canonicalize_root_paths(&roots));
            }
            compile(&m).ok()
        }) {
        Some(w) => w,
        None => return 0, // fail-open
    };

    // The action's absolute target path for path-scope (roots), if this tool carries
    // one. Bash's `command` is not a path key, so Bash is path-scope-exempt.
    let action_path = base
        .as_deref()
        .and_then(|b| resolve_action_path(&ti, b, home.as_deref()));

    let req = GateRequest {
        v: ABI_VERSION,
        tool: normalize_tool(&world, tool),
        arguments: ti,
        path: action_path,
        context: GateContext {
            session_id: sid,
            mode: Some(mode.to_string()),
            taint: Some(if tainted { "tainted" } else { "clean" }.to_string()),
            source_channel: Some("user_prompt".to_string()),
            approval_token: None,
            usage: carried_usage,
        },
    };
    let res = gate(&world, &req);

    // Persist the kernel-computed monotonic taint for the next call. The note
    // records the host tool and the kernel's effective action (D36).
    //
    // If the marker cannot be written the escalation is invisible to every later
    // call, so the taint floor silently stops engaging (finding #16). That is a
    // governance failure rather than the process failure fail-open covers, so it
    // fails CLOSED — but only for the one call that would escalate, leaving the
    // rest of the session usable.
    // Persist the charged budget counters before anything else can proceed. Same
    // discipline as taint (D59): a counter that silently fails to land is a budget
    // that silently stops counting, which is the failure this closes (finding #16).
    if carried_usage
        .map(|c| c != res.context.usage)
        .unwrap_or(true)
        && !persist_usage(state_dir, &usage_file, &res.context.usage)
    {
        eprintln!(
            "harness cc-hook: cannot record budget counters under {} — refusing the call they would charge",
            state_dir.display()
        );
        emit(
            "deny",
            "budget counters could not be recorded, so this call cannot be counted against the world's limits (untracked_budget)",
        );
    }

    if res.context.taint == "tainted" && !tainted {
        let note = format!("tainted by {tool} ({})", res.action);
        if !persist_taint(state_dir, &taint_file, &note) {
            eprintln!(
                "harness cc-hook: cannot record session taint under {} — refusing the call that would escalate",
                state_dir.display()
            );
            emit(
                "deny",
                "session taint could not be recorded, so this ingestion cannot be governed (untracked_taint)",
            );
        }
    }

    match host_outcome(&res) {
        // ALLOW. Additive default: stay silent (exit 0) and defer to the host's
        // normal permission flow. With `--grant` (replace mode) emit an explicit
        // `allow`, which *grants* — the host skips its Allow/Deny prompt — so the
        // manifest is the authoritative allowlist, not an overlay. An explicit
        // `allow` still cannot override a native deny/ask rule, so replace mode
        // wants an emptied settings.json baseline (docs/demos/replace-permissions).
        HostOutcome::Proceed => {
            if grant {
                emit("allow", &format!("manifest ALLOW: {}", res.action));
            }
            0 // additive default: defer to the host's permission flow
        }
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
