//! `harness doctor` finds `bash` and `claude` on PATH on every OS (AI2-9 / #62).
//!
//! The fixture suite in `doctor.rs` is Unix-only. On Windows, doctor split PATH
//! on `:` (every entry there carries a drive colon, and the separator is `;`) and
//! looked for a file named exactly `bash`, so a working install with Git Bash
//! and npm's `claude.cmd` was reported `INSTALLATION BROKEN`. These tests build
//! PATH with the platform's own `join_paths` and the file names each platform
//! actually uses.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn harness_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_harness"))
}

/// The names a shell finds for `bash` and `claude`: Git for Windows ships
/// `bash.exe`, npm installs `claude.cmd`; Unix has extensionless files.
#[cfg(windows)]
const BASH_FILE: &str = "bash.exe";
#[cfg(windows)]
const CLAUDE_FILE: &str = "claude.cmd";
#[cfg(not(windows))]
const BASH_FILE: &str = "bash";
#[cfg(not(windows))]
const CLAUDE_FILE: &str = "claude";

fn touch(path: &Path) {
    fs::write(path, b"").expect("write stub");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod");
    }
}

fn path_of(dirs: &[&Path]) -> String {
    std::env::join_paths(dirs)
        .expect("join PATH")
        .into_string()
        .expect("utf-8 PATH")
}

/// Run a harness subcommand with only HOME and PATH set (plus what Windows
/// needs to start a process), so nothing from the developer's machine leaks in.
fn harness(args: &[&str], cwd: &Path, home: &Path, path: &str) -> std::process::Output {
    let mut cmd = Command::new(harness_bin());
    cmd.args(args).current_dir(cwd).env_clear();
    cmd.env("HOME", home).env("PATH", path);
    if let Ok(root) = std::env::var("SystemRoot") {
        cmd.env("SystemRoot", root);
    }
    cmd.output().expect("run harness")
}

fn findings(report: &Value) -> Vec<String> {
    report["findings"]
        .as_array()
        .expect("findings array")
        .iter()
        .filter_map(|f| f["code"].as_str().map(String::from))
        .collect()
}

struct Fixture {
    _tmp: tempfile::TempDir,
    project: PathBuf,
    home: PathBuf,
    shell_bin: PathBuf,
    npm_bin: PathBuf,
}

fn governed_project() -> Fixture {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    let shell_bin = tmp.path().join("shell-bin");
    let npm_bin = tmp.path().join("npm-bin");
    for d in [&project, &home, &shell_bin, &npm_bin] {
        fs::create_dir_all(d).expect("mkdir");
    }
    let path = path_of(&[&shell_bin]);
    let project_arg = project.to_str().expect("utf-8 path");
    let out = harness(&["init", project_arg], &project, &home, &path);
    assert!(
        out.status.success(),
        "init failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Fixture {
        _tmp: tmp,
        project,
        home,
        shell_bin,
        npm_bin,
    }
}

fn doctor(fx: &Fixture, path: &str) -> Value {
    let out = harness(&["doctor", "--json"], &fx.project, &fx.home, path);
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "doctor --json is not JSON ({e}): {}",
            String::from_utf8_lossy(&out.stdout)
        )
    })
}

#[test]
fn doctor_finds_bash_and_claude_in_separate_path_entries() {
    let fx = governed_project();
    touch(&fx.shell_bin.join(BASH_FILE));
    touch(&fx.npm_bin.join(CLAUDE_FILE));

    let report = doctor(&fx, &path_of(&[&fx.shell_bin, &fx.npm_bin]));

    let codes = findings(&report);
    assert!(
        !codes.contains(&"CC_INTERPRETER_MISSING".to_string()),
        "bash is on PATH as {BASH_FILE}, yet: {codes:?}"
    );
    assert!(
        !codes.contains(&"CC_CLI_NOT_FOUND".to_string()),
        "claude is on PATH as {CLAUDE_FILE}, yet: {codes:?}"
    );
    assert_eq!(report["binaries"]["interpreter"]["status"], "found");
    assert_eq!(report["binaries"]["claude_candidate"]["status"], "found");
    let claude = report["binaries"]["claude_candidate"]["path"]
        .as_str()
        .expect("claude path");
    assert_eq!(
        Path::new(claude),
        fx.npm_bin.join(CLAUDE_FILE),
        "the path reported is the file that is there"
    );
}

#[test]
fn doctor_still_reports_bash_and_claude_missing_when_they_are_not_there() {
    let fx = governed_project();
    // Same layout, nothing in it: a name close to the real one must not count.
    touch(&fx.shell_bin.join("bash.txt"));
    touch(&fx.npm_bin.join("claude-code"));

    let report = doctor(&fx, &path_of(&[&fx.shell_bin, &fx.npm_bin]));

    let codes = findings(&report);
    assert!(
        codes.contains(&"CC_INTERPRETER_MISSING".to_string()),
        "{codes:?}"
    );
    assert!(codes.contains(&"CC_CLI_NOT_FOUND".to_string()), "{codes:?}");
}
