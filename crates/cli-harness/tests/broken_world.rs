//! The broken-world mode of the host hooks (D87, AI2-56 / #128): what `cc-hook`
//! and `agy-hook` do when the world cannot be read or compiled — `closed`,
//! `open`, or `last-good` (the default) — and that the person is told in every
//! mode.
//!
//! The broken worlds here are the kind an agent can produce: a world file
//! overwritten with something that does not parse, and a world file deleted.
//! Both used to switch governance off silently.

use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn repo(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

/// What an agent that can write the world file might leave behind: a policy edit
/// that does not parse.
const AGENT_WRITTEN: &str =
    "world_id: loosened\nbase_actions:\n  - { name: bash, action_type: Command\n";

#[derive(Clone, Copy)]
enum Host {
    Cc,
    Agy,
}

impl Host {
    fn subcommand(self) -> &'static str {
        match self {
            Host::Cc => "cc-hook",
            Host::Agy => "agy-hook",
        }
    }

    /// The governing world this repository ships for the host.
    fn shipped_world(self) -> PathBuf {
        match self {
            Host::Cc => repo(".claude/cc-world.yaml"),
            Host::Agy => repo(".agents/agy-world.yaml"),
        }
    }

    /// A destructive command, which both shipped worlds send to a person.
    fn destructive(self, session: &str) -> Value {
        match self {
            Host::Cc => json!({
                "tool_name": "Bash",
                "tool_input": {"command": "rm -rf /tmp/x"},
                "session_id": session,
            }),
            Host::Agy => json!({
                "conversationId": session,
                "workspacePaths": ["/workspace"],
                "toolCall": {"name": "run_command", "args": {"CommandLine": "rm -rf /tmp/x"}},
            }),
        }
    }

    /// The host's word for "a person must approve this".
    fn ask(self) -> &'static str {
        match self {
            Host::Cc => "ask",
            Host::Agy => "force_ask",
        }
    }
}

struct Answer {
    stdout: String,
    stderr: String,
}

impl Answer {
    fn json(&self) -> Value {
        serde_json::from_str(self.stdout.trim()).unwrap_or(Value::Null)
    }

    /// The decision the hook emitted, if any.
    fn decision(&self) -> Option<String> {
        let v = self.json();
        v["hookSpecificOutput"]["permissionDecision"]
            .as_str()
            .or_else(|| v["decision"].as_str())
            .map(String::from)
    }

