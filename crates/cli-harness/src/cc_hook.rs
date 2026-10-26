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
//! - **Fail-open (documented strategy):** a PROCESS error — an unreadable event —
//!   exits 0 with no output. A broken hook must never brick a session. A process
//!   failure is never an outcome (see `host.rs`).
//! - **A broken world is not a process error (D87).** A world that cannot be read
//!   or compiled is handled by `--on-broken-world` (`closed` / `open` /
//!   `last-good`, see [`crate::broken_world`]), and the warning reaches the person
//!   as a `systemMessage` on every answer while it lasts.

use crate::broken_world::{self, BrokenWorld, BrokenWorldMode, WorldLoad};
use crate::hostkit::{
    canonicalize_root_paths, normalize_tool, persist_taint, persist_usage, read_usage,
    resolve_action_path, sanitize,
};
use crate::outcome::{
    append_line, is_outcome_event, now_ms, outcome_line, redact_patterns, HostIds,
    HUMAN_ANSWER_INFERRED,
};
use compiler::{compile, loader::load_yaml, resolve_root_paths};
use harness_preview::{
    gate, host_outcome, BlockKind, GateContext, GateRequest, GateResponse, GateUsage, HostOutcome,
    ABI_VERSION,
};
use harness_types::CompiledWorld;
use serde_json::{json, Value};
use std::io::Read;
use std::path::Path;
use trace_store::secrets::mask_secrets;

/// Compile a world as the hook does: roots resolved against the project `base`
/// and `home` at the I/O boundary, then the pure compile. `None` on any failure.
/// Shared with `harness replay-transcripts`, so a replay compiles the same world.
pub(crate) fn compile_hook_world(
    content: &str,
    base: Option<&str>,
    home: Option<&str>,
) -> Option<CompiledWorld> {
    hook_world(content, base, home).ok()
}

/// As [`compile_hook_world`], keeping the error for the broken-world warning.
fn hook_world(
    content: &str,
    base: Option<&str>,
    home: Option<&str>,
) -> Result<CompiledWorld, String> {
    let mut m = load_yaml(content).map_err(|e| e.to_string())?;
    if let Some(r) = &m.roots {
        let roots = resolve_root_paths(r, home, base);
        m.roots = Some(canonicalize_root_paths(&roots));
    }
    compile(&m).map_err(|e| e.to_string())
}

/// The way out a `closed` refusal names: the kill-switch every project and user
/// install honours. A managed install has none by design (D82); its administrator
/// fixes the world.
const WAY_OUT: &str = "switch governance off with `touch ~/.claude/gate-off` (project and user \
                       installs; a managed install is fixed by its administrator)";

/// One PreToolUse event, as the hook sees it.
pub(crate) struct HookEvent<'a> {
    pub tool: &'a str,
    pub tool_input: Value,
    pub session_id: String,
}

/// The session state the hook carries between calls.
pub(crate) struct HookSession {
    pub tainted: bool,
    pub usage: Option<GateUsage>,
}

/// The `GateRequest` the hook sends for `event`. Shared with
/// `harness replay-transcripts`, so a replay asks the kernel exactly what the live
/// hook would have.
#[allow(clippy::too_many_arguments)]
pub(crate) fn hook_request(
    world: &CompiledWorld,
    event: HookEvent<'_>,
    session: &HookSession,
    base: Option<&str>,
    home: Option<&str>,
    mode: &str,
    source_channel: &str,
) -> GateRequest {
    let ti = if event.tool_input.is_object() {
        event.tool_input
    } else {
        json!({})
    };
    // The action's absolute target path for path-scope (roots), if this tool carries
    // one. Bash's `command` is not a path key, so Bash is path-scope-exempt.
    let action_path = base.and_then(|b| resolve_action_path(&ti, b, home));
    GateRequest {
        v: ABI_VERSION,
        tool: normalize_tool(world, event.tool),
        arguments: ti,
        path: action_path,
        context: GateContext {
            session_id: event.session_id,
            mode: Some(mode.to_string()),
            taint: Some(if session.tainted { "tainted" } else { "clean" }.to_string()),
            // Declared by the operator, not inferred from the event (finding #22):
            // a PreToolUse payload says what is about to run, never who asked for
            // it. See the `--source-channel` flag's documentation.
            source_channel: Some(source_channel.to_string()),
            approval_token: None,
            usage: session.usage,
            // Host hooks carry no per-value flow labels (AI2-70 v0): a world
            // that contracts an action fails closed here, by design.
            flow: None,
        },
    }
}

