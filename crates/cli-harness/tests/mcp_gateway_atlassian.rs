//! End-to-end test for `harness mcp-gateway` governing the **real-Atlassian** demo
//! manifest (`jira-atlassian.world.yaml`), the E16.E skin (DECISIONS D33 / E16).
//!
//! The live demo points the gateway at the Atlassian Rovo MCP Server via an
//! `mcp-remote` OAuth bridge. That last hop needs a human sign-in and can't run in
//! CI — but everything *before* the network (surface shaping, ABSENT, the taint
//! floor) is decided at the gate, so it can be proven offline. `harness mock-jira
//! --rovo` stands in for Rovo: it advertises the genuine Rovo tool names (plus a
//! Confluence tool as cross-product noise), so this test proves the manifest's tool
//! names actually match the Rovo surface and the kernel shapes it correctly — no
//! creds, no OAuth. If Atlassian renames a tool and the manifest drifts, this test
//! goes red instead of the live demo silently exposing nothing.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/demos/jira-copilot/jira-atlassian.world.yaml")
}

/// Drive a gateway subprocess (fronting `mock-jira --rovo`) through one batch of
/// requests; return responses by id.
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
            "--rovo",
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
        // allowed read
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"getJiraIssue","arguments":{"cloudId":"c","issueIdOrKey":"DEMO-1"}}}),
        // the one allowed write (taint-floored)
        json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"addCommentToJiraIssue","arguments":{"cloudId":"c","issueIdOrKey":"DEMO-1","commentBody":"triaged"}}}),
        // a destructive Jira tool the manifest does not declare
        json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"transitionJiraIssue","arguments":{"cloudId":"c","issueIdOrKey":"DEMO-1","transition":"Done"}}}),
        // a Confluence tool the Rovo server emits but the manifest never declares
        json!({"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"getConfluencePage","arguments":{"cloudId":"c","pageId":"1"}}}),
    ]
}

fn is_error(resp: &Value) -> bool {
    resp["result"]
        .get("isError")
        .and_then(|b| b.as_bool())
        .unwrap_or(false)
}
fn text(resp: &Value) -> String {
    resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string()
}

/// `tools/list` is shaped to exactly the manifest's declared Rovo surface: the
/// three reads + the cloudId resolver + the single comment write. Every write,
/// transition, create, and the whole Confluence surface is filtered out (ABSENT).
#[test]
fn tools_list_exposes_only_the_declared_rovo_surface() {
    let r = scenario("clean", &requests());
    let names: Vec<&str> = r[&2]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    // declared → offered
    for expected in [
        "getJiraIssue",
        "searchJiraIssuesUsingJql",
        "getVisibleJiraProjects",
        "getAccessibleAtlassianResources",
        "addCommentToJiraIssue",
    ] {
        assert!(names.contains(&expected), "expected {expected} in surface");
    }
    // undeclared → ABSENT from the surface
    for hidden in [
        "transitionJiraIssue",
        "editJiraIssue",
        "createJiraIssue",
        "addWorklogToJiraIssue",
        "getConfluencePage",
    ] {
        assert!(!names.contains(&hidden), "{hidden} must not be offered");
    }
    assert_eq!(names.len(), 5, "surface is exactly the 5 declared tools");
}

/// A clean session forwards the reads and the comment write, but the undeclared
/// destructive Jira tool and the Confluence tool do not exist for the agent.
#[test]
fn clean_session_allows_read_and_comment_but_writes_are_absent() {
    let r = scenario("clean", &requests());
    assert!(!is_error(&r[&3])); // getJiraIssue -> ALLOW (forwarded)
    assert!(!is_error(&r[&4])); // addCommentToJiraIssue -> ALLOW (forwarded)
    assert!(is_error(&r[&5])); // transitionJiraIssue -> ABSENT
    assert!(text(&r[&5]).contains("ABSENT"));
    assert!(is_error(&r[&6])); // getConfluencePage -> ABSENT
    assert!(text(&r[&6]).contains("ABSENT"));
}

/// A tainted session still reads, but the external comment write is severed at the
/// gate by the taint floor — before any call reaches Atlassian.
#[test]
fn tainted_session_severs_the_external_comment() {
    let r = scenario("tainted", &requests());
    assert!(!is_error(&r[&3])); // read still ALLOW
    assert!(is_error(&r[&4])); // comment (External) -> taint floor DENY
    assert!(text(&r[&4]).to_lowercase().contains("taint"));
    assert!(is_error(&r[&5])); // transition still ABSENT
}
