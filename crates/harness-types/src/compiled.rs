//! `CompiledWorld` — the immutable runtime artifact (architecture §5).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::action::ActionType;
use crate::decision::{Decision, EffectMode};
use crate::descriptor::{Descriptor, SideEffectClass};
use crate::ids::{ActionName, DescriptorHash, ManifestHash, WorldId};
use crate::manifest::Budget;
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
}
