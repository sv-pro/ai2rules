#![cfg(unix)]
//! Deterministic `harness doctor` fixtures and read-only behavior proof (AI2-25 / #87)
//!
//! Table-driven tests covering the fixture matrix from #87:
//! - No live Claude, accounts, credentials, network, or real user-home reads
//! - Synthetic fixtures on disk
//! - Trap executables prove no subprocess execution
//! - Before/after filesystem assertions prove no writes
//!
//! ## Fixture Matrix Coverage (28 tests)
//!
//! **Installation statuses:**
//! - `modern_init_additive_ok` - ok/0: runtime unknown, ABSENT pass-through, real hash
//! - `modern_init_with_grant_flag` - ok/0: grant=true recognized
//! - `missing_project_settings` - broken/2: settings missing
//! - `narrow_matcher_registration` - partial/1: narrow matcher registered
//! - `fake_echo_mentioning_worldgate_never_recognized` - never recognized as managed
//! - `invalid_json_settings` - broken/2: invalid JSON
//! - `missing_referenced_shim` - broken/2: shim file not found
//! - `readable_0644_shim_recognized` - ok/0: execute bit not required for script
//! - `modern_per_project_off_file_enabled` - disabled/1: project switch ON
//! - `global_user_off_file_enabled` - disabled/1: global switch ON
//! - `modern_shim_with_obsolete_project_gate_off` - disabled/1: legacy switch effective
//! - `directory_at_switch_filename` - ok/0: directory is not a regular file (-f semantics)
//! - `relative_harness_bin_override_broken` - broken/2: relative path rejected
//! - `nonexecutable_harness_bin_override_broken` - broken/2: non-executable rejected
//! - `claude_project_dir_mismatch_broken` - broken/2: project dir mismatch
//! - `missing_manifest_broken` - broken/2: manifest missing, hash null
//! - `invalid_manifest_broken` - broken/2: manifest invalid, hash null
//! - `semantic_manifest_edit_new_hash` - ok/0: semantic edit produces new hash
//! - `yaml_formatting_only_edit_same_hash` - ok/0: formatting-only produces same hash
//! - `broken_manifest_and_enabled_switch_both_findings` - both findings survive
//!
//! **Read-only behavior proofs:**
//! - `no_trap_executables_invoked` - trap binaries never executed
//! - `no_writes_to_project_or_home` - no state/lock/cache/audit created
//! - `identical_env_produces_identical_report` - deterministic output
//! - `human_and_json_mode_consistency` - both modes exit with same code
//! - `clap_parse_error_exit_2_vs_diagnostic_exit_2` - distinguishes CLI vs diagnostic errors
//!
//! **Edge cases:**
//! - `symlink_loop_in_binary_path` - detects symlink cycles
//! - `dangling_symlink_in_binary_path` - handles broken symlinks
//! - `paths_with_spaces` - handles special characters in paths

use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

// ═══════════════════════════════════════════════════════════════════════════════
// Test utilities
// ═══════════════════════════════════════════════════════════════════════════════

fn harness_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_harness"))
}

/// Run `harness doctor` with synthetic HOME and optional env overrides
fn run_doctor_with_env(
    project_dir: &Path,
    home: &Path,
    args: &[&str],
    extra_env: &[(&str, &str)],
) -> std::process::Output {
    let mut cmd = Command::new(harness_bin());
    cmd.arg("doctor");
    cmd.args(args);
    cmd.current_dir(project_dir);
    cmd.env("HOME", home);

    for (k, v) in extra_env {
        cmd.env(k, v);
    }

    cmd.output().expect("run harness doctor")
}

/// Parse JSON report from doctor output
fn parse_json_report(stdout: &[u8]) -> Value {
    let text = std::str::from_utf8(stdout).expect("valid UTF-8");
    serde_json::from_str(text).expect("valid JSON")
}

/// Write a minimal valid settings.json with one PreToolUse registration
fn write_settings_with_hook(path: &Path, matcher: &str, command: &str) {
    let settings = serde_json::json!({
        "hooks": {
            "PreToolUse": [
                {
                    "matcher": matcher,
                    "hooks": [
                        {
                            "type": "external",
                            "command": command
                        }
                    ]
                }
            ]
        }
    });

    fs::create_dir_all(path.parent().unwrap()).expect("mkdir .claude");
    fs::write(path, serde_json::to_string_pretty(&settings).unwrap()).expect("write settings.json");
}

/// Write a modern init-generated shim
fn write_modern_shim(path: &Path, harness_bin: &str, off_file: &str, grant: bool) {
    let grant_flag = if grant { " --grant" } else { "" };
    let content = format!(
        r#"#!/usr/bin/env bash
# ai2rules governance shim — written by `harness init`. Execs the Rust kernel's
# PreToolUse adapter; no governance logic lives here.

set -euo pipefail
readonly PD="${{CLAUDE_PROJECT_DIR:-$(pwd)}}"
readonly TRUSTED_BIN='{harness_bin}'
readonly BIN="${{HARNESS_BIN:-$TRUSTED_BIN}}"

if [ -f {off_file} ] || [ -f "$HOME/.claude/gate-off" ]; then exit 0; fi
exec "$BIN" cc-hook{grant_flag} --world "$PD/.claude/cc-world.yaml" --state "$PD/.claude/state"
"#
    );

    fs::create_dir_all(path.parent().unwrap()).expect("mkdir hooks");
    fs::write(path, content).expect("write shim");
    let mut perms = fs::metadata(path).expect("stat shim").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("chmod shim");
}

