//! Provenance and taint — the monotonic information-flow core (architecture §5).

use serde::{Deserialize, Serialize};

use crate::ids::{ContentHash, SessionId};

/// Trust is a property of the *channel*, never of the content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TrustLevel {
    Trusted,
    SemiTrusted,
    Untrusted,
    Derived,
}

/// The channel a value entered through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SourceChannel {
    UserPrompt,
    WorkspaceFile,
    ShellOutput,
    McpOutput,
    Web,
    Memory,
    Generated,
}

impl SourceChannel {
    /// Default channel trust (architecture §7 "Default channel trust").
    pub fn default_trust(self) -> TrustLevel {
        match self {
            SourceChannel::UserPrompt => TrustLevel::Trusted,
            SourceChannel::WorkspaceFile => TrustLevel::SemiTrusted,
            SourceChannel::ShellOutput => TrustLevel::SemiTrusted,
            SourceChannel::McpOutput => TrustLevel::Untrusted,
            SourceChannel::Web => TrustLevel::Untrusted,
            SourceChannel::Memory => TrustLevel::Derived,
            SourceChannel::Generated => TrustLevel::Derived,
        }
    }

    /// Default taint a channel introduces before any policy refinement.
    pub fn default_taint(self) -> Taint {
        match self {
            SourceChannel::UserPrompt => Taint::Clean,
            _ => Taint::Tainted,
        }
    }
}

/// Monotonic taint: `Clean ∨ Tainted = Tainted`. Never decreases.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub enum Taint {
    #[default]
    Clean,
    Tainted,
}

impl Taint {
    /// Monotonic join. The model can never move from `Tainted` back to `Clean`.
    pub fn join(self, other: Taint) -> Taint {
        match (self, other) {
            (Taint::Clean, Taint::Clean) => Taint::Clean,
            _ => Taint::Tainted,
        }
    }

    pub fn is_tainted(self) -> bool {
        matches!(self, Taint::Tainted)
    }
}

/// Lineage of a value: where it came from and through what.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub source_channel: SourceChannel,
    pub trust_level: TrustLevel,
    pub parent_sources: Vec<SourceChannel>,
    pub session_id: SessionId,
    pub content_hash: ContentHash,
}

impl Provenance {
    /// Build provenance for a value entering directly from a channel, taking
    /// the channel's default trust.
    pub fn from_channel(
        channel: SourceChannel,
        session_id: SessionId,
        content_hash: ContentHash,
    ) -> Self {
        Self {
            source_channel: channel,
            trust_level: channel.default_trust(),
            parent_sources: Vec::new(),
            session_id,
            content_hash,
        }
    }
}

/// A value paired with its taint. The return type of every executor call:
/// there is no untainted execution result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaintedValue<T> {
    pub value: T,
    pub taint: Taint,
}

impl<T> TaintedValue<T> {
    pub fn new(value: T, taint: Taint) -> Self {
        Self { value, taint }
    }

    pub fn clean(value: T) -> Self {
        Self {
            value,
            taint: Taint::Clean,
        }
    }

    pub fn tainted(value: T) -> Self {
        Self {
            value,
            taint: Taint::Tainted,
        }
    }

    /// Transform the value while preserving taint.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> TaintedValue<U> {
        TaintedValue {
            value: f(self.value),
            taint: self.taint,
        }
    }
}

/// Mandatory threading object for taint propagation between pipeline stages.
/// Callers cannot silently drop taint: building an intent requires one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaintContext {
    taint: Taint,
}

impl TaintContext {
    /// A fresh CLEAN context for pipeline entry points (no prior tainted data).
    pub fn clean() -> Self {
        Self {
            taint: Taint::Clean,
        }
    }

    /// Derive a context from prior executor outputs (monotonic join of all).
    pub fn from_outputs<T>(outputs: &[TaintedValue<T>]) -> Self {
        let mut taint = Taint::Clean;
        for output in outputs {
            taint = taint.join(output.taint);
        }
        Self { taint }
    }

    /// Derive from an explicit taint value.
    pub fn from_taint(taint: Taint) -> Self {
        Self { taint }
    }

    pub fn taint(&self) -> Taint {
        self.taint
    }
}
