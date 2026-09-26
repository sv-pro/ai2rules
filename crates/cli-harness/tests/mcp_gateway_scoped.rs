//! D79 + D80 end to end: a scoped verb over a hidden base action, through
//! `harness mcp-gateway` in front of `harness mock-jira --poisoned` (which echoes
//! the tool name and arguments that actually arrive upstream).
//!
//! Before D80 the gateway forwarded the *caller's* name and arguments: a scoped
//! verb reached the upstream under a name it did not know, and a `Literal` the
//! caller tried to override was overridden. Before D79 the unscoped base action
//! stayed callable, so the literal could be bypassed by not using the verb.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

const WORLD: &str = r#"
world_id: scoped-gateway
capabilities:
  - { trust: Trusted, actions: [Read, Mcp] }
base_actions:
  - name: jira_get_issue
    action_type: Read
    side_effect: Read
    projected: false
    backing: !McpServer { server: jira, tool: get_issue }
    schema:
      type: object
      properties:
        issue_key: { type: string }
scoped_capabilities:
  - name: read_demo_issue
    base_action: jira_get_issue
    args: { issue_key: !Literal "DEMO-1" }
"#;

/// Classification (D36) renames the *policy* action; the upstream still has only
/// the tool the client called. And a `ContextRef` the gateway cannot resolve.
const CLASSIFIED_WORLD: &str = r#"
world_id: classified-gateway
capabilities:
  - { trust: Trusted, actions: [Read, Mcp] }
base_actions:
  - name: jira_get_issue
    action_type: Read
    side_effect: Read
    schema: { type: object, properties: { issue_key: { type: string } } }
  - name: jira_get_issue_other
    action_type: Read
    side_effect: Read
    schema: { type: object, properties: { issue_key: { type: string } } }
command_classes:
  - { action: jira_get_issue, arg: issue_key, default_to: jira_get_issue_other, classes: [] }
scoped_capabilities:
  - name: read_current_issue
    base_action: jira_get_issue_other
    args: { issue_key: !ContextRef change.issue }
"#;

fn scenario(requests: &[Value]) -> std::collections::HashMap<i64, Value> {
    scenario_in(WORLD, requests)
}

fn scenario_in(world_yaml: &str, requests: &[Value]) -> std::collections::HashMap<i64, Value> {
    let dir = tempfile::tempdir().unwrap();
    let world = dir.path().join("world.yaml");
    std::fs::write(&world, world_yaml).unwrap();

    let bin = env!("CARGO_BIN_EXE_harness");
    let mut child = Command::new(bin)
        .args([
            "mcp-gateway",
            "--world",
            world.to_str().unwrap(),
            "--",
            bin,
            "mock-jira",
            "--poisoned",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn gateway");
    {
        let mut stdin = child.stdin.take().unwrap();
        for req in requests {
            writeln!(stdin, "{req}").unwrap();
        }
    }
    let mut out = std::collections::HashMap::new();
    for line in BufReader::new(child.stdout.take().unwrap()).lines() {
        let line = line.unwrap();
        if line.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(&line).expect("response json");
        if let Some(id) = v.get("id").and_then(|i| i.as_i64()) {
            out.insert(id, v);
        }
    }
    let _ = child.wait();
    out
}

fn text(resp: &Value) -> String {
    resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string()
}

#[test]
fn a_hidden_base_action_is_not_listed_and_is_absent_when_called() {
    let r = scenario(&[
        json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{
            "name":"jira_get_issue","arguments":{"issue_key":"SECRET-9"}}}),
    ]);
    let names: Vec<&str> = r[&1]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(!names.contains(&"jira_get_issue"), "listed: {names:?}");

    assert_eq!(r[&2]["result"]["isError"], json!(true));
    let t = text(&r[&2]);
    assert!(t.starts_with("ABSENT"), "{t}");
    assert!(!t.contains("received_arguments"), "reached upstream: {t}");
}

#[test]
fn a_scoped_verb_forwards_the_effective_call_not_the_proposal() {
    let r = scenario(&[
        json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{
        "name":"read_demo_issue","arguments":{"issue_key":"SECRET-9"}}}),
    ]);
    let t = text(&r[&1]);
    let echoed: Value = serde_json::from_str(&t).expect("the mock echoes the call: {t}");
    assert_eq!(echoed["received_name"], json!("jira_get_issue"));
    assert_eq!(
        echoed["received_arguments"],
        json!({"issue_key": "DEMO-1"}),
        "the literal must win over the caller's argument"
    );
}

#[test]
fn an_unscoped_call_keeps_the_requested_tool_name_after_classification() {
    let r = scenario_in(
        CLASSIFIED_WORLD,
        &[
            json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{
            "name":"jira_get_issue","arguments":{"issue_key":"DEMO-1"}}}),
        ],
    );
    let t = text(&r[&1]);
    let echoed: Value = serde_json::from_str(&t).expect("forwarded: {t}");
    assert_eq!(echoed["received_name"], json!("jira_get_issue"));
    assert_eq!(echoed["received_arguments"], json!({"issue_key": "DEMO-1"}));
}

#[test]
fn a_call_needing_runtime_context_is_refused_not_guessed() {
    let r = scenario_in(
        CLASSIFIED_WORLD,
        &[
            json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{
            "name":"read_current_issue","arguments":{}}}),
        ],
    );
    assert_eq!(r[&1]["result"]["isError"], json!(true));
    let t = text(&r[&1]);
    assert!(t.starts_with("REFUSED (gateway)"), "{t}");
    assert!(!t.contains("received_arguments"), "reached upstream: {t}");
}
