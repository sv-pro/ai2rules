//! `harness init --user` / `--managed` — govern **every** Claude Code session on a
//! machine, not one project folder (D82).
//!
//! `harness init [DIR]` writes into one project's `.claude/`, so a session opened
//! anywhere else is ungoverned. Claude Code reads hooks from two places that apply
//! to every folder, and they differ in exactly one property — who can switch them
//! off:
//!
//! * **User** — `~/.claude/settings.json` (or `$CLAUDE_CONFIG_DIR`). Applies in
//!   every project, but it is the developer's own file, and a *project* can switch
//!   it off: `disableAllHooks` in any non-managed settings file disables user hooks
//!   (and so does `claude --settings '{"disableAllHooks":true}'`). A personal
//!   seatbelt, not a control.
//! * **Managed** — `managed-settings.json` in the system policy directory. Only an
//!   administrator can write it, and nothing below the managed tier can disable a
//!   managed hook. This is the organisation answer: installed by IT, not
//!   removable by the person it governs.
//!
//! What gets written, by scope:
//!
//! ```text
//! user     <config>/ai2rules/user/cc-world.yaml     the manifest (kept unless --force)
//!          <config>/ai2rules/user/world-gate.sh     the shim
//!          <config>/ai2rules/user/state/            taint + budget sidecars (absolute)
//!          <config>/settings.json                   the hook, merged idempotently
//! managed  <policy>/ai2rules/cc-world.yaml          the manifest
//!          <policy>/ai2rules/world-gate.sh          the shim: no kill-switch, no HARNESS_BIN
//!          <policy>/ai2rules/bin/harness[.exe]      a copy of this binary, admin-owned
//!          <policy>/managed-settings.json           the hook, merged idempotently
//! ```
//!
//! **State is absolute.** `cc-hook`'s default `--state .claude/state` is relative
//! to the hook's cwd, which at machine scope is whatever folder the session is in —
//! it would scatter taint sidecars into every project. The user scope bakes an
//! absolute path; the managed scope resolves `$HOME` at run time, because one
//! policy serves every account on the machine and each needs its own state.

use crate::init::{
    apply, current_harness, merge_observers, merge_settings, refuse_symlinks, sh_quote, Merge,
    Plan, Step, HOOK_MARKER, OBSERVER_EVENTS, STARTER_WORLD,
};
use compiler::loader::load_yaml;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    User,
    Managed,
}

/// The starter's project control-plane rule. Machine-scope worlds replace it; the
/// replacement fails loudly if the starter ever stops containing it, rather than
/// silently installing a world without its self-protection.
const PROJECT_CONTROL_PLANE: &str = "- { path: \"./.claude\", access: Read }";

/// The starter's instruction-file action (D83). In a project world a clean
/// session may write `CLAUDE.md` and its kin; a machine-scope world governs every
/// project and the user's own `~/.claude`, so there a clean write asks. Same
/// loud-failure rule as [`PROJECT_CONTROL_PLANE`].
const INSTRUCTION_WRITE: &str =
    "- { name: Write_instructions, action_type: Write, side_effect: Memory }";
const INSTRUCTION_WRITE_ASK: &str =
    "  - { name: Write_instructions, action_type: Write, side_effect: Memory, approval_required: true }
";

/// The home directory: `$HOME`, else `%USERPROFILE%`. Git Bash sets `HOME`, a
/// plain Windows shell often does not.
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}

/// Claude Code's user config directory: `$CLAUDE_CONFIG_DIR`, else `~/.claude`.
pub fn user_config_dir() -> Result<PathBuf, String> {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    home_dir().map(|h| h.join(".claude")).ok_or_else(|| {
        "cannot find the home directory (neither HOME nor USERPROFILE is set)".into()
    })
}

/// The system directory Claude Code reads `managed-settings.json` from. These are
/// the documented paths; the legacy `C:\ProgramData\ClaudeCode` is not read.
pub fn managed_policy_dir() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(r"C:\Program Files\ClaudeCode")
    } else if cfg!(target_os = "macos") {
        PathBuf::from("/Library/Application Support/ClaudeCode")
    } else {
        PathBuf::from("/etc/claude-code")
    }
}