/// Write a readable 0644 shim (no execute bit, same content as modern shim)
fn write_readable_shim(path: &Path, harness_bin: &str, off_file: &str) {
    // Use the same format as modern shim, just with different permissions
    let content = format!(
        r#"#!/usr/bin/env bash
# ai2rules governance shim — written by `harness init`. Execs the Rust kernel's
# PreToolUse adapter; no governance logic lives here.

set -euo pipefail
readonly PD="${{CLAUDE_PROJECT_DIR:-$(pwd)}}"
readonly TRUSTED_BIN='{harness_bin}'
readonly BIN="${{HARNESS_BIN:-$TRUSTED_BIN}}"

if [ -f {off_file} ] || [ -f "$HOME/.claude/gate-off" ]; then exit 0; fi
exec "$BIN" cc-hook --world "$PD/.claude/cc-world.yaml" --state "$PD/.claude/state"
"#
    );

    fs::create_dir_all(path.parent().unwrap()).expect("mkdir hooks");
    fs::write(path, content).expect("write shim");
    let mut perms = fs::metadata(path).expect("stat shim").permissions();
    perms.set_mode(0o644); // no execute bit
    fs::set_permissions(path, perms).expect("chmod shim");
}

/// Write a minimal valid world manifest
fn write_manifest(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().unwrap()).expect("mkdir .claude");
    fs::write(path, content).expect("write manifest");
}

/// Write a trap executable that would create a marker if executed
fn write_trap_executable(path: &Path, marker_path: &Path) {
    let content = format!(
        r#"#!/usr/bin/env bash
touch '{}'
exit 0
"#,
        marker_path.display()
    );

    fs::create_dir_all(path.parent().unwrap()).expect("mkdir trap parent");
    fs::write(path, content).expect("write trap");
    let mut perms = fs::metadata(path).expect("stat trap").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("chmod trap");
}

/// Capture filesystem state before test
#[derive(Debug, Clone)]
struct FilesystemState {
    files: HashSet<PathBuf>,
    directories: HashSet<PathBuf>,
}

impl FilesystemState {
    fn capture(root: &Path) -> Self {
        let mut files = HashSet::new();
        let mut directories = HashSet::new();

        if let Ok(entries) = fs::read_dir(root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    files.insert(path);
                } else if path.is_dir() {
                    directories.insert(path.clone());
                    let nested = Self::capture(&path);
                    files.extend(nested.files);
                    directories.extend(nested.directories);
                }
            }
        }

        Self { files, directories }
    }

    fn assert_unchanged(&self, root: &Path, label: &str) {
        let after = Self::capture(root);

        let new_files: Vec<_> = after.files.difference(&self.files).collect();
        let new_dirs: Vec<_> = after.directories.difference(&self.directories).collect();

        assert!(
            new_files.is_empty(),
            "{label}: unexpected new files: {new_files:?}"
        );
        assert!(
            new_dirs.is_empty(),
            "{label}: unexpected new directories: {new_dirs:?}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Schema and output validation
// ═══════════════════════════════════════════════════════════════════════════════

/// Validate the doctor report schema (v0alpha3)
fn validate_schema(report: &Value) {
    assert_eq!(
        report["schema_version"].as_str(),
        Some("ai2rules.dev/harness-doctor/v0alpha3"),
        "schema_version mismatch"
    );
    assert_eq!(
        report["profile"].as_str(),
        Some("claude-code-cli/static-v1"),
        "profile mismatch"
    );

    // Required top-level keys
    for key in &[
        "summary",
        "binaries",
        "settings",
        "hook",
        "manifest",
        "switches",
        "coverage",
        "failure_behavior",
        "findings",
    ] {
        assert!(report.get(key).is_some(), "missing key: {key}");
    }

    // Summary must have installation, exit_code
    assert!(
        report["summary"]["installation"].is_string(),
        "installation must be string"
    );
    assert!(
        report["summary"]["exit_code"].is_i64(),
        "exit_code must be i64"
    );

    // Findings must be array
    assert!(report["findings"].is_array(), "findings must be array");

    // Hash must be null or 64-char hex
    if let Some(hash) = report["manifest"]["hash"].as_str() {
        assert_eq!(hash.len(), 64, "hash must be 64 hex chars");
        assert!(
            hash.chars().all(|c| c.is_ascii_hexdigit()),
            "hash must be hex"
        );
    }
}

/// Validate doctor output format (stdout exactly one JSON object + newline, no ANSI, no timestamp)
fn validate_output_format(stdout: &[u8], stderr: &[u8]) {
    // Stderr must be empty for normal operation
    assert!(
        stderr.is_empty(),
        "stderr must be empty, got: {}",
        String::from_utf8_lossy(stderr)
    );

    let text = std::str::from_utf8(stdout).expect("stdout must be UTF-8");

    // No ANSI escape sequences
    assert!(
        !text.contains("\x1b["),
        "stdout must not contain ANSI escapes"
    );

    // Must end with exactly one newline
    assert!(text.ends_with('\n'), "stdout must end with newline");
    let without_trailing = text.trim_end_matches('\n');
    assert!(
        !without_trailing.ends_with('\n'),
        "stdout must have exactly one trailing newline"
    );

    // Must be exactly one JSON object
    let parsed: Value = serde_json::from_str(without_trailing).expect("must parse as JSON");
    assert!(parsed.is_object(), "stdout must be a JSON object");
}

/// Get finding codes from report
fn finding_codes(report: &Value) -> Vec<String> {
    report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|f| f["code"].as_str().map(String::from))
        .collect()
}

/// Check if a finding code exists
fn has_finding(report: &Value, code: &str) -> bool {
    finding_codes(report).contains(&code.to_string())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Table-driven tests: fixture matrix from #87
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn modern_init_additive_ok() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).expect("mkdir project");
    fs::create_dir_all(&home).expect("mkdir home");

    // Create a fake harness binary
    let bin_dir = home.join("bin");
    fs::create_dir_all(&bin_dir).expect("mkdir bin");
    let fake_harness = bin_dir.join("harness");
    fs::write(&fake_harness, "#!/bin/sh\necho fake\n").expect("write fake harness");
    let mut perms = fs::metadata(&fake_harness).expect("stat").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&fake_harness, perms).expect("chmod");

    write_settings_with_hook(
        &project.join(".claude/settings.json"),
        "*",
        "bash '.claude/hooks/world-gate.sh'",
    );

    write_modern_shim(
        &project.join(".claude/hooks/world-gate.sh"),
        fake_harness.to_str().unwrap(),
        "$HOME/.claude/ai2rules/off/test-project",
        false,
    );

    write_manifest(
        &project.join(".claude/cc-world.yaml"),
        "world_id: test\nactions:\n  - name: read_file\n",
    );

    let state_before = FilesystemState::capture(&project);

    let out = run_doctor_with_env(&project, &home, &["--json"], &[]);

    assert_eq!(out.status.code(), Some(0), "doctor must exit 0");

    validate_output_format(&out.stdout, &out.stderr);
    let report = parse_json_report(&out.stdout);
    validate_schema(&report);

    assert_eq!(
        report["summary"]["installation"].as_str(),
        Some("ok"),
        "installation must be ok"
    );

    // ABSENT pass-through finding
    assert!(
        has_finding(&report, "CC_ABSENT_PASSTHROUGH"),
        "must report ABSENT pass-through"
    );

    // Real hash must be present
    assert!(
        report["manifest"]["hash"].is_string(),
        "hash must be present for valid manifest"
    );

    state_before.assert_unchanged(&project, "project after doctor");
}

