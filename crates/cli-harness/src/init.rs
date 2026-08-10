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
//! .gitignore                     .claude/state/
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

/// Substring identifying *our* hook entry in someone's `settings.json`, whatever
/// form it currently takes. Matching on this rather than on an exact string means
/// a re-run upgrades an older entry in place instead of appending a second one.
const HOOK_MARKER: &str = "world-gate.sh";
const HOOK_TIMEOUT: u64 = 10;

/// The `command` the host runs.
///
/// **Single-quoted, absolute, forward slashes** — each of those fixes a real
/// cross-host failure rather than being a style choice:
///
/// * *Single quotes* are literal in **both** bash and PowerShell. GitHub Copilot
///   reads this same `.claude/settings.json` but executes the command through
///   PowerShell on Windows, where `"$CLAUDE_PROJECT_DIR/..."` expands to an empty
///   variable and the hook errors — and Copilot fails **closed**, blocking every
///   tool call (github/copilot-cli#4001).
/// * *Absolute* because Copilot runs hooks from cwd `/`, so nothing relative
///   resolves.
/// * *Forward slashes* because bash on Windows (Git Bash) cannot take a
///   backslashed path, and that is the bash these hosts invoke.
///
/// The path is machine-specific, which is not a new constraint: the shim it points
/// at already bakes an absolute binary path, so `.claude/` was never portable
/// across machines. A teammate runs `harness init` and gets their own.
fn hook_command(target: &Path) -> String {
    let script = target.join(".claude/hooks/world-gate.sh");
    let path = script.to_string_lossy().replace('\\', "/");
    format!("bash '{path}'")
}

/// Lines added to `.gitignore`: runtime state and the kill-switch are per-machine,
/// never per-repo.
const GITIGNORE_LINES: [&str; 1] = [".claude/state/"];

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

/// Refuse to write through a symlink.
///
/// `init` runs inside a project directory this codebase treats as **untrusted**
/// (D37), and a checked-in `.claude/settings.json` symlink would otherwise make
/// `fs::write` land outside the project entirely — you clone a repo, run `init`,
/// and it writes somewhere you did not choose. Checked for the leaf *and* every
/// ancestor below the target, because a symlinked `.claude/` redirects all four
/// files at once.
///
/// Same class as the fixes in #36 (canonicalize hook roots) and #37 (reject
/// dangling symlink leaves); this is the third place it has come up, which is why
/// the check refuses rather than resolving. Resolving a symlink silently obeys
/// whoever planted it.
fn refuse_symlinks(root: &Path, path: &Path) -> Result<(), String> {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let mut cur = root.to_path_buf();
    for part in rel.components() {
        cur.push(part);
        match fs::symlink_metadata(&cur) {
            Ok(md) if md.file_type().is_symlink() => {
                return Err(format!(
                    "{} is a symlink.\n\
                     `init` will not write through it — the governed project is untrusted input, and\n\
                     following it would write outside {}. Remove or replace the symlink and re-run.",
                    cur.display(),
                    root.display()
                ));
            }
            _ => {} // absent is fine: we are about to create it
        }
    }
    Ok(())
}

/// Where this project's kill-switch lives — **outside the project**, under the
/// user's home directory.
///
/// Derived from the project's absolute path so each governed project gets its own
/// switch, and readable rather than hashed so a human can find it with `ls`. The
/// governed project cannot write here: out-of-root writes are denied or held for
/// approval by the very kernel this switch disables, which is the property that
/// makes the switch trustworthy.
fn off_file_for(target: &Path) -> PathBuf {
    let slug: String = target
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".claude/ai2rules/off").join(slug)
}

