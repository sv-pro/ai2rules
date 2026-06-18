//! Compile a validated `WorldManifest` into an immutable `CompiledWorld`
//! (PLAN.md E1.3, E1.5, E1.6).

use harness_types::{
    BackingIdentity, CompiledWorld, CompiledWorldParts, Descriptor, TaintRule, WorldManifest,
};
use serde_json::Value;

use crate::error::CompileError;
use crate::hashing::{hash_descriptor, hash_manifest};
use crate::loader::{load_yaml, validate};

/// Compile a manifest into an immutable, hash-addressed `CompiledWorld`.
///
/// Compilation is pure and deterministic: the same manifest always yields an
/// equal `CompiledWorld` with the same `manifest_hash`. "Hot reload" (E1.6) is
/// simply calling this again — it mints a new value and never mutates an
/// existing one.
pub fn compile(manifest: &WorldManifest) -> Result<CompiledWorld, CompileError> {
    validate(manifest)?;

    let mut parts = CompiledWorldParts {
        world_id: manifest.world_id.clone(),
        ..Default::default()
    };
    parts.manifest_hash = hash_manifest(manifest);

    // Base actions populate the ontology, descriptors, types, and side effects.
    for action in &manifest.base_actions {
        parts.ontology.insert(action.name.clone());
        parts
            .action_types
            .insert(action.name.clone(), action.action_type);
        parts
            .side_effects
            .insert(action.name.clone(), action.side_effect);
        if action.approval_required {
            parts.approval_required.insert(action.name.clone());
        }

        let backing = action
            .backing
            .clone()
            .unwrap_or_else(|| BackingIdentity::LocalHandler(action.name.to_string()));
        let descriptor = Descriptor {
            action: action.name.clone(),
            schema: action.schema.clone(),
            arg_constraints: action.arg_constraints.clone(),
            side_effect: action.side_effect,
            backing,
            metadata: Value::Null,
        };
        parts
            .descriptor_hashes
            .insert(action.name.clone(), hash_descriptor(&descriptor));
        parts.descriptors.insert(action.name.clone(), descriptor);
    }

    // Scoped capabilities inherit type, side effect, backing, and approval from
    // their base action. The actor-visible schema narrowing (stripping locked
    // args, injecting literals) is E7; here the scoping is recorded in metadata.
    for cap in &manifest.scoped_capabilities {
        let base = manifest
            .base_actions
            .iter()
            .find(|a| a.name == cap.base_action)
            .expect("validate() guarantees the base action exists");

        parts.ontology.insert(cap.name.clone());
        parts
            .action_types
            .insert(cap.name.clone(), base.action_type);
        parts
            .side_effects
            .insert(cap.name.clone(), base.side_effect);
        if base.approval_required {
            parts.approval_required.insert(cap.name.clone());
        }

        let backing = base
            .backing
            .clone()
            .unwrap_or_else(|| BackingIdentity::LocalHandler(base.name.to_string()));
        let metadata = serde_json::json!({
            "base_action": cap.base_action.as_str(),
            "args": serde_json::to_value(&cap.args).unwrap_or(Value::Null),
        });
        let descriptor = Descriptor {
            action: cap.name.clone(),
            schema: base.schema.clone(),
            arg_constraints: base.arg_constraints.clone(),
            side_effect: base.side_effect,
            backing,
            metadata,
        };
        parts
            .descriptor_hashes
            .insert(cap.name.clone(), hash_descriptor(&descriptor));
        parts.descriptors.insert(cap.name.clone(), descriptor);

        // Record the scoping so the kernel can strip locked args and inject
        // literals at spec-assembly time (E7).
        parts
            .scoped_capabilities
            .insert(cap.name.clone(), cap.clone());
    }

    // Default projection exposes the whole ontology. Dynamic narrowing by taint
    // and context is a runtime concern (E2 / Layer 2).
    parts.projected = parts.ontology.clone();

    // Capability matrix.
    for grant in &manifest.capabilities {
        let entry = parts.capability_matrix.entry(grant.trust).or_default();
        entry.extend(grant.actions.iter().copied());
    }

    // Taint-flow rules compiled from transition policies; unnamed rules get a
    // stable positional name.
    for (i, policy) in manifest.transition_policies.iter().enumerate() {
        let rule = if policy.rule.is_empty() {
            format!("transition_{i}")
        } else {
            policy.rule.clone()
        };
        parts.taint_rules.push(TaintRule {
            from_taint: policy.from_taint,
            side_effect: policy.side_effect,
            decision: policy.decision,
            rule,
        });
    }

    parts.budget = manifest.budget.clone();
    parts.redaction = manifest.observability.redact.clone();

    Ok(CompiledWorld::new(parts))
}

/// The bundled default CLI world manifest source (YAML), embedded at build time.
pub fn default_world_yaml() -> &'static str {
    include_str!("../assets/default_world.yaml")
}

/// Parse the bundled default CLI world manifest (PLAN.md E1.5).
pub fn default_cli_world() -> WorldManifest {
    load_yaml(default_world_yaml()).expect("bundled default world manifest must parse")
}

/// Compile the bundled default CLI world.
pub fn compile_default() -> CompiledWorld {
    compile(&default_cli_world()).expect("bundled default world manifest must compile")
}
