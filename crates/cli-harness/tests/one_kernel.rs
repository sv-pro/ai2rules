//! One kernel, many hosts — the conformance suite (docs/one-kernel-many-hosts.md).
//!
//! Consumes the canonical case set `docs/demos/one-kernel/cases.yaml` against
//! `docs/demos/one-kernel/demo-world.yaml` and asserts the SAME verdict —
//! decision, rule, post-call taint, manifest_hash — on every entry point:
//!
//!   (a) in-process `harness_preview::gate()` (what cc-hook / mcp-gateway link)
//!   (b) the `harness gate` CLI (the D24 wire ABI) — full-response parity with (a)
//!   (c) `harness cc-hook` — the PreToolUse event contract (decision surface +
//!       taint sidecar; the hook exposes no rule/hash channel)
//!   (d) OpenCode-shaped GateRequests — the exact wire shape the plugin sends
//!   (e) `harness mcp-gateway` over `harness mock-jira` for the jira cases
//!       (tools/list shaping + isError decision labels)
//!
//! Plus the classifier-consistency test pinning the D36 `command_classes`
//! pattern lists byte-identical across the three host manifests.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn repo_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

fn demo_world() -> PathBuf {
    repo_path("docs/demos/one-kernel/demo-world.yaml")
}

fn load_cases() -> Value {
    let text = std::fs::read_to_string(repo_path("docs/demos/one-kernel/cases.yaml"))
        .expect("read cases.yaml");
    serde_yaml::from_str(&text).expect("parse cases.yaml")
}

fn compile_demo_world() -> harness_types::CompiledWorld {
    let text = std::fs::read_to_string(demo_world()).expect("read demo-world.yaml");
    compiler::compile(&compiler::loader::load_yaml(&text).expect("parse demo world"))
        .expect("compile demo world")
}

/// (a) The in-process gate — the same call cc-hook and mcp-gateway link (D34).
fn gate_in_process(world: &harness_types::CompiledWorld, request: &Value) -> Value {
    let req: harness_preview::GateRequest =
        serde_json::from_value(request.clone()).expect("request deserializes");
    serde_json::to_value(harness_preview::gate(world, &req)).expect("response serializes")
}

