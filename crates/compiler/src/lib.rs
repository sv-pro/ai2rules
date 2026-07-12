//! # compiler
//!
//! Validates a `WorldManifest` and compiles it into an immutable
//! `CompiledWorld`, including descriptor hashing (SHA-256 over JSON-normalized
//! form) and the closed-ontology / projected-world tables.
//!
//! The compiler is the design-time half of "AI Aikido": a manifest may be
//! drafted with an LLM, but compilation here is pure and deterministic.
//!
//! Status: E1. See PLAN.md.

pub mod compile;
pub mod error;
pub mod hashing;
pub mod loader;

pub use compile::{compile, compile_default, default_cli_world, default_world_yaml};
pub use error::CompileError;
pub use hashing::{hash_descriptor, hash_manifest, sha256_hex};
pub use loader::{load_json, load_yaml, validate};

#[cfg(test)]
mod tests {
    use super::*;
    use harness_types::{ActionName, ActionType, SideEffectClass, TrustLevel, WorldId};

    #[test]
    fn default_world_parses_and_compiles() {
        let world = compile_default();
        assert_eq!(world.world_id(), &WorldId::new("dev-harness-default"));
        assert!(!world.manifest_hash().as_str().is_empty());
    }

    #[test]
    fn ontology_contains_base_actions_and_scoped_capabilities() {
        let world = compile_default();
        for action in [
            "read_workspace",
            "write_workspace",
            "apply_patch",
            "run_command",
            "start_pty",
            "call_mcp_tool",
            "fetch_web",
            "update_memory",
        ] {
            assert!(
                world.in_ontology(&ActionName::new(action)),
                "missing base action {action}"
            );
        }
        for cap in [
            "read_repo_file",
            "apply_workspace_patch",
            "run_tests",
            "git_commit",
        ] {
            assert!(
                world.in_ontology(&ActionName::new(cap)),
                "missing scoped cap {cap}"
            );
        }
    }

    #[test]
    fn whole_ontology_is_projected_by_default() {
        let world = compile_default();
        assert!(world.is_projected(&ActionName::new("read_repo_file")));
        assert!(world.is_projected(&ActionName::new("run_command")));
    }

    #[test]
    fn scoped_capability_inherits_base_type_and_side_effect() {
        let world = compile_default();
        assert_eq!(
            world.action_type(&ActionName::new("run_tests")),
            Some(ActionType::Command)
        );
        assert_eq!(
            world.side_effect(&ActionName::new("read_repo_file")),
            Some(SideEffectClass::Read)
        );
    }

    #[test]
    fn capability_matrix_compiles() {
        let world = compile_default();
        assert!(world.can_perform(TrustLevel::Trusted, ActionType::Command));
        assert!(world.can_perform(TrustLevel::SemiTrusted, ActionType::Patch));
        assert!(!world.can_perform(TrustLevel::Untrusted, ActionType::Command));
        assert!(world.can_perform(TrustLevel::Untrusted, ActionType::Read));
    }

    #[test]
    fn pty_requires_approval_and_taint_rules_present() {
        let world = compile_default();
        assert!(world.requires_approval(&ActionName::new("start_pty")));
        assert!(!world.requires_approval(&ActionName::new("read_workspace")));
        assert_eq!(world.taint_rules().len(), 5);
    }

    #[test]
    fn descriptor_hashes_are_present_and_stable() {
        let a = compile_default();
        let b = compile_default();
        let read = ActionName::new("read_workspace");
        assert!(a.descriptor_hash(&read).is_some());
        // Deterministic: two compiles of the same manifest agree (E1.6).
        assert_eq!(a, b);
        assert_eq!(a.descriptor_hash(&read), b.descriptor_hash(&read));
    }

    #[test]
    fn changed_manifest_changes_world_version() {
        let base = compile_default();
        let mut manifest = default_cli_world();
        manifest.world_id = WorldId::new("dev-harness-default");
        manifest.budget.max_commands_per_task = Some(7);
        let changed = compile(&manifest).expect("compiles");
        assert_ne!(base.manifest_hash(), changed.manifest_hash());
    }

    #[test]
    fn validate_rejects_unknown_base_action() {
        let yaml = r#"
world_id: w
scoped_capabilities:
  - name: orphan
    base_action: does_not_exist
"#;
        let manifest = load_yaml(yaml).expect("parses");
        assert!(matches!(
            compile(&manifest),
            Err(CompileError::UnknownBaseAction { .. })
        ));
    }

    #[test]
    fn validate_rejects_empty_world_id() {
        let yaml = "world_id: \"\"\n";
        let manifest = load_yaml(yaml).expect("parses");
        assert_eq!(compile(&manifest), Err(CompileError::EmptyWorldId));
    }

    #[test]
    fn validate_rejects_classifier_with_unknown_actions_or_empty_patterns() {
        // D36: a classifier's `action` and every `to` must be declared base
        // actions, and no pattern may be empty.
        let base = r#"
world_id: w
base_actions:
  - { name: bash, action_type: Command, side_effect: Process }
  - { name: bash_network, action_type: Command, side_effect: Network }
"#;
        for (fragment, what) in [
            (
                "command_classes:\n  - { action: ghost, classes: [ { to: bash_network, patterns: [\"curl \"] } ] }\n",
                "unknown classifier action",
            ),
            (
                "command_classes:\n  - { action: bash, classes: [ { to: ghost, patterns: [\"curl \"] } ] }\n",
                "unknown `to` action",
            ),
            (
                "command_classes:\n  - { action: bash, classes: [ { to: bash_network, patterns: [\"\"] } ] }\n",
                "empty pattern",
            ),
        ] {
            let manifest = load_yaml(&format!("{base}{fragment}")).expect("parses");
            assert!(
                matches!(compile(&manifest), Err(CompileError::Invalid(_))),
                "{what} must be rejected"
            );
        }
    }

    #[test]
    fn validate_rejects_duplicate_base_action() {
        let yaml = r#"
world_id: w
base_actions:
  - { name: dup, action_type: Read, side_effect: Read }
  - { name: dup, action_type: Read, side_effect: Read }
"#;
        let manifest = load_yaml(yaml).expect("parses");
        assert!(matches!(
            compile(&manifest),
            Err(CompileError::DuplicateAction(_))
        ));
    }
}