/// What the hook answers for a verdict: `Some((decision, reason))` to emit, or
/// `None` to stay silent and defer to the host's own permission flow.
pub(crate) fn hook_answer(
    res: &GateResponse,
    enforce_absent: bool,
    grant: bool,
) -> Option<(&'static str, String)> {
    match host_outcome(res) {
        // ALLOW. Additive default: stay silent and defer to the host's normal
        // permission flow. With `--grant` (replace mode) emit an explicit `allow`,
        // which *grants* — the host skips its Allow/Deny prompt — so the manifest
        // is the authoritative allowlist, not an overlay. An explicit `allow` still
        // cannot override a native deny/ask rule, so replace mode wants an emptied
        // settings.json baseline (docs/demos/replace-permissions).
        HostOutcome::Proceed => grant.then(|| ("allow", format!("manifest ALLOW: {}", res.action))),
        HostOutcome::NeedsApproval { reason } => Some(("ask", reason)),
        HostOutcome::Block {
            kind: BlockKind::Deny,
            reason,
        } => Some(("deny", reason)),
        // Additive dogfooding default: fall through to the host's flow.
        HostOutcome::Block {
            kind: BlockKind::Absent,
            reason,
        } => enforce_absent.then(|| ("deny", format!("ABSENT: {reason}"))),
        // No host channel for "smaller step" — fall through.
        HostOutcome::Block {
            kind: BlockKind::Replan,
            reason: _,
        } => None,
    }
}

/// How much of a call the decision log keeps: enough to recognise the call on
/// screen, never a whole file body or a long script.
const CALL_SUMMARY_MAX: usize = 160;

/// A one-line summary of what the call does, for the decision log: the command,
/// path or URL it carries, masked by the D66 scanner before whitespace is
/// collapsed and the text is cut to [`CALL_SUMMARY_MAX`] characters. Cutting first
/// could leave a token fragment below a detector's length floor or remove the end
/// of a PEM block. File contents (`content`, `new_string`) are deliberately not
/// keys here: the log shows what was attempted, not the data.
pub(crate) fn call_summary(tool_input: &Value) -> String {
    let raw = [
        "command",
        "file_path",
        "notebook_path",
        "path",
        "url",
        "pattern",
        "query",
        "description",
    ]
    .iter()
    .find_map(|k| tool_input.get(*k).and_then(|v| v.as_str()))
    .unwrap_or("");
    let masked = mask_secrets(raw);
    let flat = masked.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= CALL_SUMMARY_MAX {
        return flat;
    }
    let cut: String = flat.chars().take(CALL_SUMMARY_MAX - 1).collect();
    format!("{cut}…")
}

/// One line of the decision log (`--log`): what the agent tried, what the kernel
/// decided, and what the hook told the host. `answer` is `null` when the hook
/// stayed silent and left the call to the host's own permission flow.
///
/// `ids` joins the line to the host's transcript and to the outcome line the
/// observer writes after the call ran (D84). On an ASK the person's answer is not
/// visible to the hook, so the line says it is `inferred` from that outcome.
/// `call` is already masked and shortened by [`call_summary`].
pub(crate) fn decision_line(
    session: &str,
    tool: &str,
    call: &str,
    res: &GateResponse,
    answer: Option<&str>,
    reason: &str,
    ids: &HostIds,
) -> Value {
    let mut line = json!({
        "ts_ms": now_ms(),
        "kind": "decision",
        "session": session,
        "tool": tool,
        "action": res.action,
        "call": call,
        "decision": res.decision,
        "rule": res.rule,
        "reason": reason,
        "answer": answer,
        "taint": res.context.taint,
        "manifest_hash": res.manifest_hash,
    });
    if answer == Some("ask") {
        line["human_answer"] = json!(HUMAN_ANSWER_INFERRED);
    }
    ids.stamp(&mut line);
    line
}

/// Emit a PreToolUse decision (`deny`/`ask`, or `allow` in `--grant` mode) and exit 0.
/// While the world is broken the answer also carries the warning as `systemMessage`,
/// Claude Code's user-facing channel (D87).
fn emit(decision: &str, reason: &str) -> ! {
    let mut out = json!({"hookSpecificOutput": {
        "hookEventName": "PreToolUse",
        "permissionDecision": decision,
        "permissionDecisionReason": reason,
    }});
    if let Some(warning) = broken_world::warning() {
        out["systemMessage"] = json!(warning);
    }
    println!("{out}");
    std::process::exit(0);
}

