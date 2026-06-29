//! End-to-end tests for `harness shim install` and `harness shim exec`
//! (PATH-level governance shims, E16 copilot-governance-shims slice).
//!
//! Tests run against the real demo world (`.claude/cc-world.yaml`) and a
//! throwaway temp dir for the taint sidecar — the live session is never touched.

use std::path::PathBuf;
use std::process::{Command, Stdio};

fn world() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.claude/cc-world.yaml")
}

fn harness() -> &'static str {
    env!("CARGO_BIN_EXE_harness")
}

/// Run `harness shim exec` for `tool` with the given argv, an optional
/// pre-seeded taint file, and `--background` to collapse ASK→DENY.
/// Returns (stderr, exit_code).
fn run_exec(
    state: &std::path::Path,
    tool: &str,
    real: &str,
    extra_flags: &[&str],
    shim_args: &[&str],
) -> (String, i32) {
    let mut cmd = Command::new(harness());
    cmd.args(["shim", "exec"])
        .args(["--world", world().to_str().unwrap()])
        .args(["--tool", tool])
        .args(["--real", real])
        .args(["--state", state.to_str().unwrap()])
        .args(extra_flags)
        .arg("--");
    for a in shim_args {
        cmd.arg(a);
    }
    let out = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn shim exec");
    let stderr = String::from_utf8(out.stderr).unwrap();
    let code = out.status.code().unwrap_or(-1);
    (stderr, code)
}

// ---------------------------------------------------------------------------
// exec tests
// ---------------------------------------------------------------------------

#[test]
fn safe_bash_echo_passes_through() {
    let dir = tempfile::tempdir().unwrap();
    // bash -c "echo hello" → Bash (plain process) → ALLOW → exec /usr/bin/true
    let (stderr, code) = run_exec(dir.path(), "bash", "/usr/bin/true", &[], &["-c", "echo hello"]);
    assert_eq!(code, 0, "ALLOW should exec real binary (exit 0); stderr: {stderr}");
    assert!(!stderr.contains("DENIED"), "unexpected deny: {stderr}");
}

#[test]
fn destructive_bash_asks_and_background_denies() {
    let dir = tempfile::tempdir().unwrap();
    // bash -c "rm -rf /tmp/x" → Bash_destructive (approval_required) →
    // ASK → collapsed to DENY by --background → exit 1
    let (stderr, code) = run_exec(
        dir.path(),
        "bash",
        "/usr/bin/true",
        &["--background"],
        &["-c", "rm -rf /tmp/x"],
    );
    assert_eq!(code, 1, "background ASK must fail closed; stderr: {stderr}");
    assert!(
        stderr.contains("DENIED"),
        "expected DENIED in stderr: {stderr}"
    );
}

#[test]
fn curl_classifies_as_network_and_allows_in_clean_session() {
    let dir = tempfile::tempdir().unwrap();
    // tool=curl → always Bash_network; clean session → ALLOW (no taint floor hit)
    let (stderr, code) =
        run_exec(dir.path(), "curl", "/usr/bin/true", &[], &["http://example.com"]);
    assert_eq!(code, 0, "clean network tool should be allowed; stderr: {stderr}");
    assert!(!stderr.contains("DENIED"), "unexpected deny: {stderr}");
}

#[test]
fn tainted_session_denies_network_tool() {
    let dir = tempfile::tempdir().unwrap();
    // Pre-seed the taint sidecar so the session is already tainted.
    std::fs::write(dir.path().join("taint-default"), "seeded").unwrap();
    // curl (Bash_network) from a tainted session → DENY (no_tainted_network)
    let (stderr, code) =
        run_exec(dir.path(), "curl", "/usr/bin/true", &[], &["http://example.com"]);
    assert_eq!(code, 1, "tainted network call must be denied; stderr: {stderr}");
    assert!(
        stderr.contains("DENIED"),
        "expected DENIED in stderr: {stderr}"
    );
}