#[test]
fn modern_init_with_grant_flag() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).expect("mkdir project");
    fs::create_dir_all(&home).expect("mkdir home");

    let bin_dir = home.join("bin");
    fs::create_dir_all(&bin_dir).expect("mkdir bin");
    let fake_harness = bin_dir.join("harness");
    fs::write(&fake_harness, "#!/bin/sh\n").expect("write fake harness");
    let mut perms = fs::metadata(&fake_harness).expect("stat").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&fake_harness, perms).expect("chmod");

    write_settings_with_hook(
        &project.join(".claude/settings.json"),
        "*",
        "bash '.claude/hooks/world-gate.sh'",
    );

    write_modern_shim(
        &project.join(".claude/hooks/world-gate.sh"),
        fake_harness.to_str().unwrap(),
        "$HOME/.claude/ai2rules/off/test-project",
        true, // --grant flag
    );

    write_manifest(
        &project.join(".claude/cc-world.yaml"),
        "world_id: test\nactions:\n  - name: read_file\n",
    );

    let out = run_doctor_with_env(&project, &home, &["--json"], &[]);

    assert_eq!(out.status.code(), Some(0));

    let report = parse_json_report(&out.stdout);
    assert_eq!(report["summary"]["installation"].as_str(), Some("ok"));
    assert_eq!(
        report["hook"]["grant"].as_bool(),
        Some(true),
        "grant must be true"
    );
}

#[test]
fn missing_project_settings() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).expect("mkdir project");
    fs::create_dir_all(&home).expect("mkdir home");

    let out = run_doctor_with_env(&project, &home, &["--json"], &[]);

    assert_eq!(out.status.code(), Some(2), "exit code must be 2 for broken");

    let report = parse_json_report(&out.stdout);
    assert_eq!(report["summary"]["installation"].as_str(), Some("broken"));
    assert!(has_finding(&report, "CC_SETTINGS_MISSING"));
}

#[test]
fn narrow_matcher_registration() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).expect("mkdir project");
    fs::create_dir_all(&home).expect("mkdir home");

    let bin_dir = home.join("bin");
    fs::create_dir_all(&bin_dir).expect("mkdir bin");
    let fake_harness = bin_dir.join("harness");
    fs::write(&fake_harness, "#!/bin/sh\n").expect("write fake harness");
    let mut perms = fs::metadata(&fake_harness).expect("stat").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&fake_harness, perms).expect("chmod");

    // Settings with narrow matcher (not "*")
    write_settings_with_hook(
        &project.join(".claude/settings.json"),
        "write_file",
        "bash '.claude/hooks/world-gate.sh'",
    );

    write_modern_shim(
        &project.join(".claude/hooks/world-gate.sh"),
        fake_harness.to_str().unwrap(),
        "$HOME/.claude/ai2rules/off/test",
        false,
    );

    write_manifest(&project.join(".claude/cc-world.yaml"), "world_id: test\n");

    let out = run_doctor_with_env(&project, &home, &["--json"], &[]);

    assert_eq!(out.status.code(), Some(1), "narrow matcher -> partial/1");

    let report = parse_json_report(&out.stdout);
    assert_eq!(report["summary"]["installation"].as_str(), Some("partial"));
    assert!(has_finding(&report, "CC_PRETOOLUSE_NARROW"));
}

#[test]
fn fake_echo_mentioning_worldgate_never_recognized() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).expect("mkdir project");
    fs::create_dir_all(&home).expect("mkdir home");

    // Settings with a fake command that just echoes
    write_settings_with_hook(
        &project.join(".claude/settings.json"),
        "*",
        "echo 'world-gate.sh was here'",
    );

    let out = run_doctor_with_env(&project, &home, &["--json"], &[]);

    let report = parse_json_report(&out.stdout);
    // Must not recognize fake echo as a real shim
    assert!(
        report["hook"]["shim_profile"].is_null(),
        "fake echo must not be recognized"
    );
    assert!(has_finding(&report, "CC_SHIM_UNSUPPORTED"));
}

#[test]
fn invalid_json_settings() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).expect("mkdir project");
    fs::create_dir_all(&home).expect("mkdir home");

    fs::create_dir_all(project.join(".claude")).expect("mkdir .claude");
    fs::write(
        project.join(".claude/settings.json"),
        "{ invalid json syntax",
    )
    .expect("write invalid JSON");

    let out = run_doctor_with_env(&project, &home, &["--json"], &[]);

    assert_eq!(out.status.code(), Some(2), "exit code must be 2 for broken");

    let report = parse_json_report(&out.stdout);
    assert_eq!(report["summary"]["installation"].as_str(), Some("broken"));
    assert!(has_finding(&report, "CC_SETTINGS_INVALID"));
}

#[test]
fn missing_referenced_shim() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).expect("mkdir project");
    fs::create_dir_all(&home).expect("mkdir home");

    write_settings_with_hook(
        &project.join(".claude/settings.json"),
        "*",
        "bash '.claude/hooks/world-gate.sh'",
    );
    // Shim not written

    let out = run_doctor_with_env(&project, &home, &["--json"], &[]);

    assert_eq!(out.status.code(), Some(2));

    let report = parse_json_report(&out.stdout);
    assert_eq!(report["summary"]["installation"].as_str(), Some("broken"));
    assert!(has_finding(&report, "CC_SHIM_MISSING"));
}