fn shim_body(harness_bin: &str, off_file: &str, grant: bool) -> String {
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
# Project root from THIS SCRIPT's location (.claude/hooks/world-gate.sh -> up two),
# with $CLAUDE_PROJECT_DIR only as an override. Neither the environment nor the cwd
# is trustworthy across hosts: GitHub Copilot reads this same settings.json but
# never sets $CLAUDE_PROJECT_DIR and runs hooks from `/` (github/copilot-cli#4001),
# so both the variable and $(pwd) resolve to the wrong place there. The agy shim
# has derived the root this way since D48 for the same reason.
SD="$(cd "$(dirname "${{BASH_SOURCE[0]:-$0}}")" 2>/dev/null && pwd)" || exit 0
PD="${{CLAUDE_PROJECT_DIR:-$(cd "$SD/../.." 2>/dev/null && pwd)}}" || exit 0
TRUSTED_BIN={bin}
# Instant kill-switch, no restart. Both paths live OUTSIDE the governed project,
# and that is the whole point: a switch inside the project is one the agent can
# throw itself. It was `$PD/.claude/gate-off` until 2026-08-10, when a real Claude
# Code session was asked to create that file and did, in one turn, with no denial —
# disabling its own governance. Writes outside the project root are governed
# (deny/ask), so these two require a human.
#   this project : touch {off_file}
#   everywhere   : touch $HOME/.claude/gate-off
# rm to re-enable. The shim runs per call, so the toggle is immediate.
if [ -f {off_file_q} ] || [ -f "$HOME/.claude/gate-off" ]; then exit 0; fi
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
        off_file = off_file,
        off_file_q = sh_quote(off_file),
        grant_flag = grant_flag,
    )
}

/// What merging did, so the caller can report it accurately rather than
/// guessing.
pub enum Merge {
    /// A `"*"` entry with our command was already there. Nothing to do.
    AlreadyGoverned,
    /// The entry was added.
    Added,
    /// The entry was added, **and** our command also appears under narrower
    /// matchers, which will now run the kernel twice for those tools.
    AddedAlongsideNarrow(Vec<String>),
}

