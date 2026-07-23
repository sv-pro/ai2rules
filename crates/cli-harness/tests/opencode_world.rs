//! Golden-vector contract tests for the OpenCode governance target (E17.5).
//!
//! The OpenCode plugin (`.opencode/plugin/ai2rules-gate.ts`) maps each OpenCode tool
//! call onto a manifest action and asks `harness gate` to decide. These tests pin the
//! *verdicts* that mapping relies on, by driving the real `harness gate` binary against
//! the real `docs/demos/opencode/opencode-world.yaml`. If the world manifest or the
//! kernel drifts, the documented OpenCode behavior (README decision table) breaks here.
//!
//! The companion half — the OpenCode event → action *classification* (D25 bash shapes) —
//! is pinned as a unit test on the shared `classify()` in `src/cc_hook.rs`.

use serde_json::{json, Value};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn world() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/demos/opencode/opencode-world.yaml")
}

/// Govern one already-classified action via `harness gate`; return the GateResponse.
fn gate(action: &str, taint: Option<&str>, mode: Option<&str>) -> Value {
    let bin = env!("CARGO_BIN_EXE_harness");
    let req = json!({
        "v": 1,
        "tool": action,
        "arguments": {},
        "context": {
            "session_id": "oc",
            "mode": mode.unwrap_or("interactive"),
            "taint": taint.unwrap_or("clean"),
            "source_channel": "user_prompt"
        },
    });
    let mut child = Command::new(bin)
        .args(["gate", "--world", world().to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn harness gate");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(req.to_string().as_bytes())
        .unwrap();
    let out = child.wait_with_output().expect("wait harness gate");
    assert!(out.status.success(), "gate evaluated {action} (exit 0)");
    serde_json::from_slice(&out.stdout).expect("gate response json")
}

#[track_caller]
fn assert_verdict(action: &str, taint: Option<&str>, mode: Option<&str>, decision: &str) {
    let r = gate(action, taint, mode);
    assert_eq!(
        r["decision"], decision,
        "{action} taint={taint:?} mode={mode:?}"
    );
}

#[test]
fn clean_reads_and_searches_allow() {
    for a in ["read", "grep", "glob", "list", "todoread"] {
        assert_verdict(a, None, None, "ALLOW");
    }
}

#[test]
fn clean_webfetch_allows_and_escalates_session_taint() {
    let r = gate("webfetch", None, None);
    assert_eq!(r["decision"], "ALLOW");
    assert_eq!(r["context"]["taint"], "tainted"); // the next call is now tainted
}

#[test]
fn taint_floor_severs_egress() {
    let w = gate("webfetch", Some("tainted"), None);
    assert_eq!(w["decision"], "DENY");
    assert_eq!(w["rule"], "taint_invariant");
    let b = gate("bash_network", Some("tainted"), None);
    assert_eq!(b["decision"], "DENY");
    assert_eq!(b["rule"], "taint_invariant");
}

#[test]
fn workspace_writes_allowed_even_when_tainted() {
    // FilesystemWrite is not under the taint floor (only PersistentWrite is) —
    // a tainted session can still edit the workspace, matching cc-world.
    assert_verdict("edit", Some("tainted"), None, "ALLOW");
    assert_verdict("write", Some("tainted"), None, "ALLOW");
    assert_verdict("patch", Some("tainted"), None, "ALLOW");
}

#[test]
fn destructive_bash_asks_interactively_but_fails_closed_in_background() {
    let ask = gate("bash_destructive", None, None);
    assert_eq!(ask["decision"], "ASK");
    assert_eq!(ask["rule"], "approval_required");
    // ASK collapses to DENY in background mode (invariant 10).
    assert_verdict("bash_destructive", None, Some("background"), "DENY");
}

#[test]
fn subagent_spawn_allows_when_clean() {
    assert_verdict("task", None, None, "ALLOW");
}

#[test]
fn unknown_tool_is_absent_not_denied() {
    let r = gate("delete_everything", None, None);
    assert_eq!(r["decision"], "ABSENT");
    assert_eq!(r["rule"], "unknown_to_ontology");
}