#[test]
fn readable_0644_shim_recognized() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).expect("mkdir project");
    fs::create_dir_all(&home).expect("mkdir home");

    let bin_dir = home.join("bin");
    fs::create_dir_all(&bin_dir).expect("mkdir bin");
    let fake_harness = bin_dir.join("harness");
    fs::write(&fake_harness, "#!/bin/sh\n").expect("write fake harness");
    let mut perms = fs::metadata(&fake_harness).expect("stat").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&fake_harness, perms).expect("chmod");

    write_settings_with_hook(
        &project.join(".claude/settings.json"),
        "*",
        "bash '.claude/hooks/world-gate.sh'",
    );

    // Write 0644 shim (no execute bit, but still readable by bash)
    write_readable_shim(
        &project.join(".claude/hooks/world-gate.sh"),
        fake_harness.to_str().unwrap(),
        "$HOME/.claude/ai2rules/off/test",
    );

    write_manifest(&project.join(".claude/cc-world.yaml"), "world_id: test\n");

    let out = run_doctor_with_env(&project, &home, &["--json"], &[]);

    // The key assertion: shim can be read and recognized even without +x
    let report = parse_json_report(&out.stdout);

    // The shim profile should be recognized (proving it was read successfully)
    assert!(
        report["hook"]["shim_profile"].is_string(),
        "0644 shim must be readable and recognizable"
    );
    assert_eq!(
        report["hook"]["shim_profile"].as_str(),
        Some("current_init_generated"),
        "0644 shim must be recognized as current init generated"
    );
}

#[test]
fn modern_per_project_off_file_enabled() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).expect("mkdir project");
    fs::create_dir_all(&home).expect("mkdir home");
    fs::create_dir_all(home.join(".claude/ai2rules/off")).expect("mkdir off");

    let bin_dir = home.join("bin");
    fs::create_dir_all(&bin_dir).expect("mkdir bin");
    let fake_harness = bin_dir.join("harness");
    fs::write(&fake_harness, "#!/bin/sh\n").expect("write fake harness");
    let mut perms = fs::metadata(&fake_harness).expect("stat").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&fake_harness, perms).expect("chmod");

    write_settings_with_hook(
        &project.join(".claude/settings.json"),
        "*",
        "bash '.claude/hooks/world-gate.sh'",
    );

    let off_file_path = home.join(".claude/ai2rules/off/test-project");
    write_modern_shim(
        &project.join(".claude/hooks/world-gate.sh"),
        fake_harness.to_str().unwrap(),
        off_file_path.to_str().unwrap(),
        false,
    );

    write_manifest(&project.join(".claude/cc-world.yaml"), "world_id: test\n");

    // Create the off-file
    fs::write(&off_file_path, "").expect("write off-file");

    let out = run_doctor_with_env(&project, &home, &["--json"], &[]);

    assert_eq!(out.status.code(), Some(1), "disabled must exit 1");

    let report = parse_json_report(&out.stdout);
    assert_eq!(report["summary"]["installation"].as_str(), Some("disabled"));
    assert!(has_finding(&report, "CC_KILL_SWITCH_ENABLED"));
}

#[test]
fn global_user_off_file_enabled() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).expect("mkdir project");
    fs::create_dir_all(&home).expect("mkdir home");
    fs::create_dir_all(home.join(".claude")).expect("mkdir .claude");

    let bin_dir = home.join("bin");
    fs::create_dir_all(&bin_dir).expect("mkdir bin");
    let fake_harness = bin_dir.join("harness");
    fs::write(&fake_harness, "#!/bin/sh\n").expect("write fake harness");
    let mut perms = fs::metadata(&fake_harness).expect("stat").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&fake_harness, perms).expect("chmod");

    write_settings_with_hook(
        &project.join(".claude/settings.json"),
        "*",
        "bash '.claude/hooks/world-gate.sh'",
    );

    write_modern_shim(
        &project.join(".claude/hooks/world-gate.sh"),
        fake_harness.to_str().unwrap(),
        "$HOME/.claude/ai2rules/off/test",
        false,
    );

    write_manifest(&project.join(".claude/cc-world.yaml"), "world_id: test\n");

    // Create global gate-off
    fs::write(home.join(".claude/gate-off"), "").expect("write gate-off");

    let out = run_doctor_with_env(&project, &home, &["--json"], &[]);

    assert_eq!(out.status.code(), Some(1));

    let report = parse_json_report(&out.stdout);
    assert_eq!(report["summary"]["installation"].as_str(), Some("disabled"));
    assert!(has_finding(&report, "CC_KILL_SWITCH_ENABLED"));
}

#[test]
fn modern_shim_with_obsolete_project_gate_off() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).expect("mkdir project");
    fs::create_dir_all(&home).expect("mkdir home");

    let bin_dir = home.join("bin");
    fs::create_dir_all(&bin_dir).expect("mkdir bin");
    let fake_harness = bin_dir.join("harness");
    fs::write(&fake_harness, "#!/bin/sh\n").expect("write fake harness");
    let mut perms = fs::metadata(&fake_harness).expect("stat").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&fake_harness, perms).expect("chmod");

    write_settings_with_hook(
        &project.join(".claude/settings.json"),
        "*",
        "bash '.claude/hooks/world-gate.sh'",
    );

    write_modern_shim(
        &project.join(".claude/hooks/world-gate.sh"),
        fake_harness.to_str().unwrap(),
        "$HOME/.claude/ai2rules/off/test",
        false,
    );

    write_manifest(&project.join(".claude/cc-world.yaml"), "world_id: test\n");

    // Create obsolete project/.claude/gate-off
    // Even though modern shim doesn't check this, doctor reports it as effective
    fs::write(project.join(".claude/gate-off"), "").expect("write obsolete gate-off");

    let out = run_doctor_with_env(&project, &home, &["--json"], &[]);

    // Legacy switch is still effective - ANY switch ON makes installation disabled
    assert_eq!(out.status.code(), Some(1), "legacy switch is effective");

    let report = parse_json_report(&out.stdout);
    assert_eq!(report["summary"]["installation"].as_str(), Some("disabled"));
    assert!(has_finding(&report, "CC_KILL_SWITCH_LEGACY"));
    assert!(has_finding(&report, "CC_KILL_SWITCH_ENABLED"));
}