    fn reason(&self) -> String {
        let v = self.json();
        v["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .or_else(|| v["reason"].as_str())
            .unwrap_or("")
            .to_string()
    }

    /// The user-facing warning: Claude Code's `systemMessage`; on Antigravity,
    /// which has no verified message field, stderr.
    fn warning(&self, host: Host) -> String {
        match host {
            Host::Cc => self.json()["systemMessage"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            Host::Agy => self.stderr.clone(),
        }
    }
}

fn run(host: Host, world: &Path, state: &Path, event: &Value, extra: &[&str]) -> Answer {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_harness"));
    cmd.args([
        host.subcommand(),
        "--world",
        world.to_str().unwrap(),
        "--state",
        state.to_str().unwrap(),
    ])
    .args(extra)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("spawn hook");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(event.to_string().as_bytes())
        .unwrap();
    let out = child.wait_with_output().expect("wait hook");
    assert!(
        out.status.success(),
        "a hook always exits 0 and answers in JSON"
    );
    Answer {
        stdout: String::from_utf8(out.stdout).unwrap(),
        stderr: String::from_utf8(out.stderr).unwrap(),
    }
}

/// A project whose world starts as the host's shipped one.
fn project(host: Host) -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let world = dir.path().join("world.yaml");
    std::fs::copy(host.shipped_world(), &world).unwrap();
    let state = dir.path().join("state");
    (dir, world, state)
}

const HOSTS: [Host; 2] = [Host::Cc, Host::Agy];

#[test]
fn a_working_world_answers_exactly_as_before() {
    for host in HOSTS {
        let (_dir, world, state) = project(host);
        let a = run(host, &world, &state, &host.destructive("w"), &[]);
        assert_eq!(a.decision().as_deref(), Some(host.ask()));
        assert!(
            a.warning(host).is_empty(),
            "no warning while the world compiles: {}",
            a.stdout
        );
        assert!(a.json().get("systemMessage").is_none());
    }
}

#[test]
fn closed_refuses_every_call_and_names_the_way_out() {
    for host in HOSTS {
        let (_dir, world, state) = project(host);
        std::fs::write(&world, AGENT_WRITTEN).unwrap();
        let a = run(
            host,
            &world,
            &state,
            &host.destructive("c"),
            &["--on-broken-world", "closed"],
        );
        assert_eq!(a.decision().as_deref(), Some("deny"), "{}", a.stdout);
        let reason = a.reason();
        assert!(reason.contains("does not compile"), "{reason}");
        assert!(
            reason.contains("`closed`"),
            "the reason names the mode: {reason}"
        );
        assert!(
            reason.contains("line"),
            "the compile error carries its line: {reason}"
        );
        assert!(
            reason.contains("switch governance off"),
            "and the way out: {reason}"
        );
        assert!(
            a.warning(host).contains("does not compile"),
            "the person is told"
        );
    }
}

#[test]
fn open_lets_the_call_through_ungoverned_and_says_so() {
    for host in HOSTS {
        let (_dir, world, state) = project(host);
        std::fs::write(&world, AGENT_WRITTEN).unwrap();
        let a = run(
            host,
            &world,
            &state,
            &host.destructive("o"),
            &["--on-broken-world", "open"],
        );
        assert_eq!(a.decision(), None, "open defers to the host: {}", a.stdout);
        assert!(
            a.warning(host).contains("UNGOVERNED"),
            "{}",
            a.warning(host)
        );
        if let Host::Agy = host {
            assert_eq!(a.json(), json!({}), "agy still gets its no-op object");
        }
    }
}

/// The default, and the attack the issue names: the agent overwrites the world
/// with something that does not compile. The last world that compiled still
/// decides, and the person is told their (or the agent's) edit did not apply.
#[test]
fn last_good_keeps_deciding_with_the_world_that_last_compiled() {
    for host in HOSTS {
        let (_dir, world, state) = project(host);
        // A call under the working world caches it (default mode: last-good).
        run(host, &world, &state, &host.destructive("g"), &[]);

        std::fs::write(&world, AGENT_WRITTEN).unwrap();
        let a = run(host, &world, &state, &host.destructive("g"), &[]);
        assert_eq!(
            a.decision().as_deref(),
            Some(host.ask()),
            "the cached world still sends a destructive command to a person: {}",
            a.stdout
        );
        let warning = a.warning(host);
        assert!(warning.contains("`last-good`"), "{warning}");
        assert!(warning.contains("NOT taken effect"), "{warning}");

        // Deleting the world is the same failure, not an off switch.
        std::fs::remove_file(&world).unwrap();
        let a = run(
            host,
            &world,
            &state,
            &host.destructive("g"),
            &["--on-broken-world", "last-good"],
        );
        assert_eq!(a.decision().as_deref(), Some(host.ask()), "{}", a.stdout);
        assert!(
            a.warning(host).contains("cannot read"),
            "{}",
            a.warning(host)
        );
    }
}

/// An ALLOW under the cached world is still a passthrough, and on Claude Code
/// the passthrough now carries the warning (a `systemMessage` with no decision).
#[test]
fn a_cached_allow_still_warns_on_claude_code() {
    let (_dir, world, state) = project(Host::Cc);
    let read = json!({"tool_name":"Read","tool_input":{"file_path":"notes.md"},"session_id":"r"});
    assert!(run(Host::Cc, &world, &state, &read, &[])
        .stdout
        .trim()
        .is_empty());
    std::fs::write(&world, AGENT_WRITTEN).unwrap();
    let a = run(Host::Cc, &world, &state, &read, &[]);
    assert_eq!(a.decision(), None);
    assert!(
        a.warning(Host::Cc).contains("NOT taken effect"),
        "{}",
        a.stdout
    );
}

#[test]
fn last_good_without_a_world_that_ever_compiled_is_closed() {
    for host in HOSTS {
        let (_dir, world, state) = project(host);
        std::fs::write(&world, AGENT_WRITTEN).unwrap();
        let a = run(host, &world, &state, &host.destructive("n"), &[]);
        assert_eq!(a.decision().as_deref(), Some("deny"), "{}", a.stdout);
        assert!(
            a.reason().contains("no world has compiled here yet"),
            "{}",
            a.reason()
        );
    }
}

/// The cache belongs to `last-good`: `closed` and `open` keep none, so switching
/// a shim to `last-good` later never resurrects a world from before.
#[test]
fn only_last_good_keeps_a_cache() {
    for mode in ["closed", "open"] {
        let (_dir, world, state) = project(Host::Cc);
        run(
            Host::Cc,
            &world,
            &state,
            &Host::Cc.destructive("k"),
            &["--on-broken-world", mode],
        );
        assert!(
            !state.join("last-good-world.json").exists(),
            "{mode} cached a world"
        );
    }
}

/// The decision log shows the broken-world state, so the console's feed does.
#[test]
fn the_decision_log_records_the_mode_and_the_error() {
    let (dir, world, state) = project(Host::Cc);
    let log = dir.path().join("decisions.jsonl");
    let log_arg = ["--log", log.to_str().unwrap()];
    run(
        Host::Cc,
        &world,
        &state,
        &Host::Cc.destructive("l"),
        &log_arg,
    );
    std::fs::write(&world, AGENT_WRITTEN).unwrap();
    run(
        Host::Cc,
        &world,
        &state,
        &Host::Cc.destructive("l"),
        &log_arg,
    );
    let mut closed = log_arg.to_vec();
    closed.extend(["--on-broken-world", "closed"]);
    run(
        Host::Cc,
        &world,
        &state,
        &Host::Cc.destructive("l"),
        &closed,
    );

    let lines: Vec<Value> = std::fs::read_to_string(&log)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 3, "{lines:#?}");
    assert!(
        lines[0].get("broken_world").is_none(),
        "a working world: no marker"
    );

