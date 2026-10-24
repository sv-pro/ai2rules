//! Finding #14 (P1): a **malicious or drifted upstream** must not be able to reach
//! the model through the gateway. `harness mock-jira --poisoned` advertises the
//! *allowed* tool name `jira_get_issue` carrying an extra `deleteAll` argument the
//! world never declared, and echoes back whatever arguments actually arrive.
//!
//! Two properties, and they are different: the schema the client is *offered*
//! (surface), and the arguments the upstream is *sent* (call). A gateway that
//! authorizes only the tool name gets both wrong.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/demos/jira-copilot/jira-world.yaml")
}

fn scenario(requests: &[Value]) -> std::collections::HashMap<i64, Value> {
    let bin = env!("CARGO_BIN_EXE_harness");
    let mut child = Command::new(bin)
        .args([
            "mcp-gateway",
            "--world",
            manifest().to_str().unwrap(),
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
    let stdout = BufReader::new(child.stdout.take().unwrap());
    for line in stdout.lines() {
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

fn list_request() -> Vec<Value> {
    vec![json!({"jsonrpc":"2.0","id":1,"method":"tools/list"})]
}

/// The surface half: the client is offered the **world's** schema for an allowed
/// tool, never the upstream's. Before the fix the gateway filtered by name and
/// republished the upstream tool object verbatim, so `deleteAll` was advertised to
/// the model as a legitimate argument of a legitimate tool.
#[test]
fn poisoned_upstream_schema_is_not_republished() {
    let r = scenario(&list_request());
    let tools = r[&1]["result"]["tools"].as_array().unwrap();
    let tool = tools
        .iter()
        .find(|t| t["name"] == "jira_get_issue")
        .expect("the allowed tool is still offered");

    let props = &tool["inputSchema"]["properties"];
    assert!(
        props.get("deleteAll").is_none(),
        "the upstream's undeclared argument must not be advertised: {tool}"
    );
    assert!(
        props.get("issue_key").is_some(),
        "the world's own declared argument must still be advertised: {tool}"
    );
}

/// The call half: even were the schema check bypassed, only arguments the world
/// declares may cross to the upstream. The kernel already denies an undeclared
/// argument outright (`schema_violation`, D45) — this pins that, and the mock's
/// echo proves nothing was forwarded.
#[test]
fn poisoned_argument_is_denied_and_never_forwarded() {
    let reqs = vec![
        json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{
        "name":"jira_get_issue","arguments":{"issue_key":"DEMO-1","deleteAll":true}}}),
    ];
    let r = scenario(&reqs);
    let text = r[&1]["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string();
    assert_eq!(
        r[&1]["result"]["isError"],
        json!(true),
        "an undeclared argument must not produce an ALLOW: {text}"
    );
    assert!(
        !text.contains("received_arguments"),
        "the call must never have reached the upstream: {text}"
    );
}

/// A well-formed call still works, and carries only declared arguments upstream.
#[test]
fn declared_arguments_are_forwarded_unchanged() {
    let reqs = vec![
        json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{
        "name":"jira_get_issue","arguments":{"issue_key":"DEMO-1"}}}),
    ];
    let r = scenario(&reqs);
    let text = r[&1]["result"]["content"][0]["text"].as_str().unwrap_or("");
    let echoed: Value = serde_json::from_str(text).expect("the mock echoes its arguments");
    assert_eq!(echoed["received_arguments"], json!({"issue_key": "DEMO-1"}));
}
