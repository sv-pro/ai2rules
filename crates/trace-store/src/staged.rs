//! Durable two-phase finality for staged external effects (AI2-8 / D74).
//!
//! Staging is non-effective. `commit` independently validates the exact sealed
//! artifact and current compiled epochs, durably reserves its idempotency key,
//! atomically consumes the bound authorization, and only then invokes a
//! privileged actuator. A crash or timeout after reservation is ambiguous and
//! never retried automatically.

use std::collections::BTreeMap;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use compiler::sha256_hex;
use harness_types::{
    ActuatorOperation, AuthorizationInstanceId, CallId, CompiledWorld, Consequence, ContentHash,
    EffectMode, ExecutionReceipt, ExecutionReceiptId, ManifestHash, PrincipalId, Provenance,
    ReceiptOutcome, StagedEffect, StagedEffectId, TraceId, WorldId,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::approval::{
    key_path, load_or_create_key, lock_path, private_append, StoreLock,
};
use crate::integrity::{constant_time_eq, hmac_hex};
use crate::{effect_binding, ApprovalStore, AuthorizationRejection, ConsumeOutcome, EffectBinding};

const STAGED_EFFECT_VERSION: u32 = 1;

/// Result reported by the independent actuator. These are execution-finality
/// states, not policy decisions.
#[derive(Debug, Clone, PartialEq)]
pub enum ActuatorResult {
    Applied(Value),
    Simulated(Value),
    Failed(String),
    Ambiguous(String),
}

/// Narrow interface held by the trusted commit runtime. The reasoning runtime
/// receives no actuator handle.
pub trait PrivilegedActuator {
    fn supports(&self, operation: &ActuatorOperation) -> bool;
    fn commit(&mut self, staged: &StagedEffect) -> ActuatorResult;
}

/// Deterministic actuator used by the spike and reference tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FakeActuatorMode {
    #[default]
    Succeed,
    FailBeforeApply,
    TimeoutAfterApply,
}

#[derive(Debug, Default)]
pub struct FakePrivilegedActuator {
    mode: FakeActuatorMode,
    effects: BTreeMap<String, Value>,
}

impl FakePrivilegedActuator {
    pub fn with_mode(mode: FakeActuatorMode) -> Self {
        Self {
            mode,
            effects: BTreeMap::new(),
        }
    }

    pub fn effect_count(&self) -> usize {
        self.effects.len()
    }

    pub fn effect(&self, idempotency_key: &str) -> Option<&Value> {
        self.effects.get(idempotency_key)
    }
}

impl PrivilegedActuator for FakePrivilegedActuator {
    fn supports(&self, operation: &ActuatorOperation) -> bool {
        operation.actuator == "fake" && !operation.operation.is_empty()
    }

