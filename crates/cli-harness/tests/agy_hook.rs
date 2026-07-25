//! End-to-end test for `harness agy-hook` — the Rust Antigravity CLI (`agy`)
//! PreToolUse adapter (DECISIONS D48). Feeds Antigravity `PreToolUse` payload
//! JSON on stdin and asserts the kernel's verdict surfaces as the right
//! Antigravity decision, against the *real* world (`.agents/agy-world.yaml`).
//! State lives in a throwaway temp dir, so a live session's taint sidecar is
//! never touched.
//!
//! The payload shapes here are the ones captured from a live `agy` session:
//! camelCase envelope, `toolCall: {name, args}`, `conversationId`, and
//! **PascalCase** argument keys (`CommandLine`, `AbsolutePath`, `TargetFile`).

use serde_json::{json, Value};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn world() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.agents/agy-world.yaml")
}

/// Run one Antigravity PreToolUse payload through an agy-hook subprocess.
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
        "agy-hook",
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
    let mut child = cmd.spawn().expect("spawn agy-hook");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(event.to_string().as_bytes())
        .unwrap();
    let out = child.wait_with_output().expect("wait agy-hook");
    assert!(out.status.success(), "agy-hook must exit 0 (fail-open)");
    String::from_utf8(out.stdout).unwrap()
}

/// Extract the Antigravity `decision`, if any was emitted. A response carrying
/// no `decision` (`{}`) is the host's documented no-op passthrough.
fn decision(out: &str) -> Option<String> {
    let v: Value = serde_json::from_str(out.trim()).ok()?;
    v.get("decision")?.as_str().map(String::from)
}

/// Every response must be a single valid JSON object — Antigravity parses
/// stdout unconditionally, so even the passthrough is `{}`, never silence.
fn assert_passthrough(out: &str) {
    let v: Value = serde_json::from_str(out.trim())
        .unwrap_or_else(|e| panic!("agy expects JSON on stdout, got {out:?} ({e})"));
    assert!(v.is_object(), "response must be an object: {out:?}");
    assert!(
        v.get("decision").is_none(),
        "passthrough must carry no decision: {out:?}"
    );
}

/// An Antigravity PreToolUse payload in the captured shape.
fn payload(tool: &str, args: Value, conversation: &str) -> Value {
    json!({
        "conversationId": conversation,
        "stepIdx": 3,
        "modelName": "gemini-pro-agent",
        "workspacePaths": ["/workspace"],
        "toolCall": { "name": tool, "args": args },
    })
}

#[test]
fn clean_read_passes_through_as_the_no_decision_noop() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_hook(
        dir.path(),
        &payload("view_file", json!({"AbsolutePath": "/workspace/x"}), "s1"),
    );
    assert_passthrough(&out);
    assert!(!dir.path().join("taint-s1").exists());
}

#[test]
fn clean_egress_allows_but_escalates_taint() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_hook(
        dir.path(),
        &payload(
            "run_command",
            json!({"CommandLine": "curl http://x", "Cwd": "/workspace"}),
            "s2",
        ),
    );
    assert_passthrough(&out);
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
        &payload(
            "run_command",
            json!({"CommandLine": "curl http://evil"}),
            "s3",
        ),
    );
    assert_eq!(decision(&out).as_deref(), Some("deny"), "{out}");
}

/// The ASK channel: a kernel approval requirement maps to `force_ask`, which
/// ignores Antigravity's cached "Always Allow" grants, so a stale grant can
/// never silently satisfy an approval the kernel demanded.
#[test]
fn destructive_command_forces_the_approval_prompt() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_hook(
        dir.path(),
        &payload("run_command", json!({"CommandLine": "rm -rf /tmp/x"}), "s4"),
    );
    assert_eq!(decision(&out).as_deref(), Some("force_ask"), "{out}");
}

#[test]
fn soft_ask_opts_into_the_cache_respecting_channel() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_hook_args(
        dir.path(),
        &payload("run_command", json!({"CommandLine": "rm -rf /tmp/x"}), "s5"),
        &["--soft-ask"],
    );
    assert_eq!(decision(&out).as_deref(), Some("ask"), "{out}");
}

/// **The aliasing regression test.** Antigravity spells the shell argument
/// `CommandLine`; the D36 `command_classes` block classifies the neutral
/// `command`. If the adapter's alias ever stops firing, this command silently
/// stops classifying as network and falls to the `run_command_unclassified`
/// fail-closed branch — i.e. `force_ask` instead of a clean ALLOW.
///
/// The two halves must therefore differ: same command text, aliased vs not.
#[test]
fn commandline_is_aliased_so_the_shared_classifier_fires() {
    let dir = tempfile::tempdir().unwrap();
    let aliased = run_hook(
        dir.path(),
        &payload(
            "run_command",
            json!({"CommandLine": "curl https://example.com"}),
            "alias-yes",
        ),
    );
    assert_passthrough(&aliased);

    // Control: the same command text under a key the adapter does not alias
    // never reaches the classifier, so it fails closed as unclassified.
    let unaliased = run_hook(
        dir.path(),
        &payload(
            "run_command",
            json!({"NotACommandKey": "curl https://example.com"}),
            "alias-no",
        ),
    );
    assert_eq!(
        decision(&unaliased).as_deref(),
        Some("force_ask"),
        "an unclassifiable command must fail closed (D44): {unaliased}"
    );
}

