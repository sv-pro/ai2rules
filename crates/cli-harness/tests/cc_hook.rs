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

/// Finding #16: when the taint marker cannot be written, the escalation would be
/// invisible to every later call and the taint floor would silently stop
/// engaging — so the escalating call fails CLOSED instead of being allowed.
///
/// The unwritable state dir is a *file* rather than a chmod'd directory, so the
/// test reproduces the failure even when the suite runs as root (CI containers).
#[test]
fn unwritable_taint_sidecar_denies_the_escalating_call() {
    let dir = tempfile::tempdir().unwrap();
    let state = dir.path().join("state-is-a-file");
    std::fs::write(&state, "not a directory").unwrap();

    let out = run_hook(
        &state,
        &json!({"tool_name":"Bash","tool_input":{"command":"curl http://x"},"session_id":"ro"}),
    );
    assert_eq!(
        decision(&out).as_deref(),
        Some("deny"),
        "an unrecordable escalation must not be allowed: {out}"
    );
}

/// The fail-closed above is scoped to the escalating call: a session whose taint
/// cannot be recorded still runs everything that does not ingest untrusted data.
#[test]
fn unwritable_taint_sidecar_still_passes_a_clean_read_through() {
    let dir = tempfile::tempdir().unwrap();
    let state = dir.path().join("state-is-a-file");
    std::fs::write(&state, "not a directory").unwrap();

    let out = run_hook(
        &state,
        &json!({"tool_name":"Read","tool_input":{"file_path":"x"},"session_id":"ro"}),
    );
    assert!(out.trim().is_empty(), "unexpected output: {out:?}");
}

/// Finding #22: the channel cannot be derived from a PreToolUse event — it says
/// what is about to run, never who asked — but it can be *declared*, which turns
/// an inert field into a posture control. An unattended session can be run at a
/// lower trust so the capability matrix shrinks.
#[test]
fn a_lower_trust_source_channel_shrinks_the_capability_surface() {
    let dir = tempfile::tempdir().unwrap();
    let write = json!({"tool_name":"Write","tool_input":{"file_path":"/tmp/x"},"session_id":"sc1"});

    // Default posture: the live world trusts `user_prompt`, so a write is allowed.
    let out = run_hook_args(dir.path(), &write, &["--grant"]);
    assert_eq!(decision(&out).as_deref(), Some("allow"), "{out}");

    // Declared as an untrusted channel, the same call is not merely denied — it is
    // ABSENT, outside what that trust level can even express.
    let out = run_hook_args(
        dir.path(),
        &write,
        &[
            "--grant",
            "--enforce-absent",
            "--source-channel",
            "web_fetch",
        ],
    );
    assert_eq!(decision(&out).as_deref(), Some("deny"), "{out}");

    // …while a read, which that trust level *can* perform, still goes through.
    let read = json!({"tool_name":"Read","tool_input":{"file_path":"/tmp/x"},"session_id":"sc2"});
    let out = run_hook_args(
        dir.path(),
        &read,
        &[
            "--grant",
            "--enforce-absent",
            "--source-channel",
            "web_fetch",
        ],
    );
    assert_eq!(decision(&out).as_deref(), Some("allow"), "{out}");
}

/// A channel the world does not declare fails closed rather than falling back to
/// a trusted default.
#[test]
fn an_undeclared_source_channel_fails_closed() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_hook_args(
        dir.path(),
        &json!({"tool_name":"Read","tool_input":{"file_path":"x"},"session_id":"sc3"}),
        &["--grant", "--source-channel", "not_a_channel"],
    );
    assert_eq!(decision(&out).as_deref(), Some("deny"), "{out}");
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

/// A write into folders the agent has not created yet is an in-project write,
/// not `missing_path`. Found by replaying real sessions: the hook used to resolve
/// only the immediate parent, so every such write was denied.
#[cfg(unix)]
#[test]
fn grant_mode_allows_new_file_in_new_project_folders() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let project = dir.path().join("project");
    let state = dir.path().join("state");
    let world = dir.path().join("world.yaml");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&project).unwrap();
    write_roots_world(&world);

    let out = run_hook_with_world_env(
        &world,
        &state,
        &json!({"tool_name":"Write","tool_input":{"file_path":"docs/demos/new/README.md"},"session_id":"new-dirs"}),
        &["--grant"],
        &[("CLAUDE_PROJECT_DIR", &project), ("HOME", &home)],
    );
    assert_eq!(decision(&out).as_deref(), Some("allow"), "{out}");
}

