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

/// Renamed and corrected with finding #13. This test previously asserted that a
/// clean session allowed *both* the read and the comment — which is precisely the
/// vulnerability: the read pulls bytes from an upstream server, so the session is
/// no longer clean by the time the write is proposed.
#[test]
fn clean_session_allows_the_read_which_then_severs_the_write() {
    let r = scenario("clean", &requests());
    assert!(!is_error(&r[&3])); // jira_get_issue -> ALLOW (forwarded)
    assert!(is_error(&r[&4])); // jira_add_comment -> DENY: the read tainted us
    assert!(text(&r[&4]).to_lowercase().contains("taint"));
    assert!(is_error(&r[&5])); // jira_delete_issue -> ABSENT
    assert!(text(&r[&5]).contains("ABSENT"));
}

/// The gateway shapes the `tools` array; every *other* field the upstream sent has
/// to survive. Rebuilding the result instead of filtering it would silently drop
/// `_meta` — and, against a 2026-07-28 upstream, the required `ttlMs`/`cacheScope`.
#[test]
fn tools_list_passes_upstream_sibling_fields_through() {
    let r = scenario("clean", &requests());
    assert_eq!(
        r[&2]["result"]["_meta"]["mock-jira/note"].as_str(),
        Some("sibling field — a proxy must pass this through")
    );
    // ...while still shaping the surface it is there to shape.
    assert!(r[&2]["result"]["tools"].as_array().unwrap().len() < 7);
}

/// `params._meta` carries the client's protocol version, identity, capabilities and
/// OpenTelemetry trace context (2026-07-28). The gateway governs the *call*; it must
/// not eat the metadata on the way upstream. `mock-jira` echoes back what it got.
#[test]
fn tools_call_forwards_request_meta_upstream() {
    let meta = json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "traceparent": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
    });
    let reqs = vec![
        json!({"jsonrpc":"2.0","id":1,"method":"initialize"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{
            "name":"jira_get_issue","arguments":{"issue_key":"DEMO-1"},"_meta": meta}}),
    ];
    let r = scenario("clean", &reqs);
    assert!(!is_error(&r[&2])); // ALLOW — so it was actually forwarded
    assert_eq!(
        r[&2]["result"]["_meta"]["mock-jira/echoedRequestMeta"],
        meta
    );
}

/// Finding #13 (P1): data read from an upstream MCP server is remote, untrusted
/// input. It has to taint the session, so the *next* external write is severed by
/// the taint floor. Before the fix, `jira_get_issue` left the session clean —
/// both audit rows showed `taint_in:false` — and the comment was forwarded.
#[test]
fn upstream_read_taints_the_session_and_severs_the_next_write() {
    let reqs = vec![
        json!({"jsonrpc":"2.0","id":1,"method":"initialize"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{
            "name":"jira_get_issue","arguments":{"issue_key":"DEMO-1"}}}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{
            "name":"jira_add_comment","arguments":{"issue_key":"DEMO-1","body":"hi"}}}),
    ];
    let r = scenario("clean", &reqs);
    assert!(!is_error(&r[&2]), "the read itself is still allowed");
    assert!(
        is_error(&r[&3]),
        "an external write after an upstream read must be severed by the taint floor"
    );
    assert!(text(&r[&3]).to_lowercase().contains("taint"));
}

#[test]
fn tainted_session_severs_the_external_write() {
    let r = scenario("tainted", &requests());
    assert!(!is_error(&r[&3])); // read still ALLOW
    assert!(is_error(&r[&4])); // comment (external write) -> taint floor DENY
    assert!(text(&r[&4]).to_lowercase().contains("taint"));
    assert!(is_error(&r[&5])); // delete still ABSENT
}
