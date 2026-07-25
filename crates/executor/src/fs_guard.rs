//! Writable/readable-root containment (invariant 8).
//!
//! This is the policy-level filesystem backstop: a target path must resolve to
//! somewhere inside an allowed root. Canonicalization follows symlinks, so a
//! *resolvable* symlink inside a root that points outside it does not escape. A
//! **dangling** symlink leaf can't be canonicalized (its target is missing), so
//! it is rejected explicitly rather than mistaken for an in-root new file — see
//! `resolve`. The OS-level enforcement (a sandbox that makes escape impossible
//! regardless of this check) is a separate, independent backstop in E8.

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
///
/// The parent-fallback runs whenever `canonicalize` fails, and a **dangling
/// symlink leaf** (a symlink whose target does not exist) is one such case: the
/// full-path `canonicalize` follows the link, hits the missing target, and errs.
/// Left unchecked, the fallback would re-attach the *symlink's own name* to a
/// canonical in-root parent and report an in-root missing file — after which
/// `fs::write` follows the link and creates/overwrites the outside target. So the
/// fallback fails closed on any symlink leaf (`symlink_metadata` is an lstat, it
/// does not follow). Non-dangling symlinks never reach here: the `canonicalize`
/// branch above resolves them to their real target and containment is checked on
/// that. Residual TOCTOU between this check and the later `fs::write` is out of
/// scope for the policy backstop — the OS sandbox (E8) is the independent guard.
fn resolve(path: &Path) -> Option<PathBuf> {
    if let Ok(canonical) = path.canonicalize() {
        return Some(canonical);
    }
    let parent = path.parent()?;
    let file = path.file_name()?;
    // A symlink leaf reaching here dangles (its target is missing); re-attaching
    // its name would let `fs::write` follow it out of the root. Refuse it.
    if is_symlink_leaf(path) {
        return None;
    }
    let parent = parent.canonicalize().ok()?;
    Some(parent.join(file))
}

/// lstat the leaf without following it; `false` if it does not exist at all.
fn is_symlink_leaf(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}
