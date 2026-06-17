//! Writable/readable-root containment (invariant 8).
//!
//! This is the policy-level filesystem backstop: a target path must resolve to
//! somewhere inside an allowed root. Canonicalization follows symlinks, so a
//! symlink inside a root that points outside it does not escape. The OS-level
//! enforcement (a sandbox that makes escape impossible regardless of this check)
//! is a separate, independent backstop in E8.

use std::path::{Path, PathBuf};

/// Resolve `target` (relative paths are joined onto `cwd`) and return the
/// canonical path **iff** it is contained within one of `roots`. Returns `None`
/// when the target is outside every root or cannot be resolved.
pub fn contained(cwd: &Path, target: &Path, roots: &[PathBuf]) -> Option<PathBuf> {
    let joined = if target.is_absolute() {
        target.to_path_buf()
    } else {
        cwd.join(target)
    };
    let resolved = resolve(&joined)?;
    for root in roots {
        if let Some(root) = resolve(root) {
            if resolved.starts_with(&root) {
                return Some(resolved);
            }
        }
    }
    None
}

/// Canonicalize a path. If the file does not exist yet (a write target), resolve
/// its parent directory and re-attach the file name, so new files are still
/// pinned to a real, canonical directory.
fn resolve(path: &Path) -> Option<PathBuf> {
    if let Ok(canonical) = path.canonicalize() {
        return Some(canonical);
    }
    let parent = path.parent()?;
    let file = path.file_name()?;
    let parent = parent.canonicalize().ok()?;
    Some(parent.join(file))
}
