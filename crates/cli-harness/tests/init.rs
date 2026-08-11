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
    let cmd = hooks[0]["hooks"][0]["command"].as_str().expect("command");
    assert_eq!(
        cmd,
        format!("bash '{}/.claude/hooks/world-gate.sh'", target.display())
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

    // Assert against the *active* line, not the whole file. The shim carries a
    // comment explaining the old in-project switch, and a `contains` over the
    // body passes on that comment while the behaviour underneath it changed —
    // which is exactly what this test did until 2026-08-10.
    let switch_line = body
        .lines()
        .find(|l| l.trim_start().starts_with("if [ -f"))
        .expect("no kill-switch line in the shim");
    assert!(
        switch_line.contains("$HOME/.claude/gate-off"),
        "no panic kill-switch: {switch_line}"
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

// ---------------------------------------------------------------------------
// Regressions from the 2026-08-09 review of this feature. Both were found by
// running the shipped binary, not by reading it, and both passed the original
// test suite.
// ---------------------------------------------------------------------------

/// **The bad one.** Matching idempotence on the hook `command` alone meant a
/// pre-existing entry under a narrow matcher made `init` report "already
/// present", exit 0, and leave every tool except that one ungoverned — while
/// telling the operator they were covered. Silent under-coverage in a governance
/// tool is worse than a loud failure.
#[test]
fn a_narrow_matcher_does_not_satisfy_init() {
    let tmp = tempfile::tempdir().expect("tmp");
    let target = tmp.path();
    fs::create_dir_all(target.join(".claude")).expect("mkdir");
    fs::write(
        target.join(".claude/settings.json"),
        r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command",
           "command":"bash \"$CLAUDE_PROJECT_DIR/.claude/hooks/world-gate.sh\"","timeout":10}]}]}}"#,
    )
    .expect("write settings");

    let out = init(target, &[]);
    assert!(out.status.success(), "init failed: {out:?}");

    let matchers: Vec<String> = pretooluse(&settings(target))
        .iter()
        .map(|e| e["matcher"].as_str().unwrap_or("").to_string())
        .collect();
    assert!(
        matchers.iter().any(|m| m == "*"),
        "no `*` entry — governance covers only {matchers:?}, but init reported success"
    );

    // …and the duplicate has to be surfaced, not silently tolerated: it makes the
    // kernel run twice for Bash.
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Bash"),
        "the narrower duplicate was not reported to the operator:\n{stdout}"
    );
}

/// Already-correct installs must still be left alone, or "fix the narrow-matcher
/// bug" becomes "append a duplicate on every run".
#[test]
fn a_star_matcher_still_satisfies_init() {
    let tmp = tempfile::tempdir().expect("tmp");
    let target = tmp.path();
    assert!(init(target, &[]).status.success());
    assert!(init(target, &[]).status.success());
    assert_eq!(pretooluse(&settings(target)).len(), 1);
}

/// `init` runs inside a directory this codebase calls untrusted (D37). A
/// checked-in symlink must not redirect a write outside the project — clone a
/// hostile repo, run `init`, and it writes where the repo chose.
#[test]
fn init_refuses_to_write_through_a_symlink() {
    let tmp = tempfile::tempdir().expect("tmp");
    let target = tmp.path().join("proj");
    fs::create_dir_all(target.join(".claude")).expect("mkdir");

    let victim = tmp.path().join("victim.json");
    fs::write(&victim, "{\"untouched\":true}\n").expect("victim");
    std::os::unix::fs::symlink(&victim, target.join(".claude/settings.json")).expect("symlink");

    let out = init(&target, &[]);
    assert!(!out.status.success(), "init wrote through a symlink");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("symlink"), "unhelpful error:\n{stderr}");
    assert_eq!(
        fs::read_to_string(&victim).expect("victim"),
        "{\"untouched\":true}\n",
        "the symlink target was modified"
    );
}

/// A symlinked `.claude/` redirects all four files at once, so the check has to
/// cover ancestors and not just the leaf.
#[test]
fn init_refuses_a_symlinked_claude_directory() {
    let tmp = tempfile::tempdir().expect("tmp");
    let target = tmp.path().join("proj");
    fs::create_dir_all(&target).expect("mkdir proj");
    let elsewhere = tmp.path().join("elsewhere");
    fs::create_dir_all(&elsewhere).expect("mkdir elsewhere");
    std::os::unix::fs::symlink(&elsewhere, target.join(".claude")).expect("symlink dir");

    let out = init(&target, &[]);
    assert!(
        !out.status.success(),
        "init wrote through a symlinked .claude/"
    );
    assert!(
        fs::read_dir(&elsewhere).expect("read").next().is_none(),
        "files were written into the symlink target"
    );
}

