#![cfg(unix)]
//! `harness init` — the productization wedge.
//!
//! The property under test is not "it writes four files". It is that a stranger
//! with only this binary ends up governed, that running it twice is safe, and
//! that it never silently destroys the one artifact worth keeping (a tuned
//! manifest).

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

fn harness_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_harness"))
}

fn init(target: &Path, args: &[&str]) -> std::process::Output {
    let mut cmd = Command::new(harness_bin());
    cmd.arg("init").arg(target).args(args);
    cmd.output().expect("run harness init")
}

fn settings(target: &Path) -> Value {
    let text = fs::read_to_string(target.join(".claude/settings.json")).expect("settings.json");
    serde_json::from_str(&text).expect("settings.json parses")
}

fn pretooluse(v: &Value) -> Vec<Value> {
    v["hooks"]["PreToolUse"]
        .as_array()
        .expect("PreToolUse array")
        .clone()
}

/// The whole wedge in one assertion: an empty directory, one command, governed.
/// No `--source`, no checkout, no `jq`, no `cargo`.
#[test]
fn init_governs_an_empty_directory_with_nothing_but_the_binary() {
    let tmp = tempfile::tempdir().expect("tmp");
    let target = tmp.path();

    let out = init(target, &[]);
    assert!(out.status.success(), "init failed: {:?}", out);

    let manifest = target.join(".claude/cc-world.yaml");
    let shim = target.join(".claude/hooks/world-gate.sh");
    assert!(manifest.exists(), "manifest not written");
    assert!(shim.exists(), "shim not written");
    assert!(target.join(".claude/settings.json").exists());

    // The shim must be executable, or the host silently never runs it.
    use std::os::unix::fs::PermissionsExt;
    let mode = fs::metadata(&shim)
        .expect("shim metadata")
        .permissions()
        .mode();
    assert_eq!(mode & 0o111, 0o111, "shim is not executable: {mode:o}");

    // It must point at a real kernel by absolute path, not at anything in the
    // (untrusted) project.
    let body = fs::read_to_string(&shim).expect("shim body");
    let bin = harness_bin().canonicalize().expect("canonical bin");
    assert!(
        body.contains(&format!("TRUSTED_BIN='{}'", bin.display())),
        "shim does not bake the running binary's absolute path:\n{body}"
    );
    assert!(body.contains("cc-hook"), "shim does not invoke cc-hook");
    assert!(
        !body.contains(" --grant"),
        "additive mode must not pass --grant"
    );

    // The hook entry is wired where the host will find it.
    let hooks = pretooluse(&settings(target));
    assert_eq!(hooks.len(), 1, "expected exactly one PreToolUse entry");
    assert_eq!(hooks[0]["matcher"], "*");
    assert_eq!(
        hooks[0]["hooks"][0]["command"],
        r#"bash "$CLAUDE_PROJECT_DIR/.claude/hooks/world-gate.sh""#
    );
}

/// Re-running must not stack hooks. A duplicated PreToolUse entry means the
/// kernel runs twice per call, which is how a governance tool becomes the reason
/// someone turns governance off.
#[test]
fn init_is_idempotent() {
    let tmp = tempfile::tempdir().expect("tmp");
    let target = tmp.path();

    assert!(init(target, &[]).status.success());
    let first = fs::read_to_string(target.join(".claude/settings.json")).expect("first");
    assert!(init(target, &[]).status.success());
    assert!(init(target, &[]).status.success());

    let hooks = pretooluse(&settings(target));
    assert_eq!(hooks.len(), 1, "hook entry was duplicated across runs");
    assert_eq!(
        first,
        fs::read_to_string(target.join(".claude/settings.json")).expect("third"),
        "settings.json churned between identical runs"
    );

    let gitignore = fs::read_to_string(target.join(".gitignore")).expect("gitignore");
    assert_eq!(
        gitignore.matches(".claude/state/").count(),
        1,
        "gitignore line duplicated:\n{gitignore}"
    );
}

/// A tuned manifest is the valuable artifact in a governed project. Losing it to
/// a re-run would be the single worst thing this command could do.
#[test]
fn init_never_clobbers_a_tuned_manifest_without_force() {
    let tmp = tempfile::tempdir().expect("tmp");
    let target = tmp.path();
    fs::create_dir_all(target.join(".claude")).expect("mkdir");
    let tuned = "world_id: my-carefully-tuned-world\n";
    fs::write(target.join(".claude/cc-world.yaml"), tuned).expect("write tuned");

    assert!(init(target, &[]).status.success());
    assert_eq!(
        fs::read_to_string(target.join(".claude/cc-world.yaml")).expect("read"),
        tuned,
        "init destroyed a tuned manifest"
    );

    // --force is the explicit opt-in, and only then.
    assert!(init(target, &["--force"]).status.success());
    assert_ne!(
        fs::read_to_string(target.join(".claude/cc-world.yaml")).expect("read"),
        tuned,
        "--force did not replace the manifest"
    );
}

