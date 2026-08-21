use serde_json::{json, Value};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn world() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("docs/demos/deepseek-harness/deepseek-world.yaml")
}

fn run(request: &Value) -> (i32, String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_harness"))
        .args(["project", "--world", world().to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn harness project");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(request.to_string().as_bytes())
        .unwrap();
    let output = child.wait_with_output().expect("wait harness project");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

#[test]
fn project_filters_absent_and_returns_joinable_identities() {
    let read = json!({
        "name": "read",
        "description": "host registry schema",
        "parameters": {"type":"object","properties":{"path":{"type":"string"}}}
    });
    let (code, stdout, stderr) = run(&json!({
        "v": 1,
        "context": {"source_channel": "user_cli"},
        "tools": [read.clone(), {"name":"todo_write","parameters":{"type":"object"}}]
    }));
    assert_eq!(code, 0, "project evaluates: {stderr}");
    let response: Value = serde_json::from_str(&stdout).expect("projection response");
    assert_eq!(response["tools"], json!([read]));
    assert_eq!(response["absent"], json!(["todo_write"]));
    assert_eq!(response["manifest_hash"].as_str().unwrap().len(), 12);
    assert_eq!(response["schema_hash"].as_str().unwrap().len(), 64);
}

#[test]
fn project_rejects_duplicate_schema_names() {
    let (code, stdout, stderr) = run(&json!({
        "v": 1,
        "context": {"source_channel": "user_cli"},
        "tools": [{"name":"read"},{"name":"read"}]
    }));
    assert_eq!(code, 2);
    assert!(stdout.trim().is_empty());
    assert!(stderr.contains("duplicate tool name"));
}
