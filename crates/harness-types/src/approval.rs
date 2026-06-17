//! Durable approval tokens (architecture §5 `ApprovalToken`).

use serde::{Deserialize, Serialize};

use crate::decision::EffectMode;
use crate::ids::{ActionName, ApprovalTokenId, ContentHash, DescriptorHash, WorldId};
use crate::provenance::Provenance;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalState {
    Pending,
    Approved,
    Rejected,
    Executed,
}

/// Error returned for an illegal approval state transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalTransitionError {
    pub from: ApprovalState,
    pub to: ApprovalState,
}

impl std::fmt::Display for ApprovalTransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "illegal approval transition: {:?} -> {:?}",
            self.from, self.to
        )
    }
}

impl std::error::Error for ApprovalTransitionError {}

/// An approval is bound to the exact action, params, world version, descriptor
/// hash, provenance, and effect mode. It cannot be reused after any drift.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApprovalToken {
    pub id: ApprovalTokenId,
    pub state: ApprovalState,
    pub action: ActionName,
    pub params_hash: ContentHash,
    pub world_id: WorldId,
    pub descriptor_hash: DescriptorHash,
    pub provenance: Provenance,
    pub effect_mode: EffectMode,
}

impl ApprovalToken {
    #[allow(clippy::too_many_arguments)]
    pub fn pending(
        id: ApprovalTokenId,
        action: ActionName,
        params_hash: ContentHash,
        world_id: WorldId,
        descriptor_hash: DescriptorHash,
        provenance: Provenance,
        effect_mode: EffectMode,
    ) -> Self {
        Self {
            id,
            state: ApprovalState::Pending,
            action,
            params_hash,
            world_id,
            descriptor_hash,
            provenance,
            effect_mode,
        }
    }

    pub fn approve(&mut self) -> Result<(), ApprovalTransitionError> {
        self.transition(ApprovalState::Approved, &[ApprovalState::Pending])
    }

    pub fn reject(&mut self) -> Result<(), ApprovalTransitionError> {
        self.transition(ApprovalState::Rejected, &[ApprovalState::Pending])
    }

    pub fn mark_executed(&mut self) -> Result<(), ApprovalTransitionError> {
        self.transition(ApprovalState::Executed, &[ApprovalState::Approved])
    }

    fn transition(
        &mut self,
        to: ApprovalState,
        allowed_from: &[ApprovalState],
    ) -> Result<(), ApprovalTransitionError> {
        if allowed_from.contains(&self.state) {
            self.state = to;
            Ok(())
        } else {
            Err(ApprovalTransitionError {
                from: self.state,
                to,
            })
        }
    }
}
