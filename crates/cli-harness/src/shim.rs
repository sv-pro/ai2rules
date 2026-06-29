//! `harness shim` — PATH-level governance shims for hosts without native hook
//! support (e.g., VS Code Copilot). Two sub-commands:
//!
//! * `install` — generate wrapper shell scripts in a shim directory; each
//!   wrapper delegates to `harness shim exec` before forwarding to the real
//!   binary. Add the shim dir to the front of `$PATH` to activate governance.
//! * `exec` — govern one shell invocation in-process and `exec` the real
//!   binary on ALLOW; DENY prints to stderr and exits 1; ASK prompts in
//!   interactive mode or exits 1 fail-closed in background mode.
//!
//! Both `cc-hook` (PreToolUse) and shims share the same action names
//! (`Bash`, `Bash_network`, `Bash_destructive`) so one world manifest governs
//! both surfaces — the governability gap that defines the CC deep vs Copilot
//! MCP-only story (E16 scorecard).

use compiler::{compile, loader::load_yaml};
use harness_preview::{gate, GateContext, GateRequest, ABI_VERSION};
use serde_json::json;
use std::path::Path;

// Tools that are always egress regardless of arguments.
const NETWORK_TOOLS: [&str; 8] = ["curl", "wget", "nc", "ncat", "ssh", "scp", "sftp", "telnet"];
// Git remote subcommands that involve the network.
const GIT_REMOTE_SUBCMDS: [&str; 6] = ["push", "pull", "fetch", "clone", "remote", "submodule"];
// Inline command patterns for bash/sh — same lists as cc_hook (D25).
const EGRESS_PATTERNS: [&str; 8] = [
    "curl ", "wget ", "nc ", "ncat ", "telnet ", "ssh ", "scp ", "sftp ",
];
const DESTRUCTIVE_PATTERNS: [&str; 6] = ["rm -rf", "rm -fr", "sudo ", "mkfs", "dd if=", ":(){"];

/// Classify a shim invocation (tool name + argv) → world-manifest action name.
///
/// Mirrors cc_hook's `classify` (D25) and extends it with tool-level rules so
/// `curl` and `git push` map to `Bash_network` without inspecting arguments.
pub fn classify(tool: &str, args: &[String]) -> String {
    if NETWORK_TOOLS.contains(&tool) {
        return "Bash_network".to_string();
    }
    if tool == "git" {
        let sub = args.first().map(String::as_str).unwrap_or("");
        return if GIT_REMOTE_SUBCMDS.contains(&sub) {
            "Bash_network".to_string()
        } else {
            "Bash".to_string()
        };
    }
    if matches!(tool, "bash" | "sh" | "zsh" | "fish") {
        let cmd = if args.first().map(String::as_str) == Some("-c") {
            args.get(1).cloned().unwrap_or_default()
        } else {
            args.join(" ")
        };
        if EGRESS_PATTERNS.iter().any(|p| cmd.contains(p)) {
            return "Bash_network".to_string();
        }
        if DESTRUCTIVE_PATTERNS.iter().any(|p| cmd.contains(p)) {
            return "Bash_destructive".to_string();
        }
    }
    "Bash".to_string()
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "_.-".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// `harness shim install`: write wrapper shell scripts into `dir` for each tool.
///
/// Each script calls `harness shim exec` which runs the governance gate in-process
/// and execs the real binary on ALLOW. Prints the `export PATH` line to stderr so
/// the user can activate the shims in their devcontainer / IDE launch config.
pub fn install(
    world_path: &Path,
    dir: &Path,
    tools: &[String],
    state_dir: &Path,
    background: bool,
) -> i32 {
    let harness_bin = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("shim install: cannot locate harness binary: {e}");
            return 1;
        }
    };

    if let Err(e) = std::fs::create_dir_all(dir) {
        eprintln!(
            "shim install: cannot create shim dir {}: {e}",
            dir.display()
        );
        return 1;
    }

    let mut any_err = false;
    for tool in tools {
        let real = match which_real(tool, dir) {
            Some(p) => p,
            None => {
                eprintln!("shim install: {tool}: not found in PATH (skipping)");
                any_err = true;
                continue;
            }
        };

        let bg_flag = if background { "--background \\\n  " } else { "" };
        let script = format!(
            "#!/bin/sh\n\
             # ai2rules governance shim for: {tool}\n\
             # Real binary: {real}\n\
             exec {harness} shim exec \\\n  \
               --world {world} \\\n  \
               --tool {tool} \\\n  \
               --real {real} \\\n  \
               --state {state} \\\n  \
               {bg}-- \"$@\"\n",
            harness = harness_bin.display(),
            world = world_path.display(),
            real = real,
            state = state_dir.display(),
            bg = bg_flag,
        );

        let shim_path = dir.join(tool);
        if let Err(e) = std::fs::write(&shim_path, &script) {
            eprintln!("shim install: cannot write {}: {e}", shim_path.display());
            any_err = true;
            continue;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(
                &shim_path,
                std::fs::Permissions::from_mode(0o755),
            );
        }

        eprintln!("shim: installed {} → {real}", shim_path.display());
    }

    eprintln!("\nActivate: export PATH=\"{}:$PATH\"", dir.display());
    if any_err { 1 } else { 0 }
}

