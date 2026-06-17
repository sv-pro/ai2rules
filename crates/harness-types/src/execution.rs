//! `ExecutionSpec` — the only object that crosses into execution (architecture §5).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::decision::EffectMode;
use crate::ids::{ActionName, DescriptorHash, TraceId};

/// The concrete operation to run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Operation {
    /// A command invocation, already split into argv.
    Argv(Vec<String>),
    /// A structured operation (read/patch/MCP/web), interpreted by the handler.
    Structured(Value),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkPolicy {
    Disabled,
    AllowHosts(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EnvPolicy {
    pub allowlist: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FilesystemPolicy {
    pub writable_roots: Vec<PathBuf>,
    pub readable_roots: Vec<PathBuf>,
}

/// The single object the executor will accept. Immutable: constructed once and
/// only read thereafter. Carries no policy objects, model context, or handlers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionSpec {
    action: ActionName,
    operation: Operation,
    cwd: PathBuf,
    env: EnvPolicy,
    timeout_ms: u64,
    network: NetworkPolicy,
    filesystem: FilesystemPolicy,
    expected_descriptor_hash: DescriptorHash,
    effect_mode: EffectMode,
    trace_id: TraceId,
}

impl ExecutionSpec {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        action: ActionName,
        operation: Operation,
        cwd: PathBuf,
        env: EnvPolicy,
        timeout_ms: u64,
        network: NetworkPolicy,
        filesystem: FilesystemPolicy,
        expected_descriptor_hash: DescriptorHash,
        effect_mode: EffectMode,
        trace_id: TraceId,
    ) -> Self {
        Self {
            action,
            operation,
            cwd,
            env,
            timeout_ms,
            network,
            filesystem,
            expected_descriptor_hash,
            effect_mode,
            trace_id,
        }
    }

    pub fn action(&self) -> &ActionName {
        &self.action
    }
    pub fn operation(&self) -> &Operation {
        &self.operation
    }
    pub fn cwd(&self) -> &std::path::Path {
        &self.cwd
    }
    pub fn env(&self) -> &EnvPolicy {
        &self.env
    }
    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }
    pub fn network(&self) -> &NetworkPolicy {
        &self.network
    }
    pub fn filesystem(&self) -> &FilesystemPolicy {
        &self.filesystem
    }
    pub fn expected_descriptor_hash(&self) -> &DescriptorHash {
        &self.expected_descriptor_hash
    }
    pub fn effect_mode(&self) -> EffectMode {
        self.effect_mode
    }
    pub fn trace_id(&self) -> &TraceId {
        &self.trace_id
    }
}