    fn commit(&mut self, staged: &StagedEffect) -> ActuatorResult {
        if staged.effect_mode == EffectMode::Simulate {
            return ActuatorResult::Simulated(json!({
                "operation": &staged.actuator_operation.operation,
                "would_apply": &staged.actuator_operation.payload,
            }));
        }
        if let Some(previous) = self.effects.get(&staged.idempotency_key) {
            return ActuatorResult::Applied(previous.clone());
        }
        if self.mode == FakeActuatorMode::FailBeforeApply {
            return ActuatorResult::Failed("fake actuator refused before apply".to_string());
        }
        let result = json!({
            "operation": &staged.actuator_operation.operation,
            "applied": &staged.actuator_operation.payload,
        });
        self.effects
            .insert(staged.idempotency_key.clone(), result.clone());
        if self.mode == FakeActuatorMode::TimeoutAfterApply {
            ActuatorResult::Ambiguous(
                "fake actuator timed out after downstream acceptance".to_string(),
            )
        } else {
            ActuatorResult::Applied(result)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum CommitEvent {
    Staged(Box<StagedEffect>),
    AttemptStarted {
        staged_effect_id: StagedEffectId,
        staged_effect_hash: ContentHash,
        authorization_id: AuthorizationInstanceId,
        idempotency_key: String,
        attempt: u64,
        recorded_at_unix_ms: u64,
    },
    AttemptFinished(Box<ExecutionReceipt>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SignedCommitEvent {
    event: CommitEvent,
    mac: String,
}

#[derive(Default)]
struct CommitState {
    staged: BTreeMap<StagedEffectId, StagedEffect>,
    attempts: BTreeMap<String, u64>,
    started: BTreeMap<String, StagedEffectId>,
    primary: BTreeMap<String, ExecutionReceipt>,
}

/// Durable store and independent commit coordinator.
pub struct StagedEffectStore {
    path: PathBuf,
    key: Vec<u8>,
}

impl StagedEffectStore {
    pub fn open(path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = path.into();
        let key = load_or_create_key(&key_path(&path))?;
        let _ = load(&path, &key)?;
        Ok(Self { path, key })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Persist a sealed non-effective artifact.
    pub fn stage(&self, staged: StagedEffect) -> io::Result<StagedEffectId> {
        validate_shape(&staged)?;
        if staged.artifact_hash != staged_effect_hash(&staged) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "staged effect hash does not match its contents",
            ));
        }
        let _lock = StoreLock::acquire(&lock_path(&self.path))?;
        let state = load(&self.path, &self.key)?;
        if state.staged.contains_key(&staged.id) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "staged effect id already exists",
            ));
        }
        if state
            .staged
            .values()
            .any(|existing| existing.idempotency_key == staged.idempotency_key)
        {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "idempotency key already belongs to another staged effect",
            ));
        }
        let id = staged.id.clone();
        self.append(&CommitEvent::Staged(Box::new(staged)))?;
        Ok(id)
    }

    /// Independently validate and commit one exact staged artifact.
    ///
    /// The commit-store lock remains held across authorization consumption and
    /// actuation. That intentionally serializes finality for this prototype and
    /// makes one process the sole owner of an idempotency key.
    pub fn commit(
        &self,
        presented: &StagedEffect,
        world: &CompiledWorld,
        authorizations: &mut ApprovalStore,
        actuator: &mut dyn PrivilegedActuator,
        now_unix_ms: u64,
    ) -> io::Result<ExecutionReceipt> {
        let _lock = StoreLock::acquire(&lock_path(&self.path))?;
        let mut state = load(&self.path, &self.key)?;
        let attempt = state
            .attempts
            .get(&presented.idempotency_key)
            .copied()
            .unwrap_or(0)
            + 1;

        if let Some(original) = state.primary.get(&presented.idempotency_key) {
            let outcome = if original.outcome == ReceiptOutcome::Ambiguous {
                ReceiptOutcome::Ambiguous
            } else {
                ReceiptOutcome::Duplicate
            };
            let receipt = receipt(
                presented,
                attempt,
                outcome,
                Some(if outcome == ReceiptOutcome::Ambiguous {
                    "prior attempt has unknown external finality; automatic retry refused"
                        .to_string()
                } else {
                    "idempotency key already has a terminal receipt".to_string()
                }),
                original.downstream_result_hash.clone(),
                Some(original.id.clone()),
                now_unix_ms,
            );
            self.append(&CommitEvent::AttemptFinished(Box::new(receipt.clone())))?;
            return Ok(receipt);
        }

        if state.started.contains_key(&presented.idempotency_key) {
            let receipt = receipt(
                presented,
                attempt,
                ReceiptOutcome::Ambiguous,
                Some("a prior durable attempt started without a final receipt".to_string()),
                None,
                None,
                now_unix_ms,
            );
            self.append(&CommitEvent::AttemptFinished(Box::new(receipt.clone())))?;
            return Ok(receipt);
        }

        let rejection = validate_for_commit(presented, &state, world, actuator, now_unix_ms);
        if let Some(reason) = rejection {
            let receipt = receipt(
                presented,
                attempt,
                ReceiptOutcome::Rejected,
                Some(reason),
                None,
                None,
                now_unix_ms,
            );
            self.append(&CommitEvent::AttemptFinished(Box::new(receipt.clone())))?;
            return Ok(receipt);
        }

        let started = CommitEvent::AttemptStarted {
            staged_effect_id: presented.id.clone(),
            staged_effect_hash: presented.artifact_hash.clone(),
            authorization_id: presented.authorization_id.clone(),
            idempotency_key: presented.idempotency_key.clone(),
            attempt,
            recorded_at_unix_ms: now_unix_ms,
        };
        self.append(&started)?;
        apply_event(&mut state, started);

        let binding = binding_from_staged(presented);
        match authorizations.consume(&presented.authorization_id, &binding, now_unix_ms)? {
            ConsumeOutcome::Consumed(_) => {}
            ConsumeOutcome::Rejected(reason) => {
                let receipt = receipt(
                    presented,
                    attempt,
                    ReceiptOutcome::Rejected,
                    Some(authorization_reason(reason).to_string()),
                    None,
                    None,
                    now_unix_ms,
                );
                self.append(&CommitEvent::AttemptFinished(Box::new(receipt.clone())))?;
                return Ok(receipt);
            }
        }

        let (outcome, reason, result_hash) = match actuator.commit(presented) {
            ActuatorResult::Applied(value) => (
                ReceiptOutcome::Committed,
                None,
                Some(value_hash(&value)),
            ),
            ActuatorResult::Simulated(value) => (
                ReceiptOutcome::Simulated,
                None,
                Some(value_hash(&value)),
            ),
            ActuatorResult::Failed(reason) => (ReceiptOutcome::Failed, Some(reason), None),
            ActuatorResult::Ambiguous(reason) => {
                (ReceiptOutcome::Ambiguous, Some(reason), None)
            }
        };
        let receipt = receipt(
            presented,
            attempt,
            outcome,
            reason,
            result_hash,
            None,
            now_unix_ms,
        );
        self.append(&CommitEvent::AttemptFinished(Box::new(receipt.clone())))?;
        Ok(receipt)
    }

    fn append(&self, event: &CommitEvent) -> io::Result<()> {
        let payload = serde_json::to_string(event).map_err(io::Error::other)?;
        let signed = SignedCommitEvent {
            event: event.clone(),
            mac: hmac_hex(&self.key, payload.as_bytes()),
        };
        let line = serde_json::to_string(&signed).map_err(io::Error::other)?;
        let mut file = private_append(&self.path)?;
        writeln!(file, "{line}")?;
        file.flush()?;
        file.sync_data()
    }
}