/// The refusal must happen before anything lands, or a hostile symlink becomes a
/// way to leave projects half-configured.
#[test]
fn symlink_refusal_writes_nothing_at_all() {
    let tmp = tempfile::tempdir().expect("tmp");
    let target = tmp.path().join("proj");
    fs::create_dir_all(target.join(".claude")).expect("mkdir");
    let victim = tmp.path().join("victim.json");
    fs::write(&victim, "{}\n").expect("victim");
    std::os::unix::fs::symlink(&victim, target.join(".claude/settings.json")).expect("symlink");

    assert!(!init(&target, &[]).status.success());
    assert!(
        !target.join(".claude/cc-world.yaml").exists(),
        "manifest was written despite the refusal"
    );
    assert!(
        !target.join(".claude/hooks/world-gate.sh").exists(),
        "shim was written despite the refusal"
    );
}

/// Drive the shim the way a host does: JSON on stdin, verdict on stdout.
fn ask_shim(target: &Path, file_path: &str) -> String {
    use std::io::Write as _;
    let mut child = std::process::Command::new("bash")
        .arg(target.join(".claude/hooks/world-gate.sh"))
        .env("CLAUDE_PROJECT_DIR", target)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("spawn shim");
    let payload = format!(r#"{{"tool_name":"Write","tool_input":{{"file_path":"{file_path}"}}}}"#);
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(payload.as_bytes())
        .expect("write payload");
    let out = child.wait_with_output().expect("shim output");
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// Pull the per-project off-file path back out of the shim.
fn off_file_of(target: &Path) -> String {
    let body = fs::read_to_string(target.join(".claude/hooks/world-gate.sh")).expect("shim");
    body.lines()
        .find(|l| l.trim_start().starts_with("if [ -f"))
        .expect("no kill-switch line")
        .split('\'')
        .nth(1)
        .expect("no quoted off-file path")
        .to_string()
}

/// The exploit that motivated the change: a real Claude Code session was asked to
/// create `.claude/gate-off` and did, in one turn, with no denial — switching off
/// its own governance. **A switch the subject can reach is not a switch.**
#[test]
fn an_in_project_gate_off_file_no_longer_disables_governance() {
    let tmp = tempfile::tempdir().expect("tmp");
    let target = tmp.path();
    assert!(init(target, &[]).status.success());

    assert!(ask_shim(target, "/etc/passwd").contains("deny"), "baseline");

    // Exactly what the agent did.
    fs::create_dir_all(target.join(".claude")).expect("mkdir");
    fs::write(target.join(".claude/gate-off"), "").expect("decoy");

    assert!(
        ask_shim(target, "/etc/passwd").contains("deny"),
        "an in-project gate-off file disabled governance — the exploit is back"
    );
}

/// Nothing the shim actually tests may live under the governed project.
#[test]
fn the_kill_switch_paths_are_outside_the_project() {
    let tmp = tempfile::tempdir().expect("tmp");
    let target = tmp.path();
    assert!(init(target, &[]).status.success());
    let body = fs::read_to_string(target.join(".claude/hooks/world-gate.sh")).expect("shim");
    let switch_line = body
        .lines()
        .find(|l| l.trim_start().starts_with("if [ -f"))
        .expect("no kill-switch line");

    assert!(
        !switch_line.contains("$PD/"),
        "kill-switch reads a path inside the project: {switch_line}"
    );
    assert!(
        !switch_line.contains(&*target.to_string_lossy()),
        "kill-switch reads a path inside the project: {switch_line}"
    );
    let home = std::env::var("HOME").unwrap_or_default();
    assert!(
        off_file_of(target).starts_with(&home),
        "per-project off-file is not under HOME"
    );
}

/// …and it still has to work for the human it belongs to.
#[test]
fn the_out_of_project_kill_switch_still_works() {
    let tmp = tempfile::tempdir().expect("tmp");
    let target = tmp.path();
    assert!(init(target, &[]).status.success());
    let off = off_file_of(target);
    let off = Path::new(&off);

    assert!(ask_shim(target, "/etc/passwd").contains("deny"), "baseline");

    fs::create_dir_all(off.parent().expect("parent")).expect("mkdir");
    fs::write(off, "").expect("throw the switch");
    let disabled = ask_shim(target, "/etc/passwd");
    fs::remove_file(off).ok();

    assert!(
        disabled.trim().is_empty(),
        "the human kill-switch did not disable governance: {disabled:?}"
    );
    assert!(
        ask_shim(target, "/etc/passwd").contains("deny"),
        "removing the switch did not restore governance"
    );
}

// ---------------------------------------------------------------------------
// Cross-host: GitHub Copilot reads this same .claude/settings.json but executes
// the command through PowerShell on Windows, never sets $CLAUDE_PROJECT_DIR, and
// runs hooks from cwd `/` — then fails CLOSED when the hook errors, blocking
// every tool call (github/copilot-cli#4001). Reported from real use.
// ---------------------------------------------------------------------------

/// The command must survive a shell that does not expand `$VAR` the way bash does
/// and a cwd that is not the project.
#[test]
fn the_hook_command_makes_no_assumptions_about_shell_env_or_cwd() {
    let tmp = tempfile::tempdir().expect("tmp");
    let target = tmp.path();
    assert!(init(target, &[]).status.success());
    let cmd = settings(target)["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
        .as_str()
        .expect("command")
        .to_string();

    assert!(
        !cmd.contains("$CLAUDE_PROJECT_DIR"),
        "command depends on an env var Copilot never sets: {cmd}"
    );
    assert!(
        !cmd.contains('"'),
        "double quotes interpolate in PowerShell; use single quotes: {cmd}"
    );
    assert!(cmd.contains('\''), "path is not single-quoted: {cmd}");
    assert!(
        !cmd.contains('\\'),
        "backslashes are not usable by the bash these hosts invoke: {cmd}"
    );
    let quoted = cmd.split('\'').nth(1).expect("quoted path");
    assert!(
        Path::new(quoted).is_absolute(),
        "hook path is relative, but hooks run from `/` on some hosts: {cmd}"
    );
}

/// The end-to-end version of the same thing: run the shim with the variable
/// unset and the cwd somewhere else entirely, and it must still govern.
#[test]
fn the_shim_governs_without_claude_project_dir_and_from_a_foreign_cwd() {
    use std::io::Write as _;
    let tmp = tempfile::tempdir().expect("tmp");
    let target = tmp.path();
    assert!(init(target, &[]).status.success());

    let mut child = std::process::Command::new("bash")
        .arg(target.join(".claude/hooks/world-gate.sh"))
        .current_dir("/") // Copilot runs hooks from `/`
        .env_remove("CLAUDE_PROJECT_DIR") // …and never sets this
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("spawn shim");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(br#"{"tool_name":"Write","tool_input":{"file_path":"/etc/passwd"}}"#)
        .expect("write");
    let out = child.wait_with_output().expect("output");
    let verdict = String::from_utf8_lossy(&out.stdout);
    assert!(
        verdict.contains("deny"),
        "shim failed to govern without CLAUDE_PROJECT_DIR from cwd `/`: {verdict:?}"
    );
}

/// An install from an older version must be upgraded in place. Appending instead
/// would run the kernel twice per call — the duplicate-hook failure this suite
/// already guards against, arriving by a different route.
#[test]
fn an_older_hook_entry_is_upgraded_not_duplicated() {
    let tmp = tempfile::tempdir().expect("tmp");
    let target = tmp.path();
    fs::create_dir_all(target.join(".claude")).expect("mkdir");
    fs::write(
        target.join(".claude/settings.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "hooks": { "PreToolUse": [{
                "matcher": "*",
                "hooks": [{
                    "type": "command",
                    // the shape `init` wrote before 2026-08-11
                    "command": r#"bash "$CLAUDE_PROJECT_DIR/.claude/hooks/world-gate.sh""#,
                    "timeout": 10
                }]
            }]}
        }))
        .expect("serialize old settings"),
    )
    .expect("write old settings");

    assert!(init(target, &[]).status.success());
    let hooks = pretooluse(&settings(target));
    assert_eq!(
        hooks.len(),
        1,
        "old entry was duplicated rather than upgraded"
    );
    assert!(
        !hooks[0]["hooks"][0]["command"]
            .as_str()
            .expect("command")
            .contains("$CLAUDE_PROJECT_DIR"),
        "the stale command survived the upgrade"
    );
}

