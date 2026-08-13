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
    let mut classified: BTreeSet<&str> = BTreeSet::new();
    for def in &manifest.command_classes {
        if !base_names.contains(def.action.as_str()) {
            return Err(CompileError::Invalid(format!(
                "command classifier references unknown action {}",
                def.action
            )));
        }
        // One classifier per action (finding #20). `classify_command` resolves the
        // classifier with `.find()`, so a second entry for the same action never
        // runs — and splitting a long pattern list across two blocks is exactly how
        // an author would reach for that. Channels and base actions are already
        // duplicate-checked above; this was the gap in the same family.
        if !classified.insert(def.action.as_str()) {
            return Err(CompileError::Invalid(format!(
                "duplicate command classifier for action {} — only the first would run",
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
        // A classifier must declare a catch-all (finding #19). Pattern matching over
        // shell strings is a heuristic and always evadable — `"curl" http://x`,
        // `curl$IFS'...'`, base64-pipe-sh. What makes classification *safe* is not
        // the patterns but where an unmatched command lands: `default_to` is the
        // fail-closed bucket every shipped manifest points at an approval-required,
        // network-effectful action. Omit it and an unmatched command falls back to
        // the raw action instead — typically ambient `Process`, outside the taint
        // floor — so a tainted session runs arbitrary shell. Checked last, so the
        // more specific errors above still fire first for a manifest with several
        // problems.
        if def.default_to.is_none() {
            return Err(CompileError::Invalid(format!(
                "command classifier for {} declares no `default_to`: an unmatched \
                 command would fall back to the raw action instead of a fail-closed \
                 class, so classification could be bypassed by evading every pattern",
                def.action
            )));
        }
    }

    Ok(())
}