/// Seal a staged artifact from the exact authorization binding and actuator
/// request. Callers must persist it with [`StagedEffectStore::stage`].
#[allow(clippy::too_many_arguments)]
pub fn stage_effect(
    id: StagedEffectId,
    proposal_id: CallId,
    admission_trace_id: TraceId,
    authorization_id: AuthorizationInstanceId,
    binding: &EffectBinding,
    arguments: Value,
    consequence: Consequence,
    actuator_operation: ActuatorOperation,
    idempotency_key: String,
    valid_until_unix_ms: u64,
) -> StagedEffect {
    let mut staged = StagedEffect {
        id,
        artifact_hash: ContentHash::default(),
        proposal_id,
        admission_trace_id,
        authorization_id,
        principal: binding.principal.clone(),
        action: binding.action.clone(),
        arguments,
        params_hash: binding.params_hash.clone(),
        canonical_effect_hash: binding.canonical_effect_hash.clone(),
        resource: binding.resource.clone(),
        world_id: binding.world_id.clone(),
        world_epoch: binding.manifest_hash.clone(),
        schema_epoch: binding.descriptor_hash.clone(),
        provenance: binding.provenance.clone(),
        effect_mode: binding.effect_mode,
        consequence,
        actuator_operation,
        idempotency_key,
        valid_until_unix_ms,
    };
    staged.artifact_hash = staged_effect_hash(&staged);
    staged
}

