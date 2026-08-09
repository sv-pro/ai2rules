//! `harness init` — govern the project in this directory, with nothing but the
//! binary you are already running.
//!
//! This is the productization wedge (`STRATEGY.md` bet #1). `install-governance.sh`
//! could already do this, but only from inside an ai2rules checkout: it needed
//! `--source` for the templates, `cargo` or a prebuilt binary to install, and `jq`
//! to merge settings. That is three prerequisites too many for "kill one concrete
//! fear in five minutes".
//!
//! Three choices remove all three, and they are the whole design:
//!
//! 1. **The starter manifest is compiled into the binary** (`include_str!`), so no
//!    checkout is needed to write a manifest.
//! 2. **The shim bakes the path of the running executable** (`current_exe()`), so
//!    there is no separate "install the binary somewhere trusted" step — the
//!    trusted absolute path is the one you just invoked.
//! 3. **The settings merge is `serde_json`**, so `jq` is not a dependency.
//!
//! What it writes, all under the target project:
//!
//! ```text
//! .claude/cc-world.yaml          the manifest (never clobbered without --force)
//! .claude/hooks/world-gate.sh    the PreToolUse shim, kill-switch baked in
//! .claude/settings.json          the hook entry, merged idempotently
//! .gitignore                     .claude/state/, .claude/gate-off
//! ```
//!
//! **The manifest is compiled before it is written.** `init` runs the real
//! compiler over the embedded template and refuses to write anything if it does
//! not build — a governance tool that installs a manifest it never checked would
//! be asserting exactly the thing this project spends its time objecting to.

use compiler::loader::load_yaml;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

/// The portable starter manifest, compiled in. Kept byte-identical to
/// `scripts/starter-world.yaml` so the shell installer and `init` can never
/// disagree about what a new project gets; `tests/init.rs` pins that.
pub const STARTER_WORLD: &str = include_str!("../../../scripts/starter-world.yaml");

/// The exact `command` string the hook entry carries. Matched on merge so a
/// second `init` updates in place instead of appending a duplicate.
const HOOK_COMMAND: &str = r#"bash "$CLAUDE_PROJECT_DIR/.claude/hooks/world-gate.sh""#;
const HOOK_TIMEOUT: u64 = 10;

/// Lines added to `.gitignore`: runtime state and the kill-switch are per-machine,
/// never per-repo.
const GITIGNORE_LINES: [&str; 2] = [".claude/state/", ".claude/gate-off"];

/// One thing `init` did or would do. Collected rather than printed inline so
/// `--dry-run` and the real run share a single code path — the plan is the
/// execution, minus the writes.
pub struct Step {
    pub verb: &'static str,
    pub path: String,
    pub note: String,
}

pub struct Plan {
    pub steps: Vec<Step>,
    pub writes: Vec<(PathBuf, String)>,
}

/// Single-quote a path for embedding in the shim. The project directory is
/// untrusted input (D37) and so is whatever path the binary was installed to.
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

