//! Manifest loading and validation (PLAN.md E1.2). Design-time only.

use std::collections::BTreeSet;

use harness_types::{SourceChannel, WorldManifest};

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

    let mut channels = BTreeSet::new();
    for channel in &manifest.channels {
        let Some(source_channel) = SourceChannel::from_name(&channel.name) else {
            return Err(CompileError::Invalid(format!(
                "unknown channel {}",
                channel.name
            )));
        };
        if !channels.insert(source_channel) {
            return Err(CompileError::Invalid(format!(
                "duplicate channel {}",
                channel.name
            )));
        }
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

    // Command classifiers (D36) must reference declared base actions only, and
    // carry no empty pattern (an empty pattern would match nothing meaningfully
    // and hints at an authoring mistake).
    for def in &manifest.command_classes {
        if !base_names.contains(def.action.as_str()) {
            return Err(CompileError::Invalid(format!(
                "command classifier references unknown action {}",
                def.action
            )));
        }
        if let Some(default_to) = &def.default_to {
            if !base_names.contains(default_to.as_str()) {
                return Err(CompileError::Invalid(format!(
                    "command classifier for {} defaults to unknown action {}",
                    def.action, default_to
                )));
            }
        }
        for class in &def.classes {
            if !base_names.contains(class.to.as_str()) {
                return Err(CompileError::Invalid(format!(
                    "command classifier for {} maps to unknown action {}",
                    def.action, class.to
                )));
            }
            if class.patterns.iter().any(|p| p.is_empty()) {
                return Err(CompileError::Invalid(format!(
                    "command classifier for {} contains an empty pattern",
                    def.action
                )));
            }
        }
    }

    Ok(())
}
