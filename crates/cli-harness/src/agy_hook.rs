//! `harness agy-hook` — the **Antigravity CLI (`agy`) PreToolUse adapter, in
//! Rust** (D48). Third live host after Claude Code (`cc-hook`, D37) and OpenCode
//! (D35). It reads an Antigravity `PreToolUse` payload on stdin, maps it onto a
//! `GateRequest`, runs the real kernel via `gate()` **in-process** (no
//! subprocess), persists monotonic per-conversation taint in a sidecar, and
//! emits an Antigravity hook decision.
//!
//! Like `cc_hook`, this is a **thin adapter**: it translates the host event
//! shape, restores and persists session taint, passes the execution mode, and
//! maps the verdict via the shared [`host_outcome`] layer. It holds **no
//! governance logic** — no policy, no taint algebra, no command classification.
//! Shell commands are classified by the *kernel* from the world's
//! `command_classes` (D36).
//!
//! ## What is host-specific here (all shape, no policy)
//!
//! Antigravity's hook ABI differs from Claude Code's in four ways that this
//! adapter — and only this adapter — absorbs:
//!
//! 1. **Envelope.** protojson camelCase: the tool call is
//!    `toolCall: {name, args}`, not `tool_name`/`tool_input`.
//! 2. **Session key.** `conversationId`, not `session_id`.
//! 3. **Argument casing.** Antigravity tool arguments are PascalCase and
//!    host-specific (`CommandLine`, `AbsolutePath`, `TargetFile`, `Query`). The
//!    shared world data — `command_classes` (D36) classifies the `command` arg,
//!    path-scope reads `path`/`file_path` — is deliberately host-neutral, so the
//!    adapter *aliases* those keys into the neutral vocabulary before gating.
//!    Originals are preserved: aliasing only adds keys, so nothing a rule or an
//!    audit record might reference is lost.
//! 4. **Project base.** Antigravity supplies `workspacePaths` (and a per-call
//!    `Cwd`) in the payload itself, so the base for relative-path resolution is
//!    read from the event rather than from an environment variable.
//!
//! ## Decision channel
//!
//! Antigravity expects a JSON object on stdout with a `decision` of `allow` /
//! `deny` / `ask` / `force_ask`. A response carrying **no** `decision` is a
//! no-op — the host proceeds with its own permission flow — which is this
//! adapter's additive passthrough (`{}`).
//!
//! - **Additive by default:** it emits `deny`/`force_ask`; ALLOW / REPLAN fall
//!   through to Antigravity's normal permission flow (the hook never
//!   auto-allows). With `--grant` (replace mode) ALLOW instead emits an explicit
//!   `allow` that *grants* — bypassing the host's prompt — so the manifest
//!   becomes the authoritative allowlist, not an overlay. `ABSENT` passes
//!   through too unless `--enforce-absent`: a PreToolUse hook cannot remove
//!   native tools from the host's surface, and denying every tool outside the
//!   manifest would brick the host — so ABSENT-enforcement is an explicit
//!   opt-in.
//! - **ASK maps to `force_ask`, not `ask`.** Antigravity's `ask` respects the
//!   host's cached "Always Allow" grants; `force_ask` always prompts. A kernel
//!   ASK verdict means *a human must decide this time*, and a stale cached grant
//!   silently satisfying it would void the approval guarantee — so the strict
//!   channel is the default. `--soft-ask` opts back into cache-respecting `ask`
//!   for a less intrusive dogfooding posture.
//! - **Fail-open (documented strategy):** any PROCESS error — unreadable event,
//!   uncompilable world — exits 0 emitting `{}` (the no-decision no-op). A
//!   broken hook must never brick a session. A process failure is never an
//!   outcome (see `host.rs`).

use crate::hostkit::{
    canonicalize_root_paths, normalize_tool, persist_taint, resolve_action_path, sanitize,
};
use compiler::{compile, loader::load_yaml, resolve_root_paths};
use harness_preview::{
    gate, host_outcome, BlockKind, GateContext, GateRequest, HostOutcome, ABI_VERSION,
};
use serde_json::{json, Map, Value};
use std::io::Read;
use std::path::Path;

/// Antigravity's no-op response: valid JSON carrying no `decision`, so the host
/// continues with its own permission flow. Also the fail-open response.
fn passthrough() -> i32 {
    println!("{}", json!({}));
    0
}

/// Emit an Antigravity hook decision and exit 0.
fn emit(decision: &str, reason: &str) -> ! {
    println!("{}", json!({ "decision": decision, "reason": reason }));
    std::process::exit(0);
}

/// Antigravity tool arguments are PascalCase and host-specific; the shared world
/// data is host-neutral. Alias the known argument keys into the neutral
/// vocabulary the kernel reads, **preserving the originals**.
///
/// | Antigravity | neutral | why |
/// |---|---|---|
/// | `CommandLine` | `command` | `command_classes` (D36) classifies the `command` arg |
/// | `AbsolutePath`, `TargetFile`, `FilePath` | `path` | path-scope (roots) |
/// | `Url` | `url` | url-shaped rules |
///
/// An existing neutral key is never overwritten — a host that already speaks the
/// neutral vocabulary wins over the alias.
fn alias_neutral_args(args: &Value) -> Value {
    const ALIASES: [(&str, &str); 5] = [
        ("CommandLine", "command"),
        ("AbsolutePath", "path"),
        ("TargetFile", "path"),
        ("FilePath", "path"),
        ("Url", "url"),
    ];
    let mut out: Map<String, Value> = match args.as_object() {
        Some(m) => m.clone(),
        None => Map::new(),
    };
    for (host_key, neutral) in ALIASES {
        if out.contains_key(neutral) {
            continue;
        }
        if let Some(v) = args.get(host_key).cloned() {
            out.insert(neutral.to_string(), v);
        }
    }
    Value::Object(out)
}