#[test]
fn network_tool_escalates_taint_in_clean_session() {
    let dir = tempfile::tempdir().unwrap();
    // A clean curl call is ALLOW but the kernel reports taint escalation;
    // the shim must persist the taint file for the next call.
    let taint_file = dir.path().join("taint-default");
    assert!(!taint_file.exists(), "precondition: no taint file");
    let (_stderr, code) =
        run_exec(dir.path(), "curl", "/usr/bin/true", &[], &["http://example.com"]);
    assert_eq!(code, 0, "clean curl must be allowed");
    assert!(
        taint_file.exists(),
        "shim must persist taint after a network action"
    );
}

#[test]
fn git_push_classifies_as_network() {
    let dir = tempfile::tempdir().unwrap();
    // git push → Bash_network; clean → ALLOW
    let (stderr, code) = run_exec(dir.path(), "git", "/usr/bin/true", &[], &["push"]);
    assert_eq!(code, 0, "clean git push should be allowed; stderr: {stderr}");
}

#[test]
fn unknown_world_path_fails_open() {
    let dir = tempfile::tempdir().unwrap();
    // A non-existent world path → fail-open → exec real binary → exit 0
    let out = Command::new(harness())
        .args(["shim", "exec"])
        .args(["--world", "/nonexistent/world.yaml"])
        .args(["--tool", "bash"])
        .args(["--real", "/usr/bin/true"])
        .args(["--state", dir.path().to_str().unwrap()])
        .arg("--")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .expect("spawn");
    assert_eq!(
        out.status.code().unwrap_or(-1),
        0,
        "fail-open: broken world must not block execution"
    );
}

// ---------------------------------------------------------------------------
// install tests
// ---------------------------------------------------------------------------

#[test]
fn install_creates_executable_wrapper_scripts() {
    let shim_dir = tempfile::tempdir().unwrap();
    let state_dir = tempfile::tempdir().unwrap();

    let out = Command::new(harness())
        .args(["shim", "install"])
        .args(["--world", world().to_str().unwrap()])
        .args(["--dir", shim_dir.path().to_str().unwrap()])
        .args(["--state", state_dir.path().to_str().unwrap()])
        .args(["--tools", "bash"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn shim install");

    let stderr = String::from_utf8(out.stderr).unwrap();
    assert_eq!(
        out.status.code().unwrap_or(-1),
        0,
        "install should succeed; stderr: {stderr}"
    );

    let shim_path = shim_dir.path().join("bash");
    assert!(shim_path.exists(), "shim script should be created");

    // The wrapper must reference `harness shim exec` and the world path.
    let content = std::fs::read_to_string(&shim_path).unwrap();
    assert!(
        content.contains("shim exec"),
        "wrapper must call `shim exec`; content: {content}"
    );
    assert!(
        content.contains(world().to_str().unwrap()),
        "wrapper must embed world path; content: {content}"
    );

    // Must be executable on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::metadata(&shim_path).unwrap().permissions();
        assert_ne!(perms.mode() & 0o111, 0, "shim script must be executable");
    }

    // stderr must print the activation hint.
    assert!(
        stderr.contains("export PATH"),
        "install must print activation hint; stderr: {stderr}"
    );
}

#[test]
fn install_background_flag_appears_in_wrapper() {
    let shim_dir = tempfile::tempdir().unwrap();
    let state_dir = tempfile::tempdir().unwrap();

    Command::new(harness())
        .args(["shim", "install"])
        .args(["--world", world().to_str().unwrap()])
        .args(["--dir", shim_dir.path().to_str().unwrap()])
        .args(["--state", state_dir.path().to_str().unwrap()])
        .args(["--tools", "bash"])
        .arg("--background")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn");

    let content = std::fs::read_to_string(shim_dir.path().join("bash")).unwrap();
    assert!(
        content.contains("--background"),
        "background flag must propagate into wrapper; content: {content}"
    );
}
