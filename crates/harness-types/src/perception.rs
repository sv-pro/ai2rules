//! Typed inputs to the model. Raw bytes never enter context directly.

use serde::{Deserialize, Serialize};

use crate::ids::{ContentHash, PerceptionId};
use crate::provenance::{Provenance, SourceChannel, Taint, TrustLevel};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PerceptionKind {
    UserPrompt,
    FileContent,
    CommandStdout,
    CommandStderr,
    McpResponse,
    WebResponse,
    MemoryRecall,
}

/// How a perception's payload should be treated by the trace redactor.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RedactionPolicy {
    #[default]
    None,
    Redact,
    Custom(String),
}

/// A handle to the raw payload kept outside the model context window.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PayloadRef(pub String);

/// The typed form of anything entering model context (architecture §5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Perception {
    pub id: PerceptionId,
    pub kind: PerceptionKind,
    pub source_channel: SourceChannel,
    pub trust_level: TrustLevel,
    pub taint: Taint,
    pub content_hash: ContentHash,
    pub provenance: Provenance,
    pub payload_ref: PayloadRef,
    pub redaction_policy: RedactionPolicy,
}
