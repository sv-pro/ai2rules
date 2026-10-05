//! End-to-end test for `harness mcp-gateway` serving a **scoped MCP surface**
//! (DECISIONS D78, PLAN E13.4b): the world presents itself as `My_Jira`, offers
//! upstream tools under names of its own with renamed and limited arguments, and
//! forwards the kernel's lowered call — never the call as the model proposed it.
//!
//! The upstream is `harness mock-jira --rovo --echo`, which answers every call
//! with the tool name and arguments that actually arrived, so each assertion is
//! about what crossed the gateway, not about what the gateway said it did.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/my-jira.world.yaml")
}

/// Drive one gateway session through `requests`; return responses by id.
fn scenario(requests: &[Value], audit: Option<&Path>) -> HashMap<i64, Value> {
    let bin = env!("CARGO_BIN_EXE_harness");
    let world = manifest();
    let mut args = vec!["mcp-gateway", "--world", world.to_str().unwrap()];
    if let Some(audit) = audit {
        args.extend(["--audit", audit.to_str().unwrap()]);
    }
    args.extend(["--", bin, "mock-jira", "--rovo", "--echo"]);
    let mut child = Command::new(bin)
        .args(&args)
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
    let mut out = HashMap::new();
    for line in BufReader::new(child.stdout.take().unwrap()).lines() {
        let line = line.unwrap();
        if line.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(&line).expect("response json");
        if let Some(id) = v.get("id").and_then(Value::as_i64) {
            out.insert(id, v);
        }
    }
    let _ = child.wait();
    out
}

fn call(id: i64, name: &str, arguments: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call",
           "params": {"name": name, "arguments": arguments}})
}

/// The text of a tool result's first content block.
fn text(resp: &Value) -> &str {
    resp["result"]["content"][0]["text"].as_str().unwrap_or("")
}

/// What the echoing upstream received, or `None` when the call was refused.
fn received(resp: &Value) -> Option<Value> {
    if resp["result"]["isError"] == json!(true) {
        return None;
    }
    serde_json::from_str(text(resp)).ok()
}

#[test]
fn the_gateway_announces_the_surface_name() {
    let out = scenario(
        &[json!({"jsonrpc": "2.0", "id": 1, "method": "initialize"})],
        None,
    );
    assert_eq!(out[&1]["result"]["serverInfo"]["name"], json!("My_Jira"));
}

#[test]
fn tools_list_offers_the_worlds_names_schemas_and_descriptions() {
    let out = scenario(
        &[json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})],
        None,
    );
    let tools = out[&1]["result"]["tools"].as_array().expect("tools");
    let by_name: HashMap<&str, &Value> = tools
        .iter()
        .map(|t| (t["name"].as_str().unwrap(), t))
        .collect();

    // Hidden backings, undeclared tools, and a tool backed by another server are
    // all absent; the scoped tools appear under their own names.
    let mut names: Vec<&str> = by_name.keys().copied().collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            "comment_on_platform_issue",
            "get_platform_issue",
            "searchJiraIssuesUsingJql"
        ]
    );

    let get = by_name["get_platform_issue"];
    assert_eq!(get["description"], json!("Read one PLAT issue by its key."));
    assert_eq!(
        get["inputSchema"],
        json!({
            "type": "object",
            "properties": {"issue": {"type": "string", "pattern": "^PLAT-"}},
            "required": ["issue"],
            "additionalProperties": false,
        })
    );

    // No world description: the upstream's is kept.
    let comment = by_name["comment_on_platform_issue"];
    assert_eq!(comment["description"], json!("Comment on a Jira issue."));
    assert_eq!(
        comment["inputSchema"]["properties"]["issue"]["enum"],
        json!(["PLAT-1", "PLAT-2"])
    );
    assert_eq!(
        comment["inputSchema"]["required"],
        json!(["comment", "issue"])
    );

    // A base action served as-is keeps the world's own schema (D51).
    assert_eq!(
        by_name["searchJiraIssuesUsingJql"]["inputSchema"]["required"],
        json!(["cloudId", "jql"])
    );
}

