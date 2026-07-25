//! Shared host-adapter helpers — the *shape* utilities every native adapter
//! needs, factored out so a second host cannot fork them (D48).
//!
//! `docs/one-kernel-many-hosts.md` records a duplication survey whose entries are
//! literally "copy #2 / copy #3" of helpers that drifted between adapters. When
//! `agy_hook` landed as the second Rust adapter, the path/name helpers in
//! `cc_hook` were about to become the next row in that table — so they live
//! here, used by both.
//!
//! Nothing in this module is policy: no taint algebra, no classification, no
//! decisions. It is path handling plus an ontology name lookup.
//!
//! The path helpers carry the D46 hardening (findings #8/#9): action targets and
//! manifest roots are canonicalized **through the filesystem** at this adapter
//! boundary, so project-local symlinks cannot choose their own root. Keep that
//! property — it is the reason these are shared rather than copied.

use harness_types::RootsDef;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Filesystem-safe form of a host session identifier, for the taint sidecar
/// filename. Host session ids are untrusted input (Claude Code's `session_id`,
/// Antigravity's `conversationId`) and must never escape the state directory.
pub fn sanitize(s: &str) -> String {
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

/// Extract and absolutize the target path of a file action, for path-scope (roots).
/// Reads the common path arg keys; returns `None` for tools without one (a shell
/// command string is not a path, so shell actions are path-scope-exempt). Relative
/// paths resolve against the project `base`, `~` against `$HOME`, and the result is
/// canonicalized through the filesystem so project-local symlinks cannot choose
/// their own root.
///
/// The key list is the **neutral** vocabulary. Hosts that spell their path
/// arguments differently (Antigravity's `AbsolutePath`/`TargetFile`) alias into
/// these in their own adapter before calling.
pub fn resolve_action_path(args: &Value, base: &str, home: Option<&str>) -> Option<String> {
    let raw = ["file_path", "path", "notebook_path"]
        .iter()
        .find_map(|k| args.get(*k).and_then(|v| v.as_str()))?;
    let path = if raw.starts_with('/') {
        PathBuf::from(raw)
    } else if raw == "~" {
        PathBuf::from(home?)
    } else if let Some(rest) = raw.strip_prefix("~/") {
        PathBuf::from(home?).join(rest)
    } else {
        Path::new(base).join(raw)
    };
    canonicalize_action_path(&path).map(path_to_string)
}

/// Lexically normalize `.`/`..`/empty segments of an absolute path (no FS access).
pub fn normalize_dots(p: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for seg in p.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            s => out.push(s),
        }
    }
    format!("/{}", out.join("/"))
}

/// Canonicalize an action target for root policy. Existing files/directories are
/// resolved directly; new file writes are classified by a canonicalized existing
/// parent plus the proposed leaf name. If the parent cannot be resolved, return
/// `None` so roots-enabled file actions fail closed as `missing_path`.
pub fn canonicalize_action_path(path: &Path) -> Option<PathBuf> {
    if let Ok(canonical) = std::fs::canonicalize(path) {
        return Some(canonical);
    }
    let parent = path.parent()?;
    let leaf = path.file_name()?;
    let parent = std::fs::canonicalize(parent).ok()?;
    Some(parent.join(leaf))
}

/// Canonicalize manifest roots at the same adapter boundary as action paths.
/// Missing roots stay lexical, preserving portable manifests for paths that may
/// not exist yet while still resolving real symlinked roots such as `.` and `~/.ssh`.
pub fn canonicalize_root_paths(roots: &RootsDef) -> RootsDef {
    let mut out = roots.clone();
    for rule in &mut out.rules {
        rule.path = std::fs::canonicalize(&rule.path)
            .map(path_to_string)
            .unwrap_or_else(|_| {
                if rule.path.starts_with('/') {
                    normalize_dots(&rule.path)
                } else {
                    rule.path.clone()
                }
            });
    }
    out
}

pub fn path_to_string(path: PathBuf) -> String {
    path.to_string_lossy().into_owned()
}

/// Host-tool-name normalization — a *mapping*, not policy: use the exact host
/// tool name if the world's ontology declares it; else its lowercase form if
/// that is declared; else unchanged (the kernel will report it ABSENT).
pub fn normalize_tool(world: &harness_types::CompiledWorld, tool: &str) -> String {
    use harness_types::ActionName;
    if world.in_ontology(&ActionName::new(tool)) {
        return tool.to_string();
    }
    let lower = tool.to_lowercase();
    if world.in_ontology(&ActionName::new(&lower)) {
        return lower;
    }
    tool.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalize_dots_collapses_dot_and_dotdot() {
        assert_eq!(normalize_dots("/a/./b/../c"), "/a/c");
        assert_eq!(normalize_dots("/a//b/"), "/a/b");
        assert_eq!(normalize_dots("/a/b/.."), "/a");
    }

    #[test]
    fn resolve_action_path_reads_file_path_and_absolutizes() {
        let dir = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(dir.path()).unwrap();
        std::fs::create_dir_all(base.join("src")).unwrap();
        std::fs::write(base.join("src/x.rs"), "").unwrap();

        let rel = json!({"file_path": "src/x.rs"});
        assert_eq!(
            resolve_action_path(&rel, base.to_str().unwrap(), None),
            Some(path_to_string(base.join("src/x.rs")))
        );

        // A not-yet-existing leaf is classified by its canonicalized parent.
        let new_file = json!({"file_path": "src/new.rs"});
        assert_eq!(
            resolve_action_path(&new_file, base.to_str().unwrap(), None),
            Some(path_to_string(base.join("src/new.rs")))
        );
    }

    /// The D46 property (findings #8/#9): a project-local symlink cannot choose
    /// its own root — the target resolves to the symlink's real destination.
    #[test]
    fn resolve_action_path_canonicalizes_through_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(dir.path()).unwrap();
        let outside = base.join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret"), "").unwrap();
        let project = base.join("project");
        std::fs::create_dir_all(&project).unwrap();
        std::os::unix::fs::symlink(&outside, project.join("link")).unwrap();

        let via_link = json!({"file_path": "link/secret"});
        assert_eq!(
            resolve_action_path(&via_link, project.to_str().unwrap(), None),
            Some(path_to_string(outside.join("secret"))),
            "a symlinked path must resolve to its real destination"
        );
    }

    #[test]
    fn resolve_action_path_is_none_for_non_path_tools() {
        // A shell command string is not a path key -> None -> path-scope exempt.
        let args = json!({"command": "rm -rf /"});
        assert_eq!(resolve_action_path(&args, "/proj", None), None);
    }

    #[test]
    fn sanitize_contains_session_ids_to_the_state_dir() {
        assert_eq!(sanitize("../../etc/passwd"), ".._.._etc_passwd");
        assert_eq!(sanitize("ec33ebf9-0cba-4100"), "ec33ebf9-0cba-4100");
    }
}
