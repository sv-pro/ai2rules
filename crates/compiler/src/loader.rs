//! Manifest loading and validation (PLAN.md E1.2). Design-time only.

use std::collections::BTreeSet;

use harness_types::WorldManifest;

use crate::error::CompileError;

/// Parse a manifest from YAML.
pub fn load_yaml(text: &str) -> Result<WorldManifest, CompileError> {
    serde_yaml::from_str(text).map_err(|e| CompileError::Parse(e.to_string()))
}

/// Parse a manifest from JSON.
pub fn load_json(text: &str) -> Result<WorldManifest, CompileError> {
    serde_json::from_str(text).map_err(|e| CompileError::Parse(e.to_string()))
}

/// Check referential integrity before compilation. Returns the first problem
/// found with a human-readable message.
pub fn validate(manifest: &WorldManifest) -> Result<(), CompileError> {
    if manifest.world_id.as_str().is_empty() {
        return Err(CompileError::EmptyWorldId);
    }

    let mut base_names: BTreeSet<&str> = BTreeSet::new();
    for action in &manifest.base_actions {
        if !base_names.insert(action.name.as_str()) {
            return Err(CompileError::DuplicateAction(action.name.to_string()));
        }
    }

    let mut cap_names: BTreeSet<&str> = BTreeSet::new();
    for cap in &manifest.scoped_capabilities {
        if base_names.contains(cap.name.as_str()) {
            return Err(CompileError::NameCollision(cap.name.to_string()));
        }
        if !cap_names.insert(cap.name.as_str()) {
            return Err(CompileError::DuplicateAction(cap.name.to_string()));
        }
        if !base_names.contains(cap.base_action.as_str()) {
            return Err(CompileError::UnknownBaseAction {
                capability: cap.name.to_string(),
                base: cap.base_action.to_string(),
            });
        }
    }

    Ok(())
}