#[test]
fn an_allowed_call_is_forwarded_as_the_kernel_lowered_it() {
    let out = scenario(
        &[
            call(1, "get_platform_issue", json!({"issue": "PLAT-7"})),
            // A model that also sends the base name and a fixed argument gets
            // neither through.
            call(
                2,
                "get_platform_issue",
                json!({"issue": "PLAT-1", "issueIdOrKey": "SEC-9", "cloudId": "evil"}),
            ),
            call(
                3,
                "searchJiraIssuesUsingJql",
                json!({"cloudId": "c", "jql": "project = PLAT"}),
            ),
        ],
        None,
    );
    assert_eq!(
        received(&out[&1]).expect("forwarded"),
        json!({"received_tool": "getJiraIssue",
               "received_arguments": {"cloudId": "abc-123", "issueIdOrKey": "PLAT-7"}})
    );
    assert_eq!(
        received(&out[&2]).expect("forwarded")["received_arguments"],
        json!({"cloudId": "abc-123", "issueIdOrKey": "PLAT-1"})
    );
    assert_eq!(
        received(&out[&3]).expect("forwarded"),
        json!({"received_tool": "searchJiraIssuesUsingJql",
               "received_arguments": {"cloudId": "c", "jql": "project = PLAT"}})
    );
}

#[test]
fn values_outside_the_limits_never_reach_the_upstream() {
    let out = scenario(
        &[
            call(1, "get_platform_issue", json!({"issue": "SEC-1"})),
            call(
                2,
                "comment_on_platform_issue",
                json!({"issue": "PLAT-3", "comment": "hi"}),
            ),
            // The hidden backing, called by its upstream name.
            call(
                3,
                "getJiraIssue",
                json!({"cloudId": "c", "issueIdOrKey": "SEC-1"}),
            ),
        ],
        None,
    );
    assert!(received(&out[&1]).is_none());
    assert!(text(&out[&1]).starts_with("DENY"), "{}", text(&out[&1]));
    assert!(received(&out[&2]).is_none());
    assert!(text(&out[&2]).starts_with("DENY"), "{}", text(&out[&2]));
    assert!(received(&out[&3]).is_none());
    assert!(text(&out[&3]).starts_with("ABSENT"), "{}", text(&out[&3]));
}

#[test]
fn a_call_this_gateway_does_not_serve_is_refused_without_tainting() {
    let audit = std::env::temp_dir().join(format!(
        "mcp-gateway-scoped-{}-{}.jsonl",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let out = scenario(
        &[
            // Allowed by the kernel, but backed by another server: refused here,
            // and since nothing ran, the session stays clean...
            call(
                1,
                "getConfluencePage",
                json!({"cloudId": "c", "pageId": "1"}),
            ),
            // ...so the external write is still allowed and forwarded.
            call(
                2,
                "comment_on_platform_issue",
                json!({"issue": "PLAT-1", "comment": "triaged"}),
            ),
            // A real read ingests upstream bytes and taints the session...
            call(3, "get_platform_issue", json!({"issue": "PLAT-1"})),
            // ...after which the taint floor severs the write.
            call(
                4,
                "comment_on_platform_issue",
                json!({"issue": "PLAT-1", "comment": "again"}),
            ),
        ],
        Some(&audit),
    );

    assert!(received(&out[&1]).is_none());
    assert!(
        text(&out[&1]).starts_with("REFUSED (gateway)"),
        "{}",
        text(&out[&1])
    );
    assert_eq!(
        received(&out[&2]).expect("forwarded"),
        json!({"received_tool": "addCommentToJiraIssue",
               "received_arguments": {"cloudId": "abc-123", "issueIdOrKey": "PLAT-1",
                                      "commentBody": "triaged"}})
    );
    assert!(received(&out[&3]).is_some());
    assert!(received(&out[&4]).is_none());
    assert!(text(&out[&4]).starts_with("DENY"), "{}", text(&out[&4]));

    let log = std::fs::read_to_string(&audit).expect("audit log");
    let _ = std::fs::remove_file(&audit);
    let refusal: Value = log
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .find(|e| e["stage"] == json!("forward"))
        .expect("the refusal is audited");
    assert_eq!(refusal["tool"], json!("getConfluencePage"));
    assert_eq!(refusal["rule"], json!("not_forwardable"));
}