/// The project base for resolving relative paths: Antigravity reports it in the
/// payload (`workspacePaths[0]`, else the call's `Cwd`), so the adapter prefers
/// the event over the process environment. Falls back to `$PWD` for a bare
/// invocation (e.g. a piped test).
fn project_base(ev: &Value, args: &Value) -> Option<String> {
    ev.get("workspacePaths")
        .and_then(|w| w.as_array())
        .and_then(|a| a.first())
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| args.get("Cwd").and_then(|c| c.as_str()).map(str::to_string))
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .map(|p| p.display().to_string())
        })
}

pub fn run(
    world_path: &Path,
    state_dir: &Path,
    mode: &str,
    enforce_absent: bool,
    grant: bool,
    soft_ask: bool,
) -> i32 {
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        return passthrough(); // fail-open
    }
    let ev: Value = match serde_json::from_str(&input) {
        Ok(v) => v,
        Err(_) => return passthrough(), // fail-open
    };

    // (1) envelope: `toolCall: {name, args}`.
    let tool = ev
        .get("toolCall")
        .and_then(|t| t.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("");
    let raw_args = ev
        .get("toolCall")
        .and_then(|t| t.get("args"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let raw_args = if raw_args.is_object() {
        raw_args
    } else {
        json!({})
    };
    // (3) argument casing: alias PascalCase into the neutral vocabulary.
    let args = alias_neutral_args(&raw_args);

    // (2) session key: `conversationId`.
    let sid = ev
        .get("conversationId")
        .and_then(|s| s.as_str())
        .unwrap_or("default")
        .to_string();

    let taint_file = state_dir.join(format!("taint-{}", sanitize(&sid)));
    let tainted = taint_file.exists();

    // (4) project base from the payload; $HOME for `~`, read at the I/O boundary
    // so the compiler/kernel stay pure.
    let base = project_base(&ev, &raw_args);
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
        None => return passthrough(), // fail-open
    };

    let action_path = base
        .as_deref()
        .and_then(|b| resolve_action_path(&args, b, home.as_deref()));

    let req = GateRequest {
        v: ABI_VERSION,
        tool: normalize_tool(&world, tool),
        arguments: args,
        path: action_path,
        context: GateContext {
            session_id: sid,
            mode: Some(mode.to_string()),
            taint: Some(if tainted { "tainted" } else { "clean" }.to_string()),
            source_channel: Some("user_prompt".to_string()),
            approval_token: None,
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
    if res.context.taint == "tainted" && !tainted {
        let note = format!("tainted by {tool} ({})", res.action);
        if !persist_taint(state_dir, &taint_file, &note) {
            eprintln!(
                "harness agy-hook: cannot record session taint under {} — refusing the call that would escalate",
                state_dir.display()
            );
            emit(
                "deny",
                "session taint could not be recorded, so this ingestion cannot be governed (untracked_taint)",
            );
        }
    }

    // The approval channel: `force_ask` ignores the host's cached "Always Allow"
    // grants, so a kernel ASK always reaches a human. `--soft-ask` opts into the
    // cache-respecting `ask`.
    let ask_channel = if soft_ask { "ask" } else { "force_ask" };

    match host_outcome(&res) {
        // ALLOW. Additive default: emit the no-decision no-op and defer to the
        // host's normal permission flow. With `--grant` (replace mode) emit an
        // explicit `allow`, which *grants* — the host skips its prompt — so the
        // manifest is the authoritative allowlist, not an overlay.
        HostOutcome::Proceed => {
            if grant {
                emit("allow", &format!("manifest ALLOW: {}", res.action));
            }
            passthrough()
        }
        HostOutcome::NeedsApproval { reason } => emit(ask_channel, &reason),
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
            passthrough() // additive dogfooding default
        }
        HostOutcome::Block {
            kind: BlockKind::Replan,
            reason: _,
        } => passthrough(), // no host channel for "smaller step"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_commandline_so_the_shared_classifier_fires() {
        // D36 classifies the `command` arg; Antigravity spells it `CommandLine`.
        let args = alias_neutral_args(&json!({"CommandLine": "rm -rf /tmp/x", "Cwd": "/w"}));
        assert_eq!(args["command"], "rm -rf /tmp/x");
        // originals preserved
        assert_eq!(args["CommandLine"], "rm -rf /tmp/x");
        assert_eq!(args["Cwd"], "/w");
    }

    #[test]
    fn aliases_the_path_shaped_arguments() {
        for key in ["AbsolutePath", "TargetFile", "FilePath"] {
            let args = alias_neutral_args(&json!({ key: "/w/src/x.rs" }));
            assert_eq!(args["path"], "/w/src/x.rs", "{key} must alias to path");
        }
    }

    #[test]
    fn an_existing_neutral_key_is_never_overwritten() {
        let args = alias_neutral_args(&json!({"command": "ls", "CommandLine": "rm -rf /"}));
        assert_eq!(args["command"], "ls");
    }

    #[test]
    fn non_object_args_degrade_to_an_empty_object() {
        assert_eq!(alias_neutral_args(&json!("nope")), json!({}));
    }

    #[test]
    fn project_base_prefers_workspace_then_cwd() {
        let ev = json!({"workspacePaths": ["/ws"]});
        assert_eq!(
            project_base(&ev, &json!({"Cwd": "/other"})).as_deref(),
            Some("/ws")
        );
        let ev = json!({"workspacePaths": []});
        assert_eq!(
            project_base(&ev, &json!({"Cwd": "/other"})).as_deref(),
            Some("/other")
        );
    }
}
