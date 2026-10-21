use std::process::Command;

#[test]
fn init_then_demo_proves_kernel_without_side_effects() {
    let project = tempfile::tempdir().expect("temp project");
    let harness = env!("CARGO_BIN_EXE_harness");

    let init = Command::new(harness)
        .arg("init")
        .arg(project.path())
        .output()
        .expect("run harness init");
    assert!(
        init.status.success(),
        "init failed:\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&init.stdout),
        String::from_utf8_lossy(&init.stderr)
    );

    let demo = Command::new(harness)
        .arg("demo")
        .current_dir(project.path())
        .output()
        .expect("run harness demo");
    let stdout = String::from_utf8_lossy(&demo.stdout);
    let stderr = String::from_utf8_lossy(&demo.stderr);
    assert!(
        demo.status.success(),
        "demo failed:\nstdout={stdout}\nstderr={stderr}"
    );

    assert!(stdout.contains("1. workspace-local write"));
    assert!(stdout.contains("verdict: ALLOW"));
    assert!(stdout.contains("2. out-of-root write (background fail-closed)"));
    assert!(stdout.contains("3. tainted input -> network effect"));
    assert!(stdout.contains("4. approval-required destructive command (interactive)"));
    assert!(stdout.contains("verdict: ASK"));
    assert!(stdout.contains("5. approval-required destructive command (background)"));
    assert!(stdout.contains("Replay: 5/5 decisions reproduced"));
    assert!(stdout.trim_end().ends_with("Next: harness doctor"));

    // The gate is decision-only. The ALLOW scenario must not create the proposed file.
    assert!(!project.path().join("ai2rules-demo-local.txt").exists());

    let trace_line = stdout
        .lines()
        .find_map(|line| line.strip_prefix("Trace: "))
        .expect("trace path printed");
    let trace = std::fs::read_to_string(trace_line).expect("trace persists after demo");
    assert_eq!(trace.lines().count(), 5);
    assert!(trace.contains("workspace-local write"));
    assert!(trace.contains("manifest_hash"));
}

#[test]
fn demo_without_init_fails_with_one_clear_next_step() {
    let project = tempfile::tempdir().expect("temp project");
    let harness = env!("CARGO_BIN_EXE_harness");

    let output = Command::new(harness)
        .arg("demo")
        .current_dir(project.path())
        .output()
        .expect("run harness demo");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("run `harness init` in this project first"));
}

#[test]
fn law_firm_story_needs_no_init_and_earns_its_refusals() {
    // Runs from an empty folder: the story brings its own world.
    let dir = tempfile::tempdir().expect("temp dir");
    let harness = env!("CARGO_BIN_EXE_harness");

    let story = Command::new(harness)
        .args(["demo", "--story", "law-firm"])
        .current_dir(dir.path())
        .output()
        .expect("run the story");
    let stdout = String::from_utf8_lossy(&story.stdout);
    let stderr = String::from_utf8_lossy(&story.stderr);
    assert!(
        story.status.success(),
        "story failed:\nstdout={stdout}\nstderr={stderr}"
    );

    for marker in [
        "[1] Open the case file",
        "[3] Read the new email",
        "ALLOW | session now TAINTED",
        "[4] Email settlement-terms.docx to archive-backup@proton.me",
        "DENY (taint_invariant)",
        "ASK (approval_required)",
        "DENY (background_denies_ask)",
        "ABSENT (unknown_to_ontology)",
        "== Control: a session that never opened the inbox ==",
        "Replay: 10/10 decisions reproduced from the log",
    ] {
        assert!(stdout.contains(marker), "missing {marker:?} in:\n{stdout}");
    }

    // Decision-only: nothing was written where it ran.
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);

    // The taint is earned, not set: the session goes in clean until the email is
    // read, and the refusals arrive with the taint the kernel itself returned.
    let trace_line = stdout
        .lines()
        .find_map(|line| line.split(" decisions written to ").nth(1))
        .expect("trace path printed");
    let trace = std::fs::read_to_string(trace_line).expect("trace persists");
    let records: Vec<serde_json::Value> = trace
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(records.len(), 10);
    let taint_in = |i: usize| records[i]["request"]["context"]["taint"].as_str().unwrap();
    assert_eq!(
        taint_in(2),
        "clean",
        "the email is read from a clean session"
    );
    assert_eq!(records[2]["response"]["context"]["taint"], "tainted");
    assert_eq!(
        taint_in(3),
        "tainted",
        "the refusal inherits the kernel's taint"
    );
    assert_eq!(records[3]["response"]["decision"], "DENY");
    assert_eq!(taint_in(9), "clean", "the control session starts clean");
    assert_eq!(records[9]["response"]["decision"], "ALLOW");
}
