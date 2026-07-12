//! `CompiledWorld` — the immutable runtime artifact (architecture §5).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::action::ActionType;
use crate::decision::{Decision, EffectMode};
use crate::descriptor::{Descriptor, SideEffectClass};
use crate::ids::{ActionName, DescriptorHash, ManifestHash, WorldId};
use crate::manifest::{Budget, CommandClassDef, ScopedCapabilityDef};
use crate::provenance::{Taint, TrustLevel};

/// A compiled taint-flow rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaintRule {
    pub from_taint: Taint,
    pub side_effect: SideEffectClass,
    pub decision: Decision,
    pub rule: String,
}

/// A compiled effect-mode rule (which effect mode an allowed action runs under).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectRule {
    pub action: ActionName,
    pub effect_mode: EffectMode,
}

/// The plain, fully-owned parts a compiler assembles. Consumed by
/// [`CompiledWorld::new`], after which the world is immutable.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CompiledWorldParts {
    pub world_id: WorldId,
    pub manifest_hash: ManifestHash,
    /// The closed full ontology: every action that can exist in this world.
    pub ontology: BTreeSet<ActionName>,
    /// The subset currently projected (visible/proposable).
    pub projected: BTreeSet<ActionName>,
    pub descriptors: BTreeMap<ActionName, Descriptor>,
    pub descriptor_hashes: BTreeMap<ActionName, DescriptorHash>,
    pub capability_matrix: BTreeMap<TrustLevel, BTreeSet<ActionType>>,
    pub action_types: BTreeMap<ActionName, ActionType>,
    pub side_effects: BTreeMap<ActionName, SideEffectClass>,
    pub taint_rules: Vec<TaintRule>,
    pub approval_required: BTreeSet<ActionName>,
    pub budget: Budget,
    pub effect_rules: Vec<EffectRule>,
    pub redaction: Vec<String>,
    /// Scoped capabilities by name: how each narrows its base action's args
    /// (E7). Drives stripping locked/unknown args and injecting literals.
    pub scoped_capabilities: BTreeMap<ActionName, ScopedCapabilityDef>,
    /// Host-syntactic command classifiers (DECISIONS D36). Skipped when empty so
    /// pre-D36 compiled worlds keep a stable serialized form.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub command_classes: Vec<CommandClassDef>,
}

/// Immutable, hash-addressed runtime artifact. No setters; read-only after
/// construction. Hot reload mints a new value, never mutates an existing one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledWorld {
    parts: CompiledWorldParts,
}

impl CompiledWorld {
    pub fn new(parts: CompiledWorldParts) -> Self {
        Self { parts }
    }

    pub fn world_id(&self) -> &WorldId {
        &self.parts.world_id
    }
    pub fn manifest_hash(&self) -> &ManifestHash {
        &self.parts.manifest_hash
    }

    /// Is the action part of the closed full ontology?
    pub fn in_ontology(&self, action: &ActionName) -> bool {
        self.parts.ontology.contains(action)
    }
    /// Is the action projected into this world/context?
    pub fn is_projected(&self, action: &ActionName) -> bool {
        self.parts.projected.contains(action)
    }
    /// Iterate the projected actions (the model-facing tool surface). Read-only;
    /// the world stays immutable.
    pub fn projected_actions(&self) -> impl Iterator<Item = &ActionName> {
        self.parts.projected.iter()
    }
    pub fn descriptor(&self, action: &ActionName) -> Option<&Descriptor> {
        self.parts.descriptors.get(action)
    }
    pub fn descriptor_hash(&self, action: &ActionName) -> Option<&DescriptorHash> {
        self.parts.descriptor_hashes.get(action)
    }
    pub fn action_type(&self, action: &ActionName) -> Option<ActionType> {
        self.parts.action_types.get(action).copied()
    }
    pub fn side_effect(&self, action: &ActionName) -> Option<SideEffectClass> {
        self.parts.side_effects.get(action).copied()
    }
    /// Does `trust` grant capability for `action_type`?
    pub fn can_perform(&self, trust: TrustLevel, action_type: ActionType) -> bool {
        self.parts
            .capability_matrix
            .get(&trust)
            .map(|set| set.contains(&action_type))
            .unwrap_or(false)
    }
    pub fn requires_approval(&self, action: &ActionName) -> bool {
        self.parts.approval_required.contains(action)
    }
    /// The scoped-capability definition for an action, if it is one.
    pub fn scoped_capability(&self, action: &ActionName) -> Option<&ScopedCapabilityDef> {
        self.parts.scoped_capabilities.get(action)
    }
    pub fn taint_rules(&self) -> &[TaintRule] {
        &self.parts.taint_rules
    }
    pub fn effect_rules(&self) -> &[EffectRule] {
        &self.parts.effect_rules
    }
    pub fn budget(&self) -> &Budget {
        &self.parts.budget
    }
    pub fn redaction_patterns(&self) -> &[String] {
        &self.parts.redaction
    }
    /// The manifest-declared command classifiers (DECISIONS D36).
    pub fn command_classes(&self) -> &[CommandClassDef] {
        &self.parts.command_classes
    }

    /// Resolve the **effective action** for a proposed call (DECISIONS D36): for
    /// the first classifier declared for `action`, read `arguments[arg]` as a
    /// string and return the first class's `to` whose any pattern matches at a
    /// left word boundary. Everything else — including actions without a
    /// classifier — resolves to the raw action. Classification is pure world
    /// data; no adapter may carry its own copy.
    pub fn classify_command(
        &self,
        action: &ActionName,
        arguments: &serde_json::Value,
    ) -> ActionName {
        let Some(def) = self
            .parts
            .command_classes
            .iter()
            .find(|d| &d.action == action)
        else {
            return action.clone();
        };
        let Some(cmd) = arguments.get(&def.arg).and_then(|c| c.as_str()) else {
            return action.clone();
        };
        for class in &def.classes {
            if class.patterns.iter().any(|p| left_word_match(cmd, p)) {
                return class.to.clone();
            }
        }
        action.clone()
    }
}

/// True iff `pat` occurs in `cmd` at a LEFT word boundary (an occurrence not
/// preceded by `[A-Za-z0-9_]`). Patterns carry their own right boundary (a
/// trailing space or `=`), so `"nc "` matches `"; nc x"` but not `"jsonc x"`,
/// and `"rm -rf"` does not match inside `"warm -rf"`.
fn left_word_match(cmd: &str, pat: &str) -> bool {
    if pat.is_empty() {
        return false;
    }
    let bytes = cmd.as_bytes();
    let mut start = 0;
    while let Some(i) = cmd[start..].find(pat) {
        let at = start + i;
        let boundary =
            at == 0 || !matches!(bytes[at - 1], b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_');
        if boundary {
            return true;
        }
        start = at + 1;
    }
    false
}