fn shim_body(harness_bin: &str, grant: bool) -> String {
    let grant_flag = if grant { " --grant" } else { "" };
    format!(
        r#"#!/usr/bin/env bash
# ai2rules governance shim — written by `harness init`. Execs the Rust kernel's
# PreToolUse adapter; no governance logic lives here. Fail-open: no binary -> exit 0
# (the tool call falls through to the host's normal permission flow).
#
# The governed project is untrusted. Never resolve harness from $PD/target; use
# HARNESS_BIN only when it is an explicit absolute executable, otherwise use the
# binary baked in below — the one that ran `harness init`.
set -u
PD="${{CLAUDE_PROJECT_DIR:-$(pwd)}}"
TRUSTED_BIN={bin}
# Instant kill-switch, no restart: touch .claude/gate-off (this project) or
# ~/.claude/gate-off (panic, everywhere) to disable governance on the NEXT call;
# rm to re-enable. The shim runs per call, so the toggle is immediate.
if [ -f "$PD/.claude/gate-off" ] || [ -f "$HOME/.claude/gate-off" ]; then exit 0; fi
BIN="${{HARNESS_BIN:-}}"
if [ -n "$BIN" ]; then
  case "$BIN" in /*) [ -x "$BIN" ] || exit 0 ;; *) exit 0 ;; esac
else
  BIN="$TRUSTED_BIN"
fi
[ -x "$BIN" ] || exit 0  # fail-open: no trusted kernel, normal permissions
exec "$BIN" cc-hook{grant_flag} --world "$PD/.claude/cc-world.yaml" --state "$PD/.claude/state"
"#,
        bin = sh_quote(harness_bin),
        grant_flag = grant_flag,
    )
}

/// Merge the PreToolUse entry into an existing `settings.json` value.
///
/// Idempotent by `command`: if an entry with our command already exists under a
/// `"*"` matcher it is left alone, so running `init` twice does not stack hooks.
/// Any other hooks the user has are preserved untouched — this file is theirs.
pub fn merge_settings(existing: Option<&str>) -> Result<(String, bool), String> {
    let mut root: Value = match existing {
        Some(text) if !text.trim().is_empty() => serde_json::from_str(text)
            .map_err(|e| format!("settings.json is not valid JSON: {e}"))?,
        _ => json!({}),
    };
    if !root.is_object() {
        return Err("settings.json does not contain a JSON object".into());
    }

    let pre = root
        .as_object_mut()
        .expect("object")
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or("settings.json `hooks` is not an object")?
        .entry("PreToolUse")
        .or_insert_with(|| json!([]));

    let arr = pre
        .as_array_mut()
        .ok_or("settings.json `hooks.PreToolUse` is not an array")?;

    let already = arr.iter().any(|entry| {
        entry
            .get("hooks")
            .and_then(Value::as_array)
            .map(|hooks| {
                hooks
                    .iter()
                    .any(|h| h.get("command").and_then(Value::as_str) == Some(HOOK_COMMAND))
            })
            .unwrap_or(false)
    });

    if !already {
        arr.push(json!({
            "matcher": "*",
            "hooks": [{ "type": "command", "command": HOOK_COMMAND, "timeout": HOOK_TIMEOUT }]
        }));
    }

    let mut out = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    out.push('\n');
    Ok((out, !already))
}

fn merge_gitignore(existing: Option<&str>) -> Option<String> {
    let current = existing.unwrap_or("");
    let missing: Vec<&str> = GITIGNORE_LINES
        .iter()
        .copied()
        .filter(|line| !current.lines().any(|l| l.trim_end() == *line))
        .collect();
    if missing.is_empty() {
        return None;
    }
    let mut out = current.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    for line in missing {
        out.push_str(line);
        out.push('\n');
    }
    Some(out)
}

/// Build the full plan without touching the filesystem beyond reading.
pub fn plan(target: &Path, harness_bin: &str, grant: bool, force: bool) -> Result<Plan, String> {
    // Compile the manifest we are about to install. A tool that ships governance
    // it never checked has no business asking anyone to trust it.
    load_yaml(STARTER_WORLD)
        .map_err(|e| format!("embedded starter manifest failed to load: {e:?}"))
        .and_then(|m| {
            compiler::compile(&m)
                .map_err(|e| format!("embedded starter manifest failed to compile: {e:?}"))
        })?;

    let mut steps = Vec::new();
    let mut writes = Vec::new();

    // 1. manifest — never clobber a tuned one without --force
    let manifest = target.join(".claude/cc-world.yaml");
    if manifest.exists() && !force {
        steps.push(Step {
            verb: "keep",
            path: ".claude/cc-world.yaml".into(),
            note: "already present — tune it, or pass --force to replace".into(),
        });
    } else {
        writes.push((manifest, STARTER_WORLD.to_string()));
        steps.push(Step {
            verb: "write",
            path: ".claude/cc-world.yaml".into(),
            note: "starter manifest, roots-confined".into(),
        });
    }

    // 2. shim — always rewritten: it encodes the binary path and the mode, both
    //    of which are properties of *this* invocation, not of the project.
    writes.push((
        target.join(".claude/hooks/world-gate.sh"),
        shim_body(harness_bin, grant),
    ));
    steps.push(Step {
        verb: "write",
        path: ".claude/hooks/world-gate.sh".into(),
        note: format!(
            "{} mode; kernel = {}",
            if grant { "grant / replace" } else { "additive" },
            harness_bin
        ),
    });

    // 3. settings.json
    let settings_path = target.join(".claude/settings.json");
    let current = fs::read_to_string(&settings_path).ok();
    let (merged, added) = merge_settings(current.as_deref())?;
    writes.push((settings_path, merged));
    steps.push(Step {
        verb: "write",
        path: ".claude/settings.json".into(),
        note: if added {
            "PreToolUse hook added".into()
        } else {
            "PreToolUse hook already present — left as is".to_string()
        },
    });

    // 4. gitignore
    let gitignore = target.join(".gitignore");
    if let Some(body) = merge_gitignore(fs::read_to_string(&gitignore).ok().as_deref()) {
        writes.push((gitignore, body));
        steps.push(Step {
            verb: "write",
            path: ".gitignore".into(),
            note: "ignore .claude/state/ and .claude/gate-off".into(),
        });
    }

    Ok(Plan { steps, writes })
}

/// Resolve the absolute path of the running binary, for baking into the shim.
fn current_harness() -> Result<String, String> {
    let exe =
        std::env::current_exe().map_err(|e| format!("cannot resolve current executable: {e}"))?;
    let exe = exe.canonicalize().unwrap_or(exe);
    Ok(exe.to_string_lossy().into_owned())
}

pub fn run(target: &Path, grant: bool, force: bool, dry_run: bool) -> i32 {
    let target = match target.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "harness init: cannot resolve target {}: {e}",
                target.display()
            );
            return 2;
        }
    };
    let harness_bin = match current_harness() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("harness init: {e}");
            return 2;
        }
    };

    let plan = match plan(&target, &harness_bin, grant, force) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("harness init: {e}");
            return 1;
        }
    };

    println!("harness init — {}", target.display());
    for step in &plan.steps {
        println!("  {:<5} {:<30} {}", step.verb, step.path, step.note);
    }

    if dry_run {
        println!("\n(dry run — nothing written)");
        return 0;
    }

    for (path, body) in &plan.writes {
        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                eprintln!("harness init: cannot create {}: {e}", parent.display());
                return 1;
            }
        }
        if let Err(e) = fs::write(path, body) {
            eprintln!("harness init: cannot write {}: {e}", path.display());
            return 1;
        }
        if path.extension().and_then(|e| e.to_str()) == Some("sh") {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o755));
            }
        }
    }

    let mode = if grant {
        "grant (replace)"
    } else {
        "additive (overlay)"
    };
    let t = target.display();
    println!(
        "\nGoverned in {mode} mode.

Prove it in five seconds, without starting a session — ask the kernel directly:

  echo '{{\"tool_name\":\"Write\",\"tool_input\":{{\"file_path\":\"/etc/passwd\"}}}}' \\
    | CLAUDE_PROJECT_DIR={t} bash {t}/.claude/hooks/world-gate.sh

  A deny verdict comes back. The same call against a file inside the project
  stays silent — that is the roots policy, and it is doing the work no
  allowlist of command strings can do.

Then the one worth seeing in a real session:
  1) fetch any web page          -> taints the session
  2) then fetch or curl again    -> DENIED (no_tainted_external)
     An ungoverned session would just prompt. That deny is the proof.

Kill-switch, effective on the next call, no restart:
  touch {t}/.claude/gate-off     (rm to re-enable)"
    );
    0
}
