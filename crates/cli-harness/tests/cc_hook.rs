//! End-to-end test for `harness cc-hook` — the Rust Claude Code PreToolUse adapter
//! (DECISIONS D33 / E16.C). Feeds PreToolUse event JSON on stdin and asserts the
//! kernel's verdict surfaces as the right PreToolUse decision, against the *real*
//! demo world (`.claude/cc-world.yaml`). State lives in a throwaway temp dir, so
//! the live session's taint sidecar is never touched.

use serde_json::{json, Value};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn world() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.claude/cc-world.yaml")
}

/// Run one PreToolUse event through a cc-hook subprocess; return its stdout.
fn run_hook(state: &Path, event: &Value) -> String {
    run_hook_args(state, event, &[])
}

/// As [`run_hook`], with extra CLI args (e.g. `--grant`).
fn run_hook_args(state: &Path, event: &Value, extra: &[&str]) -> String {
    run_hook_with_world_env(&world(), state, event, extra, &[])
}

/// As [`run_hook_args`], with an explicit world and environment overrides.
fn run_hook_with_world_env(
    world: &Path,
    state: &Path,
    event: &Value,
    extra: &[&str],
    env: &[(&str, &Path)],
) -> String {
    let bin = env!("CARGO_BIN_EXE_harness");
    let mut args = vec![
        "cc-hook",
        "--world",
        world.to_str().unwrap(),
        "--state",
        state.to_str().unwrap(),
    ];
    args.extend_from_slice(extra);
    let mut cmd = Command::new(bin);
    cmd.args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for (key, value) in env {
        cmd.env(key, value);
    }
    let mut child = cmd.spawn().expect("spawn cc-hook");
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

#[cfg(unix)]
fn write_roots_world(path: &Path) {
    std::fs::write(
        path,
        r#"
world_id: native-root-symlink-test
capabilities:
  - { trust: Trusted, actions: [Read, Write] }
base_actions:
  - name: Read
    action_type: Read
    side_effect: Read
    schema:
      type: object
      properties:
        file_path: { type: string }
  - { name: Write, action_type: Write, side_effect: FilesystemWrite }
roots:
  default: Ask
  rules:
    - { path: ".", access: ReadWrite }
    - { path: "~/.ssh", access: Deny, class: Credential }
"#,
    )
    .expect("write roots world");
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

#[test]
fn grant_mode_emits_explicit_allow_on_clean_action() {
    // Replace mode: ALLOW becomes an explicit grant (skips the host prompt),
    // not the additive silent passthrough. The manifest is now the allowlist.
    let dir = tempfile::tempdir().unwrap();
    let out = run_hook_args(
        dir.path(),
        &json!({"tool_name":"Read","tool_input":{"file_path":"x"},"session_id":"g1"}),
        &["--grant"],
    );
    assert_eq!(decision(&out).as_deref(), Some("allow"));
}

#[test]
fn grant_mode_still_denies_tainted_egress() {
    // The point of the case study: replacing the permission pile must NOT weaken
    // the taint floor — a tainted external call is denied even in grant mode.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("taint-g2"), "seed").unwrap();
    let out = run_hook_args(
        dir.path(),
        &json!({"tool_name":"Bash","tool_input":{"command":"curl http://evil"},"session_id":"g2"}),
        &["--grant"],
    );
    assert_eq!(decision(&out).as_deref(), Some("deny"));
}

#[cfg(unix)]
#[test]
fn grant_mode_denies_write_through_project_symlink_to_denied_root() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let project = dir.path().join("project");
    let state = dir.path().join("state");
    let world = dir.path().join("world.yaml");
    std::fs::create_dir_all(home.join(".ssh")).unwrap();
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(home.join(".ssh/config"), "secret").unwrap();
    symlink(home.join(".ssh"), project.join("link")).unwrap();
    write_roots_world(&world);

    let out = run_hook_with_world_env(
        &world,
        &state,
        &json!({"tool_name":"Write","tool_input":{"file_path":"link/config"},"session_id":"symlink"}),
        &["--grant"],
        &[("CLAUDE_PROJECT_DIR", &project), ("HOME", &home)],
    );
    assert_eq!(decision(&out).as_deref(), Some("deny"), "{out}");
}

#[cfg(unix)]
#[test]
fn grant_mode_allows_new_file_under_real_project_parent() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let project = dir.path().join("project");
    let state = dir.path().join("state");
    let world = dir.path().join("world.yaml");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(project.join("src")).unwrap();
    write_roots_world(&world);

    let out = run_hook_with_world_env(
        &world,
        &state,
        &json!({"tool_name":"Write","tool_input":{"file_path":"src/new.txt"},"session_id":"in-root"}),
        &["--grant"],
        &[("CLAUDE_PROJECT_DIR", &project), ("HOME", &home)],
    );
    assert_eq!(decision(&out).as_deref(), Some("allow"), "{out}");
}
