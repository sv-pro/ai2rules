//! Compile a validated `WorldManifest` into an immutable `CompiledWorld`
//! (PLAN.md E1.3, E1.5, E1.6).

use harness_types::{
    BackingIdentity, CompiledWorld, CompiledWorldParts, Descriptor, RootRule, RootsDef, TaintRule,
    WorldManifest,
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
            arg_roles: action.arg_roles.clone(),
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
            // Scoped capabilities inherit their base action's argument roles;
            // arguments pinned to a Literal are design-time-clean and are
            // filtered out at the L2 check, not here.
            arg_roles: base.arg_roles.clone(),
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

    // Command classifiers are world data (D36): compiled in, so every host's
    // gate call classifies identically — no adapter carries a pattern copy.
    parts.command_classes = manifest.command_classes.clone();

    // Path-scoped capabilities (roots). Copied as authored — `compile` stays pure
    // (no env reads). `~`/`.` expansion is the adapter's job via `resolve_root_paths`
    // at the I/O boundary, so rule paths reaching here are already absolute.
    parts.roots = manifest.roots.clone();

    parts.budget = manifest.budget.clone();
    parts.redaction = manifest.observability.redact.clone();

    Ok(CompiledWorld::new(parts))
}

/// Resolve `~`, `.`, and relative rule paths to absolute — the env-dependent step
/// the *adapter* runs before `compile`, keeping `compile` itself pure. `home` and
/// `base` (the project dir) are passed in explicitly so this function is pure too.
/// Absolute rule paths are returned unchanged.
pub fn resolve_root_paths(roots: &RootsDef, home: Option<&str>, base: Option<&str>) -> RootsDef {
    RootsDef {
        default: roots.default,
        rules: roots
            .rules
            .iter()
            .map(|r| RootRule {
                path: expand_root_path(&r.path, home, base),
                ..r.clone()
            })
            .collect(),
    }
}

fn expand_root_path(p: &str, home: Option<&str>, base: Option<&str>) -> String {
    let out = if p == "~" {
        home.map(str::to_string).unwrap_or_else(|| p.to_string())
    } else if let Some(rest) = p.strip_prefix("~/") {
        match home {
            Some(h) => format!("{}/{}", h.trim_end_matches('/'), rest),
            None => p.to_string(),
        }
    } else if p.starts_with('/') {
        p.to_string()
    } else {
        // relative (".", "./x", "x") -> under the project base
        let rel = p.strip_prefix("./").unwrap_or(p);
        match base {
            Some(b) => {
                let b = b.trim_end_matches('/');
                if rel == "." {
                    b.to_string()
                } else {
                    format!("{b}/{rel}")
                }
            }
            None => p.to_string(),
        }
    };
    out.trim_end_matches('/').to_string()
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

#[cfg(test)]
mod roots_tests {
    use super::*;
    use harness_types::{RootAccess, RootRule, RootsDef};

    fn r(path: &str) -> RootRule {
        RootRule {
            path: path.to_string(),
            access: RootAccess::Read,
            class: None,
            taint_source: false,
        }
    }

    #[test]
    fn resolve_root_paths_expands_home_dot_and_relative() {
        let roots = RootsDef {
            default: RootAccess::Ask,
            rules: vec![
                r("~/.ssh"),
                r("~"),
                r("."),
                r("./src"),
                r("logs"),
                r("/etc"),
            ],
        };
        let out = resolve_root_paths(&roots, Some("/home/u"), Some("/proj"));
        let paths: Vec<&str> = out.rules.iter().map(|x| x.path.as_str()).collect();
        assert_eq!(
            paths,
            vec![
                "/home/u/.ssh",
                "/home/u",
                "/proj",
                "/proj/src",
                "/proj/logs",
                "/etc"
            ]
        );
    }

    #[test]
    fn resolve_root_paths_is_pure_given_inputs() {
        let roots = RootsDef {
            default: RootAccess::Deny,
            rules: vec![r("~/x")],
        };
        let a = resolve_root_paths(&roots, Some("/h"), Some("/b"));
        let b = resolve_root_paths(&roots, Some("/h"), Some("/b"));
        assert_eq!(a, b);
    }
}