pub fn staged_effect_hash(staged: &StagedEffect) -> ContentHash {
    #[derive(Serialize)]
    struct Envelope<'a> {
        v: u32,
        id: &'a StagedEffectId,
        proposal_id: &'a CallId,
        admission_trace_id: &'a TraceId,
        authorization_id: &'a AuthorizationInstanceId,
        principal: &'a PrincipalId,
        action: &'a harness_types::ActionName,
        arguments: &'a Value,
        params_hash: &'a ContentHash,
        canonical_effect_hash: &'a ContentHash,
        resource: &'a str,
        world_id: &'a WorldId,
        world_epoch: &'a ManifestHash,
        schema_epoch: &'a harness_types::DescriptorHash,
        provenance: &'a Provenance,
        effect_mode: EffectMode,
        consequence: &'a Consequence,
        actuator_operation: &'a ActuatorOperation,
        idempotency_key: &'a str,
        valid_until_unix_ms: u64,
    }
    let envelope = Envelope {
        v: STAGED_EFFECT_VERSION,
        id: &staged.id,
        proposal_id: &staged.proposal_id,
        admission_trace_id: &staged.admission_trace_id,
        authorization_id: &staged.authorization_id,
        principal: &staged.principal,
        action: &staged.action,
        arguments: &staged.arguments,
        params_hash: &staged.params_hash,
        canonical_effect_hash: &staged.canonical_effect_hash,
        resource: &staged.resource,
        world_id: &staged.world_id,
        world_epoch: &staged.world_epoch,
        schema_epoch: &staged.schema_epoch,
        provenance: &staged.provenance,
        effect_mode: staged.effect_mode,
        consequence: &staged.consequence,
        actuator_operation: &staged.actuator_operation,
        idempotency_key: &staged.idempotency_key,
        valid_until_unix_ms: staged.valid_until_unix_ms,
    };
    let canonical = serde_json::to_vec(&envelope)
        .expect("staged effect contains only serializable contract values");
    ContentHash::new(sha256_hex(&canonical))
}

fn validate_shape(staged: &StagedEffect) -> io::Result<()> {
    if staged.id.as_str().is_empty()
        || staged.authorization_id.as_str().is_empty()
        || staged.principal.as_str().is_empty()
        || staged.resource.is_empty()
        || staged.idempotency_key.is_empty()
        || staged.valid_until_unix_ms == 0
        || staged.actuator_operation.actuator.is_empty()
        || staged.actuator_operation.operation.is_empty()
        || !staged.consequence.is_well_formed()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "staged effect is incomplete or has malformed consequence metadata",
        ));
    }
    Ok(())
}

fn validate_for_commit(
    presented: &StagedEffect,
    state: &CommitState,
    world: &CompiledWorld,
    actuator: &dyn PrivilegedActuator,
    now_unix_ms: u64,
) -> Option<String> {
    if let Err(error) = validate_shape(presented) {
        return Some(format!("malformed_staged_effect:{error}"));
    }
    if presented.artifact_hash != staged_effect_hash(presented) {
        return Some("staged_effect_hash_mismatch".to_string());
    }
    let Some(persisted) = state.staged.get(&presented.id) else {
        return Some("unknown_staged_effect".to_string());
    };
    if persisted != presented {
        return Some("presented_staged_effect_differs_from_persisted_artifact".to_string());
    }
    if now_unix_ms >= presented.valid_until_unix_ms {
        return Some("staged_effect_expired".to_string());
    }
    if presented.world_id != *world.world_id()
        || presented.world_epoch != *world.manifest_hash()
    {
        return Some("world_epoch_drift".to_string());
    }
    if world.descriptor_hash(&presented.action) != Some(&presented.schema_epoch) {
        return Some("schema_epoch_drift".to_string());
    }
    let binding = binding_from_staged(presented);
    if binding.params_hash != presented.params_hash
        || binding.canonical_effect_hash != presented.canonical_effect_hash
    {
        return Some("canonical_effect_mismatch".to_string());
    }
    if !matches!(presented.effect_mode, EffectMode::Execute | EffectMode::Simulate) {
        return Some("unsupported_staged_effect_mode".to_string());
    }
    if !actuator.supports(&presented.actuator_operation) {
        return Some("unmappable_actuator_operation".to_string());
    }
    None
}

