//! Durable approval store (E6.2–E6.4).
//!
//! Human-in-the-loop approval as durable state, not memory. The store is an
//! append-only JSONL log of lifecycle transitions (`Minted → Approved/Rejected →
//! Executed`), folded into the current token set on load — same shape as the
//! trace store, and replayable. An approval is bound to the exact call (action,
//! params, world, descriptor hash, provenance, effect mode); any drift in those
//! voids reuse (E6.4).

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use compiler::sha256_hex;
use harness_types::{
    ActionName, ApprovalState, ApprovalToken, ApprovalTokenId, ApprovalTransitionError,
    ContentHash, DescriptorHash, EffectMode, Provenance, WorldId,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One durable transition in the approval log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ApprovalEvent {
    Minted(Box<ApprovalToken>),
    Approved(ApprovalTokenId),
    Rejected(ApprovalTokenId),
    Executed(ApprovalTokenId),
}

/// The canonical params hash used in an approval binding.
pub fn params_hash(params: &Value) -> ContentHash {
    let canonical = serde_json::to_string(params).unwrap_or_default();
    ContentHash::new(sha256_hex(canonical.as_bytes()))
}

/// Durable, append-only store of approval tokens and their lifecycle.
pub struct ApprovalStore {
    path: PathBuf,
    tokens: BTreeMap<ApprovalTokenId, ApprovalToken>,
}

impl ApprovalStore {
    /// Open (or start) a store, folding any existing log into current state.
    pub fn open(path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = path.into();
        let tokens = load(&path)?;
        Ok(Self { path, tokens })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn token(&self, id: &ApprovalTokenId) -> Option<&ApprovalToken> {
        self.tokens.get(id)
    }

    /// Persist a freshly-minted pending token.
    pub fn mint(&mut self, token: ApprovalToken) -> io::Result<ApprovalTokenId> {
        let id = token.id.clone();
        self.append(&ApprovalEvent::Minted(Box::new(token.clone())))?;
        self.tokens.insert(id.clone(), token);
        Ok(id)
    }

    pub fn approve(&mut self, id: &ApprovalTokenId) -> io::Result<()> {
        self.transition(
            id,
            ApprovalToken::approve,
            ApprovalEvent::Approved(id.clone()),
        )
    }

    pub fn reject(&mut self, id: &ApprovalTokenId) -> io::Result<()> {
        self.transition(
            id,
            ApprovalToken::reject,
            ApprovalEvent::Rejected(id.clone()),
        )
    }

    pub fn mark_executed(&mut self, id: &ApprovalTokenId) -> io::Result<()> {
        self.transition(
            id,
            ApprovalToken::mark_executed,
            ApprovalEvent::Executed(id.clone()),
        )
    }

    /// True iff an **Approved** token binds to this exact call (E6.4). Any drift
    /// — different world, descriptor hash, params, provenance, or effect mode —
    /// means no match, so an approval is never reused after the world changes.
    #[allow(clippy::too_many_arguments)]
    pub fn is_granted(
        &self,
        action: &ActionName,
        params: &Value,
        world_id: &WorldId,
        descriptor_hash: &DescriptorHash,
        provenance: &Provenance,
        effect_mode: EffectMode,
    ) -> bool {
        let ph = params_hash(params);
        self.tokens.values().any(|t| {
            t.state == ApprovalState::Approved
                && t.action == *action
                && t.params_hash == ph
                && t.world_id == *world_id
                && t.descriptor_hash == *descriptor_hash
                && t.provenance == *provenance
                && t.effect_mode == effect_mode
        })
    }

    fn transition(
        &mut self,
        id: &ApprovalTokenId,
        apply: impl FnOnce(&mut ApprovalToken) -> Result<(), ApprovalTransitionError>,
        event: ApprovalEvent,
    ) -> io::Result<()> {
        {
            let token = self
                .tokens
                .get_mut(id)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "unknown approval token"))?;
            apply(token).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
        }
        self.append(&event)
    }

    fn append(&self, event: &ApprovalEvent) -> io::Result<()> {
        let line = serde_json::to_string(event).map_err(io::Error::other)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(file, "{line}")?;
        file.flush()
    }
}