#[test]
fn directory_at_switch_filename() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).expect("mkdir project");
    fs::create_dir_all(&home).expect("mkdir home");
    fs::create_dir_all(home.join(".claude")).expect("mkdir .claude");

    let bin_dir = home.join("bin");
    fs::create_dir_all(&bin_dir).expect("mkdir bin");
    let fake_harness = bin_dir.join("harness");
    fs::write(&fake_harness, "#!/bin/sh\n").expect("write fake harness");
    let mut perms = fs::metadata(&fake_harness).expect("stat").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&fake_harness, perms).expect("chmod");

    write_settings_with_hook(
        &project.join(".claude/settings.json"),
        "*",
        "bash '.claude/hooks/world-gate.sh'",
    );

    write_modern_shim(
        &project.join(".claude/hooks/world-gate.sh"),
        fake_harness.to_str().unwrap(),
        "$HOME/.claude/ai2rules/off/test",
        false,
    );

    write_manifest(&project.join(".claude/cc-world.yaml"), "world_id: test\n");

    // Create a directory where off-file would be (not a regular file)
    fs::create_dir_all(home.join(".claude/gate-off")).expect("mkdir gate-off as dir");

    let out = run_doctor_with_env(&project, &home, &["--json"], &[]);

    // Directory is not ON per -f semantics
    assert_eq!(
        out.status.code(),
        Some(0),
        "directory is not a regular file, so OFF"
    );

    let report = parse_json_report(&out.stdout);
    assert_eq!(report["summary"]["installation"].as_str(), Some("ok"));
}

#[test]
fn relative_harness_bin_override_broken() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).expect("mkdir project");
    fs::create_dir_all(&home).expect("mkdir home");

    write_settings_with_hook(
        &project.join(".claude/settings.json"),
        "*",
        "bash '.claude/hooks/world-gate.sh'",
    );

    write_modern_shim(
        &project.join(".claude/hooks/world-gate.sh"),
        "/usr/local/bin/harness",
        "$HOME/.claude/ai2rules/off/test",
        false,
    );

    write_manifest(&project.join(".claude/cc-world.yaml"), "world_id: test\n");

    // Relative HARNESS_BIN is invalid
    let out = run_doctor_with_env(
        &project,
        &home,
        &["--json"],
        &[("HARNESS_BIN", "./relative/path")],
    );

    assert_eq!(out.status.code(), Some(2));

    let report = parse_json_report(&out.stdout);
    assert_eq!(report["summary"]["installation"].as_str(), Some("broken"));
    assert!(has_finding(&report, "CC_HARNESS_OVERRIDE_INVALID"));
}

#[test]
fn nonexecutable_harness_bin_override_broken() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).expect("mkdir project");
    fs::create_dir_all(&home).expect("mkdir home");

    write_settings_with_hook(
        &project.join(".claude/settings.json"),
        "*",
        "bash '.claude/hooks/world-gate.sh'",
    );

    write_modern_shim(
        &project.join(".claude/hooks/world-gate.sh"),
        "/usr/local/bin/harness",
        "$HOME/.claude/ai2rules/off/test",
        false,
    );

    write_manifest(&project.join(".claude/cc-world.yaml"), "world_id: test\n");

    // Create a non-executable file
    let nonexec = tmp.path().join("harness-nonexec");
    fs::write(&nonexec, "#!/bin/sh\necho test\n").expect("write nonexec");

    let out = run_doctor_with_env(
        &project,
        &home,
        &["--json"],
        &[("HARNESS_BIN", nonexec.to_str().unwrap())],
    );

    assert_eq!(out.status.code(), Some(2));

    let report = parse_json_report(&out.stdout);
    assert!(has_finding(&report, "CC_HARNESS_OVERRIDE_INVALID"));
}

#[test]
fn claude_project_dir_mismatch_broken() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project");
    let wrong_dir = tmp.path().join("wrong");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).expect("mkdir project");
    fs::create_dir_all(&wrong_dir).expect("mkdir wrong");
    fs::create_dir_all(&home).expect("mkdir home");

    let bin_dir = home.join("bin");
    fs::create_dir_all(&bin_dir).expect("mkdir bin");
    let fake_harness = bin_dir.join("harness");
    fs::write(&fake_harness, "#!/bin/sh\n").expect("write fake harness");
    let mut perms = fs::metadata(&fake_harness).expect("stat").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&fake_harness, perms).expect("chmod");

    write_settings_with_hook(
        &project.join(".claude/settings.json"),
        "*",
        "bash '.claude/hooks/world-gate.sh'",
    );

    write_modern_shim(
        &project.join(".claude/hooks/world-gate.sh"),
        fake_harness.to_str().unwrap(),
        "$HOME/.claude/ai2rules/off/test",
        false,
    );

    write_manifest(&project.join(".claude/cc-world.yaml"), "world_id: test\n");

    let out = run_doctor_with_env(
        &project,
        &home,
        &["--json"],
        &[("CLAUDE_PROJECT_DIR", wrong_dir.to_str().unwrap())],
    );

    assert_eq!(out.status.code(), Some(2));

    let report = parse_json_report(&out.stdout);
    assert!(has_finding(&report, "CC_SHIM_PROJECT_MISMATCH"));
}