/// Merge the PreToolUse entry into an existing `settings.json` value.
///
/// Idempotent on the pair `(matcher == "*", command)` — **not on command
/// alone**. That distinction is the whole point: matching on command alone made
/// `init` see our hook under a narrow matcher like `"Bash"`, report "already
/// present", exit 0, and leave Read, Write, WebFetch and every MCP tool
/// ungoverned while telling the user they were covered. A governance tool must
/// never finish a successful run with less coverage than was asked for.
///
/// So when the command is present but only under narrower matchers, the `"*"`
/// entry is **added anyway** and the duplicates are reported. That fails toward
/// more governance; the cost is the kernel running twice for those tools, which
/// is a nuisance the caller is told about rather than a silent hole.
///
/// Any other hooks the user has are preserved untouched — this file is theirs.
pub fn merge_settings(existing: Option<&str>, command: &str) -> Result<(String, Merge), String> {
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

    // Ours is anything invoking the shim, in whatever form a previous version
    // wrote it — an exact-string match would append a second entry every time the
    // command shape changed, which is how a governance hook ends up running twice.
    let carries_our_command = |entry: &Value| -> bool {
        entry
            .get("hooks")
            .and_then(Value::as_array)
            .map(|hooks| {
                hooks.iter().any(|h| {
                    h.get("command")
                        .and_then(Value::as_str)
                        .is_some_and(|c| c.contains(HOOK_MARKER))
                })
            })
            .unwrap_or(false)
    };

    // Upgrade any of our entries that carry an outdated command, in place.
    let mut upgraded = false;
    for entry in arr.iter_mut() {
        if !carries_our_command(entry) {
            continue;
        }
        if let Some(hooks) = entry.get_mut("hooks").and_then(Value::as_array_mut) {
            for h in hooks.iter_mut() {
                let stale = h
                    .get("command")
                    .and_then(Value::as_str)
                    .is_some_and(|c| c.contains(HOOK_MARKER) && c != command);
                if stale {
                    h["command"] = json!(command);
                    upgraded = true;
                }
            }
        }
    }

    let governed_everywhere = arr
        .iter()
        .any(|e| carries_our_command(e) && e.get("matcher").and_then(Value::as_str) == Some("*"));

    // Narrower matchers carrying our command: real coverage gaps if we stopped
    // here, and duplicate work once we add the `*` entry. Either way the user
    // needs to be told they exist.
    let narrow: Vec<String> = arr
        .iter()
        .filter(|e| carries_our_command(e))
        .filter_map(|e| e.get("matcher").and_then(Value::as_str))
        .filter(|m| *m != "*")
        .map(str::to_string)
        .collect();

    let outcome = if governed_everywhere {
        if upgraded {
            Merge::Added // the entry existed but its command was rewritten
        } else {
            Merge::AlreadyGoverned
        }
    } else {
        arr.push(json!({
            "matcher": "*",
            "hooks": [{ "type": "command", "command": command, "timeout": HOOK_TIMEOUT }]
        }));
        if narrow.is_empty() {
            Merge::Added
        } else {
            Merge::AddedAlongsideNarrow(narrow)
        }
    };

    let mut out = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    out.push('\n');
    Ok((out, outcome))
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
    let off_file = off_file_for(target);
    writes.push((
        target.join(".claude/hooks/world-gate.sh"),
        shim_body(harness_bin, &off_file.to_string_lossy(), grant),
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
    let (merged, outcome) = merge_settings(current.as_deref(), &hook_command(target))?;
    writes.push((settings_path, merged));
    steps.push(Step {
        verb: "write",
        path: ".claude/settings.json".into(),
        note: match &outcome {
            Merge::AlreadyGoverned => {
                "PreToolUse hook already covers all tools — left as is".into()
            }
            Merge::Added => "PreToolUse hook added".into(),
            Merge::AddedAlongsideNarrow(m) => format!(
                "PreToolUse hook added for ALL tools — note: also present under {}, \
                 which will now run the kernel twice for those; remove the narrow entr{} \
                 unless you meant it",
                m.join(", "),
                if m.len() == 1 { "y" } else { "ies" }
            ),
        },
    });

    // 4. gitignore
    let gitignore = target.join(".gitignore");
    if let Some(body) = merge_gitignore(fs::read_to_string(&gitignore).ok().as_deref()) {
        writes.push((gitignore, body));
        steps.push(Step {
            verb: "write",
            path: ".gitignore".into(),
            note: "ignore the runtime taint state".into(),
        });
    }

    // Nothing is written through a symlink — checked here rather than at write
    // time so `--dry-run` reports it and a refusal never leaves a half-written
    // project behind.
    for (path, _) in &writes {
        refuse_symlinks(target, path)?;
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
        // Write-then-rename. `settings.json` is the user's own config and a
        // truncating write that dies midway corrupts it; the shell installer this
        // replaces used `> tmp && mv` for the same reason, so a direct write was a
        // regression rather than a new risk.
        let tmp = path.with_extension(format!(
            "{}.harness-tmp",
            path.extension().and_then(|e| e.to_str()).unwrap_or("")
        ));
        if let Err(e) = fs::write(&tmp, body) {
            eprintln!("harness init: cannot write {}: {e}", tmp.display());
            return 1;
        }
        if path.extension().and_then(|e| e.to_str()) == Some("sh") {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755));
            }
        }
        if let Err(e) = fs::rename(&tmp, path) {
            let _ = fs::remove_file(&tmp);
            eprintln!("harness init: cannot write {}: {e}", path.display());
            return 1;
        }
    }

    let mode = if grant {
        "grant (replace)"
    } else {
        "additive (overlay)"
    };
    let t = target.display();
    let off = off_file_for(&target).display().to_string();
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
  2) then fetch or curl again    -> DENIED (taint_invariant)
     An ungoverned session would just prompt. That deny is the proof.

Kill-switch, effective on the next call, no restart. It lives OUTSIDE this
project on purpose — a switch the agent can reach is a switch the agent can throw:
  touch {off}                    (this project)
  touch $HOME/.claude/gate-off   (everywhere)
  rm the file to re-enable."
    );
    0
}