/// A path as the shim and the hook command spell it: forward slashes (Git Bash
/// cannot take backslashes) and no `\\?\` verbatim prefix, which
/// `canonicalize()` adds on Windows and no shell understands.
fn shell_path(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    s.strip_prefix("//?/").map(str::to_string).unwrap_or(s)
}

/// A path as a manifest root spells it: `~/…` when it is under the home directory
/// (the adapter expands `~` itself, on every OS), otherwise absolute.
fn root_path(p: &Path) -> String {
    if let Some(home) = home_dir() {
        if let Ok(rel) = p.strip_prefix(&home) {
            return format!("~/{}", shell_path(rel));
        }
    }
    shell_path(p)
}

/// Where each scope keeps its files.
pub struct Layout {
    pub scope: Scope,
    /// The directory every write is confined to (symlink checks start here).
    pub root: PathBuf,
    /// The settings file the hook is merged into.
    pub settings: PathBuf,
    /// Our own directory: manifest, shim, and (user scope) state.
    pub dir: PathBuf,
}

impl Layout {
    pub fn user(config_dir: &Path) -> Self {
        Layout {
            scope: Scope::User,
            root: config_dir.to_path_buf(),
            settings: config_dir.join("settings.json"),
            dir: config_dir.join("ai2rules").join("user"),
        }
    }

    pub fn managed(policy_dir: &Path) -> Self {
        Layout {
            scope: Scope::Managed,
            root: policy_dir.to_path_buf(),
            settings: policy_dir.join("managed-settings.json"),
            dir: policy_dir.join("ai2rules"),
        }
    }

    pub fn manifest(&self) -> PathBuf {
        self.dir.join("cc-world.yaml")
    }

    pub fn shim(&self) -> PathBuf {
        self.dir.join("world-gate.sh")
    }

    /// The kernel the managed shim execs: an admin-owned copy, so the governed
    /// user cannot swap it for a no-op.
    pub fn managed_bin(&self) -> PathBuf {
        self.dir.join("bin").join(if cfg!(windows) {
            "harness.exe"
        } else {
            "harness"
        })
    }

    /// The `command` registered with Claude Code. Single-quoted, absolute, forward
    /// slashes — the same shape as the project hook, for the same reasons
    /// (`init::hook_command`). The managed path on Windows has a space in it.
    pub fn hook_command(&self) -> String {
        format!("bash '{}'", shell_path(&self.shim()))
    }
}

/// The control-plane rules for a machine-scope world, replacing the starter's
/// `./.claude` rule.
///
/// The starter makes all of a project's `.claude/` read-only, which is right when
/// that directory holds the project's own governance. At machine scope it would
/// forbid ordinary work in every repository (adding a skill, a command, an agent).
/// What must stay unwritable is narrower: the files that can **switch this hook
/// off**. `disableAllHooks` in a project's `settings.json` or `settings.local.json`
/// disables user-level hooks, so an agent that can write either can turn its own
/// governance off in one edit — the in-project `gate-off` exploit again, through a
/// different file.
fn control_plane_rules(layout: &Layout) -> String {
    let mut out = String::from(
        "    # MACHINE-SCOPE CONTROL PLANE (written by `harness init --user/--managed`).\n\
         \x20   # A project's settings files can switch a user-level hook off\n\
         \x20   # (`disableAllHooks`), so they are readable, never writable, by the agent.\n\
         \x20   - { path: \"./.claude/settings.json\", access: Read }\n\
         \x20   - { path: \"./.claude/settings.local.json\", access: Read }\n",
    );
    match layout.scope {
        Scope::User => {
            let config = &layout.root;
            out.push_str(&format!(
                "    # This world, its shim, its taint state, and every ai2rules kill-switch.\n\
                 \x20   - {{ path: \"{}\", access: Read }}\n\
                 \x20   # The registration that invokes the shim (and could disable it).\n\
                 \x20   - {{ path: \"{}\", access: Read }}\n",
                root_path(&config.join("ai2rules")),
                root_path(&config.join("settings.json")),
            ));
        }
        Scope::Managed => {
            // The policy directory is admin-owned; the OS protects it. The per-user
            // taint state is not — it must be writable by the hook, which runs as
            // the user — so the agent's file tools are kept off it here.
            out.push_str(
                "    # Per-user taint state for the managed hook. The policy files themselves\n\
                 \x20   # are admin-owned and protected by the OS, not by this rule.\n\
                 \x20   - { path: \"~/.claude/ai2rules\", access: Read }\n",
            );
        }
    }
    out
}