    // last-good: a real kernel verdict, from the cached world, marked as such.
    assert_eq!(lines[1]["decision"], "ASK");
    assert_eq!(lines[1]["broken_world"]["applied"], "last-good");
    assert!(lines[1]["broken_world"]["last_good_sha256"].is_string());
    assert!(lines[1]["broken_world"]["error"]
        .as_str()
        .unwrap()
        .contains("line"));

    // closed: no world decided, and the line says so rather than faking a verdict.
    assert_eq!(lines[2]["decision"], "BROKEN_WORLD");
    assert_eq!(lines[2]["answer"], "deny");
    assert_eq!(lines[2]["broken_world"]["mode"], "closed");
    assert_eq!(lines[2]["call"], "rm -rf /tmp/x");
}

/// Antigravity's decision log (D84) carries the same broken-world state, in the
/// same shape, with the host's own identity stamped on the line.
#[test]
fn the_agy_decision_log_records_the_broken_world_too() {
    let (dir, world, state) = project(Host::Agy);
    let log = dir.path().join("decisions.jsonl");
    let log_arg = ["--log", log.to_str().unwrap()];
    run(
        Host::Agy,
        &world,
        &state,
        &Host::Agy.destructive("a"),
        &log_arg,
    );
    std::fs::write(&world, AGENT_WRITTEN).unwrap();
    run(
        Host::Agy,
        &world,
        &state,
        &Host::Agy.destructive("a"),
        &log_arg,
    );
    let mut open = log_arg.to_vec();
    open.extend(["--on-broken-world", "open"]);
    run(
        Host::Agy,
        &world,
        &state,
        &Host::Agy.destructive("a"),
        &open,
    );

    let lines: Vec<Value> = std::fs::read_to_string(&log)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 3, "{lines:#?}");
    assert!(lines[0].get("broken_world").is_none());
    assert_eq!(lines[1]["decision"], "ASK");
    assert_eq!(lines[1]["broken_world"]["applied"], "last-good");
    // open: nobody decided and the call went through, so the answer is null.
    assert_eq!(lines[2]["decision"], "BROKEN_WORLD");
    assert!(lines[2]["answer"].is_null(), "{}", lines[2]);
    assert_eq!(lines[2]["broken_world"]["mode"], "open");
    for line in &lines {
        assert_eq!(line["host"], "antigravity", "{line}");
    }
}