/// New folders do not open a way around a symlink: the existing part still
/// resolves to its real destination, here a denied root.
#[cfg(unix)]
#[test]
fn new_folders_under_a_symlink_to_a_denied_root_are_still_denied() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let project = dir.path().join("project");
    let state = dir.path().join("state");
    let world = dir.path().join("world.yaml");
    std::fs::create_dir_all(home.join(".ssh")).unwrap();
    std::fs::create_dir_all(&project).unwrap();
    symlink(home.join(".ssh"), project.join("link")).unwrap();
    write_roots_world(&world);

    let out = run_hook_with_world_env(
        &world,
        &state,
        &json!({"tool_name":"Write","tool_input":{"file_path":"link/new/dir/key"},"session_id":"symlink-new"}),
        &["--grant"],
        &[("CLAUDE_PROJECT_DIR", &project), ("HOME", &home)],
    );
    assert_eq!(decision(&out).as_deref(), Some("deny"), "{out}");
}

/// Run `events` in order through cc-hook with `--log`, returning the log's lines.
fn run_logged(state: &Path, log: &Path, events: &[Value], extra: &[&str]) -> Vec<Value> {
    let mut args = vec!["--log", log.to_str().unwrap()];
    args.extend_from_slice(extra);
    for ev in events {
        run_hook_args(state, ev, &args);
    }
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).expect("each log line is JSON"))
        .collect()
}

/// #116: `--log` records one line per decision — what was tried, the kernel's
/// verdict and rule, what the hook told the host, and the taint after the call —
/// for every kind of verdict.
#[test]
fn decision_log_records_every_verdict_with_its_answer_and_taint() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("logs").join("decisions.jsonl");
    let ev =
        |tool: &str, input: Value| json!({"tool_name":tool,"tool_input":input,"session_id":"log1"});
    let lines = run_logged(
        dir.path(),
        &log,
        &[
            ev("Read", json!({"file_path":"notes.md"})),
            ev("Bash", json!({"command":"rm -rf archive"})),
            ev("Bash", json!({"command":"curl http://portal.example/page"})),
            ev(
                "Bash",
                json!({"command":"curl http://files.example/upload"}),
            ),
            ev("NotARealTool", json!({})),
        ],
        &["--grant", "--enforce-absent"],
    );
    assert_eq!(lines.len(), 5, "one line per call: {lines:#?}");

    let field = |i: usize, k: &str| lines[i][k].as_str().unwrap_or("").to_string();
    assert_eq!(
        (field(0, "decision"), field(0, "answer")),
        ("ALLOW".into(), "allow".into())
    );
    assert_eq!(field(0, "call"), "notes.md");
    assert_eq!(
        (field(1, "decision"), field(1, "answer")),
        ("ASK".into(), "ask".into())
    );
    assert_eq!(field(1, "call"), "rm -rf archive");
    // Reading from outside is allowed, and the line carries the taint it caused.
    assert_eq!(field(2, "decision"), "ALLOW");
    assert_eq!(field(2, "taint"), "tainted");
    // The same kind of call after that is refused by a named rule.
    assert_eq!(
        (field(3, "decision"), field(3, "answer")),
        ("DENY".into(), "deny".into())
    );
    assert!(
        lines[3]["rule"].is_string(),
        "a DENY names its rule: {}",
        lines[3]
    );
    assert_eq!(
        (field(4, "decision"), field(4, "answer")),
        ("ABSENT".into(), "deny".into())
    );

    for line in &lines {
        assert_eq!(line["session"], "log1");
        assert!(line["ts_ms"].as_u64().unwrap_or(0) > 0, "{line}");
        assert!(line["manifest_hash"].is_string(), "{line}");
    }
}

/// Without `--grant` an ALLOW stays a silent passthrough; the log says so with a
/// null answer rather than pretending the hook granted it.
#[test]
fn decision_log_records_a_silent_passthrough_as_a_null_answer() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("decisions.jsonl");
    let lines = run_logged(
        dir.path(),
        &log,
        &[json!({"tool_name":"Read","tool_input":{"file_path":"x"},"session_id":"log2"})],
        &[],
    );
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["decision"], "ALLOW");
    assert!(lines[0]["answer"].is_null(), "{}", lines[0]);
}