#[test]
fn missing_manifest_broken() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).expect("mkdir project");
    fs::create_dir_all(&home).expect("mkdir home");

    let bin_dir = home.join("bin");
    fs::create_dir_all(&bin_dir).expect("mkdir bin");
    let fake_harness = bin_dir.join("harness");
    fs::write(&fake_harness, "#!/bin/sh\n").expect("write fake harness");
    let mut perms = fs::metadata(&fake_harness).expect("stat").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&fake_harness, perms).expect("chmod");

    write_settings_with_hook(
        &project.join(".claude/settings.json"),
        "*",
        "bash '.claude/hooks/world-gate.sh'",
    );

    write_modern_shim(
        &project.join(".claude/hooks/world-gate.sh"),
        fake_harness.to_str().unwrap(),
        "$HOME/.claude/ai2rules/off/test",
        false,
    );

    // Manifest not written

    let out = run_doctor_with_env(&project, &home, &["--json"], &[]);

    assert_eq!(out.status.code(), Some(2));

    let report = parse_json_report(&out.stdout);
    assert!(has_finding(&report, "CC_MANIFEST_MISSING"));
    assert!(
        report["manifest"]["hash"].is_null(),
        "hash must be null for missing manifest"
    );
}

#[test]
fn invalid_manifest_broken() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).expect("mkdir project");
    fs::create_dir_all(&home).expect("mkdir home");

    let bin_dir = home.join("bin");
    fs::create_dir_all(&bin_dir).expect("mkdir bin");
    let fake_harness = bin_dir.join("harness");
    fs::write(&fake_harness, "#!/bin/sh\n").expect("write fake harness");
    let mut perms = fs::metadata(&fake_harness).expect("stat").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&fake_harness, perms).expect("chmod");

    write_settings_with_hook(
        &project.join(".claude/settings.json"),
        "*",
        "bash '.claude/hooks/world-gate.sh'",
    );

    write_modern_shim(
        &project.join(".claude/hooks/world-gate.sh"),
        fake_harness.to_str().unwrap(),
        "$HOME/.claude/ai2rules/off/test",
        false,
    );

    write_manifest(
        &project.join(".claude/cc-world.yaml"),
        "invalid: yaml: syntax",
    );

    let out = run_doctor_with_env(&project, &home, &["--json"], &[]);

    assert_eq!(out.status.code(), Some(2));

    let report = parse_json_report(&out.stdout);
    assert!(
        has_finding(&report, "CC_MANIFEST_INVALID")
            || has_finding(&report, "CC_MANIFEST_COMPILE_FAILED"),
        "must report manifest error"
    );
    assert!(report["manifest"]["hash"].is_null());
}

#[test]
fn semantic_manifest_edit_new_hash() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).expect("mkdir project");
    fs::create_dir_all(&home).expect("mkdir home");

    let bin_dir = home.join("bin");
    fs::create_dir_all(&bin_dir).expect("mkdir bin");
    let fake_harness = bin_dir.join("harness");
    fs::write(&fake_harness, "#!/bin/sh\n").expect("write fake harness");
    let mut perms = fs::metadata(&fake_harness).expect("stat").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&fake_harness, perms).expect("chmod");

    write_settings_with_hook(
        &project.join(".claude/settings.json"),
        "*",
        "bash '.claude/hooks/world-gate.sh'",
    );

    write_modern_shim(
        &project.join(".claude/hooks/world-gate.sh"),
        fake_harness.to_str().unwrap(),
        "$HOME/.claude/ai2rules/off/test",
        false,
    );

    // First manifest
    write_manifest(
        &project.join(".claude/cc-world.yaml"),
        "world_id: test_v1\nactions:\n  - name: read_file\n",
    );

    let out1 = run_doctor_with_env(&project, &home, &["--json"], &[]);
    let report1 = parse_json_report(&out1.stdout);
    let hash1 = report1["manifest"]["hash"].as_str().unwrap();

    // Semantic edit
    write_manifest(
        &project.join(".claude/cc-world.yaml"),
        "world_id: test_v2\nactions:\n  - name: write_file\n",
    );

    let out2 = run_doctor_with_env(&project, &home, &["--json"], &[]);
    let report2 = parse_json_report(&out2.stdout);
    let hash2 = report2["manifest"]["hash"].as_str().unwrap();

    assert_ne!(hash1, hash2, "semantic edit must produce different hash");
}

#[test]
fn yaml_formatting_only_edit_same_hash() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).expect("mkdir project");
    fs::create_dir_all(&home).expect("mkdir home");

    let bin_dir = home.join("bin");
    fs::create_dir_all(&bin_dir).expect("mkdir bin");
    let fake_harness = bin_dir.join("harness");
    fs::write(&fake_harness, "#!/bin/sh\n").expect("write fake harness");
    let mut perms = fs::metadata(&fake_harness).expect("stat").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&fake_harness, perms).expect("chmod");

    write_settings_with_hook(
        &project.join(".claude/settings.json"),
        "*",
        "bash '.claude/hooks/world-gate.sh'",
    );

    write_modern_shim(
        &project.join(".claude/hooks/world-gate.sh"),
        fake_harness.to_str().unwrap(),
        "$HOME/.claude/ai2rules/off/test",
        false,
    );

    // First manifest
    write_manifest(
        &project.join(".claude/cc-world.yaml"),
        "world_id: test\nactions:\n  - name: read_file\n",
    );

    let out1 = run_doctor_with_env(&project, &home, &["--json"], &[]);
    let report1 = parse_json_report(&out1.stdout);
    let hash1 = report1["manifest"]["hash"].as_str().unwrap();

    // Formatting-only edit (extra spaces)
    write_manifest(
        &project.join(".claude/cc-world.yaml"),
        "world_id:   test\nactions:\n  -  name:  read_file\n",
    );

    let out2 = run_doctor_with_env(&project, &home, &["--json"], &[]);
    let report2 = parse_json_report(&out2.stdout);
    let hash2 = report2["manifest"]["hash"].as_str().unwrap();

    assert_eq!(
        hash1, hash2,
        "formatting-only edit must produce same resolved hash"
    );
}

