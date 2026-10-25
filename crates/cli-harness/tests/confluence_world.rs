//! PR #16 / AI2-71: the shipped Confluence world must retain remote-read taint
//! and identify a write target by tenant AND object. No live Atlassian calls.

use harness_types::CompiledWorld;
use serde_json::{json, Value};
use std::io::{Seek, Write};
use std::process::{Command, Stdio};

const WORLD: &str = include_str!("../../../docs/demos/confluence-docs/confluence-docs.world.yaml");
const CLOUD: &str = "test-tenant";
const SPACE: &str = "65709";
const PAGE: &str = "196809";

fn configured_world() -> String {
    WORLD
        .replace("REPLACE_WITH_CLOUD_ID", CLOUD)
        .replace("REPLACE_WITH_SPACE_ID", SPACE)
        .replace("REPLACE_WITH_PAGE_ID", PAGE)
}

fn world() -> CompiledWorld {
    compiler::compile(&compiler::load_yaml(&configured_world()).unwrap()).unwrap()
}

fn clean_context() -> Value {
    json!({"source_channel":"user_cli", "taint":"clean", "mode":"interactive",
           "session_id":"confluence-test", "usage":{}})
}

fn gate(world: &CompiledWorld, tool: &str, arguments: Value, context: Value) -> Value {
    let request = serde_json::from_value(json!({
        "tool":tool, "arguments":arguments, "context":context
    }))
    .unwrap();
    serde_json::to_value(harness_preview::gate(world, &request)).unwrap()
}

fn create_args() -> Value {
    json!({"cloudId":CLOUD, "spaceId":SPACE, "title":"Test", "body":"Test body"})
}

fn update_args() -> Value {
    json!({"cloudId":CLOUD, "pageId":PAGE, "body":"Test update"})
}

#[test]
fn every_remote_read_taints_and_severs_subsequent_writes() {
    let world = world();
    for (tool, args) in [
        ("getAccessibleAtlassianResources", json!({})),
        ("getConfluenceSpaces", json!({"cloudId":CLOUD})),
        (
            "getPagesInConfluenceSpace",
            json!({"cloudId":CLOUD, "spaceId":SPACE}),
        ),
        ("getConfluencePage", json!({"cloudId":CLOUD, "pageId":PAGE})),
        (
            "searchConfluenceUsingCql",
            json!({"cloudId":CLOUD, "cql":"type = page"}),
        ),
    ] {
        let read = gate(&world, tool, args.clone(), clean_context());
        assert_eq!(read["decision"], "ALLOW", "{tool}: {read}");
        assert_eq!(read["context"]["taint"], "tainted", "{tool}: {read}");
        assert_eq!(
            read["effective"]["backing"],
            json!({"kind":"mcp", "server":"confluence", "tool":tool})
        );
        // Carry the actual returned state, retaining the host's channel/mode/session.
        let mut carried = clean_context();
        carried["taint"] = read["context"]["taint"].clone();
        carried["usage"] = read["context"]["usage"].clone();
        for (write, args) in [
            ("createConfluencePage", create_args()),
            ("updateConfluencePage", update_args()),
        ] {
            let result = gate(&world, write, args, carried.clone());
            assert_eq!(result["decision"], "DENY", "{tool} -> {write}: {result}");
            assert_eq!(result["rule"], "taint_invariant", "{result}");
        }
        let again = gate(&world, tool, args, carried);
        assert_eq!(
            again["decision"], "ALLOW",
            "reads remain available: {again}"
        );
        assert_eq!(again["context"]["taint"], "tainted");
    }
}

fn assert_write_scope(tool: &str, args: Value, object_key: &str, valid_decision: &str) {
    let world = world();
    let correct = gate(&world, tool, args.clone(), clean_context());
    assert_eq!(correct["decision"], valid_decision, "{correct}");
    for key in ["cloudId", object_key] {
        // Change one coordinate at a time, holding the other pinned coordinate fixed.
        for wrong in [json!("another-object-or-tenant"), Value::Null, json!(42)] {
            let mut altered = args.clone();
            altered[key] = wrong;
            let result = gate(&world, tool, altered, clean_context());
            assert_eq!(result["decision"], "DENY", "{key}: {result}");
            assert_eq!(result["rule"], "schema_violation", "{result}");
        }
        let mut missing = args.clone();
        missing.as_object_mut().unwrap().remove(key);
        let result = gate(&world, tool, missing, clean_context());
        assert_eq!(result["decision"], "DENY", "missing {key}: {result}");
        assert_eq!(result["rule"], "schema_violation", "{result}");
    }
}

#[test]
fn create_requires_both_the_pinned_tenant_and_space() {
    assert_write_scope("createConfluencePage", create_args(), "spaceId", "ALLOW");
}

#[test]
fn update_requires_both_the_pinned_tenant_and_page_before_asking() {
    assert_write_scope("updateConfluencePage", update_args(), "pageId", "ASK");
}

#[test]
fn background_update_and_undeclared_tools_remain_blocked() {
    let world = world();
    let mut context = clean_context();
    context["mode"] = json!("background");
    let result = gate(&world, "updateConfluencePage", update_args(), context);
    assert_eq!(result["decision"], "DENY", "{result}");
    assert_eq!(result["rule"], "background_denies_ask");
    for tool in ["createJiraIssue", "createConfluenceComment"] {
        let result = gate(&world, tool, json!({}), clean_context());
        assert_eq!(result["decision"], "ABSENT", "{result}");
    }
}

#[test]
fn gateway_carries_read_taint_into_the_next_create() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = dir.path().join("world.yaml");
    let audit = dir.path().join("audit.jsonl");
    std::fs::write(&manifest, configured_world()).unwrap();
    let mut input = tempfile::tempfile().unwrap();
    for (id, tool, args) in [
        (
            1,
            "getConfluencePage",
            json!({"cloudId":CLOUD, "pageId":PAGE}),
        ),
        (2, "createConfluencePage", create_args()),
    ] {
        writeln!(
            input,
            "{}",
            json!({"jsonrpc":"2.0", "id":id, "method":"tools/call",
            "params":{"name":tool, "arguments":args}})
        )
        .unwrap();
    }
    input.rewind().unwrap();
    let bin = env!("CARGO_BIN_EXE_harness");
    let output = Command::new(bin)
        .arg("mcp-gateway")
        .arg("--world")
        .arg(manifest)
        .arg("--audit")
        .arg(&audit)
        .args(["--taint", "clean", "--", bin, "mock-jira", "--rovo"])
        .stdin(Stdio::from(input))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(responses.len(), 2, "{responses:?}");
    assert_eq!(responses[0]["id"], 1);
    assert_ne!(
        responses[0]["result"]["isError"], true,
        "{:?}",
        responses[0]
    );
    assert!(
        responses[0]["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Mock Page"),
        "the read must reach the upstream"
    );
    assert_eq!(responses[1]["id"], 2);
    assert_eq!(responses[1]["result"]["isError"], true);
    assert!(
        responses[1]["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("taint_invariant"),
        "{:?}",
        responses[1]
    );
    let records: Vec<Value> = std::fs::read_to_string(audit)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let create = records
        .iter()
        .find(|v| v["tool"] == "createConfluencePage" && v["stage"] == "call")
        .unwrap();
    assert_eq!(create["taint_in"], true);
    assert_eq!(create["decision"], "DENY");
    assert_eq!(create["rule"], "taint_invariant");
}