/// Render the machine-scope manifest from the embedded starter.
pub fn render_world(layout: &Layout) -> Result<String, String> {
    if !STARTER_WORLD
        .lines()
        .any(|l| l.trim() == PROJECT_CONTROL_PLANE)
    {
        return Err(
            "embedded starter manifest has no `./.claude` control-plane rule to replace".into(),
        );
    }
    if !STARTER_WORLD.lines().any(|l| l.trim() == INSTRUCTION_WRITE) {
        return Err(
            "embedded starter manifest has no `Write_instructions` action to make ask".into(),
        );
    }
    let world_id = match layout.scope {
        Scope::User => "claude-code-user",
        Scope::Managed => "claude-code-managed",
    };
    let header = match layout.scope {
        Scope::User => {
            "# USER-LEVEL world — written by `harness init --user`. Governs every Claude\n\
             # Code session on this machine for this account; `.` is whichever project the\n\
             # session is in. Tune freely; `harness init --user` keeps it unless --force.\n"
        }
        Scope::Managed => {
            "# MANAGED world — written by `harness init --managed`. Governs every Claude Code\n\
             # session on this machine, for every account. Admin-owned: users cannot edit it.\n"
        }
    };
    // Line by line, so a CRLF checkout of the starter renders the same world.
    let mut world = String::from(header);
    for line in STARTER_WORLD.lines() {
        if line.trim() == PROJECT_CONTROL_PLANE {
            world.push_str(&control_plane_rules(layout));
        } else if line.trim() == INSTRUCTION_WRITE {
            world.push_str(INSTRUCTION_WRITE_ASK);
        } else if line.trim() == "world_id: claude-code-starter" {
            world.push_str(&format!("world_id: {world_id}\n"));
        } else {
            world.push_str(line);
            world.push('\n');
        }
    }
    load_yaml(&world)
        .map_err(|e| format!("machine-scope manifest failed to load: {e:?}"))
        .and_then(|m| {
            compiler::compile(&m)
                .map_err(|e| format!("machine-scope manifest failed to compile: {e:?}"))
        })?;
    Ok(world)
}