fn load(path: &Path) -> io::Result<BTreeMap<ApprovalTokenId, ApprovalToken>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(e) => return Err(e),
    };
    let mut tokens = BTreeMap::new();
    for line in BufReader::new(file).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let event: ApprovalEvent = serde_json::from_str(&line).map_err(io::Error::other)?;
        apply_event(&mut tokens, event);
    }
    Ok(tokens)
}

fn apply_event(tokens: &mut BTreeMap<ApprovalTokenId, ApprovalToken>, event: ApprovalEvent) {
    match event {
        ApprovalEvent::Minted(token) => {
            tokens.insert(token.id.clone(), *token);
        }
        ApprovalEvent::Approved(id) => {
            if let Some(t) = tokens.get_mut(&id) {
                let _ = t.approve();
            }
        }
        ApprovalEvent::Rejected(id) => {
            if let Some(t) = tokens.get_mut(&id) {
                let _ = t.reject();
            }
        }
        ApprovalEvent::Executed(id) => {
            if let Some(t) = tokens.get_mut(&id) {
                let _ = t.mark_executed();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harness_types::{ContentHash, EffectMode, Provenance, SessionId, SourceChannel};
    use serde_json::json;

    fn token(id: &str, params: &Value, descriptor: &str) -> ApprovalToken {
        ApprovalToken::pending(
            ApprovalTokenId::new(id),
            ActionName::new("start_pty"),
            params_hash(params),
            WorldId::new("w"),
            DescriptorHash::new(descriptor),
            prov(),
            EffectMode::Simulate,
        )
    }

    fn prov() -> Provenance {
        Provenance::from_channel(
            SourceChannel::UserPrompt,
            SessionId::new("s"),
            ContentHash::new("c"),
        )
    }

    fn granted(store: &ApprovalStore, params: &Value, descriptor: &str) -> bool {
        store.is_granted(
            &ActionName::new("start_pty"),
            params,
            &WorldId::new("w"),
            &DescriptorHash::new(descriptor),
            &prov(),
            EffectMode::Simulate,
        )
    }

    #[test]
    fn mint_approve_then_granted() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ApprovalStore::open(dir.path().join("approvals.jsonl")).unwrap();
        let params = json!({"shell": "bash"});
        let id = store.mint(token("t1", &params, "desc-1")).unwrap();
        assert!(!granted(&store, &params, "desc-1")); // pending, not yet approved
        store.approve(&id).unwrap();
        assert!(granted(&store, &params, "desc-1"));
    }

    #[test]
    fn drift_voids_the_approval() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ApprovalStore::open(dir.path().join("approvals.jsonl")).unwrap();
        let params = json!({"shell": "bash"});
        let id = store.mint(token("t1", &params, "desc-1")).unwrap();
        store.approve(&id).unwrap();
        // Descriptor drift → the approval no longer matches.
        assert!(!granted(&store, &params, "desc-2"));
        // Param change → no match either.
        assert!(!granted(&store, &json!({"shell": "zsh"}), "desc-1"));
    }

    #[test]
    fn reject_is_not_granted() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ApprovalStore::open(dir.path().join("approvals.jsonl")).unwrap();
        let params = json!({});
        let id = store.mint(token("t1", &params, "desc-1")).unwrap();
        store.reject(&id).unwrap();
        assert!(!granted(&store, &params, "desc-1"));
    }

    #[test]
    fn state_survives_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("approvals.jsonl");
        let params = json!({"shell": "bash"});
        {
            let mut store = ApprovalStore::open(&path).unwrap();
            let id = store.mint(token("t1", &params, "desc-1")).unwrap();
            store.approve(&id).unwrap();
        }
        // Reopen from disk — the approval is still in force (durable).
        let store = ApprovalStore::open(&path).unwrap();
        assert!(granted(&store, &params, "desc-1"));
    }
}
