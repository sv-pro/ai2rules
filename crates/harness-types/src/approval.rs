//! Durable, effect-bound authorization instances for approved `ASK` decisions.

use serde::{Deserialize, Serialize};

use crate::decision::EffectMode;
use crate::ids::{
    ActionName, AuthorizationInstanceId, ContentHash, DescriptorHash, ManifestHash, PrincipalId,
    WorldId,
};
use crate::provenance::Provenance;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalState {
    Pending,
    Approved,
    /// The single-use grant was durably claimed before the external effect.
    Consumed,
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

/// A durable, narrowing authorization for one exact effect.
///
/// The compiled world remains the capability ceiling. This artifact can satisfy
/// an `ASK`, but cannot turn a world `DENY`/`ABSENT` into permission. It is bound
/// to principal, canonical effect, resource, world/schema epochs, provenance,
/// effect mode, expiry, and a single-use consumption budget.
///
/// `manifest_hash` is the binding that `world_id` does not provide (finding #15):
/// a world keeps its id while its rules are rewritten, so an approval granted
/// under one policy stayed valid under the next. Both are kept — `world_id` says
/// *which* world, `manifest_hash` says *which version of it*.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthorizationInstance {
    pub id: AuthorizationInstanceId,
    pub state: ApprovalState,
    /// Authenticated runtime principal. An empty legacy value never consumes.
    #[serde(default)]
    pub principal: PrincipalId,
    pub action: ActionName,
    pub params_hash: ContentHash,
    /// SHA-256 over the versioned canonical effect envelope.
    #[serde(default)]
    pub canonical_effect_hash: ContentHash,
    /// Normalized resource label for evidence and exact binding.
    #[serde(default)]
    pub resource: String,
    pub world_id: WorldId,
    /// World epoch: the exact compiled manifest hash.
    pub manifest_hash: ManifestHash,
    /// Schema epoch: the exact effective action descriptor hash.
    pub descriptor_hash: DescriptorHash,
    pub provenance: Provenance,
    pub effect_mode: EffectMode,
    /// Exclusive expiry instant. Zero marks a legacy/non-consumable artifact.
    #[serde(default)]
    pub valid_until_unix_ms: u64,
    /// Remaining successful claims. AI2-10 mints exactly one.
    #[serde(default)]
    pub remaining_uses: u32,
}

impl AuthorizationInstance {
    #[allow(clippy::too_many_arguments)]
    pub fn pending(
        id: AuthorizationInstanceId,
        principal: PrincipalId,
        action: ActionName,
        params_hash: ContentHash,
        canonical_effect_hash: ContentHash,
        resource: String,
        world_id: WorldId,
        manifest_hash: ManifestHash,
        descriptor_hash: DescriptorHash,
        provenance: Provenance,
        effect_mode: EffectMode,
        valid_until_unix_ms: u64,
    ) -> Self {
        Self {
            id,
            state: ApprovalState::Pending,
            principal,
            action,
            params_hash,
            canonical_effect_hash,
            resource,
            world_id,
            manifest_hash,
            descriptor_hash,
            provenance,
            effect_mode,
            valid_until_unix_ms,
            remaining_uses: 1,
        }
    }

    pub fn approve(&mut self) -> Result<(), ApprovalTransitionError> {
        self.transition(ApprovalState::Approved, &[ApprovalState::Pending])
    }

    pub fn reject(&mut self) -> Result<(), ApprovalTransitionError> {
        self.transition(ApprovalState::Rejected, &[ApprovalState::Pending])
    }

    pub fn consume(&mut self) -> Result<(), ApprovalTransitionError> {
        self.transition(ApprovalState::Consumed, &[ApprovalState::Approved])?;
        self.remaining_uses = self.remaining_uses.saturating_sub(1);
        Ok(())
    }

    pub fn mark_executed(&mut self) -> Result<(), ApprovalTransitionError> {
        self.transition(ApprovalState::Executed, &[ApprovalState::Consumed])
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

/// Canonical public name. `ApprovalToken` remains as a source-compatible alias
/// while host integrations move to the effect-bound vocabulary.
pub type ApprovalToken = AuthorizationInstance;