#[test]
fn broken_manifest_and_enabled_switch_both_findings() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).expect("mkdir project");
    fs::create_dir_all(&home).expect("mkdir home");
    fs::create_dir_all(home.join(".claude")).expect("mkdir .claude");

    let bin_dir = home.join("bin");
    fs::create_dir_all(&bin_dir).expect("mkdir bin");
    let fake_harness = bin_dir.join("harness");
    fs::write(&fake_harness, "#!/bin/sh\n").expect("write fake harness");
    let mut perms = fs::metadata(&fake_harness).expect("stat").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&fake_harness, perms).expect("chmod");

    write_settings_with_hook(
        &project.join(".claude/settings.json"),
        "*",
        "bash '.claude/hooks/world-gate.sh'",
    );

    write_modern_shim(
        &project.join(".claude/hooks/world-gate.sh"),
        fake_harness.to_str().unwrap(),
        "$HOME/.claude/ai2rules/off/test",
        false,
    );

    // Invalid manifest
    write_manifest(&project.join(".claude/cc-world.yaml"), "invalid yaml");

    // Kill switch enabled
    fs::write(home.join(".claude/gate-off"), "").expect("write gate-off");

    let out = run_doctor_with_env(&project, &home, &["--json"], &[]);

    // When both broken and disabled, disabled takes priority (exit 1)
    assert_eq!(out.status.code(), Some(1), "disabled takes priority");

    let report = parse_json_report(&out.stdout);
    // Both findings must be present
    let codes = finding_codes(&report);
    let has_manifest_error = codes.iter().any(|c| c.starts_with("CC_MANIFEST_"));
    let has_switch = codes.contains(&"CC_KILL_SWITCH_ENABLED".to_string());

    assert!(has_manifest_error, "manifest finding must survive");
    assert!(has_switch, "switch finding must survive");
}

// ═══════════════════════════════════════════════════════════════════════════════
// Read-only behavior proofs
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn no_trap_executables_invoked() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).expect("mkdir project");
    fs::create_dir_all(&home).expect("mkdir home");

    // Write trap executables that would create markers
    let bash_marker = tmp.path().join("bash-executed");
    let harness_marker = tmp.path().join("harness-executed");

    let bin_dir = home.join("bin");
    fs::create_dir_all(&bin_dir).expect("mkdir bin");

    write_trap_executable(&bin_dir.join("bash"), &bash_marker);
    write_trap_executable(&bin_dir.join("harness"), &harness_marker);

    write_settings_with_hook(
        &project.join(".claude/settings.json"),
        "*",
        "bash '.claude/hooks/world-gate.sh'",
    );

    write_modern_shim(
        &project.join(".claude/hooks/world-gate.sh"),
        bin_dir.join("harness").to_str().unwrap(),
        "$HOME/.claude/ai2rules/off/test",
        false,
    );

    write_manifest(&project.join(".claude/cc-world.yaml"), "world_id: test\n");

    let _out = run_doctor_with_env(&project, &home, &["--json"], &[]);

    // Assert no markers were created
    assert!(!bash_marker.exists(), "bash trap must not be executed");
    assert!(
        !harness_marker.exists(),
        "harness trap must not be executed"
    );
}

#[test]
fn no_writes_to_project_or_home() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).expect("mkdir project");
    fs::create_dir_all(&home).expect("mkdir home");

    let bin_dir = home.join("bin");
    fs::create_dir_all(&bin_dir).expect("mkdir bin");
    let fake_harness = bin_dir.join("harness");
    fs::write(&fake_harness, "#!/bin/sh\n").expect("write fake harness");
    let mut perms = fs::metadata(&fake_harness).expect("stat").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&fake_harness, perms).expect("chmod");

    write_settings_with_hook(
        &project.join(".claude/settings.json"),
        "*",
        "bash '.claude/hooks/world-gate.sh'",
    );

    write_modern_shim(
        &project.join(".claude/hooks/world-gate.sh"),
        fake_harness.to_str().unwrap(),
        "$HOME/.claude/ai2rules/off/test",
        false,
    );

    write_manifest(&project.join(".claude/cc-world.yaml"), "world_id: test\n");

    let project_before = FilesystemState::capture(&project);
    let home_before = FilesystemState::capture(&home);

    let _out = run_doctor_with_env(&project, &home, &["--json"], &[]);

    project_before.assert_unchanged(&project, "project");
    home_before.assert_unchanged(&home, "home");

    // No .claude/state, lock, cache, audit, or trace created
    assert!(!project.join(".claude/state").exists());
    assert!(!project.join(".claude/.lock").exists());
    assert!(!home.join(".claude/cache").exists());
}

#[test]
fn identical_env_produces_identical_report() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).expect("mkdir project");
    fs::create_dir_all(&home).expect("mkdir home");

    let bin_dir = home.join("bin");
    fs::create_dir_all(&bin_dir).expect("mkdir bin");
    let fake_harness = bin_dir.join("harness");
    fs::write(&fake_harness, "#!/bin/sh\n").expect("write fake harness");
    let mut perms = fs::metadata(&fake_harness).expect("stat").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&fake_harness, perms).expect("chmod");

    write_settings_with_hook(
        &project.join(".claude/settings.json"),
        "*",
        "bash '.claude/hooks/world-gate.sh'",
    );

    write_modern_shim(
        &project.join(".claude/hooks/world-gate.sh"),
        fake_harness.to_str().unwrap(),
        "$HOME/.claude/ai2rules/off/test",
        false,
    );

    write_manifest(&project.join(".claude/cc-world.yaml"), "world_id: test\n");

    // Run twice with identical environment
    let out1 = run_doctor_with_env(&project, &home, &["--json"], &[]);
    let out2 = run_doctor_with_env(&project, &home, &["--json"], &[]);

    let report1 = parse_json_report(&out1.stdout);
    let report2 = parse_json_report(&out2.stdout);

    // Reports must be identical (after normalizing paths)
    assert_eq!(
        report1["summary"], report2["summary"],
        "summary must be deterministic"
    );
    assert_eq!(
        report1["hook"], report2["hook"],
        "hook must be deterministic"
    );
    assert_eq!(
        report1["manifest"]["hash"], report2["manifest"]["hash"],
        "manifest hash must be deterministic"
    );
}

