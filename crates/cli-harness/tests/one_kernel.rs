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
//!   (f) `harness agy-hook` — the Antigravity CLI PreToolUse contract, fed the
//!       host's real payload shape (camelCase envelope + PascalCase argument
//!       keys), so the adapter's shape translation is inside the parity claim
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

/// (f) `harness agy-hook`: an Antigravity PreToolUse payload on stdin; stdout.
fn agy_hook(state: &Path, event: &Value, mode: &str, enforce_absent: bool) -> String {
    let bin = env!("CARGO_BIN_EXE_harness");
    let mut args = vec![
        "agy-hook".to_string(),
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
        .expect("spawn agy-hook");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(event.to_string().as_bytes())
        .unwrap();
    let out = child.wait_with_output().expect("wait agy-hook");
    assert!(out.status.success(), "agy-hook must exit 0 (fail-open)");
    String::from_utf8(out.stdout).unwrap()
}

/// Spell a case's neutral arguments the way Antigravity actually sends them
/// (PascalCase), so the conformance run exercises the adapter's alias step
/// rather than bypassing it. The kernel must still reach the same verdict.
fn agy_args(arguments: &Value) -> Value {
    let mut out = serde_json::Map::new();
    for (k, v) in arguments.as_object().cloned().unwrap_or_default() {
        let host_key = match k.as_str() {
            "command" => "CommandLine",
            "file_path" | "path" | "notebook_path" => "AbsolutePath",
            other => other,
        };
        out.insert(host_key.to_string(), v);
    }
    Value::Object(out)
}

/// The Antigravity decision channel: `{}` (no `decision`) is the passthrough.
fn agy_decision(out: &str) -> Option<(String, String)> {
    let v: Value = serde_json::from_str(out.trim()).ok()?;
    Some((
        v.get("decision")?.as_str()?.to_string(),
        v.get("reason")
            .and_then(|r| r.as_str())
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
                "taint": if taint == "tainted" { json!("tainted") } else { json!("clean") },
                "source_channel": "user_prompt",
                "approval_token": Value::Null,
                "usage": {},
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

#[test]
fn harness_gate_does_not_trust_request_supplied_approval_tokens() {
    let request = json!({
        "v": 1,
        "tool": "bash",
        "arguments": { "command": "rm -rf /tmp/x" },
        "context": {
            "session_id": "forged-token-session",
            "mode": "interactive",
            "taint": "clean",
            "source_channel": "user_prompt",
            "approval_token": "forged-token",
            "usage": {}
        }
    });

    let (code, stdout) = gate_cli(&request.to_string());
    assert_eq!(code, 0, "gate CLI evaluates forged approval token request");
    let response: Value = serde_json::from_str(&stdout).expect("gate CLI response json");
    assert_eq!(response["action"], "bash_destructive");
    assert_eq!(response["decision"], "ASK");
    assert_eq!(response["rule"], "approval_required");
    assert_eq!(response["approval"]["required"], true);
}

#[test]
fn harness_gate_fails_closed_on_missing_or_malformed_context() {
    for (name, context, rule) in [
        (
            "missing_taint",
            json!({"session_id": "bad-context", "mode": "interactive", "source_channel": "user_prompt", "usage": {}}),
            "missing_taint",
        ),
        (
            "invalid_taint",
            json!({"session_id": "bad-context", "mode": "interactive", "taint": "clean-ish", "source_channel": "user_prompt", "usage": {}}),
            "invalid_taint",
        ),
        (
            "missing_source",
            json!({"session_id": "bad-context", "mode": "interactive", "taint": "clean", "usage": {}}),
            "missing_source_channel",
        ),
        (
            "invalid_source",
            json!({"session_id": "bad-context", "mode": "interactive", "taint": "clean", "source_channel": "probably_user", "usage": {}}),
            "invalid_source_channel",
        ),
        // Finding #16: demo-world.yaml counts calls, so a caller that omits its
        // budget counters is refused rather than handed an unlimited session.
        (
            "missing_usage",
            json!({"session_id": "bad-context", "mode": "interactive", "taint": "clean", "source_channel": "user_prompt"}),
            "missing_usage",
        ),
    ] {
        let request = json!({
            "v": 1,
            "tool": "jira_add_comment",
            "arguments": { "issue_key": "DEMO-1", "body": name },
            "context": context
        });

        let (code, stdout) = gate_cli(&request.to_string());
        assert_eq!(code, 0, "{name}: gate CLI evaluates");
        let response: Value = serde_json::from_str(&stdout).expect("gate CLI response json");
        assert_eq!(response["decision"], "DENY", "{name}");
        assert_eq!(response["rule"], rule, "{name}");
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

/// (f) The agy-hook PreToolUse contract: the same case set, sent in
/// Antigravity's real payload shape, must produce the same kernel verdict —
/// mapped onto Antigravity's decision vocabulary (`force_ask` for ASK, because
/// its plain `ask` respects cached "Always Allow" grants) — and the same
/// post-call taint. This puts the adapter's envelope + PascalCase translation
/// inside the one-kernel parity claim (D48).
#[test]
fn agy_hook_contract_matches_the_case_set() {
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
        let sid = name; // one conversation per case
        if inbound_tainted {
            std::fs::write(dir.path().join(format!("taint-{sid}")), "seed\n").unwrap();
        }
        let arguments = request.get("arguments").cloned().unwrap_or(json!({}));
        let event = json!({
            "conversationId": sid,
            "stepIdx": 1,
            "workspacePaths": [],
            "toolCall": { "name": request["tool"], "args": agy_args(&arguments) },
        });
        // --enforce-absent for the ABSENT cases: the deny channel is the only
        // way a PreToolUse hook can surface "does not exist".
        let out = agy_hook(dir.path(), &event, mode, decision == "ABSENT");

        match decision {
            "ALLOW" => {
                let v: Value = serde_json::from_str(out.trim())
                    .unwrap_or_else(|e| panic!("{name}: agy expects JSON, got {out:?} ({e})"));
                assert!(
                    v.get("decision").is_none(),
                    "{name}: ALLOW passes through as the no-decision no-op, got {out:?}"
                );
            }
            "ASK" => {
                let (d, _) = agy_decision(&out).expect("ASK emits a decision");
                assert_eq!(d, "force_ask", "{name}: ASK uses the strict channel");
            }
            "DENY" => {
                let (d, _) = agy_decision(&out).expect("DENY emits a decision");
                assert_eq!(d, "deny", "{name}");
            }
            "ABSENT" => {
                let (d, reason) = agy_decision(&out).expect("--enforce-absent emits a deny");
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
/// byte-identical across the host manifests (the class/default targets differ
/// only in the hosts' action-name casing).
#[test]
fn command_class_patterns_are_identical_across_host_manifests() {
    let manifests = [
        "docs/demos/one-kernel/demo-world.yaml",
        ".claude/cc-world.yaml",
        "docs/demos/opencode/opencode-world.yaml",
        "scripts/starter-world.yaml",
        ".agents/agy-world.yaml",
    ];
    let mut pattern_sets: Vec<Vec<Vec<String>>> = Vec::new();
    let mut default_targets: Vec<String> = Vec::new();
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
        let default_to = def
            .default_to
            .as_ref()
            .expect("shell classifier default_to");
        assert!(
            default_to
                .as_str()
                .to_ascii_lowercase()
                .ends_with("_unclassified"),
            "{rel}: default_to must be an unclassified shell fallback"
        );
        default_targets.push(default_to.as_str().to_ascii_lowercase());
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
    assert_eq!(
        pattern_sets[0], pattern_sets[3],
        "demo-world vs starter-world pattern drift"
    );
    assert_eq!(
        pattern_sets[0], pattern_sets[4],
        "demo-world vs agy-world pattern drift"
    );
    assert!(
        default_targets.iter().all(|d| d.ends_with("_unclassified")),
        "all shell classifiers fail closed to an unclassified fallback"
    );
}

// ---------------------------------------------------------------------------
// Path-scope parity (spatial confinement, D46) — docs/demos/one-kernel/roots-*.yaml
// ---------------------------------------------------------------------------
//
// Finding #15 shipped because the section above had no path cases: the suite
// that compares adapters against each other never exercised the one feature
// where they had silently diverged. cc-hook canonicalized manifest roots and
// agy-hook did not, so the same world denied a write on one host and granted it
// on the other. These tests close that gap for every entry point that can carry
// a resolved path.

/// A real project tree for path-scope, with the layout `roots-world.yaml`
/// describes. Returns the CANONICALIZED root, because every path comparison
/// downstream happens after canonicalization and the fixture must not be the
/// one thing that is lexical.
#[cfg(unix)]
fn roots_project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    for sub in ["src", "inbox", "vendor", "private", "untracked"] {
        std::fs::create_dir_all(root.join(sub)).unwrap();
    }
    std::fs::write(root.join("src/lib.rs"), "// code\n").unwrap();
    std::fs::write(root.join("inbox/message.txt"), "untrusted\n").unwrap();
    std::fs::write(root.join("vendor/dep.js"), "// dep\n").unwrap();
    std::fs::write(root.join("vendor/.env"), "SECRET=1\n").unwrap();
    std::fs::write(root.join("private/id_rsa"), "KEY\n").unwrap();
    // The non-canonical root rule: {project}/link resolves to {project}/private.
    std::os::unix::fs::symlink(root.join("private"), root.join("link")).unwrap();
    (dir, root)
}

#[cfg(unix)]
fn subst(text: &str, project: &Path) -> String {
    text.replace("{project}", project.to_str().unwrap())
}

/// Render `roots-world.yaml` against a real project into a temp file.
#[cfg(unix)]
fn roots_world_file(project: &Path, out_dir: &Path) -> PathBuf {
    let src = std::fs::read_to_string(repo_path("docs/demos/one-kernel/roots-world.yaml"))
        .expect("read roots-world.yaml");
    let path = out_dir.join("roots-world.yaml");
    std::fs::write(&path, subst(&src, project)).unwrap();
    path
}

#[cfg(unix)]
fn roots_cases(project: &Path) -> Vec<Value> {
    let src = std::fs::read_to_string(repo_path("docs/demos/one-kernel/roots-cases.yaml"))
        .expect("read roots-cases.yaml");
    let doc: Value = serde_yaml::from_str(&subst(&src, project)).expect("parse roots-cases.yaml");
    doc["cases"].as_array().expect("cases list").clone()
}

/// Compile the roots world the way an adapter does: canonicalize the rule paths
/// through the filesystem before the pure compile. This mirrors
/// `hostkit::canonicalize_root_paths`, which lives in the binary and so cannot be
/// imported here; the whole point of the test is that every real entry point
/// performs this step, so the in-process leg has to perform it too.
#[cfg(unix)]
fn compile_roots_world(world_path: &Path) -> harness_types::CompiledWorld {
    let text = std::fs::read_to_string(world_path).expect("read roots world");
    let mut manifest = compiler::loader::load_yaml(&text).expect("parse roots world");
    if let Some(roots) = &manifest.roots {
        let mut resolved = roots.clone();
        for rule in &mut resolved.rules {
            if let Ok(c) = std::fs::canonicalize(&rule.path) {
                rule.path = c.to_string_lossy().into_owned();
            }
        }
        manifest.roots = Some(resolved);
    }
    compiler::compile(&manifest).expect("compile roots world")
}

/// Run one `GateRequest` through the `harness gate` CLI against an explicit world.
fn gate_cli_with_world(world: &Path, stdin_payload: &str) -> (i32, String) {
    let bin = env!("CARGO_BIN_EXE_harness");
    let mut child = Command::new(bin)
        .args(["gate", "--world", world.to_str().unwrap()])
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

/// Run a host adapter subprocess against an explicit world, with `HOME` pinned
/// so `~` expansion cannot reach the developer's real home directory.
fn hook_with_world(
    subcommand: &str,
    world: &Path,
    state: &Path,
    home: &Path,
    event: &Value,
    mode: &str,
) -> String {
    let bin = env!("CARGO_BIN_EXE_harness");
    let mut child = Command::new(bin)
        .args([
            subcommand,
            "--world",
            world.to_str().unwrap(),
            "--state",
            state.to_str().unwrap(),
            "--mode",
            mode,
        ])
        .env("HOME", home)
        .env("CLAUDE_PROJECT_DIR", home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn hook");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(event.to_string().as_bytes())
        .unwrap();
    let out = child.wait_with_output().expect("wait hook");
    assert!(out.status.success(), "{subcommand} must exit 0 (fail-open)");
    String::from_utf8(out.stdout).unwrap()
}

/// The path-scope parity beat: (a) in-process and (b) the wire ABI must agree,
/// case by case, on decision / rule / post-call taint.
#[cfg(unix)]
#[test]
fn every_entry_point_agrees_on_path_scope() {
    let (_tmp, project) = roots_project();
    let out = tempfile::tempdir().unwrap();
    let world_path = roots_world_file(&project, out.path());
    let world = compile_roots_world(&world_path);

    for case in roots_cases(&project) {
        let name = case["name"].as_str().unwrap();
        let request = &case["request"];
        let expect = &case["expect"];

        // (a) in-process — the pinned expectation is the kernel's own answer.
        let a = gate_in_process(&world, request);
        assert_eq!(a["decision"], expect["decision"], "{name}: decision");
        assert_eq!(a["rule"], expect["rule"], "{name}: rule");
        assert_eq!(a["context"]["taint"], expect["taint"], "{name}: taint");

        // (b) the wire ABI: full-response parity with (a). This is what catches a
        // `harness gate` that compiles the world without resolving its roots
        // (finding #26) — the rule paths would stay lexical and the symlinked
        // Deny would fall through to `default`.
        let (code, stdout) = gate_cli_with_world(&world_path, &request.to_string());
        assert_eq!(code, 0, "{name}: gate CLI evaluates");
        let b: Value = serde_json::from_str(&stdout).expect("gate CLI response json");
        assert_eq!(a, b, "{name}: CLI response must equal in-process response");
    }
}

/// The same case set through both host adapters, in each host's real payload
/// shape, with the path spelled the way the host actually sends it — so the
/// adapters' own path resolution is inside the parity claim. This is the test
/// that fails if either adapter stops canonicalizing manifest roots.
#[cfg(unix)]
#[test]
fn both_host_adapters_agree_on_path_scope() {
    let (_tmp, project) = roots_project();
    let out = tempfile::tempdir().unwrap();
    let world_path = roots_world_file(&project, out.path());

    for case in roots_cases(&project) {
        let name = case["name"].as_str().unwrap();
        let request = &case["request"];
        let expect = &case["expect"];
        let decision = expect["decision"].as_str().unwrap();
        let mode = case_field(&case, &["request", "context", "mode"])
            .as_str()
            .unwrap_or("interactive");
        // The RAW host spelling — pre-canonicalization. For the symlink case this
        // is {project}/link/id_rsa, which only reaches the Deny rule if the
        // adapter resolves both the target and the root through the filesystem.
        let arguments = request.get("arguments").cloned().unwrap_or(json!({}));

        let cc_state = tempfile::tempdir().unwrap();
        let cc = hook_with_world(
            "cc-hook",
            &world_path,
            cc_state.path(),
            &project,
            &json!({
                "tool_name": request["tool"],
                "tool_input": arguments,
                "session_id": name,
            }),
            mode,
        );

        let agy_state = tempfile::tempdir().unwrap();
        let agy = hook_with_world(
            "agy-hook",
            &world_path,
            agy_state.path(),
            &project,
            &json!({
                "conversationId": name,
                "stepIdx": 1,
                "workspacePaths": [project.to_str().unwrap()],
                "toolCall": { "name": request["tool"], "args": agy_args(&arguments) },
            }),
            mode,
        );

        match decision {
            "ALLOW" => {
                assert!(
                    cc.trim().is_empty(),
                    "{name}: cc-hook ALLOW is silent, got {cc:?}"
                );
                let v: Value = serde_json::from_str(agy.trim())
                    .unwrap_or_else(|e| panic!("{name}: agy expects JSON, got {agy:?} ({e})"));
                assert!(
                    v.get("decision").is_none(),
                    "{name}: agy ALLOW is the no-decision no-op, got {agy:?}"
                );
            }
            "ASK" => {
                let (d, _) = hook_decision(&cc).expect("cc-hook ASK emits a decision");
                assert_eq!(d, "ask", "{name}: cc-hook");
                let (d, _) = agy_decision(&agy).expect("agy-hook ASK emits a decision");
                assert_eq!(d, "force_ask", "{name}: agy-hook uses the strict channel");
            }
            "DENY" => {
                let (d, reason) = hook_decision(&cc).expect("cc-hook DENY emits a decision");
                assert_eq!(d, "deny", "{name}: cc-hook — {reason}");
                let (d, reason) = agy_decision(&agy).expect("agy-hook DENY emits a decision");
                assert_eq!(d, "deny", "{name}: agy-hook — {reason}");
            }
            other => panic!("{name}: unexpected expected decision {other}"),
        }

        // Post-call taint: the sidecar exists iff the kernel escalated. This is
        // what pins `taint_source` roots across both hosts.
        let expect_tainted = expect["taint"] == "tainted";
        assert_eq!(
            cc_state.path().join(format!("taint-{name}")).exists(),
            expect_tainted,
            "{name}: cc-hook sidecar tracks post-call taint"
        );
        assert_eq!(
            agy_state.path().join(format!("taint-{name}")).exists(),
            expect_tainted,
            "{name}: agy-hook sidecar tracks post-call taint"
        );
    }
}
