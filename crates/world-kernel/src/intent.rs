//! Sealed `IntentIR` and `IRBuilder` (architecture §5–§6).
//!
//! `IntentIR` has only private fields and no public constructor; the *only* way
//! to obtain one is [`IRBuilder::build`]. Its existence is therefore a witness
//! that the representability checks passed — the executor never re-checks.

use harness_types::{
    ActionName, ActionType, BuildError, CompiledWorld, DescriptorHash, Provenance, SideEffectClass,
    Taint, TaintContext, ToolCall,
};
use serde_json::Value;

use crate::{invariants, schema};

/// A sealed, validated execution intent. Cannot be constructed outside this
/// crate — see [`IRBuilder::build`].
#[derive(Debug, Clone, PartialEq)]
pub struct IntentIR {
    action: ActionName,
    action_type: ActionType,
    side_effect: SideEffectClass,
    params: Value,
    source: Provenance,
    taint: Taint,
    expected_descriptor_hash: DescriptorHash,
}

impl IntentIR {
    pub fn action(&self) -> &ActionName {
        &self.action
    }
    pub fn action_type(&self) -> ActionType {
        self.action_type
    }
    pub fn side_effect(&self) -> SideEffectClass {
        self.side_effect
    }
    pub fn params(&self) -> &Value {
        &self.params
    }
    pub fn source(&self) -> &Provenance {
        &self.source
    }
    pub fn taint(&self) -> Taint {
        self.taint
    }
    pub fn expected_descriptor_hash(&self) -> &DescriptorHash {
        &self.expected_descriptor_hash
    }
}

/// Builds `IntentIR` from a neutral `ToolCall` against a `CompiledWorld`.
///
/// Representability stage (§6): ontology → projection → capability → schema →
/// descriptor → hard taint invariant. A built `IntentIR` is representable by
/// construction; the contextual rules (taint policy, approval, budgets) are the
/// *disposition* stage in [`crate::disposition`].
pub struct IRBuilder<'w> {
    world: &'w CompiledWorld,
}

impl<'w> IRBuilder<'w> {
    pub fn new(world: &'w CompiledWorld) -> Self {
        Self { world }
    }

    pub fn build(
        &self,
        call: &ToolCall,
        source: Provenance,
        taint_context: &TaintContext,
    ) -> Result<IntentIR, BuildError> {
        let action = call.action_name.clone();

        // 1. Ontology existence — level-1 absence.
        if !self.world.in_ontology(&action) {
            return Err(BuildError::UnknownToOntology { action });
        }
        // 2. Projection — level-2 absence.
        if !self.world.is_projected(&action) {
            return Err(BuildError::Absent { action });
        }

        // Metadata is expected to exist for anything in the ontology; a gap is a
        // compiler invariant violation, not a user-facing absence.
        let action_type =
            self.world
                .action_type(&action)
                .ok_or_else(|| BuildError::InvariantViolation {
                    law: "ontology_consistency".to_string(),
                    detail: format!("no action_type for {action}"),
                })?;
        let side_effect = self
            .world
            .side_effect(&action)
            .unwrap_or(SideEffectClass::None);

        // 3. Capability — the actor's channel trust must grant this action type.
        //    A capability gap is ABSENT-class (§7); it surfaces as
        //    `CapabilityViolation` and maps to `Decision::Absent` in `decide`.
        if !self.world.can_perform(source.trust_level, action_type) {
            return Err(BuildError::CapabilityViolation {
                trust: source.trust_level,
                action_type,
            });
        }

        // 4. Schema — validate arguments against the frozen descriptor.
        if let Some(descriptor) = self.world.descriptor(&action) {
            schema::validate(
                &action,
                &call.arguments,
                &descriptor.schema,
                &descriptor.arg_constraints,
            )?;
        }

        // 5. Descriptor hash recorded for forward drift checks. Within one world
        //    this equals the world's current hash by construction; the cross-world
        //    drift gate (`invariants::check_descriptor_drift`) fires in E3/E6/E7.
        let expected_descriptor_hash = self
            .world
            .descriptor_hash(&action)
            .cloned()
            .unwrap_or_default();

        // Taint is read structurally from the context; callers cannot drop it,
        // and `build` never lowers it (monotonicity).
        let taint = taint_context.taint();

        // 6. Hard taint invariant — physics floor, before any manifest policy and
        //    non-overridable. A tainted value cannot drive an externally
        //    effectful action; such an intent is not representable at all.
        invariants::check_taint(&action, taint, side_effect)?;

        Ok(IntentIR {
            action,
            action_type,
            side_effect,
            params: call.arguments.clone(),
            source,
            taint,
            expected_descriptor_hash,
        })
    }
}