/// The shim, by scope.
///
/// Both derive the governed project from the session, never from where the shim
/// lives: `$CLAUDE_PROJECT_DIR`, else the hook's cwd — which `cc-hook` itself
/// falls back to. Nothing project-specific is baked in.
///
/// They differ in what the governed user can do to them:
///
/// * **user** keeps the escape hatches a person needs on their own machine — the
///   `$HOME/.claude/gate-off` switch every ai2rules shim honours, a user-only
///   switch beside the world, and the `HARNESS_BIN` override.
/// * **managed** has **none**. A kill-switch in the user's home is a switch the
///   user can throw, and `HARNESS_BIN` is an environment variable the user sets —
///   either one would make the policy optional, which is the property an
///   organisation is paying not to have. Turning it off means editing admin-owned
///   files.
pub fn shim_body(layout: &Layout, harness_bin: &str, grant: bool) -> String {
    let grant_flag = if grant { " --grant" } else { "" };
    let world = shell_path(&layout.manifest());
    match layout.scope {
        Scope::User => {
            let state = shell_path(&layout.dir.join("state"));
            let off = shell_path(&layout.dir.join("off"));
            format!(
                r#"#!/usr/bin/env bash
# ai2rules USER-LEVEL governance shim — written by `harness init --user`. Registered
# in the user settings file, so it runs for every Claude Code session this account
# opens, in any folder. Execs the Rust kernel's PreToolUse adapter; no governance
# logic lives here. Fail-open: no binary -> exit 0 (normal permissions).
#
# The governed project is the session's: $CLAUDE_PROJECT_DIR, else the hook's cwd
# (cc-hook resolves it; nothing is baked here). The world and the taint state are
# absolute — a cwd-relative state dir would scatter sidecars into every project.
set -u
TRUSTED_BIN={bin}
WORLD={world}
STATE={state}
# Kill-switches, both outside every project and both read-only to the agent under
# this world:
#   user-level hook only : touch {off}
#   every ai2rules hook  : touch $HOME/.claude/gate-off
if [ -f {off_q} ] || [ -f "$HOME/.claude/gate-off" ]; then exit 0; fi
BIN="${{HARNESS_BIN:-}}"
if [ -n "$BIN" ]; then
  case "$BIN" in /*) [ -x "$BIN" ] || exit 0 ;; *) exit 0 ;; esac
else
  BIN="$TRUSTED_BIN"
fi
[ -x "$BIN" ] || exit 0  # fail-open: no trusted kernel, normal permissions
# A world that does not compile (D87): decide with the last one that did, and warn.
# `closed` refuses every call instead; `open` lets calls through ungoverned.
exec "$BIN" cc-hook{grant_flag} --world "$WORLD" --state "$STATE" --log "$STATE/decisions.jsonl" \
  --on-broken-world last-good
"#,
                bin = sh_quote(harness_bin),
                world = sh_quote(&world),
                state = sh_quote(&state),
                off = off,
                off_q = sh_quote(&off),
                grant_flag = grant_flag,
            )
        }
        Scope::Managed => format!(
            r#"#!/usr/bin/env bash
# ai2rules MANAGED governance shim — written by `harness init --managed` into the
# admin-owned policy directory and registered in managed-settings.json. Nothing
# below the managed tier can disable it (a project's or user's `disableAllHooks`
# does not reach managed hooks).
#
# Deliberately NO kill-switch and NO HARNESS_BIN override: both live where the
# governed user can write, and either would make this policy optional. To turn it
# off, an administrator edits managed-settings.json.
set -u
BIN={bin}
WORLD={world}
[ -x "$BIN" ] || exit 0  # fail-open: a broken install must not brick every session
# Per-user state: one policy serves every account on the machine.
STATE="${{HOME:-/tmp}}/.claude/ai2rules/managed/state"
# A world that does not compile refuses every call (D87). Not `last-good`: its cache
# would live in STATE, which the user can write, so a planted "last good" world
# could stand in for this one. Not `open`: that makes the policy optional.
exec "$BIN" cc-hook{grant_flag} --world "$WORLD" --state "$STATE" --log "$STATE/decisions.jsonl" \
  --on-broken-world closed
"#,
            bin = sh_quote(harness_bin),
            world = sh_quote(&world),
            grant_flag = grant_flag,
        ),
    }
}

/// Build the plan for a machine-scope install. Reads only.
///
/// `harness_bin` is the kernel the shim will exec: the running binary for the user
/// scope, the admin-owned copy for the managed one (the caller copies it).
pub fn plan(layout: &Layout, harness_bin: &str, grant: bool, force: bool) -> Result<Plan, String> {
    let world = render_world(layout)?;
    let mut steps = Vec::new();
    let mut writes = Vec::new();
    let show = |p: &Path| p.display().to_string();

    let manifest = layout.manifest();
    if manifest.exists() && !force {
        steps.push(Step {
            verb: "keep",
            path: show(&manifest),
            note: "already present — tune it, or pass --force to replace".into(),
        });
    } else {
        writes.push((manifest.clone(), world));
        steps.push(Step {
            verb: "write",
            path: show(&manifest),
            note: "starter manifest, machine-scope control plane".into(),
        });
    }

    writes.push((layout.shim(), shim_body(layout, harness_bin, grant)));
    steps.push(Step {
        verb: "write",
        path: show(&layout.shim()),
        note: format!(
            "{} mode; kernel = {}",
            if grant { "grant / replace" } else { "additive" },
            harness_bin
        ),
    });

    let current = fs::read_to_string(&layout.settings).ok();
    let (merged, outcome) = merge_settings(current.as_deref(), &layout.hook_command())?;
    let (merged, observer_added) = merge_observers(&merged, &layout.hook_command())?;
    writes.push((layout.settings.clone(), merged));
    steps.push(Step {
        verb: "write",
        path: show(&layout.settings),
        note: match &outcome {
            Merge::AlreadyGoverned => {
                "PreToolUse hook already covers all tools — left as is".into()
            }
            Merge::Added => "PreToolUse hook added (every other key kept)".into(),
            Merge::AddedAlongsideNarrow(m) => format!(
                "PreToolUse hook added for ALL tools — also present under {}",
                m.join(", ")
            ),
        },
    });
    if observer_added {
        steps.push(Step {
            verb: "write",
            path: show(&layout.settings),
            note: "PostToolUse / PostToolUseFailure observer added (D84)".into(),
        });
    }

    for (path, _) in &writes {
        refuse_symlinks(&layout.root, path)?;
    }
    Ok(Plan { steps, writes })
}

/// Remove our hook from a settings document, leaving everything else as it was.
///
/// Matches on the shim path, not the generic `world-gate.sh` marker, so a
/// project-style ai2rules hook someone registered here by hand is not touched.
/// The gate (`PreToolUse`) and the observer events (D84) are all cleaned.
/// Containers we may have created (the event arrays, `hooks`) are dropped once empty,
/// so install-then-remove gives back the document that was there before.
/// `Ok(None)` when there was nothing of ours to remove.
pub fn unmerge_settings(existing: &str, shim: &str) -> Result<Option<String>, String> {
    let mut root: Value = serde_json::from_str(existing)
        .map_err(|e| format!("settings file is not valid JSON: {e}"))?;
    let Some(obj) = root.as_object_mut() else {
        return Err("settings file does not contain a JSON object".into());
    };
    let ours = |h: &Value| {
        h.get("command")
            .and_then(Value::as_str)
            .is_some_and(|c| c.contains(HOOK_MARKER) && c.contains(shim))
    };

    let mut removed = false;
    if let Some(hooks) = obj.get_mut("hooks").and_then(Value::as_object_mut) {
        for event in std::iter::once("PreToolUse").chain(OBSERVER_EVENTS) {
            let Some(arr) = hooks.get_mut(event).and_then(Value::as_array_mut) else {
                continue;
            };
            for entry in arr.iter_mut() {
                if let Some(list) = entry.get_mut("hooks").and_then(Value::as_array_mut) {
                    let before = list.len();
                    list.retain(|h| !ours(h));
                    removed |= list.len() != before;
                }
            }
            arr.retain(|e| {
                e.get("hooks")
                    .and_then(Value::as_array)
                    .is_none_or(|l| !l.is_empty())
            });
            if arr.is_empty() {
                hooks.remove(event);
            }
        }
        if hooks.is_empty() {
            obj.remove("hooks");
        }
    }
    if !removed {
        return Ok(None);
    }
    let mut out = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    out.push('\n');
    Ok(Some(out))
}

fn label(scope: Scope) -> &'static str {
    match scope {
        Scope::User => "--user",
        Scope::Managed => "--managed",
    }
}

fn remove(layout: &Layout, dry_run: bool) -> i32 {
    let flag = label(layout.scope);
    println!(
        "harness init {flag} --remove — {}",
        layout.settings.display()
    );
    let shim = shell_path(&layout.shim());
    let new_settings = match fs::read_to_string(&layout.settings) {
        Ok(text) => match unmerge_settings(&text, &shim) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("harness init: {e}");
                return 1;
            }
        },
        Err(_) => None,
    };
    match &new_settings {
        Some(_) => println!(
            "  write {:<40} hook removed, every other key kept",
            layout.settings.display()
        ),
        None => println!(
            "  keep  {:<40} no ai2rules hook registered here",
            layout.settings.display()
        ),
    }
    let shim_exists = layout.shim().exists();
    if shim_exists {
        println!("  remove {}", layout.shim().display());
    }
    if dry_run {
        println!("\n(dry run — nothing written)");
        return 0;
    }
    if let Some(body) = new_settings {
        if let Err(e) = refuse_symlinks(&layout.root, &layout.settings)
            .and_then(|_| apply(&[(layout.settings.clone(), body)]))
        {
            eprintln!("harness init: {e}");
            return 1;
        }
    }
    if shim_exists {
        if let Err(e) = fs::remove_file(layout.shim()) {
            eprintln!(
                "harness init: cannot remove {}: {e}",
                layout.shim().display()
            );
            return 1;
        }
    }
    println!(
        "\nRemoved. The manifest and taint state are kept in {} — delete the folder to purge.",
        layout.dir.display()
    );
    0
}

pub struct Options<'a> {
    pub scope: Scope,
    /// Managed scope: the policy directory, overriding the OS default (staging,
    /// tests). Ignored for the user scope, which follows `$CLAUDE_CONFIG_DIR`.
    pub policy_dir: Option<&'a Path>,
    pub grant: bool,
    pub force: bool,
    pub dry_run: bool,
    pub remove: bool,
}

pub fn run(o: &Options<'_>) -> i32 {
    let layout = match o.scope {
        Scope::User => match user_config_dir() {
            Ok(dir) => Layout::user(&dir),
            Err(e) => {
                eprintln!("harness init: {e}");
                return 2;
            }
        },
        Scope::Managed => Layout::managed(
            &o.policy_dir
                .map(Path::to_path_buf)
                .unwrap_or_else(managed_policy_dir),
        ),
    };
    if o.remove {
        return remove(&layout, o.dry_run);
    }

    let running = match current_harness() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("harness init: {e}");
            return 2;
        }
    };
    let running = shell_path(Path::new(&running));
    let harness_bin = match o.scope {
        Scope::User => running.clone(),
        Scope::Managed => shell_path(&layout.managed_bin()),
    };

    let plan = match plan(&layout, &harness_bin, o.grant, o.force) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("harness init: {e}");
            return 1;
        }
    };

    println!(
        "harness init {} — {}",
        label(o.scope),
        layout.root.display()
    );
    if o.scope == Scope::Managed {
        println!(
            "  copy  {:<40} from {running}",
            layout.managed_bin().display()
        );
    }
    for step in &plan.steps {
        println!("  {:<5} {:<40} {}", step.verb, step.path, step.note);
    }
    if o.dry_run {
        println!("\n(dry run — nothing written)");
        return 0;
    }

    if o.scope == Scope::Managed {
        let dest = layout.managed_bin();
        let copied = refuse_symlinks(&layout.root, &dest).and_then(|_| {
            fs::create_dir_all(dest.parent().expect("bin has a parent"))
                .and_then(|_| fs::copy(&running, &dest))
                .map_err(|e| format!("cannot install the kernel at {}: {e}", dest.display()))
        });
        if let Err(e) = copied {
            eprintln!("harness init: {e}");
            if o.policy_dir.is_none() {
                eprintln!("  The policy directory is admin-only: run this from an elevated shell.");
            }
            return 1;
        }
    }
    if let Err(e) = apply(&plan.writes) {
        eprintln!("harness init: {e}");
        if o.scope == Scope::Managed && o.policy_dir.is_none() {
            eprintln!("  The policy directory is admin-only: run this from an elevated shell.");
        }
        return 1;
    }

    let mode = if o.grant {
        "grant (replace)"
    } else {
        "additive (overlay)"
    };
    match o.scope {
        Scope::User => println!(
            "\nGoverned in {mode} mode: every Claude Code session this account opens, in any folder.

A project's own ai2rules hook, if it has one, still runs too: both answer, and the
stricter verdict wins (deny > ask > allow).

What this does NOT stop: a project or a command line setting `disableAllHooks`
switches user-level hooks off. The world above makes the project settings files
read-only to the agent, but a person can still do it. For a hook nobody but an
administrator can remove, use `harness init --managed` (docs/machine-wide-governance.md).

Kill-switches (no restart):
  touch {off}   (this hook only)
  touch $HOME/.claude/gate-off   (every ai2rules hook)
Remove: harness init --user --remove",
            off = shell_path(&layout.dir.join("off")),
        ),
        Scope::Managed => println!(
            "\nGoverned in {mode} mode: every Claude Code session on this machine, every account.
No user-side kill-switch exists by design. Optional hardening, in the same file:
  \"allowManagedHooksOnly\": true   — run only managed hooks (blocks user/project hooks)
Confirm in a session with /status: `Setting sources` lists Enterprise managed settings (file)."
        ),
    }
    0
}
