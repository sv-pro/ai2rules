//! Action taxonomy shared across manifest, compiled world, and kernel.

use serde::{Deserialize, Serialize};

/// The coarse type of an action, used by the capability matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ActionType {
    Read,
    Write,
    Patch,
    Command,
    Pty,
    Mcp,
    Web,
    Memory,
}

/// Who or what an actor is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ActorKind {
    User,
    Model,
    Subagent,
    McpServer,
    ShellWorker,
}

/// Data sensitivity classes (architecture §5 WorldManifest "data classes").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DataClass {
    Public,
    Workspace,
    Secret,
    Credential,
    Generated,
    External,
}
