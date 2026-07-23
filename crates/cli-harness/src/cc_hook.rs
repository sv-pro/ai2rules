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

use compiler::{compile, loader::load_yaml, resolve_root_paths};
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

/// Extract and absolutize the target path of a file action, for path-scope (roots).
/// Reads the common path arg keys; returns `None` for tools without one (Bash's
/// `command` is not a path, so Bash is path-scope-exempt). Absolutization is lexical
/// — relative paths resolve against the project `base`, `~` against `$HOME`, and
/// `.`/`..` are normalized — matching the compiler's lexical rule resolution.
/// Symlink resolution is a documented v1 gap (the symlink-TOCTOU caveat).
fn resolve_action_path(args: &Value, base: &str, home: Option<&str>) -> Option<String> {
    let raw = ["file_path", "path", "notebook_path"]
        .iter()
        .find_map(|k| args.get(*k).and_then(|v| v.as_str()))?;
    let joined = if raw.starts_with('/') {
        raw.to_string()
    } else if let Some(rest) = raw.strip_prefix("~/") {
        match home {
            Some(h) => format!("{}/{}", h.trim_end_matches('/'), rest),
            None => return Some(raw.to_string()),
        }
    } else {
        format!("{}/{}", base.trim_end_matches('/'), raw)
    };
    Some(normalize_dots(&joined))
}

/// Lexically normalize `.`/`..`/empty segments of an absolute path (no FS access).
fn normalize_dots(p: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for seg in p.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            s => out.push(s),
        }
    }
    format!("/{}", out.join("/"))
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
                m.roots = Some(resolve_root_paths(r, home.as_deref(), base.as_deref()));
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
        tool: normalize(&world, tool),
        arguments: ti,
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
    if res.context.taint == "tainted" && !tainted {
        let _ = std::fs::create_dir_all(state_dir);
        if let Ok(mut f) = std::fs::File::create(&taint_file) {
            let _ = writeln!(f, "tainted by {tool} ({})", res.action);
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

#[cfg(test)]
mod path_tests {
    use super::*;

    #[test]
    fn normalize_dots_collapses_dot_and_dotdot() {
        assert_eq!(normalize_dots("/a/./b/../c"), "/a/c");
        assert_eq!(normalize_dots("/a//b/"), "/a/b");
        assert_eq!(normalize_dots("/a/b/.."), "/a");
    }

    #[test]
    fn resolve_action_path_reads_file_path_and_absolutizes() {
        let rel = json!({"file_path": "src/x.rs"});
        assert_eq!(
            resolve_action_path(&rel, "/proj", None).as_deref(),
            Some("/proj/src/x.rs")
        );
        let abs = json!({"file_path": "/etc/./shadow"});
        assert_eq!(
            resolve_action_path(&abs, "/proj", None).as_deref(),
            Some("/etc/shadow")
        );
        let home = json!({"path": "~/.ssh/id_rsa"});
        assert_eq!(
            resolve_action_path(&home, "/proj", Some("/home/u")).as_deref(),
            Some("/home/u/.ssh/id_rsa")
        );
    }

    #[test]
    fn resolve_action_path_is_none_for_non_path_tools() {
        // Bash's `command` is not a path key -> None -> path-scope exempt.
        let args = json!({"command": "rm -rf /"});
        assert_eq!(resolve_action_path(&args, "/proj", None), None);
    }
}
