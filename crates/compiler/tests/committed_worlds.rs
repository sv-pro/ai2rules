//! Every world manifest committed to the repository compiles (D86).
//!
//! The compiler refuses keywords the kernel does not enforce, so a world that
//! used to load could stop loading. A host hook that cannot compile its world
//! does not govern, which makes a broken committed world a governance gap, not
//! just a failed build. This walks the repository for manifests and compiles
//! each one.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root")
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if path.is_dir() {
            if !matches!(
                name.to_str(),
                Some("node_modules" | "target" | "pkg" | "worktrees")
            ) {
                collect(&path, out);
            }
        } else if path.extension().is_some_and(|e| e == "yaml" || e == "yml") {
            let text = fs::read_to_string(&path).unwrap_or_default();
            if text.lines().any(|l| l.starts_with("base_actions:")) {
                out.push(path);
            }
        }
    }
}

#[test]
fn every_committed_world_compiles() {
    let root = repo_root();
    let mut worlds = Vec::new();
    for dir in ["docs", "scripts", "crates", ".agents"] {
        collect(&root.join(dir), &mut worlds);
    }
    worlds.push(root.join(".claude/cc-world.yaml"));

    // The default world, the two dogfood worlds, and the demo and benchmark
    // worlds: if this drops, the walk stopped finding them.
    assert!(worlds.len() >= 15, "found only {worlds:?}");

    let failures: Vec<String> = worlds
        .iter()
        .filter_map(|path| {
            let text = fs::read_to_string(path).ok()?;
            let result = compiler::load_yaml(&text).and_then(|m| compiler::compile(&m));
            result.err().map(|e| format!("{}: {e}", path.display()))
        })
        .collect();
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The dogfood world holds the starter's path scope (D57, D91, AI2-80). The
/// repository that ships the starter demonstrates its guarantees; it does not
/// govern itself with a looser profile. A rule added to one and not the other
/// fails here.
#[test]
fn the_dogfood_world_holds_the_starter_roots() {
    let root = repo_root();
    let load = |rel: &str| {
        let text = fs::read_to_string(root.join(rel)).expect(rel);
        compiler::load_yaml(&text).expect(rel).roots
    };
    let starter = load("scripts/starter-world.yaml");
    assert!(starter.is_some(), "the starter lost its roots block");
    assert_eq!(load(".claude/cc-world.yaml"), starter);
}
