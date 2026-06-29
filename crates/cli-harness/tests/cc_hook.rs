//! End-to-end test for `harness cc-hook` — the Rust Claude Code PreToolUse adapter
//! (DECISIONS D33 / E16.C). Feeds PreToolUse event JSON on stdin and asserts the
//! kernel's verdict surfaces as the right PreToolUse decision, against the *real*
//! demo world (`.claude/cc-world.yaml`). State lives in a throwaway temp dir, so
//! the live session's taint sidecar is never touched.

use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn world() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.claude/cc-world.yaml")
}

/// Run one PreToolUse event through a cc-hook subprocess; return its stdout.
fn run_hook(state: &Path, event: &Value) -> String {
    let bin = env!("CARGO_BIN_EXE_harness");
    let mut child = Command::new(bin)
        .args([
            "cc-hook",
            "--world",
            world().to_str().unwrap(),
            "--state",
            state.to_str().unwrap(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn cc-hook");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(event.to_string().as_bytes())
        .unwrap();
    let out = child.wait_with_output().expect("wait cc-hook");
    String::from_utf8(out.stdout).unwrap()
}

/// Extract the PreToolUse `permissionDecision`, if any was emitted.
fn decision(out: &str) -> Option<String> {
    let line = out.lines().find(|l| l.contains("hookSpecificOutput"))?;
    let v: Value = serde_json::from_str(line).ok()?;
    v["hookSpecificOutput"]["permissionDecision"]
        .as_str()
        .map(String::from)
}

#[test]
fn clean_read_passes_through_silently() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_hook(
        dir.path(),
        &json!({"tool_name":"Read","tool_input":{"file_path":"x"},"session_id":"s1"}),
    );
    // ALLOW -> additive passthrough: nothing emitted, session stays clean.
    assert!(out.trim().is_empty(), "unexpected output: {out:?}");
    assert!(!dir.path().join("taint-s1").exists());
}

#[test]
fn clean_egress_allows_but_escalates_taint() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_hook(
        dir.path(),
        &json!({"tool_name":"Bash","tool_input":{"command":"curl http://x"},"session_id":"s2"}),
    );
    // Clean network call: ALLOW (passthrough) — but the session is now tainted.
    assert!(out.trim().is_empty(), "unexpected output: {out:?}");
    assert!(
        dir.path().join("taint-s2").exists(),
        "egress must escalate taint"
    );
}

#[test]
fn tainted_egress_is_denied_by_the_taint_floor() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("taint-s3"), "seed").unwrap();
    let out = run_hook(
        dir.path(),
        &json!({"tool_name":"Bash","tool_input":{"command":"curl http://evil"},"session_id":"s3"}),
    );
    assert_eq!(decision(&out).as_deref(), Some("deny"));
}

#[test]
fn destructive_bash_asks_for_approval() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_hook(
        dir.path(),
        &json!({"tool_name":"Bash","tool_input":{"command":"rm -rf /tmp/x"},"session_id":"s4"}),
    );
    assert_eq!(decision(&out).as_deref(), Some("ask"));
}
