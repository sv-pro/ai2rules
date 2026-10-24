//! Real gateway/CLI/store/upstream round trips. Unix until the independent
//! Windows approval-key implementation (#129) lands. Requires Python 3.
#![cfg(unix)]

use harness_types::{ApprovalState, ApprovalToken, ApprovalTokenId};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use trace_store::ApprovalStore;

struct Fixture {
    dir: tempfile::TempDir,
    world: PathBuf,
}

impl Fixture {
    fn new(mode: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let world = dir.path().join("world.yaml");
        let yaml = include_str!("fixtures/my-jira.world.yaml")
            .replace("\r\n", "\n")
            .replace(
                "  - name: addCommentToJiraIssue\n",
                "  - name: addCommentToJiraIssue\n    approval_required: true\n",
            );
        std::fs::write(&world, yaml).unwrap();
        std::fs::write(dir.path().join("mode"), mode).unwrap();
        Self { dir, world }
    }
    fn store(&self) -> PathBuf {
        self.dir.path().join("approvals.jsonl")
    }
    fn tokens(&self) -> Vec<ApprovalToken> {
        ApprovalStore::open(self.store())
            .unwrap()
            .all()
            .cloned()
            .collect()
    }
    fn effects(&self) -> Vec<Value> {
        std::fs::read_to_string(self.dir.path().join("effects.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect()
    }
    fn gateway(&self, principal: &str, flags: &[&str]) -> Gateway {
        self.gateway_with_upstream(principal, flags, &[])
    }
    fn gateway_with_upstream(&self, principal: &str, flags: &[&str], extra: &[&str]) -> Gateway {
        let mut child = Command::new(env!("CARGO_BIN_EXE_harness"))
            .args(["mcp-gateway", "--world"])
            .arg(&self.world)
            .arg("--approvals")
            .arg(self.store())
            .args(["--principal", principal])
            .args(flags)
            .args(["--", "python3"])
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/approval_upstream.py"))
            .arg(self.dir.path())
            .args(extra)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        Gateway {
            child,
            input,
            output,
        }
    }
    fn prepare(&self) -> ApprovalTokenId {
        let response = self.gateway("alice", &[]).call(proposal());
        assert!(text(&response).contains("pending approval"), "{response}");
        let token = self
            .tokens()
            .into_iter()
            .find(|t| t.state == ApprovalState::Pending)
            .unwrap();
        self.approve(&token.id);
        token.id
    }
    fn approve(&self, id: &ApprovalTokenId) {
        let out = Command::new(env!("CARGO_BIN_EXE_harness"))
            .args(["approvals", "approve", id.as_str(), "--store"])
            .arg(self.store())
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

struct Gateway {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}
impl Gateway {
    fn call(&mut self, params: Value) -> Value {
        writeln!(
            self.input,
            "{}",
            json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":params})
        )
        .unwrap();
        self.input.flush().unwrap();
        let mut line = String::new();
        self.output.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }
}
impl Drop for Gateway {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn proposal() -> Value {
    json!({"name":"comment_on_platform_issue", "arguments":{
        "issue":"PLAT-1", "comment":"reviewed", "cloudId":"attacker"},
        "_meta":{"traceparent":"host-trace"}})
}
fn text(response: &Value) -> &str {
    response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or("")
}

#[test]
fn pending_reused_cli_approval_claimed_before_scoped_effect_and_replay_stops() {
    let f = Fixture::new("ok");
    let mut gateway = f.gateway("alice", &[]);
    assert!(text(&gateway.call(proposal())).contains("pending approval"));
    assert!(text(&gateway.call(proposal())).contains("pending approval"));
    assert_eq!(f.tokens().len(), 1);
    let id = f.tokens()[0].id.clone();
    f.approve(&id);
    let response = gateway.call(proposal());
    assert_ne!(response["result"]["isError"], true, "{response}");
    let effects = f.effects();
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0]["claimed"], true);
    assert_eq!(
        effects[0]["params"],
        json!({"name":"addCommentToJiraIssue", "arguments":{
        "cloudId":"abc-123", "issueIdOrKey":"PLAT-1", "commentBody":"reviewed"},
        "_meta":{"traceparent":"host-trace"}})
    );
    assert_eq!(f.tokens()[0].state, ApprovalState::Executed);
    // Restart removes carried taint but never restores spent consent.
    assert!(text(&f.gateway("alice", &[]).call(proposal())).contains("pending approval"));
    assert_eq!(f.effects().len(), 1);
}

#[test]
fn completed_effect_then_disconnect_remains_consumed_across_restart() {
    let f = Fixture::new("disconnect");
    let id = f.prepare();
    assert!(text(&f.gateway("alice", &[]).call(proposal())).contains("outcome unknown"));
    assert_eq!(
        f.tokens().iter().find(|t| t.id == id).unwrap().state,
        ApprovalState::Consumed
    );
    let response = f.gateway("alice", &[]).call(proposal());
    assert!(text(&response).contains("reconcile upstream"), "{response}");
    assert_eq!(f.effects().len(), 1);
    assert_eq!(f.effects()[0]["claimed"], true);
}

#[test]
fn concurrent_gateways_cannot_spend_one_grant_twice() {
    let f = Fixture::new("ok");
    f.prepare();
    let mut a = f.gateway("alice", &[]);
    let mut b = f.gateway("alice", &[]);
    std::thread::scope(|s| {
        let first = s.spawn(move || a.call(proposal()));
        let second = s.spawn(move || b.call(proposal()));
        first.join().unwrap();
        second.join().unwrap();
    });
    assert_eq!(f.effects().len(), 1);
}

#[test]
fn claim_persistence_failure_never_reaches_upstream() {
    let f = Fixture::new("ok");
    f.prepare();
    std::fs::create_dir(f.dir.path().join("approvals.jsonl.lock")).unwrap();
    let response = f.gateway("alice", &[]).call(proposal());
    assert!(text(&response).contains("no call forwarded"), "{response}");
    assert!(f.effects().is_empty());
}

#[test]
fn execution_evidence_failure_cannot_revive_consent() {
    let f = Fixture::new("evidence-failure");
    let id = f.prepare();
    let response = f.gateway("alice", &[]).call(proposal());
    assert!(
        text(&response).contains("evidence could not be persisted"),
        "{response}"
    );
    std::fs::remove_dir(f.dir.path().join("approvals.jsonl.lock")).unwrap();
    assert_eq!(
        f.tokens().iter().find(|t| t.id == id).unwrap().state,
        ApprovalState::Consumed
    );
    f.gateway("alice", &[]).call(proposal());
    assert_eq!(f.effects().len(), 1);
}

#[test]
fn principal_arguments_metadata_and_world_drift_require_new_consent() {
    let f = Fixture::new("ok");
    f.prepare();
    for (principal, request) in [
        ("bob", proposal()),
        ("alice", {
            let mut p = proposal();
            p["arguments"]["comment"] = json!("changed");
            p
        }),
        ("alice", {
            let mut p = proposal();
            p["_meta"] = json!({"traceparent":"other"});
            p
        }),
    ] {
        assert!(text(&f.gateway(principal, &[]).call(request)).contains("pending approval"));
    }
    assert!(text(
        &f.gateway_with_upstream("alice", &[], &["different-command-argument"])
            .call(proposal())
    )
    .contains("pending approval"));
    assert!(f.effects().is_empty());
    // A descriptor edit changes the schema epoch even when the world id is stable.
    let yaml = std::fs::read_to_string(&f.world)
        .unwrap()
        .replace("type: string", "type: [string, 'null']");
    std::fs::write(&f.world, &yaml).unwrap();
    assert!(text(&f.gateway("alice", &[]).call(proposal())).contains("pending approval"));
    let yaml = yaml.replace("world_id: my-jira", "world_id: changed");
    std::fs::write(&f.world, yaml).unwrap();
    assert!(text(&f.gateway("alice", &[]).call(proposal())).contains("pending approval"));
    assert!(f.effects().is_empty());
}

#[test]
fn expired_grant_does_not_hide_new_valid_consent() {
    let f = Fixture::new("ok");
    f.gateway("alice", &[]).call(proposal());
    let original = f.tokens()[0].clone();
    let mut expired = original.clone();
    expired.id = ApprovalTokenId::new("000-expired");
    expired.valid_until_unix_ms = 1;
    let mut store = ApprovalStore::open(f.store()).unwrap();
    store.mint(expired.clone()).unwrap();
    store.approve(&expired.id).unwrap();
    store.reject(&original.id).unwrap();
    assert!(text(&f.gateway("alice", &[]).call(proposal())).contains("pending approval"));
    let pending = f
        .tokens()
        .into_iter()
        .find(|t| t.state == ApprovalState::Pending)
        .unwrap();
    f.approve(&pending.id);
    assert_ne!(
        f.gateway("alice", &[]).call(proposal())["result"]["isError"],
        true
    );
    assert_eq!(f.effects().len(), 1);
}

#[test]
fn approved_resume_preserves_background_taint_budget_and_mrtr_refusals() {
    let f = Fixture::new("mrtr");
    f.prepare();
    for flags in [&["--mode", "background"][..], &["--taint", "tainted"][..]] {
        assert_eq!(
            f.gateway("alice", flags).call(proposal())["result"]["isError"],
            true
        );
    }
    assert!(f.effects().is_empty());
    let response = f.gateway("alice", &[]).call(proposal());
    assert!(text(&response).contains("input_required"), "{response}");
    assert_eq!(f.effects().len(), 1);
    assert_eq!(f.tokens()[0].state, ApprovalState::Executed);

    let f = Fixture::new("ok");
    let yaml = std::fs::read_to_string(&f.world)
        .unwrap()
        .replace("side_effect: External", "side_effect: Network")
        + "\nbudget: { max_network_calls: 0 }\n";
    std::fs::write(&f.world, yaml).unwrap();
    f.prepare();
    let response = f.gateway("alice", &[]).call(proposal());
    assert!(text(&response).starts_with("REPLAN"), "{response}");
    assert!(f.effects().is_empty());
    assert_eq!(f.tokens()[0].state, ApprovalState::Consumed);
}