/// Locate `tool` in PATH, excluding `shim_dir` to prevent recursive shimming.
fn which_real(tool: &str, shim_dir: &Path) -> Option<String> {
    let path_var = std::env::var("PATH").unwrap_or_default();
    let shim_canonical = shim_dir.canonicalize().ok();

    for entry in path_var.split(':') {
        let dir = Path::new(entry);
        let is_shim_dir = shim_canonical
            .as_deref()
            .and_then(|sc| dir.canonicalize().ok().map(|dc| dc == sc))
            .unwrap_or(false)
            || dir == shim_dir;
        if is_shim_dir {
            continue;
        }
        let candidate = dir.join(tool);
        if !candidate.is_file() {
            continue;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = candidate.metadata() {
                if meta.permissions().mode() & 0o111 != 0 {
                    return Some(candidate.to_string_lossy().into_owned());
                }
            }
        }
        #[cfg(not(unix))]
        return Some(candidate.to_string_lossy().into_owned());
    }
    None
}

/// `harness shim exec`: govern one shell invocation and exec the real binary on
/// ALLOW, replacing the current process (no intermediate shell, no double wait).
///
/// Fail-open: if the world cannot be compiled (broken manifest, missing file),
/// the real binary is exec'd anyway so dev tooling is never bricked by a bad
/// governance config.
pub fn exec_shim(
    world_path: &Path,
    tool: &str,
    real: &Path,
    state_dir: &Path,
    background: bool,
    args: &[String],
) -> i32 {
    // Fail-open: corrupt / missing world → pass through.
    let world = match std::fs::read_to_string(world_path)
        .ok()
        .and_then(|c| load_yaml(&c).ok())
        .and_then(|m| compile(&m).ok())
    {
        Some(w) => w,
        None => return do_exec(real, args),
    };

    let session_id = std::env::var("HARNESS_SESSION_ID")
        .unwrap_or_else(|_| "default".to_string());
    let taint_file = state_dir.join(format!("taint-{}", sanitize(&session_id)));
    let tainted = taint_file.exists();

    let action = classify(tool, args);
    let req = GateRequest {
        v: ABI_VERSION,
        tool: action.clone(),
        arguments: json!({ "command": args.join(" ") }),
        context: GateContext {
            session_id: session_id.clone(),
            mode: Some(if background { "background" } else { "interactive" }.to_string()),
            taint: tainted.then(|| "tainted".to_string()),
            source_channel: None,
            approval_token: None,
        },
    };

    let res = gate(&world, &req);

    // Persist monotonic taint: the kernel's post-call taint is authoritative.
    if res.context.taint == "tainted" && !tainted {
        let _ = std::fs::create_dir_all(state_dir);
        if let Ok(mut f) = std::fs::File::create(&taint_file) {
            use std::io::Write;
            let _ = writeln!(f, "tainted by {tool} ({action})");
        }
    }

    match res.decision.as_str() {
        "DENY" => {
            eprintln!("harness [shim]: DENIED — {tool}: {}", res.reason);
            1
        }
        "ASK" => {
            if background {
                eprintln!(
                    "harness [shim]: DENIED (background, approval required) — {tool}: {}",
                    res.reason
                );
                return 1;
            }
            if prompt_approval(tool, args) {
                do_exec(real, args)
            } else {
                eprintln!("harness [shim]: action rejected.");
                1
            }
        }
        // ALLOW / ABSENT / REPLAN → pass through to real binary.
        _ => do_exec(real, args),
    }
}