fn binding_from_staged(staged: &StagedEffect) -> EffectBinding {
    effect_binding(
        staged.principal.clone(),
        staged.action.clone(),
        &staged.arguments,
        staged.resource.clone(),
        staged.world_id.clone(),
        staged.world_epoch.clone(),
        staged.schema_epoch.clone(),
        staged.provenance.clone(),
        staged.effect_mode,
    )
}

#[allow(clippy::too_many_arguments)]
fn receipt(
    staged: &StagedEffect,
    attempt: u64,
    outcome: ReceiptOutcome,
    reason: Option<String>,
    downstream_result_hash: Option<ContentHash>,
    original_receipt_id: Option<ExecutionReceiptId>,
    now_unix_ms: u64,
) -> ExecutionReceipt {
    ExecutionReceipt {
        id: ExecutionReceiptId::new(format!("receipt-{}-{attempt}", staged.id.as_str())),
        staged_effect_id: staged.id.clone(),
        staged_effect_hash: staged.artifact_hash.clone(),
        proposal_id: staged.proposal_id.clone(),
        admission_trace_id: staged.admission_trace_id.clone(),
        authorization_id: staged.authorization_id.clone(),
        idempotency_key: staged.idempotency_key.clone(),
        attempt,
        outcome,
        reason,
        downstream_result_hash,
        original_receipt_id,
        recorded_at_unix_ms: now_unix_ms,
    }
}

fn value_hash(value: &Value) -> ContentHash {
    let canonical = serde_json::to_vec(value).expect("JSON Value serialization is infallible");
    ContentHash::new(sha256_hex(&canonical))
}

fn load(path: &Path, key: &[u8]) -> io::Result<CommitState> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(CommitState::default()),
        Err(error) => return Err(error),
    };
    let mut state = CommitState::default();
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let signed: SignedCommitEvent = serde_json::from_str(&line).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("commit log {} line {}: {error}", path.display(), index + 1),
            )
        })?;
        let payload = serde_json::to_string(&signed.event).map_err(io::Error::other)?;
        let expected = hmac_hex(key, payload.as_bytes());
        if !constant_time_eq(&expected, &signed.mac) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "commit log {} line {}: MAC does not verify",
                    path.display(),
                    index + 1
                ),
            ));
        }
        apply_event(&mut state, signed.event);
    }
    Ok(state)
}

fn apply_event(state: &mut CommitState, event: CommitEvent) {
    match event {
        CommitEvent::Staged(staged) => {
            state.staged.insert(staged.id.clone(), *staged);
        }
        CommitEvent::AttemptStarted {
            staged_effect_id,
            idempotency_key,
            attempt,
            ..
        } => {
            state.attempts.insert(idempotency_key.clone(), attempt);
            state.started.insert(idempotency_key, staged_effect_id);
        }
        CommitEvent::AttemptFinished(receipt) => {
            state
                .attempts
                .insert(receipt.idempotency_key.clone(), receipt.attempt);
            state.started.remove(&receipt.idempotency_key);
            if !matches!(receipt.outcome, ReceiptOutcome::Duplicate) {
                state
                    .primary
                    .entry(receipt.idempotency_key.clone())
                    .or_insert_with(|| (*receipt).clone());
            }
        }
    }
}