/// Defer to the host's permission flow: no output, or only the broken-world
/// warning, which carries no decision.
fn silent() -> i32 {
    if let Some(warning) = broken_world::warning() {
        println!("{}", json!({ "systemMessage": warning }));
    }
    0
}

/// Answer a call that no world can decide: `closed` refuses it, `open` lets it
/// through ungoverned. Both say so, to the person and in the decision log.
fn answer_unworlded(
    broken: &BrokenWorld,
    log: Option<&Path>,
    line: impl Fn(Option<&str>) -> Value,
) -> i32 {
    let refuse = broken.applied == BrokenWorldMode::Closed;
    if let Some(path) = log {
        append_line(path, &line(refuse.then_some("deny")), "cc-hook");
    }
    if refuse {
        emit(
            "deny",
            broken_world::warning().unwrap_or("the world does not compile"),
        );
    }
    silent()
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    world_path: &Path,
    state_dir: &Path,
    mode: &str,
    source_channel: &str,
    enforce_absent: bool,
    grant: bool,
    log: Option<&Path>,
    on_broken_world: BrokenWorldMode,
) -> i32 {
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        return 0; // fail-open
    }
    let ev: Value = match serde_json::from_str(&input) {
        Ok(v) => v,
        Err(_) => return 0, // fail-open
    };

    // The outcome observer (D84): the same binary, wired to `PostToolUse` and
    // `PostToolUseFailure`. Read-only — it prints nothing, exits 0, and touches
    // neither taint nor budget, so it can never change what the gate decided.
    if is_outcome_event(&ev) {
        if let Some(path) = log {
            if let Some(line) = outcome_line(&ev, &redact_patterns(world_path)) {
                append_line(path, &line, "cc-hook");
            }
        }
        return 0;
    }

    let tool = ev.get("tool_name").and_then(|t| t.as_str()).unwrap_or("");
    let ids = HostIds::claude_code(&ev);
    let ti = ev.get("tool_input").cloned().unwrap_or_else(|| json!({}));
    let ti = if ti.is_object() { ti } else { json!({}) };
    let call = call_summary(&ti);
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
    // absolute here (env-dependent) before the pure compile. A world that does not
    // compile is handled by the broken-world mode (D87).
    let loaded = broken_world::load(world_path, state_dir, on_broken_world, |text| {
        hook_world(text, base.as_deref(), home.as_deref())
    });
    let (world, broken) = match loaded {
        WorldLoad::Live(w) => (w, None),
        WorldLoad::Broken { world, broken } => {
            let warning = broken.message(WAY_OUT);
            eprintln!("{warning}");
            broken_world::set_warning(warning.clone());
            match world {
                Some(w) => (w, Some(broken)),
                None => {
                    return answer_unworlded(&broken, log, |answer| {
                        broken_world::log_line(
                            &sid, tool, &call, tainted, &broken, answer, &warning, &ids,
                        )
                    })
                }
            }
        }
    };

    let req = hook_request(
        &world,
        HookEvent {
            tool,
            tool_input: ti,
            session_id: sid.clone(),
        },
        &HookSession {
            tainted,
            usage: carried_usage,
        },
        base.as_deref(),
        home.as_deref(),
        mode,
        source_channel,
    );
    let res = gate(&world, &req);
    // Every exit below that answers the host records the answer first (`--log`).
    let record = |answer: Option<&str>, reason: &str| {
        if let Some(path) = log {
            let mut line = decision_line(&sid, tool, &call, &res, answer, reason, &ids);
            if let Some(b) = &broken {
                line["broken_world"] = b.log_fields();
            }
            append_line(path, &line, "cc-hook");
        }
    };

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
        let reason = "budget counters could not be recorded, so this call cannot be counted against the world's limits (untracked_budget)";
        record(Some("deny"), reason);
        emit("deny", reason);
    }

    if res.context.taint == "tainted" && !tainted {
        let note = format!("tainted by {tool} ({})", res.action);
        if !persist_taint(state_dir, &taint_file, &note) {
            eprintln!(
                "harness cc-hook: cannot record session taint under {} — refusing the call that would escalate",
                state_dir.display()
            );
            let reason = "session taint could not be recorded, so this ingestion cannot be governed (untracked_taint)";
            record(Some("deny"), reason);
            emit("deny", reason);
        }
    }

    match hook_answer(&res, enforce_absent, grant) {
        Some((decision, reason)) => {
            record(Some(decision), &reason);
            emit(decision, &reason)
        }
        None => {
            record(None, &res.reason);
            silent() // defer to the host's permission flow
        }
    }
}
