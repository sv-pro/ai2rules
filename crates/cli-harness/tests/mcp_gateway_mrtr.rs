//! D49 / issue #40: MCP `2026-07-28` lets a server answer `tools/call` with an
//! **`InputRequiredResult`** — a demand that the client gather input (an
//! elicitation, or an LLM completion) and resend. That inverts the direction of
//! untrusted influence: a call the kernel **ALLOWed** comes back carrying a
//! server-authored instruction to the host.
//!
//! The kernel has no verdict shape for a demand yet. Until it does, the gateway
//! **refuses to relay one** — the interim default D49 records. These tests pin
//! that the demand never reaches the client.
//!
//! Note what this does and does not do: the upstream call already happened, so
//! this protects the *host* from the demand, not the upstream from the call.

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
            "--input-required",
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

fn allowed_call() -> Vec<Value> {
    vec![
        json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{
        "name":"jira_get_issue","arguments":{"issue_key":"DEMO-1"}}}),
    ]
}

/// The demand must not be relayed: no `inputRequests`, no `requestState`, and no
/// `resultType: "input_required"` reaching the client. The whole point is that the
/// model never sees "paste your API token" as though the harness had sanctioned it.
#[test]
fn an_input_required_result_is_not_relayed_to_the_client() {
    let r = scenario(&allowed_call());
    let result = &r[&1]["result"];
    let serialized = result.to_string();

    assert_eq!(
        result["resultType"].as_str(),
        None,
        "the client must not receive an input_required result: {serialized}"
    );
    assert!(
        result.get("inputRequests").is_none(),
        "the server's demand must not be relayed: {serialized}"
    );
    assert!(
        !serialized.contains("requestState"),
        "the opaque server state must not be relayed: {serialized}"
    );
    assert!(
        !serialized.contains("Atlassian API token"),
        "the elicitation prompt must not reach the model: {serialized}"
    );
}

/// It is refused *as an error*, and the reason says which layer refused and why —
/// this is a gateway-level interim deny, not a kernel verdict, and the label must
/// not pretend otherwise.
#[test]
fn the_refusal_is_an_error_that_names_itself() {
    let r = scenario(&allowed_call());
    let result = &r[&1]["result"];
    assert_eq!(result["isError"], json!(true));
    let text = result["content"][0]["text"].as_str().unwrap_or("");
    assert!(
        text.contains("input_required"),
        "the refusal should say what it refused: {text}"
    );
    assert!(
        text.contains("D49"),
        "the refusal should point at the decision that mandates it: {text}"
    );
}