fn authorization_reason(reason: AuthorizationRejection) -> &'static str {
    match reason {
        AuthorizationRejection::NotApproved => "authorization_not_approved",
        AuthorizationRejection::Revoked => "authorization_revoked",
        AuthorizationRejection::Exhausted => "authorization_exhausted",
        AuthorizationRejection::Expired => "authorization_expired",
        AuthorizationRejection::PrincipalMismatch => "authorization_principal_mismatch",
        AuthorizationRejection::EffectMismatch => "authorization_effect_mismatch",
        AuthorizationRejection::ResourceMismatch => "authorization_resource_mismatch",
        AuthorizationRejection::WorldEpochMismatch => "authorization_world_epoch_mismatch",
        AuthorizationRejection::SchemaEpochMismatch => "authorization_schema_epoch_mismatch",
        AuthorizationRejection::ProvenanceMismatch => "authorization_provenance_mismatch",
        AuthorizationRejection::EffectModeMismatch => "authorization_effect_mode_mismatch",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harness_types::{
        ActionName, ApprovalToken, ApprovalTokenId, CompiledWorldParts, DescriptorHash,
        ManifestHash, SessionId, SourceChannel,
    };
    use serde_json::json;

    const NOW: u64 = 1_000;

    struct Fixture {
        _dir: tempfile::TempDir,
        world: CompiledWorld,
        authorizations: ApprovalStore,
        stages: StagedEffectStore,
        staged: StagedEffect,
    }

    impl Fixture {
        fn new(
            effect_mode: EffectMode,
            consequence: Consequence,
            actuator: &str,
            authorization_expiry: u64,
            staged_expiry: u64,
        ) -> Self {
            let dir = tempfile::tempdir().unwrap();
            let world = world("world-epoch-1", "schema-epoch-1");
            let arguments = json!({"account":"cash","amount":7});
            let provenance = Provenance::from_channel(
                SourceChannel::UserPrompt,
                SessionId::new("session-a"),
                ContentHash::new("proposal-content"),
            );
            let binding = effect_binding(
                PrincipalId::new("user-a"),
                ActionName::new("place_order"),
                &arguments,
                "account:cash".to_string(),
                world.world_id().clone(),
                world.manifest_hash().clone(),
                world
                    .descriptor_hash(&ActionName::new("place_order"))
                    .unwrap()
                    .clone(),
                provenance,
                effect_mode,
            );
            let authorization_id = ApprovalTokenId::new("auth-1");
            let token = ApprovalToken::pending(
                authorization_id.clone(),
                binding.principal.clone(),
                binding.action.clone(),
                binding.params_hash.clone(),
                binding.canonical_effect_hash.clone(),
                binding.resource.clone(),
                binding.world_id.clone(),
                binding.manifest_hash.clone(),
                binding.descriptor_hash.clone(),
                binding.provenance.clone(),
                binding.effect_mode,
                authorization_expiry,
            );
            let mut authorizations =
                ApprovalStore::open(dir.path().join("authorizations.jsonl")).unwrap();
            authorizations.mint(token).unwrap();
            authorizations.approve(&authorization_id).unwrap();

            let staged = stage_effect(
                StagedEffectId::new("stage-1"),
                CallId::new("proposal-1"),
                TraceId::new("admission-1"),
                authorization_id,
                &binding,
                arguments.clone(),
                consequence,
                ActuatorOperation {
                    actuator: actuator.to_string(),
                    operation: "ledger.apply".to_string(),
                    payload: arguments,
                },
                "idem-1".to_string(),
                staged_expiry,
            );
            let stages = StagedEffectStore::open(dir.path().join("staged.jsonl")).unwrap();
            stages.stage(staged.clone()).unwrap();
            Self {
                _dir: dir,
                world,
                authorizations,
                stages,
                staged,
            }
        }
    }

    fn world(world_epoch: &str, schema_epoch: &str) -> CompiledWorld {
        let action = ActionName::new("place_order");
        let mut parts = CompiledWorldParts {
            world_id: WorldId::new("trading"),
            manifest_hash: ManifestHash::new(world_epoch),
            ..CompiledWorldParts::default()
        };
        parts.ontology.insert(action.clone());
        parts.projected.insert(action.clone());
        parts
            .descriptor_hashes
            .insert(action, DescriptorHash::new(schema_epoch));
        CompiledWorld::new(parts)
    }

    #[test]
    fn stage_is_non_effective_then_commit_emits_causal_receipt() {
        let mut fixture = Fixture::new(
            EffectMode::Execute,
            Consequence::irreversible(),
            "fake",
            2_000,
            2_000,
        );
        let mut actuator = FakePrivilegedActuator::default();
        assert_eq!(actuator.effect_count(), 0, "staging must be non-effective");

        let receipt = fixture
            .stages
            .commit(
                &fixture.staged,
                &fixture.world,
                &mut fixture.authorizations,
                &mut actuator,
                NOW,
            )
            .unwrap();
        assert_eq!(receipt.outcome, ReceiptOutcome::Committed);
        assert_eq!(actuator.effect_count(), 1);
        assert_eq!(receipt.proposal_id, CallId::new("proposal-1"));
        assert_eq!(receipt.admission_trace_id, TraceId::new("admission-1"));
        assert_eq!(receipt.authorization_id, ApprovalTokenId::new("auth-1"));
        assert!(receipt.downstream_result_hash.is_some());
    }

    #[test]
    fn argument_mutation_after_staging_is_rejected() {
        let mut fixture = Fixture::new(
            EffectMode::Execute,
            Consequence::irreversible(),
            "fake",
            2_000,
            2_000,
        );
        let mut mutated = fixture.staged.clone();
        mutated.arguments["amount"] = json!(8);
        let mut actuator = FakePrivilegedActuator::default();
        let receipt = fixture
            .stages
            .commit(
                &mutated,
                &fixture.world,
                &mut fixture.authorizations,
                &mut actuator,
                NOW,
            )
            .unwrap();
        assert_eq!(receipt.outcome, ReceiptOutcome::Rejected);
        assert_eq!(receipt.reason.as_deref(), Some("staged_effect_hash_mismatch"));
        assert_eq!(actuator.effect_count(), 0);
    }

    #[test]
    fn authorization_expiry_and_revocation_fail_before_effect() {
        let mut expired = Fixture::new(
            EffectMode::Execute,
            Consequence::irreversible(),
            "fake",
            1_200,
            3_000,
        );
        let mut actuator = FakePrivilegedActuator::default();
        let receipt = expired
            .stages
            .commit(
                &expired.staged,
                &expired.world,
                &mut expired.authorizations,
                &mut actuator,
                1_200,
            )
            .unwrap();
        assert_eq!(receipt.reason.as_deref(), Some("authorization_expired"));
        assert_eq!(actuator.effect_count(), 0);

        let mut revoked = Fixture::new(
            EffectMode::Execute,
            Consequence::irreversible(),
            "fake",
            2_000,
            2_000,
        );
        revoked
            .authorizations
            .revoke(&ApprovalTokenId::new("auth-1"))
            .unwrap();
        let receipt = revoked
            .stages
            .commit(
                &revoked.staged,
                &revoked.world,
                &mut revoked.authorizations,
                &mut actuator,
                NOW,
            )
            .unwrap();
        assert_eq!(receipt.reason.as_deref(), Some("authorization_revoked"));
        assert_eq!(actuator.effect_count(), 0);
    }

    #[test]
    fn world_and_schema_drift_force_readmission() {
        for (current, expected_reason) in [
            (world("world-epoch-2", "schema-epoch-1"), "world_epoch_drift"),
            (world("world-epoch-1", "schema-epoch-2"), "schema_epoch_drift"),
        ] {
            let mut fixture = Fixture::new(
                EffectMode::Execute,
                Consequence::irreversible(),
                "fake",
                2_000,
                2_000,
            );
            let mut actuator = FakePrivilegedActuator::default();
            let receipt = fixture
                .stages
                .commit(
                    &fixture.staged,
                    &current,
                    &mut fixture.authorizations,
                    &mut actuator,
                    NOW,
                )
                .unwrap();
            assert_eq!(receipt.outcome, ReceiptOutcome::Rejected);
            assert_eq!(receipt.reason.as_deref(), Some(expected_reason));
            assert_eq!(actuator.effect_count(), 0);
        }
    }

    #[test]
    fn duplicate_commit_survives_restart_and_applies_once() {
        let mut fixture = Fixture::new(
            EffectMode::Execute,
            Consequence::irreversible(),
            "fake",
            2_000,
            2_000,
        );
        let mut actuator = FakePrivilegedActuator::default();
        let first = fixture
            .stages
            .commit(
                &fixture.staged,
                &fixture.world,
                &mut fixture.authorizations,
                &mut actuator,
                NOW,
            )
            .unwrap();
        let reopened = StagedEffectStore::open(fixture.stages.path()).unwrap();
        let duplicate = reopened
            .commit(
                &fixture.staged,
                &fixture.world,
                &mut fixture.authorizations,
                &mut actuator,
                NOW + 1,
            )
            .unwrap();
        assert_eq!(first.outcome, ReceiptOutcome::Committed);
        assert_eq!(duplicate.outcome, ReceiptOutcome::Duplicate);
        assert_eq!(duplicate.original_receipt_id, Some(first.id));
        assert_eq!(actuator.effect_count(), 1);
    }

    #[test]
    fn malformed_or_unmappable_evidence_fails_closed() {
        let mut malformed = Fixture::new(
            EffectMode::Execute,
            Consequence::irreversible(),
            "fake",
            2_000,
            2_000,
        );
        malformed.staged.consequence = Consequence {
            class: harness_types::ConsequenceClass::Compensable,
            compensation_operation: None,
        };
        malformed.staged.artifact_hash = staged_effect_hash(&malformed.staged);
        let mut actuator = FakePrivilegedActuator::default();
        let receipt = malformed
            .stages
            .commit(
                &malformed.staged,
                &malformed.world,
                &mut malformed.authorizations,
                &mut actuator,
                NOW,
            )
            .unwrap();
        assert!(receipt
            .reason
            .as_deref()
            .unwrap()
            .starts_with("malformed_staged_effect:"));

        let mut unmappable = Fixture::new(
            EffectMode::Execute,
            Consequence::irreversible(),
            "unknown-actuator",
            2_000,
            2_000,
        );
        let receipt = unmappable
            .stages
            .commit(
                &unmappable.staged,
                &unmappable.world,
                &mut unmappable.authorizations,
                &mut actuator,
                NOW,
            )
            .unwrap();
        assert_eq!(
            receipt.reason.as_deref(),
            Some("unmappable_actuator_operation")
        );
        assert_eq!(actuator.effect_count(), 0);
    }

    #[test]
    fn timeout_after_apply_stays_ambiguous_and_is_not_retried() {
        let mut fixture = Fixture::new(
            EffectMode::Execute,
            Consequence::irreversible(),
            "fake",
            2_000,
            2_000,
        );
        let mut actuator =
            FakePrivilegedActuator::with_mode(FakeActuatorMode::TimeoutAfterApply);
        let first = fixture
            .stages
            .commit(
                &fixture.staged,
                &fixture.world,
                &mut fixture.authorizations,
                &mut actuator,
                NOW,
            )
            .unwrap();
        let retry = fixture
            .stages
            .commit(
                &fixture.staged,
                &fixture.world,
                &mut fixture.authorizations,
                &mut actuator,
                NOW + 1,
            )
            .unwrap();
        assert_eq!(first.outcome, ReceiptOutcome::Ambiguous);
        assert_eq!(retry.outcome, ReceiptOutcome::Ambiguous);
        assert_eq!(retry.original_receipt_id, Some(first.id));
        assert_eq!(actuator.effect_count(), 1);
    }

    #[test]
    fn consequence_metadata_and_simulation_are_explicit() {
        let irreversible = Fixture::new(
            EffectMode::Execute,
            Consequence::irreversible(),
            "fake",
            2_000,
            2_000,
        );
        let compensable = Fixture::new(
            EffectMode::Execute,
            Consequence::compensable("ledger.reverse"),
            "fake",
            2_000,
            2_000,
        );
        assert_ne!(
            irreversible.staged.artifact_hash,
            compensable.staged.artifact_hash
        );
        assert!(irreversible.staged.consequence.is_well_formed());
        assert!(compensable.staged.consequence.is_well_formed());

        let mut simulated = Fixture::new(
            EffectMode::Simulate,
            Consequence::compensable("ledger.reverse"),
            "fake",
            2_000,
            2_000,
        );
        let mut actuator = FakePrivilegedActuator::default();
        let receipt = simulated
            .stages
            .commit(
                &simulated.staged,
                &simulated.world,
                &mut simulated.authorizations,
                &mut actuator,
                NOW,
            )
            .unwrap();
        assert_eq!(receipt.outcome, ReceiptOutcome::Simulated);
        assert_eq!(actuator.effect_count(), 0);
    }
}
