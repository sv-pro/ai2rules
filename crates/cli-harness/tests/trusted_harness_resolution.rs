#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

fn write_executable(path: &Path, body: &str) {
    fs::create_dir_all(path.parent().expect("parent")).expect("mkdir parent");
    fs::write(path, body).expect("write executable");
    let mut perms = fs::metadata(path).expect("metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("chmod executable");
}

fn fake_harness_script(marker_env: &str) -> String {
    format!("#!/usr/bin/env bash\nprintf '%s\\n' \"$0 $*\" > \"${{{marker_env}}}\"\n")
}

fn make_target_with_fake_harness(root: &Path, pwned_marker: &Path) -> PathBuf {
    let target = root.join("governed");
    write_executable(
        &target.join("target/debug/harness"),
        &fake_harness_script("PWNED_MARKER"),
    );
    fs::create_dir_all(target.join(".claude")).expect("mkdir target .claude");
    fs::write(target.join(".claude/cc-world.yaml"), "world_id: governed\n").expect("world");
    fs::write(pwned_marker, "").expect("create pwned marker parent");
    fs::remove_file(pwned_marker).expect("remove pwned marker");
    target
}

#[test]
fn installer_generated_shim_uses_installed_harness_not_project_target() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let source = tmp.path().join("source");
    fs::create_dir_all(source.join(".claude")).expect("mkdir source .claude");
    fs::create_dir_all(source.join("scripts")).expect("mkdir source scripts");
    fs::write(source.join(".claude/cc-world.yaml"), "world_id: source\n").expect("source world");
    fs::write(
        source.join("scripts/starter-world.yaml"),
        "world_id: starter\n",
    )
    .expect("starter");

    let trusted_marker = tmp.path().join("trusted.txt");
    let pwned_marker = tmp.path().join("pwned.txt");
    write_executable(
        &source.join("target/release/harness"),
        &fake_harness_script("TRUSTED_MARKER"),
    );
    let target = make_target_with_fake_harness(tmp.path(), &pwned_marker);
    let bin_dir = tmp.path().join("bin");

    let status = Command::new("bash")
        .arg(repo_root().join("scripts/install-governance.sh"))
        .arg("--source")
        .arg(&source)
        .arg("--bin-dir")
        .arg(&bin_dir)
        .arg(&target)
        .status()
        .expect("run installer");
    assert!(status.success(), "installer must succeed");

    let status = Command::new("bash")
        .arg(target.join(".claude/hooks/world-gate.sh"))
        .env("CLAUDE_PROJECT_DIR", &target)
        .env("HOME", tmp.path())
        .env("TRUSTED_MARKER", &trusted_marker)
        .env("PWNED_MARKER", &pwned_marker)
        .env_remove("HARNESS_BIN")
        .env_remove("AI2RULES_HARNESS")
        .status()
        .expect("run generated shim");
    assert!(
        status.success(),
        "generated shim must fail open or exec cleanly"
    );
    assert!(
        trusted_marker.exists(),
        "generated shim should execute the installer-owned binary"
    );
    assert!(
        !pwned_marker.exists(),
        "generated shim must not execute the governed project's target/debug/harness"
    );
}

#[test]
fn claude_sh_shim_does_not_search_project_target_and_honors_absolute_override() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let trusted_marker = tmp.path().join("trusted.txt");
    let pwned_marker = tmp.path().join("pwned.txt");
    let target = make_target_with_fake_harness(tmp.path(), &pwned_marker);
    let trusted = tmp.path().join("trusted/harness");
    write_executable(&trusted, &fake_harness_script("TRUSTED_MARKER"));
    let shim = repo_root().join(".claude/hooks/world-gate.sh");

    let status = Command::new("bash")
        .arg(&shim)
        .env("CLAUDE_PROJECT_DIR", &target)
        .env("HOME", tmp.path().join("empty-home"))
        .env("TRUSTED_MARKER", &trusted_marker)
        .env("PWNED_MARKER", &pwned_marker)
        .env_remove("HARNESS_BIN")
        .env_remove("AI2RULES_HARNESS")
        .status()
        .expect("run shim without override");
    assert!(
        status.success(),
        "shim should fail open without a trusted binary"
    );
    assert!(
        !pwned_marker.exists(),
        "shim must not execute the governed project's target/debug/harness"
    );

    let status = Command::new("bash")
        .arg(&shim)
        .env("CLAUDE_PROJECT_DIR", &target)
        .env("HOME", tmp.path().join("empty-home"))
        .env("HARNESS_BIN", &trusted)
        .env("TRUSTED_MARKER", &trusted_marker)
        .env("PWNED_MARKER", &pwned_marker)
        .env_remove("AI2RULES_HARNESS")
        .status()
        .expect("run shim with absolute override");
    assert!(status.success(), "shim should execute the trusted override");
    assert!(
        trusted_marker.exists(),
        "absolute HARNESS_BIN should be honored"
    );
    assert!(
        !pwned_marker.exists(),
        "trusted override must still not fall back to project target"
    );
}

#[test]
fn python_compat_shim_does_not_search_project_target_and_honors_absolute_override() {
    if Command::new("python3").arg("--version").status().is_err() {
        return;
    }

    let tmp = tempfile::tempdir().expect("tempdir");
    let trusted_marker = tmp.path().join("trusted.txt");
    let pwned_marker = tmp.path().join("pwned.txt");
    let target = make_target_with_fake_harness(tmp.path(), &pwned_marker);
    let trusted = tmp.path().join("trusted/harness");
    write_executable(&trusted, &fake_harness_script("TRUSTED_MARKER"));
    let shim = repo_root().join(".claude/hooks/world-gate.py");

    let status = Command::new("python3")
        .arg(&shim)
        .env("CLAUDE_PROJECT_DIR", &target)
        .env("HOME", tmp.path().join("empty-home"))
        .env("TRUSTED_MARKER", &trusted_marker)
        .env("PWNED_MARKER", &pwned_marker)
        .env_remove("HARNESS_BIN")
        .env_remove("AI2RULES_HARNESS")
        .status()
        .expect("run python shim without override");
    assert!(
        status.success(),
        "python shim should fail open without a trusted binary"
    );
    assert!(
        !pwned_marker.exists(),
        "python shim must not execute the governed project's target/debug/harness"
    );

    let status = Command::new("python3")
        .arg(&shim)
        .env("CLAUDE_PROJECT_DIR", &target)
        .env("HOME", tmp.path().join("empty-home"))
        .env("HARNESS_BIN", &trusted)
        .env("TRUSTED_MARKER", &trusted_marker)
        .env("PWNED_MARKER", &pwned_marker)
        .env_remove("AI2RULES_HARNESS")
        .status()
        .expect("run python shim with absolute override");
    assert!(
        status.success(),
        "python shim should execute the trusted override"
    );
    assert!(
        trusted_marker.exists(),
        "absolute HARNESS_BIN should be honored"
    );
    assert!(
        !pwned_marker.exists(),
        "trusted override must still not fall back to project target"
    );
}

#[test]
fn opencode_plugin_does_not_search_project_targets() {
    let plugin =
        fs::read_to_string(repo_root().join(".opencode/plugin/ai2rules-gate.ts")).expect("plugin");
    assert!(
        !plugin.contains("target/release/harness"),
        "OpenCode plugin must not search the governed project's release target"
    );
    assert!(
        !plugin.contains("target/debug/harness"),
        "OpenCode plugin must not search the governed project's debug target"
    );
    assert!(plugin.contains("AI2RULES_HARNESS"));
    assert!(plugin.contains("isAbsolute"));
}