/// The log shows what was attempted, never the data: a write logs its path, not
/// its content, and a long command is cut short.
#[test]
fn decision_log_keeps_the_target_not_the_content() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("decisions.jsonl");
    let long = format!("echo {}", "a".repeat(400));
    let lines = run_logged(
        dir.path(),
        &log,
        &[
            json!({"tool_name":"Write","tool_input":{"file_path":"report.md","content":"ACCOUNT 4417-SECRET"},"session_id":"log3"}),
            json!({"tool_name":"Bash","tool_input":{"command":long},"session_id":"log3"}),
        ],
        &["--grant"],
    );
    assert_eq!(lines[0]["call"], "report.md");
    assert!(
        !lines[0].to_string().contains("4417-SECRET"),
        "{}",
        lines[0]
    );
    assert_eq!(lines[1]["call"].as_str().unwrap().chars().count(), 160);
}

/// Observability, not enforcement: a log that cannot be written changes nothing
/// about the verdict the host receives.
#[test]
fn an_unwritable_decision_log_does_not_change_the_verdict() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = dir.path().join("a-file");
    std::fs::write(&blocker, "not a directory").unwrap();
    let log = blocker.join("decisions.jsonl");
    let out = run_hook_args(
        dir.path(),
        &json!({"tool_name":"Bash","tool_input":{"command":"rm -rf /tmp/x"},"session_id":"log4"}),
        &["--log", log.to_str().unwrap()],
    );
    assert_eq!(decision(&out).as_deref(), Some("ask"), "{out}");
    assert!(!log.exists());
}

/// D84: the outcome observer is read-only. A `PostToolUse` event for a call
/// that would taint the session prints nothing, leaves no taint or budget
/// sidecar, and only appends its line; without `--log` it does nothing at all.
#[test]
fn the_outcome_observer_never_answers_and_never_touches_session_state() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("decisions.jsonl");
    let post = json!({"hook_event_name": "PostToolUse", "session_id": "obs1",
        "tool_name": "WebFetch", "tool_input": {"url": "https://example.com"},
        "tool_use_id": "toolu_obs", "tool_response": {"code": 200, "result": "ok"},
        "agent_id": "agent-7", "agent_type": "Explore", "cwd": "/p"});

    let out = run_hook_args(dir.path(), &post, &[]);
    assert_eq!(out.trim(), "", "{out}");
    assert!(!log.exists());

    let out = run_hook_args(dir.path(), &post, &["--log", log.to_str().unwrap()]);
    assert_eq!(out.trim(), "", "{out}");
    let names: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        names,
        ["decisions.jsonl"],
        "the observer wrote session state"
    );

    let line: Value = serde_json::from_str(std::fs::read_to_string(&log).unwrap().trim()).unwrap();
    assert_eq!(line["kind"], "outcome");
    assert_eq!(line["tool_use_id"], "toolu_obs");
    assert_eq!(line["agent_id"], "agent-7");
    assert_eq!(line["agent_type"], "Explore");
    assert_eq!(line["status"], "ok");

    // A world that does not load does not stop the observer either: it just has
    // no manifest patterns, and the D66 scanner still runs.
    let out = run_hook_with_world_env(
        &dir.path().join("missing.yaml"),
        dir.path(),
        &post,
        &["--log", log.to_str().unwrap()],
        &[],
    );
    assert_eq!(out.trim(), "");
    assert_eq!(std::fs::read_to_string(&log).unwrap().lines().count(), 2);
}

/// D84: the gate's decision line carries the host's ids, and a call summary that
/// contains a secret is masked (D66) as a trace would be.
#[test]
fn decision_lines_carry_host_ids_and_mask_secrets_in_the_call() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("decisions.jsonl");
    let token = format!("ghp_{}", "Q1w2E3r4T5".repeat(4));
    let command = format!("curl -H 'Authorization: token {token}' https://api.example");
    let lines = run_logged(
        dir.path(),
        &log,
        &[
            json!({"tool_name": "Bash", "tool_input": {"command": command},
                   "session_id": "ids1", "tool_use_id": "toolu_9", "agent_id": "a-1",
                   "agent_type": "general-purpose", "cwd": "/work"}),
            json!({"tool_name": "Read", "tool_input": {"file_path": "x"}, "session_id": "ids1"}),
        ],
        &[],
    );
    assert_eq!(lines[0]["kind"], "decision");
    assert_eq!(lines[0]["tool_use_id"], "toolu_9");
    assert_eq!(lines[0]["agent_id"], "a-1");
    assert_eq!(lines[0]["agent_type"], "general-purpose");
    assert_eq!(lines[0]["cwd"], "/work");
    assert!(!lines[0].to_string().contains("Q1w2E3r4T5"), "{}", lines[0]);
    // A host that sends no ids gets nulls, not invented values.
    assert!(lines[1]["tool_use_id"].is_null() && lines[1]["agent_id"].is_null());
}
