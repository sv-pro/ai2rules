//! `WorldManifest` — the design-time authoring artifact (architecture §5).
//!
//! Parsing and validation live in the `compiler` crate (E1). These are the
//! typed shapes a manifest deserializes into.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::action::{ActionType, ActorKind, DataClass};
use crate::decision::Decision;
use crate::descriptor::{BackingIdentity, SideEffectClass};
use crate::ids::{ActionName, WorldId};
use crate::provenance::{Taint, TrustLevel};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Actor {
    pub name: String,
    pub kind: ActorKind,
}

/// One row of the capability matrix: which action types a trust level may
/// perform. Modeled as a list (not a map keyed by enum) so manifests stay
/// straightforward to author and parse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityGrant {
    pub trust: TrustLevel,
    #[serde(default)]
    pub actions: Vec<ActionType>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelDef {
    pub name: String,
    pub trust: TrustLevel,
    #[serde(default)]
    pub taint: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BaseActionDef {
    pub name: ActionName,
    pub action_type: ActionType,
    pub side_effect: SideEffectClass,
    /// Model-facing JSON schema for the action's arguments.
    #[serde(default)]
    pub schema: Value,
    /// Additional argument constraints (regex, enums, …).
    #[serde(default)]
    pub arg_constraints: Value,
    /// What backs the action. Defaults to a local handler named after the
    /// action when omitted.
    #[serde(default)]
    pub backing: Option<BackingIdentity>,
    #[serde(default)]
    pub approval_required: bool,
}

/// Where a scoped-capability argument's value comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArgSource {
    /// Fixed value baked in; invisible to the actor, injected at execution.
    Literal(String),
    /// Supplied by the actor at call time.
    ActorInput,
    /// Resolved from a named runtime context key.
    ContextRef(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedCapabilityDef {
    pub name: ActionName,
    pub base_action: ActionName,
    #[serde(default)]
    pub args: BTreeMap<String, ArgSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionPolicy {
    pub from_taint: Taint,
    pub side_effect: SideEffectClass,
    pub decision: Decision,
    #[serde(default)]
    pub rule: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Budget {
    #[serde(default)]
    pub max_tokens_per_session: Option<u64>,
    #[serde(default)]
    pub max_commands_per_task: Option<u64>,
    #[serde(default)]
    pub command_timeout_ms: Option<u64>,
    #[serde(default)]
    pub max_network_calls: Option<u64>,
    #[serde(default)]
    pub max_file_writes: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observability {
    #[serde(default)]
    pub redact: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldManifest {
    pub world_id: WorldId,
    #[serde(default)]
    pub actors: Vec<Actor>,
    #[serde(default)]
    pub channels: Vec<ChannelDef>,
    #[serde(default)]
    pub data_classes: Vec<DataClass>,
    /// Capability matrix: which action types each trust level may perform.
    #[serde(default)]
    pub capabilities: Vec<CapabilityGrant>,
    #[serde(default)]
    pub base_actions: Vec<BaseActionDef>,
    #[serde(default)]
    pub scoped_capabilities: Vec<ScopedCapabilityDef>,
    #[serde(default)]
    pub transition_policies: Vec<TransitionPolicy>,
    #[serde(default)]
    pub budget: Budget,
    #[serde(default)]
    pub observability: Observability,
}