#[test]
fn human_and_json_mode_consistency() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).expect("mkdir project");
    fs::create_dir_all(&home).expect("mkdir home");

    let bin_dir = home.join("bin");
    fs::create_dir_all(&bin_dir).expect("mkdir bin");
    let fake_harness = bin_dir.join("harness");
    fs::write(&fake_harness, "#!/bin/sh\n").expect("write fake harness");
    let mut perms = fs::metadata(&fake_harness).expect("stat").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&fake_harness, perms).expect("chmod");

    write_settings_with_hook(
        &project.join(".claude/settings.json"),
        "*",
        "bash '.claude/hooks/world-gate.sh'",
    );

    write_modern_shim(
        &project.join(".claude/hooks/world-gate.sh"),
        fake_harness.to_str().unwrap(),
        "$HOME/.claude/ai2rules/off/test",
        false,
    );

    write_manifest(&project.join(".claude/cc-world.yaml"), "world_id: test\n");

    let human_out = run_doctor_with_env(&project, &home, &[], &[]);
    let json_out = run_doctor_with_env(&project, &home, &["--json"], &[]);

    // Both must exit with same code
    assert_eq!(human_out.status.code(), json_out.status.code());

    // Human output must contain semantic phrases
    let human_text = String::from_utf8_lossy(&human_out.stdout);
    assert!(
        human_text.contains("Installation:"),
        "must contain Installation:"
    );
    assert!(human_text.contains("Schema:"), "must contain Schema:");
    assert!(
        !human_text.contains("protected"),
        "must not say 'protected'"
    );

    // JSON output must be valid
    let report = parse_json_report(&json_out.stdout);
    validate_schema(&report);
}

#[test]
fn clap_parse_error_exit_2_vs_diagnostic_exit_2() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).expect("mkdir project");
    fs::create_dir_all(&home).expect("mkdir home");

    // Clap parse error (unknown flag)
    let clap_out = Command::new(harness_bin())
        .arg("doctor")
        .arg("--invalid-flag")
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run with invalid flag");

    assert_eq!(clap_out.status.code(), Some(2), "clap must exit 2");
    // Clap errors go to stderr
    assert!(!clap_out.stderr.is_empty(), "clap error must write stderr");

    // Now create a broken installation (diagnostic exit 2)
    write_settings_with_hook(
        &project.join(".claude/settings.json"),
        "*",
        "bash '.claude/hooks/world-gate.sh'",
    );
    // No shim written -> broken

    let diag_out = run_doctor_with_env(&project, &home, &["--json"], &[]);

    assert_eq!(diag_out.status.code(), Some(2), "diagnostic must exit 2");
    // Diagnostic produces valid JSON on stdout, empty stderr
    assert!(
        diag_out.stderr.is_empty(),
        "diagnostic stderr must be empty"
    );
    let report = parse_json_report(&diag_out.stdout);
    assert_eq!(report["summary"]["installation"].as_str(), Some("broken"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// Symlinks and edge cases
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn symlink_loop_in_binary_path() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).expect("mkdir project");
    fs::create_dir_all(&home).expect("mkdir home");

    let bin_a = tmp.path().join("harness-a");
    let bin_b = tmp.path().join("harness-b");

    // Create symlink loop
    std::os::unix::fs::symlink(&bin_b, &bin_a).expect("symlink a->b");
    std::os::unix::fs::symlink(&bin_a, &bin_b).expect("symlink b->a");

    write_settings_with_hook(
        &project.join(".claude/settings.json"),
        "*",
        "bash '.claude/hooks/world-gate.sh'",
    );

    write_modern_shim(
        &project.join(".claude/hooks/world-gate.sh"),
        bin_a.to_str().unwrap(),
        "$HOME/.claude/ai2rules/off/test",
        false,
    );

    write_manifest(&project.join(".claude/cc-world.yaml"), "world_id: test\n");

    let out = run_doctor_with_env(&project, &home, &["--json"], &[]);

    assert_eq!(out.status.code(), Some(2));

    let report = parse_json_report(&out.stdout);
    assert_eq!(report["summary"]["installation"].as_str(), Some("broken"));
}

#[test]
fn dangling_symlink_in_binary_path() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).expect("mkdir project");
    fs::create_dir_all(&home).expect("mkdir home");

    let bin_link = tmp.path().join("harness-link");
    let target = tmp.path().join("nonexistent-target");

    std::os::unix::fs::symlink(&target, &bin_link).expect("dangling symlink");

    write_settings_with_hook(
        &project.join(".claude/settings.json"),
        "*",
        "bash '.claude/hooks/world-gate.sh'",
    );

    write_modern_shim(
        &project.join(".claude/hooks/world-gate.sh"),
        bin_link.to_str().unwrap(),
        "$HOME/.claude/ai2rules/off/test",
        false,
    );

    write_manifest(&project.join(".claude/cc-world.yaml"), "world_id: test\n");

    let out = run_doctor_with_env(&project, &home, &["--json"], &[]);

    let report = parse_json_report(&out.stdout);
    assert_eq!(
        report["binaries"]["hook_harness"]["status"].as_str(),
        Some("missing")
    );
}

#[test]
fn paths_with_spaces() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("project with spaces");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).expect("mkdir project");
    fs::create_dir_all(&home).expect("mkdir home");

    let bin_dir = home.join("bin");
    fs::create_dir_all(&bin_dir).expect("mkdir bin");
    let fake_harness = bin_dir.join("harness");
    fs::write(&fake_harness, "#!/bin/sh\n").expect("write fake harness");
    let mut perms = fs::metadata(&fake_harness).expect("stat").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&fake_harness, perms).expect("chmod");

    write_settings_with_hook(
        &project.join(".claude/settings.json"),
        "*",
        "bash '.claude/hooks/world-gate.sh'",
    );

    write_modern_shim(
        &project.join(".claude/hooks/world-gate.sh"),
        fake_harness.to_str().unwrap(),
        "$HOME/.claude/ai2rules/off/test-project",
        false,
    );

    write_manifest(&project.join(".claude/cc-world.yaml"), "world_id: test\n");

    let out = run_doctor_with_env(&project, &home, &["--json"], &[]);

    // Should handle paths with spaces correctly
    assert!(out.status.success() || out.status.code() == Some(1));

    let report = parse_json_report(&out.stdout);
    validate_schema(&report);
}