fn prompt_approval(tool: &str, args: &[String]) -> bool {
    use std::io::Write;
    eprint!(
        "harness [shim]: approval required\n  {} {}\nAllow? [y/N] ",
        tool,
        args.join(" ")
    );
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).is_ok()
        && matches!(line.trim().to_lowercase().as_str(), "y" | "yes")
}

/// Exec the real binary, replacing this process (Unix exec syscall).
/// Only returns if exec fails; on success the kernel replaces the process image.
fn do_exec(real: &Path, args: &[String]) -> i32 {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = std::process::Command::new(real).args(args).exec();
        eprintln!("shim: exec {}: {err}", real.display());
        1
    }
    #[cfg(not(unix))]
    {
        match std::process::Command::new(real).args(args).status() {
            Ok(s) => s.code().unwrap_or(1),
            Err(e) => {
                eprintln!("shim: spawn {}: {e}", real.display());
                1
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_tools_always_classify_as_bash_network() {
        for tool in ["curl", "wget", "ssh", "scp", "nc", "ncat"] {
            assert_eq!(classify(tool, &[]), "Bash_network", "{tool}");
            assert_eq!(
                classify(tool, &["http://x".to_string()]),
                "Bash_network",
                "{tool} with arg"
            );
        }
    }

    #[test]
    fn git_remote_subcmds_are_network() {
        for sub in ["push", "pull", "fetch", "clone", "remote"] {
            assert_eq!(classify("git", &[sub.to_string()]), "Bash_network", "git {sub}");
        }
    }

    #[test]
    fn git_local_subcmds_are_plain_bash() {
        for sub in ["commit", "add", "log", "diff", "status", "tag"] {
            assert_eq!(classify("git", &[sub.to_string()]), "Bash", "git {sub}");
        }
    }

    #[test]
    fn bash_minus_c_destructive_classified_correctly() {
        let args = vec!["-c".to_string(), "rm -rf /tmp/x".to_string()];
        assert_eq!(classify("bash", &args), "Bash_destructive");
        let args = vec!["-c".to_string(), "sudo apt install foo".to_string()];
        assert_eq!(classify("bash", &args), "Bash_destructive");
    }

    #[test]
    fn bash_minus_c_egress_classified_correctly() {
        let args = vec!["-c".to_string(), "curl http://example.com".to_string()];
        assert_eq!(classify("bash", &args), "Bash_network");
        let args = vec!["-c".to_string(), "wget http://x | tar xz".to_string()];
        assert_eq!(classify("bash", &args), "Bash_network");
    }

    #[test]
    fn bash_safe_is_plain_bash() {
        let args = vec!["-c".to_string(), "echo hello && ls -la".to_string()];
        assert_eq!(classify("bash", &args), "Bash");
        assert_eq!(classify("sh", &[]), "Bash");
        assert_eq!(classify("zsh", &["-c".to_string(), "pwd".to_string()]), "Bash");
    }

    #[test]
    fn unknown_tools_default_to_bash() {
        assert_eq!(classify("make", &[]), "Bash");
        assert_eq!(classify("python3", &[]), "Bash");
        assert_eq!(classify("node", &[]), "Bash");
    }
}