/// (b)/(d) The `harness gate` CLI: one GateRequest on stdin → GateResponse on stdout.
fn gate_cli(stdin_payload: &str) -> (i32, String) {
    let bin = env!("CARGO_BIN_EXE_harness");
    let mut child = Command::new(bin)
        .args(["gate", "--world", demo_world().to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn harness gate");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin_payload.as_bytes())
        .unwrap();
    let out = child.wait_with_output().expect("wait harness gate");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8(out.stdout).unwrap(),
    )
}

/// (c) `harness cc-hook`: a PreToolUse event on stdin; returns stdout.
fn cc_hook(state: &Path, event: &Value, mode: &str, enforce_absent: bool) -> String {
    let bin = env!("CARGO_BIN_EXE_harness");
    let mut args = vec![
        "cc-hook".to_string(),
        "--world".to_string(),
        demo_world().to_str().unwrap().to_string(),
        "--state".to_string(),
        state.to_str().unwrap().to_string(),
        "--mode".to_string(),
        mode.to_string(),
    ];
    if enforce_absent {
        args.push("--enforce-absent".to_string());
    }
    let mut child = Command::new(bin)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn cc-hook");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(event.to_string().as_bytes())
        .unwrap();
    let out = child.wait_with_output().expect("wait cc-hook");
    assert!(out.status.success(), "cc-hook must exit 0 (fail-open)");
    String::from_utf8(out.stdout).unwrap()
}

fn hook_decision(out: &str) -> Option<(String, String)> {
    let line = out.lines().find(|l| l.contains("hookSpecificOutput"))?;
    let v: Value = serde_json::from_str(line).ok()?;
    Some((
        v["hookSpecificOutput"]["permissionDecision"]
            .as_str()?
            .to_string(),
        v["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap_or("")
            .to_string(),
    ))
}

/// (e) Drive a gateway subprocess over mock-jira; returns responses by id.
fn gateway_scenario(
    taint: &str,
    mode: &str,
    requests: &[Value],
) -> std::collections::HashMap<i64, Value> {
    let bin = env!("CARGO_BIN_EXE_harness");
    let mut child = Command::new(bin)
        .args([
            "mcp-gateway",
            "--world",
            demo_world().to_str().unwrap(),
            "--taint",
            taint,
            "--mode",
            mode,
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

fn case_field<'a>(case: &'a Value, path: &[&str]) -> &'a Value {
    let mut v = case;
    for p in path {
        v = &v[*p];
    }
    v
}

fn cases() -> Vec<Value> {
    load_cases()["cases"]
        .as_array()
        .expect("cases list")
        .clone()
}

/// The parity beat: for every case, (a) in-process, (b) the wire ABI, and
/// (d) the OpenCode-shaped wire request agree on decision / rule / post-call
/// taint / manifest_hash — the one-kernel guarantee.
#[test]
fn every_entry_point_agrees_with_the_in_process_kernel() {
    let world = compile_demo_world();
    for case in cases() {
        let name = case["name"].as_str().unwrap();
        let request = &case["request"];
        let expect = &case["expect"];

        // (a) in-process — also assert the case's pinned expectations hold.
        let a = gate_in_process(&world, request);
        assert_eq!(a["decision"], expect["decision"], "{name}: decision");
        assert_eq!(a["rule"], expect["rule"], "{name}: rule");
        assert_eq!(a["context"]["taint"], expect["taint"], "{name}: taint");
        if let Some(action) = expect.get("action").filter(|a| !a.is_null()) {
            assert_eq!(&a["action"], action, "{name}: effective action (D36)");
        }
        assert!(
            !a["manifest_hash"].as_str().unwrap().is_empty(),
            "{name}: manifest_hash present"
        );

        // (b) the wire ABI: full-response parity with (a).
        let (code, stdout) = gate_cli(&request.to_string());
        assert_eq!(code, 0, "{name}: gate CLI evaluates");
        let b: Value = serde_json::from_str(&stdout).expect("gate CLI response json");
        assert_eq!(a, b, "{name}: CLI response must equal in-process response");

        // (d) OpenCode-shaped: the plugin's exact wire shape (nulls spelled out,
        // lowercase tool names 1:1). Same four fields must agree.
        let taint = case_field(&case, &["request", "context", "taint"]);
        let oc_req = json!({
            "v": 1,
            "tool": request["tool"],
            "arguments": request.get("arguments").cloned().unwrap_or(json!({})),
            "context": {
                "session_id": name,
                "mode": case_field(&case, &["request", "context", "mode"]),
                "taint": if taint == "tainted" { json!("tainted") } else { Value::Null },
                "source_channel": Value::Null,
                "approval_token": Value::Null,
            },
        });
        let (code, stdout) = gate_cli(&oc_req.to_string());
        assert_eq!(code, 0, "{name}: gate CLI evaluates the OpenCode shape");
        let d: Value = serde_json::from_str(&stdout).expect("opencode response json");
        for field in ["decision", "rule", "action", "manifest_hash"] {
            assert_eq!(
                a[field], d[field],
                "{name}: {field} parity (OpenCode shape)"
            );
        }
        assert_eq!(
            a["context"]["taint"], d["context"]["taint"],
            "{name}: taint parity"
        );
    }
}

/// (c) The cc-hook PreToolUse contract: the kernel's verdict surfaces as the
/// right permissionDecision (or silence), and the taint sidecar tracks the
/// kernel's post-call taint.
#[test]
fn cc_hook_contract_matches_the_case_set() {
    for case in cases() {
        let name = case["name"].as_str().unwrap();
        let request = &case["request"];
        let expect = &case["expect"];
        let decision = expect["decision"].as_str().unwrap();
        let mode = case_field(&case, &["request", "context", "mode"])
            .as_str()
            .unwrap_or("interactive");
        let inbound_tainted = case_field(&case, &["request", "context", "taint"]) == "tainted";

        let dir = tempfile::tempdir().unwrap();
        let sid = name; // one session per case
        if inbound_tainted {
            std::fs::write(dir.path().join(format!("taint-{sid}")), "seed\n").unwrap();
        }
        let event = json!({
            "tool_name": request["tool"],
            "tool_input": request.get("arguments").cloned().unwrap_or(json!({})),
            "session_id": sid,
        });
        // --enforce-absent for the ABSENT cases: the deny channel is the only
        // way a PreToolUse hook can surface "does not exist".
        let out = cc_hook(dir.path(), &event, mode, decision == "ABSENT");

        match decision {
            "ALLOW" => {
                assert!(
                    out.trim().is_empty(),
                    "{name}: ALLOW passes through silently"
                );
            }
            "ASK" => {
                let (d, _) = hook_decision(&out).expect("ASK emits a decision");
                assert_eq!(d, "ask", "{name}");
            }
            "DENY" => {
                let (d, _) = hook_decision(&out).expect("DENY emits a decision");
                assert_eq!(d, "deny", "{name}");
            }
            "ABSENT" => {
                let (d, reason) = hook_decision(&out).expect("--enforce-absent emits a deny");
                assert_eq!(d, "deny", "{name}");
                assert!(
                    reason.starts_with("ABSENT: "),
                    "{name}: ABSENT stays distinguishable from DENY, got {reason:?}"
                );
            }
            other => panic!("{name}: unexpected expected decision {other}"),
        }

        // Post-call taint: the sidecar must exist iff the kernel says tainted.
        let expect_tainted = expect["taint"] == "tainted";
        assert_eq!(
            dir.path().join(format!("taint-{sid}")).exists(),
            expect_tainted,
            "{name}: sidecar tracks the kernel's post-call taint"
        );
    }
}

/// (e) The gateway over mock-jira: shaping (ABSENT tools never offered) and
/// per-call verdicts with distinguishable labels, for the jira-flagged cases.
#[test]
fn mcp_gateway_agrees_on_the_jira_cases() {
    let world = compile_demo_world();
    for case in cases() {
        if case.get("gateway").and_then(|g| g.as_bool()) != Some(true) {
            continue;
        }
        let name = case["name"].as_str().unwrap();
        let request = &case["request"];
        let expect = &case["expect"];
        let decision = expect["decision"].as_str().unwrap();
        let taint = case_field(&case, &["request", "context", "taint"])
            .as_str()
            .unwrap_or("clean");
        let mode = case_field(&case, &["request", "context", "mode"])
            .as_str()
            .unwrap_or("interactive");

        let r = gateway_scenario(
            taint,
            mode,
            &[
                json!({"jsonrpc":"2.0","id":1,"method":"initialize"}),
                json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
                json!({"jsonrpc":"2.0","id":3,"method":"tools/call",
                       "params":{"name": request["tool"],
                                 "arguments": request.get("arguments").cloned().unwrap_or(json!({}))}}),
            ],
        );

        let listed: Vec<String> = r[&2]["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect();
        let tool = request["tool"].as_str().unwrap();
        let is_error = r[&3]["result"]["isError"].as_bool().unwrap_or(false);
        let text = r[&3]["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string();

        match decision {
            "ALLOW" => {
                assert!(listed.contains(&tool.to_string()), "{name}: offered");
                assert!(!is_error, "{name}: forwarded upstream, got {text:?}");
            }
            "ABSENT" => {
                assert!(
                    !listed.contains(&tool.to_string()),
                    "{name}: an ABSENT tool is never offered in tools/list"
                );
                assert!(is_error, "{name}");
                assert!(text.starts_with("ABSENT: "), "{name}: label, got {text:?}");
            }
            "DENY" => {
                assert!(is_error, "{name}");
                assert!(text.starts_with("DENY: "), "{name}: label, got {text:?}");
            }
            "ASK" => {
                assert!(is_error, "{name}");
                assert!(text.starts_with("ASK: "), "{name}: label, got {text:?}");
            }
            other => panic!("{name}: unexpected expected decision {other}"),
        }

        // Cross-check: the gateway's verdict is the in-process kernel's verdict.
        let a = gate_in_process(&world, request);
        assert_eq!(a["decision"], expect["decision"], "{name}: kernel agrees");
    }
}

/// A PROCESS failure is never a verdict: garbage stdin → `harness gate` exits 2
/// with no verdict on stdout; `harness cc-hook` exits 0 emitting nothing
/// (fail-open). Neither surface turns a process error into a DENY.
#[test]
fn malformed_gate_request_is_a_process_error_not_a_deny() {
    let garbage = load_cases()["malformed_input"]
        .as_str()
        .expect("malformed_input in cases.yaml")
        .to_string();

    let (code, stdout) = gate_cli(&garbage);
    assert_eq!(code, 2, "malformed request is a process error (exit 2)");
    assert!(stdout.trim().is_empty(), "no verdict on stdout");

    let dir = tempfile::tempdir().unwrap();
    let bin = env!("CARGO_BIN_EXE_harness");
    let mut child = Command::new(bin)
        .args([
            "cc-hook",
            "--world",
            demo_world().to_str().unwrap(),
            "--state",
            dir.path().to_str().unwrap(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn cc-hook");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(garbage.as_bytes())
        .unwrap();
    let out = child.wait_with_output().expect("wait cc-hook");
    assert!(out.status.success(), "cc-hook fails open (exit 0)");
    assert!(
        out.stdout.is_empty(),
        "cc-hook emits nothing on a process error"
    );
}

/// D36 classifier consistency: the `command_classes` pattern lists are
/// byte-identical across the three host manifests (the class targets differ
/// only in the hosts' action-name casing).
#[test]
fn command_class_patterns_are_identical_across_host_manifests() {
    let manifests = [
        "docs/demos/one-kernel/demo-world.yaml",
        ".claude/cc-world.yaml",
        "docs/demos/opencode/opencode-world.yaml",
    ];
    let mut pattern_sets: Vec<Vec<Vec<String>>> = Vec::new();
    for rel in manifests {
        let text = std::fs::read_to_string(repo_path(rel)).expect("read manifest");
        let manifest = compiler::loader::load_yaml(&text).expect("parse manifest");
        assert_eq!(
            manifest.command_classes.len(),
            1,
            "{rel}: exactly one classifier"
        );
        let def = &manifest.command_classes[0];
        assert_eq!(def.arg, "command", "{rel}: classifies the `command` arg");
        pattern_sets.push(
            def.classes
                .iter()
                .map(|c| c.patterns.clone())
                .collect::<Vec<_>>(),
        );
    }
    assert_eq!(
        pattern_sets[0], pattern_sets[1],
        "demo-world vs cc-world pattern drift"
    );
    assert_eq!(
        pattern_sets[0], pattern_sets[2],
        "demo-world vs opencode-world pattern drift"
    );
}
