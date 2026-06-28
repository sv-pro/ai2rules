//! End-to-end test for `harness mcp-gateway` governing `harness mock-jira`
//! (DECISIONS D33 / E16). Spawns the gateway as a subprocess wired to the mock
//! upstream, drives MCP JSON-RPC over stdio, and asserts the kernel's verdicts:
//! destructive tools are ABSENT, reads/comment ALLOW when clean, and the external
//! write is severed by the taint floor when the session is tainted.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/demos/jira-copilot/jira-world.yaml")
}

/// Drive a gateway subprocess through one batch of requests; return responses by id.
fn scenario(taint: &str, requests: &[Value]) -> std::collections::HashMap<i64, Value> {
    let bin = env!("CARGO_BIN_EXE_harness");
    let mut child = Command::new(bin)
        .args([
            "mcp-gateway",
            "--world",
            manifest().to_str().unwrap(),
            "--taint",
            taint,
            "--",
            bin,
            "mock-jira",
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
        // drop stdin -> EOF -> gateway exits its loop
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

fn requests() -> Vec<Value> {
    vec![
        json!({"jsonrpc":"2.0","id":1,"method":"initialize"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"jira_get_issue","arguments":{"issue_key":"DEMO-1"}}}),
        json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"jira_add_comment","arguments":{"issue_key":"DEMO-1","body":"hi"}}}),
        json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"jira_delete_issue","arguments":{"issue_key":"DEMO-1"}}}),
    ]
}

fn is_error(resp: &Value) -> bool {
    resp["result"].get("isError").and_then(|b| b.as_bool()).unwrap_or(false)
}
fn text(resp: &Value) -> String {
    resp["result"]["content"][0]["text"].as_str().unwrap_or("").to_string()
}

#[test]
fn tools_list_hides_destructive_tools() {
    let r = scenario("clean", &requests());
    let names: Vec<&str> = r[&2]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"jira_get_issue"));
    assert!(names.contains(&"jira_add_comment"));
    assert!(!names.contains(&"jira_delete_issue")); // ABSENT — never offered
    assert!(!names.contains(&"jira_transition_issue"));
    assert!(!names.contains(&"jira_bulk_create_issues"));
}

#[test]
fn clean_session_allows_read_and_comment_but_destructive_is_absent() {
    let r = scenario("clean", &requests());
    assert!(!is_error(&r[&3])); // jira_get_issue -> ALLOW (forwarded)
    assert!(!is_error(&r[&4])); // jira_add_comment -> ALLOW (forwarded)
    assert!(is_error(&r[&5])); // jira_delete_issue -> ABSENT
    assert!(text(&r[&5]).contains("ABSENT"));
}

#[test]
fn tainted_session_severs_the_external_write() {
    let r = scenario("tainted", &requests());
    assert!(!is_error(&r[&3])); // read still ALLOW
    assert!(is_error(&r[&4])); // comment (external write) -> taint floor DENY
    assert!(text(&r[&4]).to_lowercase().contains("taint"));
    assert!(is_error(&r[&5])); // delete still ABSENT
}