#[test]
fn grant_mode_emits_explicit_allow_on_clean_action() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_hook_args(
        dir.path(),
        &payload("view_file", json!({"AbsolutePath": "/workspace/x"}), "g1"),
        &["--grant"],
    );
    assert_eq!(decision(&out).as_deref(), Some("allow"), "{out}");
}

#[test]
fn grant_mode_still_denies_tainted_egress() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("taint-g2"), "seed").unwrap();
    let out = run_hook_args(
        dir.path(),
        &payload(
            "run_command",
            json!({"CommandLine": "curl http://evil"}),
            "g2",
        ),
        &["--grant"],
    );
    assert_eq!(decision(&out).as_deref(), Some("deny"), "{out}");
}

#[test]
fn absent_passes_through_by_default_and_denies_under_enforcement() {
    let dir = tempfile::tempdir().unwrap();
    let event = payload("some_native_tool_we_do_not_declare", json!({}), "a1");

    // Additive default: a PreToolUse hook cannot remove native tools, so an
    // undeclared tool falls through rather than bricking the host.
    assert_passthrough(&run_hook(dir.path(), &event));

    let enforced = run_hook_args(dir.path(), &event, &["--enforce-absent"]);
    assert_eq!(decision(&enforced).as_deref(), Some("deny"), "{enforced}");
    let v: Value = serde_json::from_str(enforced.trim()).unwrap();
    assert!(
        v["reason"].as_str().unwrap().starts_with("ABSENT: "),
        "ABSENT stays distinguishable from DENY: {enforced}"
    );
}

/// A PROCESS failure is never a verdict: garbage stdin exits 0 emitting the
/// no-op, so a broken hook cannot brick a live session.
#[test]
fn malformed_payload_fails_open_with_the_noop() {
    let dir = tempfile::tempdir().unwrap();
    let bin = env!("CARGO_BIN_EXE_harness");
    let mut child = Command::new(bin)
        .args([
            "agy-hook",
            "--world",
            world().to_str().unwrap(),
            "--state",
            dir.path().to_str().unwrap(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn agy-hook");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"this is not json")
        .unwrap();
    let out = child.wait_with_output().expect("wait agy-hook");
    assert!(out.status.success(), "agy-hook fails open (exit 0)");
    assert_passthrough(&String::from_utf8(out.stdout).unwrap());
}

/// An unreadable/uncompilable world is also a process failure, not a DENY.
#[test]
fn unreadable_world_fails_open_with_the_noop() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("nope.yaml");
    let out = run_hook_with_world_env(
        &missing,
        dir.path(),
        &payload("run_command", json!({"CommandLine": "rm -rf /"}), "w1"),
        &[],
        &[],
    );
    assert_passthrough(&out);
}

#[cfg(unix)]
fn write_roots_world(path: &Path) {
    std::fs::write(
        path,
        r#"
world_id: agy-root-symlink-test
capabilities:
  - { trust: Trusted, actions: [Read, Write] }
base_actions:
  - name: view_file
    action_type: Read
    side_effect: Read
    schema:
      type: object
      properties:
        path: { type: string }
  - { name: write_to_file, action_type: Write, side_effect: FilesystemWrite }
roots:
  default: Ask
  rules:
    - { path: ".", access: ReadWrite }
    - { path: "~/.ssh", access: Deny, class: Credential }
"#,
    )
    .expect("write roots world");
}

/// Path-scope parity with cc-hook: the D46 symlink hardening lives in the shared
/// `hostkit`, so it must hold for Antigravity's `TargetFile` argument too — a
/// project-local symlink cannot smuggle a write into a denied root.
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

    // Antigravity supplies the project base in the payload (`workspacePaths`),
    // not via an environment variable — that is the host difference under test.
    let event = json!({
        "conversationId": "symlink",
        "workspacePaths": [project.to_str().unwrap()],
        "toolCall": { "name": "write_to_file", "args": { "TargetFile": "link/config" } },
    });
    let out = run_hook_with_world_env(&world, &state, &event, &["--grant"], &[("HOME", &home)]);
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

    let event = json!({
        "conversationId": "in-root",
        "workspacePaths": [project.to_str().unwrap()],
        "toolCall": { "name": "write_to_file", "args": { "TargetFile": "src/new.txt" } },
    });
    let out = run_hook_with_world_env(&world, &state, &event, &["--grant"], &[("HOME", &home)]);
    assert_eq!(decision(&out).as_deref(), Some("allow"), "{out}");
}
