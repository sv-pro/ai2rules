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
