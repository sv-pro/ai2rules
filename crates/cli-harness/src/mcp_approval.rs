//! Out-of-band MCP consent. The durable claim precedes all upstream effects.
use harness_preview::GateResponse;
use harness_types::{
    ActionName, ApprovalState, ApprovalToken, ApprovalTokenId, CompiledWorld, ContentHash,
    EffectMode, PrincipalId, Provenance, SessionId,
};
use serde_json::{json, Value};
use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use trace_store::{effect_binding, effect_resource, ApprovalStore, ConsumeOutcome, EffectBinding};

const TTL_MS: u64 = 5 * 60 * 1000;

pub(crate) fn binding(
    world: &CompiledWorld,
    verdict: &GateResponse,
    params: &Value,
    source: &str,
    principal: &str,
    upstream: &[String],
) -> Result<EffectBinding, String> {
    let action = ActionName::new(&verdict.action);
    let effective = verdict.effective.as_ref().ok_or("missing effective call")?;
    let descriptor = world.descriptor_hash(&action).ok_or("missing descriptor")?;
    let channel = world
        .channel_policy(source)
        .ok_or("unknown source channel")?;
    // Include the actual proposal and metadata as well as the lowered call:
    // neither a changed upstream command nor changed host metadata may reuse consent.
    let envelope = json!({"v": 1, "request": params, "effective": effective,
                          "upstream": upstream});
    Ok(effect_binding(
        PrincipalId::new(principal),
        action,
        &envelope,
        effect_resource(&effective.arguments),
        world.world_id().clone(),
        world.manifest_hash().clone(),
        descriptor.clone(),
        Provenance::from_channel_with_trust(
            channel.channel,
            channel.trust,
            SessionId::new("mcp-gateway"),
            ContentHash::new("gate"),
        ),
        EffectMode::Execute,
    ))
}

pub(crate) struct Claim {
    store: ApprovalStore,
    pub(crate) id: ApprovalTokenId,
}

impl Claim {
    pub(crate) fn finish(&mut self, response: io::Result<Value>) -> io::Result<Value> {
        match response {
            Err(e) => Err(io::Error::other(format!(
                "approval {} consumed; outcome unknown: {e}; reconcile the upstream effect before requesting new consent",
                self.id.as_str()))),
            Ok(result) => {
                self.store.mark_executed(&self.id).map_err(|e| io::Error::other(format!(
                    "approval {} consumed; upstream replied but execution evidence could not be persisted: {e}; reconcile before retrying",
                    self.id.as_str())))?;
                Ok(result)
            }
        }
    }
}

pub(crate) fn claim(path: &Path, binding: &EffectBinding) -> Result<Claim, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("approval clock error: {e}"))?;
    let now_ms = u64::try_from(now.as_millis()).map_err(|_| "approval clock overflow")?;
    let mut store = ApprovalStore::open(path)
        .map_err(|e| format!("approval store unavailable; no call forwarded: {e}"))?;
    let matches = |t: &&ApprovalToken| {
        // The canonical hash covers every binding field. consume independently
        // validates all fields under its cross-process lock before authorizing.
        t.canonical_effect_hash == binding.canonical_effect_hash
            && t.valid_until_unix_ms > now_ms
            && t.remaining_uses > 0
    };
    let approved = store
        .all()
        .filter(matches)
        .find(|t| t.state == ApprovalState::Approved)
        .map(|t| t.id.clone());
    if let Some(id) = approved {
        match store
            .consume(&id, binding, now_ms)
            .map_err(|e| format!("approval claim failed; no call forwarded: {e}"))?
        {
            ConsumeOutcome::Consumed(_) => return Ok(Claim { store, id }),
            ConsumeOutcome::Rejected(reason) => {
                return Err(format!(
                    "approval claim rejected ({reason:?}); no call forwarded"
                ))
            }
        }
    }
    let pending = store
        .all()
        .filter(matches)
        .find(|t| t.state == ApprovalState::Pending)
        .map(|t| t.id.clone());
    let uncertain = store.all().any(|t| {
        t.canonical_effect_hash == binding.canonical_effect_hash
            && t.state == ApprovalState::Consumed
    });
    let id = if let Some(id) = pending {
        id
    } else {
        let id = ApprovalTokenId::new(format!("mcp-{}-{}", std::process::id(), now.as_nanos()));
        store
            .mint(ApprovalToken::pending(
                id.clone(),
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
                now_ms
                    .checked_add(TTL_MS)
                    .ok_or("approval expiry overflow")?,
            ))
            .map_err(|e| format!("cannot persist approval request; no call forwarded: {e}"))?;
        id
    };
    let caution = if uncertain {
        " Prior consent was consumed with no confirmed outcome; reconcile upstream before approving again."
    } else {
        ""
    };
    Err(format!("pending approval {} (expires five minutes after creation); an operator must review and approve via `harness approvals` using the same store.{caution}", id.as_str()))
}