/// Other people's hooks are theirs. A governance tool that eats unrelated
/// configuration on install does not get installed twice.
#[test]
fn init_preserves_foreign_hooks_and_settings() {
    let tmp = tempfile::tempdir().expect("tmp");
    let target = tmp.path();
    fs::create_dir_all(target.join(".claude")).expect("mkdir");
    fs::write(
        target.join(".claude/settings.json"),
        r#"{
          "model": "opus",
          "hooks": {
            "PreToolUse": [
              {"matcher": "Bash", "hooks": [{"type": "command", "command": "echo mine"}]}
            ],
            "PostToolUse": [
              {"matcher": "*", "hooks": [{"type": "command", "command": "echo after"}]}
            ]
          }
        }"#,
    )
    .expect("write settings");

    assert!(init(target, &[]).status.success());
    let s = settings(target);

    assert_eq!(s["model"], "opus", "unrelated settings key was dropped");
    assert!(
        s["hooks"]["PostToolUse"].is_array(),
        "unrelated hook event was dropped"
    );
    let pre = pretooluse(&s);
    assert_eq!(pre.len(), 2, "expected the foreign hook plus ours");
    assert!(
        pre.iter().any(|e| e["hooks"][0]["command"] == "echo mine"),
        "foreign PreToolUse hook was dropped"
    );
}

/// `--grant` is the difference between an overlay and an authority, so it has to
/// actually reach the kernel invocation.
#[test]
fn grant_mode_reaches_the_shim() {
    let tmp = tempfile::tempdir().expect("tmp");
    let target = tmp.path();
    assert!(init(target, &["--grant"]).status.success());
    let body = fs::read_to_string(target.join(".claude/hooks/world-gate.sh")).expect("shim");
    assert!(
        body.contains("cc-hook --grant --world"),
        "grant flag missing from the shim's exec line:\n{body}"
    );
}

/// A dry run has to be genuinely dry — this is the command people will reach for
/// before letting a governance tool write to their project.
#[test]
fn dry_run_writes_nothing() {
    let tmp = tempfile::tempdir().expect("tmp");
    let target = tmp.path();

    let out = init(target, &["--dry-run"]);
    assert!(out.status.success());
    assert!(!target.join(".claude").exists(), "--dry-run created files");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("dry run"), "no dry-run notice:\n{stdout}");
    assert!(
        stdout.contains("cc-world.yaml"),
        "dry run did not describe the plan:\n{stdout}"
    );
}

/// The kill-switch is the thing that makes installing this reversible in one
/// command. If it silently stopped being written, nobody would notice until they
/// needed it.
#[test]
fn shim_carries_the_kill_switch_and_fails_open() {
    let tmp = tempfile::tempdir().expect("tmp");
    let target = tmp.path();
    assert!(init(target, &[]).status.success());
    let body = fs::read_to_string(target.join(".claude/hooks/world-gate.sh")).expect("shim");

    assert!(body.contains(".claude/gate-off"), "no project kill-switch");
    assert!(
        body.contains("$HOME/.claude/gate-off"),
        "no panic kill-switch"
    );
    assert!(
        body.contains(r#"[ -x "$BIN" ] || exit 0"#),
        "shim does not fail open when the kernel is missing"
    );
}

/// The manifest `init` embeds and the one the shell installer copies must be the
/// same bytes, or two supported install paths quietly diverge.
#[test]
fn embedded_starter_matches_the_shipped_one() {
    let tmp = tempfile::tempdir().expect("tmp");
    let target = tmp.path();
    assert!(init(target, &[]).status.success());

    let written = fs::read_to_string(target.join(".claude/cc-world.yaml")).expect("written");
    let shipped =
        fs::read_to_string(repo_root().join("scripts/starter-world.yaml")).expect("shipped");
    assert_eq!(
        written, shipped,
        "the manifest compiled into the binary has drifted from scripts/starter-world.yaml"
    );
}

/// The manifest is compiled before it is written, so what lands in a stranger's
/// project is known to build. Cheap here, and the whole reason to trust the
/// command at all.
#[test]
fn the_written_manifest_actually_compiles() {
    let tmp = tempfile::tempdir().expect("tmp");
    let target = tmp.path();
    assert!(init(target, &[]).status.success());

    let out = Command::new(harness_bin())
        .arg("gate")
        .arg("--world")
        .arg(target.join(".claude/cc-world.yaml"))
        .arg("--help")
        .output()
        .expect("gate --help");
    assert!(
        out.status.success(),
        "installed manifest is not usable: {out:?}"
    );

    let yaml = fs::read_to_string(target.join(".claude/cc-world.yaml")).expect("yaml");
    let manifest = compiler::loader::load_yaml(&yaml).expect("starter manifest loads");
    compiler::compile(&manifest).expect("starter manifest compiles");
}
