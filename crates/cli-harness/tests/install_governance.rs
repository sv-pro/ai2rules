#![cfg(unix)]

use serde_json::Value;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const MANAGED_COMMAND: &str = r#"bash "$CLAUDE_PROJECT_DIR/.claude/hooks/world-gate.sh""#;
const FAKE_COMMAND: &str = "bash -c 'exit 0 # world-gate'";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

fn jq_available() -> bool {
    Command::new("jq")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn write_executable(path: &Path, body: &str) {
    fs::create_dir_all(path.parent().expect("parent")).expect("mkdir parent");
    fs::write(path, body).expect("write executable");
    let mut perms = fs::metadata(path).expect("metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("chmod executable");
}

fn make_source(root: &Path) -> PathBuf {
    let source = root.join("source");
    fs::create_dir_all(source.join(".claude")).expect("mkdir source .claude");
    fs::create_dir_all(source.join("scripts")).expect("mkdir source scripts");
    fs::write(source.join(".claude/cc-world.yaml"), "world_id: source\n").expect("source world");
    fs::write(
        source.join("scripts/starter-world.yaml"),
        "world_id: starter\n",
    )
    .expect("starter world");
    source
}

fn read_settings(target: &Path) -> Value {
    serde_json::from_str(
        &fs::read_to_string(target.join(".claude/settings.json")).expect("settings"),
    )
    .expect("settings json")
}

fn run_installer(source: &Path, target: &Path, bin_dir: &Path) {
    write_executable(&bin_dir.join("harness"), "#!/usr/bin/env bash\nexit 0\n");
    let path = format!(
        "{}:{}",
        bin_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let output = Command::new("bash")
        .arg(repo_root().join("scripts/install-governance.sh"))
        .arg("--source")
        .arg(source)
        .arg(target)
        .env("PATH", path)
        .output()
        .expect("run installer");
    assert!(
        output.status.success(),
        "installer failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn managed_hook_count(settings: &Value) -> usize {
    settings
        .pointer("/hooks/PreToolUse")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|entry| {
            entry.get("matcher").and_then(Value::as_str) == Some("*")
                && entry
                    .get("hooks")
                    .and_then(Value::as_array)
                    .map(|hooks| {
                        hooks.iter().any(|hook| {
                            hook.get("type").and_then(Value::as_str) == Some("command")
                                && hook.get("command").and_then(Value::as_str)
                                    == Some(MANAGED_COMMAND)
                                && hook.get("timeout").and_then(Value::as_i64) == Some(10)
                        })
                    })
                    .unwrap_or(false)
        })
        .count()
}

fn hook_commands(settings: &Value) -> Vec<&str> {
    settings
        .pointer("/hooks/PreToolUse")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|entry| {
            entry
                .get("hooks")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter_map(|hook| hook.get("command").and_then(Value::as_str))
        .collect()
}

#[test]
fn installer_adds_real_hook_when_fake_world_gate_command_exists() {
    if !jq_available() {
        return;
    }

    let tmp = tempfile::tempdir().expect("tempdir");
    let source = make_source(tmp.path());
    let target = tmp.path().join("target");
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(target.join(".claude")).expect("mkdir target .claude");
    fs::write(
        target.join(".claude/settings.json"),
        serde_json::json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "*",
                    "hooks": [{
                        "type": "command",
                        "command": FAKE_COMMAND,
                        "timeout": 10
                    }]
                }]
            }
        })
        .to_string(),
    )
    .expect("write fake settings");

    run_installer(&source, &target, &bin_dir);

    let settings = read_settings(&target);
    assert_eq!(
        managed_hook_count(&settings),
        1,
        "installer must add the real managed hook even when a target-controlled hook mentions world-gate"
    );
    assert!(
        hook_commands(&settings).contains(&FAKE_COMMAND),
        "unrelated pre-existing hooks are preserved"
    );
}

#[test]
fn installer_does_not_duplicate_existing_real_hook() {
    if !jq_available() {
        return;
    }

    let tmp = tempfile::tempdir().expect("tempdir");
    let source = make_source(tmp.path());
    let target = tmp.path().join("target");
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(target.join(".claude")).expect("mkdir target .claude");
    fs::write(
        target.join(".claude/settings.json"),
        serde_json::json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "*",
                    "hooks": [{
                        "type": "command",
                        "command": MANAGED_COMMAND,
                        "timeout": 10
                    }]
                }]
            }
        })
        .to_string(),
    )
    .expect("write managed settings");

    run_installer(&source, &target, &bin_dir);
    run_installer(&source, &target, &bin_dir);

    let settings = read_settings(&target);
    assert_eq!(
        managed_hook_count(&settings),
        1,
        "installer should be idempotent for the exact managed hook shape"
    );
}
