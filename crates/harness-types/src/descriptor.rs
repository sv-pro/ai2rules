//! Action descriptor and its hashable identity (architecture §5 `Descriptor`).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ids::ActionName;

/// The side-effect surface an action can touch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SideEffectClass {
    /// No observable effect beyond reading.
    None,
    Read,
    FilesystemWrite,
    PersistentWrite,
    Process,
    Network,
    Credential,
    Memory,
    /// Any externally-visible effect (send, publish, push, …).
    External,
}

/// What backs an action at execution time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackingIdentity {
    LocalHandler(String),
    McpServer { server: String, tool: String },
}

/// The frozen, hashable identity of an exposed action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Descriptor {
    pub action: ActionName,
    /// Model-facing JSON schema.
    pub schema: Value,
    /// Additional argument constraints (regex, enums, …).
    pub arg_constraints: Value,
    pub side_effect: SideEffectClass,
    pub backing: BackingIdentity,
    /// Policy-relevant metadata.
    pub metadata: Value,
}

impl Descriptor {
    /// Canonical bytes used as the input to descriptor hashing.
    ///
    /// The SHA-256 computation and stable key ordering are finalized in the
    /// `compiler` crate (E1.4); this only fixes a total, deterministic-per-run
    /// representation so callers have a single hashing input.
    pub fn canonical_input(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}