// ---------------------------------------------------------------------------
// The control plane. Until 2026-08-11 an agent could rewrite the policy binding
// it: `.claude/` sat inside the project's own writable root, the write drew no
// approval at all, and the shim recompiles the manifest on every call — so a
// flipped rule took effect on the NEXT tool call, not the next session.
// ---------------------------------------------------------------------------

fn verdict(target: &Path, tool: &str, file: &str) -> String {
    use std::io::Write as _;
    let mut child = std::process::Command::new("bash")
        .arg(target.join(".claude/hooks/world-gate.sh"))
        .env("CLAUDE_PROJECT_DIR", target)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("spawn shim");
    let payload = format!(r#"{{"tool_name":"{tool}","tool_input":{{"file_path":"{file}"}}}}"#);
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(payload.as_bytes())
        .expect("write");
    let out = child.wait_with_output().expect("output");
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// The agent may not rewrite the rules it is running under — via any structured
/// file tool, and for any file in the control plane.
#[test]
fn the_agent_cannot_write_the_policy_that_governs_it() {
    let tmp = tempfile::tempdir().expect("tmp");
    let target = tmp.path();
    assert!(init(target, &[]).status.success());

    for file in [
        ".claude/cc-world.yaml",       // the policy
        ".claude/hooks/world-gate.sh", // the shim that enforces it
        ".claude/settings.json",       // the registration that invokes the shim
    ] {
        let path = target.join(file);
        let path = path.to_string_lossy();
        for tool in ["Write", "Edit"] {
            let v = verdict(target, tool, &path);
            assert!(
                v.contains("deny"),
                "{tool} {file} was not denied — the agent can rewrite its own governance: {v:?}"
            );
        }
    }
}

/// Read must stay allowed. An agent that cannot see its own rules cannot explain
/// a denial or propose a sensible change to them, and opacity is not a security
/// property here — the manifest is committed to the repo anyway.
#[test]
fn the_agent_can_still_read_the_policy() {
    let tmp = tempfile::tempdir().expect("tmp");
    let target = tmp.path();
    assert!(init(target, &[]).status.success());
    let manifest = target.join(".claude/cc-world.yaml");
    let v = verdict(target, "Read", &manifest.to_string_lossy());
    assert!(
        v.trim().is_empty(),
        "reading the manifest should pass through silently, got: {v:?}"
    );
}

/// Locking the control plane must not lock the project: ordinary work continues.
#[test]
fn ordinary_project_files_are_still_writable() {
    let tmp = tempfile::tempdir().expect("tmp");
    let target = tmp.path();
    assert!(init(target, &[]).status.success());
    fs::write(target.join("main.rs"), "// work\n").expect("seed file");
    let v = verdict(target, "Write", &target.join("main.rs").to_string_lossy());
    assert!(
        v.trim().is_empty(),
        "an ordinary project write was blocked by the control-plane rule: {v:?}"
    );
}

/// A kernel inside the project it governs is a kernel the agent can replace.
/// Measured on a local npm install: `Write` to `node_modules/…/harness` returns
/// ALLOW, and swapping in a no-op makes every verdict disappear — the same shape
/// as the in-project kill-switch, arriving through the package manager.
#[test]
fn init_refuses_a_kernel_that_lives_inside_the_project() {
    let tmp = tempfile::tempdir().expect("tmp");
    let target = tmp.path();
    let inside = target.join("node_modules/ai2rules-harness-linux-x64");
    fs::create_dir_all(&inside).expect("mkdir");
    let planted = inside.join("harness");
    fs::copy(harness_bin(), &planted).expect("copy binary into the project");

    let out = std::process::Command::new(&planted)
        .arg("init")
        .arg(target)
        .output()
        .expect("run planted binary");
    assert!(
        !out.status.success(),
        "init accepted a kernel inside the governed project"
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("inside the project"),
        "unhelpful refusal:\n{err}"
    );
    assert!(
        !target.join(".claude").exists(),
        "refusal still wrote into the project"
    );

    // --force is the deliberate override, for anyone protecting the binary
    // another way (immutable image, read-only mount).
    let out = std::process::Command::new(&planted)
        .arg("init")
        .arg(target)
        .arg("--force")
        .output()
        .expect("run with --force");
    assert!(out.status.success(), "--force did not override the refusal");
}
