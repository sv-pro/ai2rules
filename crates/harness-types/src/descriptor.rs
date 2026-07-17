//! Action descriptor and its hashable identity (architecture §5 `Descriptor`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ids::ActionName;

/// The policy role of a tool argument (PACT §3.2, arXiv:2605.11039).
///
/// This is **not** a linguistic taxonomy — it is the policy interface that
/// separates arguments which *bind authority* (a destination, an executed
/// command, a secret) from arguments that primarily *carry content*. It is the
/// design-time input the L2 taint check needs to be precise instead of blocking
/// on ambient taint (the granularity mismatch — see
/// `_tasks/1_discovery/pact-granularity-mismatch.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ArgRole {
    /// Authority-bearing destination: recipient, URL, endpoint.
    Target,
    /// Executable command or query.
    Command,
    /// A secret.
    Credential,
    /// Payload text — carries content, does not bind authority.
    Content,
    /// Object selection.
    Selector,
    /// Behaviour-modifying flag.
    Control,
}

impl ArgRole {
    /// PACT's split: `Target`/`Command`/`Credential` bind authority; the rest
    /// primarily carry content. A tainted authority-bearing argument is the real
    /// hazard the taint floor exists to stop; tainted *content* flowing to a
    /// clean destination is benign (provided taint is preserved on outputs).
    pub fn is_authority_bearing(self) -> bool {
        matches!(
            self,
            ArgRole::Target | ArgRole::Command | ArgRole::Credential
        )
    }
}

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
    /// Per-argument policy roles (PACT §3.2). Empty means "no argument-level
    /// contract" — the action falls back to the ambient-taint floor (L0/L1),
    /// exactly today's behavior. Populated per argument, it enables the L2
    /// authority-bearing check in `world-kernel`.
    #[serde(default)]
    pub arg_roles: BTreeMap<String, ArgRole>,
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
