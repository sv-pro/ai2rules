//! Data contracts for staged external effects and their causal receipts.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    ActionName, AuthorizationInstanceId, CallId, ContentHash, DescriptorHash, EffectMode,
    ExecutionReceiptId, ManifestHash, PrincipalId, Provenance, StagedEffectId, TraceId, WorldId,
};

/// Operational consequence metadata carried to the finality boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsequenceClass {
    Compensable,
    Irreversible,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Consequence {
    pub class: ConsequenceClass,
    /// Required for compensable effects and forbidden for irreversible effects.
    pub compensation_operation: Option<String>,
}

impl Consequence {
    pub fn compensable(operation: impl Into<String>) -> Self {
        Self {
            class: ConsequenceClass::Compensable,
            compensation_operation: Some(operation.into()),
        }
    }

    pub fn irreversible() -> Self {
        Self {
            class: ConsequenceClass::Irreversible,
            compensation_operation: None,
        }
    }

    pub fn is_well_formed(&self) -> bool {
        match self.class {
            ConsequenceClass::Compensable => self
                .compensation_operation
                .as_deref()
                .is_some_and(|operation| !operation.is_empty()),
            ConsequenceClass::Irreversible => self.compensation_operation.is_none(),
        }
    }
}

/// Exact operation understood by the privileged actuator, not by the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActuatorOperation {
    pub actuator: String,
    pub operation: String,
    pub payload: Value,
}

/// A sealed proposal for an external effect. Creating or persisting this object
/// is explicitly non-effective; only the independent commit path may actuate it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StagedEffect {
    pub id: StagedEffectId,
    /// SHA-256 of every field below, under the versioned staging envelope.
    pub artifact_hash: ContentHash,
    pub proposal_id: CallId,
    pub admission_trace_id: TraceId,
    pub authorization_id: AuthorizationInstanceId,
    pub principal: PrincipalId,
    /// Kernel-classified effective action.
    pub action: ActionName,
    /// Complete normalized argument value used during admission.
    pub arguments: Value,
    pub params_hash: ContentHash,
    pub canonical_effect_hash: ContentHash,
    pub resource: String,
    pub world_id: WorldId,
    pub world_epoch: ManifestHash,
    pub schema_epoch: DescriptorHash,
    pub provenance: Provenance,
    pub effect_mode: EffectMode,
    pub consequence: Consequence,
    pub actuator_operation: ActuatorOperation,
    pub idempotency_key: String,
    /// Exclusive artifact expiry; authorization may expire earlier.
    pub valid_until_unix_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptOutcome {
    Committed,
    Simulated,
    Rejected,
    Failed,
    Ambiguous,
    Duplicate,
}

/// Causal evidence for one commit request. A receipt reports finality; it is not
/// a kernel verdict and must never be collapsed into ALLOW/DENY.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionReceipt {
    pub id: ExecutionReceiptId,
    pub staged_effect_id: StagedEffectId,
    pub staged_effect_hash: ContentHash,
    pub proposal_id: CallId,
    pub admission_trace_id: TraceId,
    pub authorization_id: AuthorizationInstanceId,
    pub idempotency_key: String,
    pub attempt: u64,
    pub outcome: ReceiptOutcome,
    pub reason: Option<String>,
    pub downstream_result_hash: Option<ContentHash>,
    pub original_receipt_id: Option<ExecutionReceiptId>,
    pub recorded_at_unix_ms: u64,
}
