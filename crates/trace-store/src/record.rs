//! Trace record types (E4.1) — pure, serializable data (no kernel dependency).
//!
//! A record has a common header plus a per-stage payload. The `Decision` payload
//! is the *replayable* one: it carries enough input to re-run `decide` and the
//! summarized outcome to compare against. Stages the harness does not yet emit
//! (perception, projection, proposal, approval) get enum room as they are wired
//! in E5/E6.

use harness_types::{
    ActionName, Decision, DescriptorHash, EffectMode, ExecutionMode, ManifestHash, Provenance,
    SessionId, Taint, TraceId, WorldId,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One append-only trace entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceRecord {
    pub trace_id: TraceId,
    pub session_id: SessionId,
    pub world_id: WorldId,
    pub manifest_hash: ManifestHash,
    /// Monotonic order within the trace.
    pub seq: u64,
    pub payload: RecordPayload,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RecordPayload {
    Decision(DecisionRecord),
    Execution(ExecutionRecord),
}

/// A recorded kernel decision plus the inputs needed to reproduce it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionRecord {
    pub action: ActionName,
    /// Arguments **after** redaction — secrets never reach disk.
    pub params: Value,
    pub provenance: Provenance,
    pub context: ContextSnapshot,
    pub outcome: OutcomeSummary,
}

/// A serializable stand-in for the kernel's `EvalContext` / `BudgetUsage`
/// (which are not `Serialize`). Replay reconstructs the context from this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextSnapshot {
    pub taint: Taint,
    pub mode: ExecutionMode,
    pub commands_run: u64,
    pub tokens_used: u64,
    pub file_writes: u64,
    pub network_calls: u64,
}

/// Which kind of `KernelOutcome` produced this summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutcomeKind {
    Evaluated,
    NotRepresentable,
    UnknownToOntology,
}

/// The comparable shape of a kernel decision: what replay checks for equality.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutcomeSummary {
    pub kind: OutcomeKind,
    pub decision: Decision,
    pub rule: String,
    pub effect_mode: Option<EffectMode>,
    pub descriptor_hash: Option<DescriptorHash>,
}

/// A recorded execution result. Not replayed (execution is not pure); kept for
/// audit. The result is a redacted *summary*, never raw file contents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub action: ActionName,
    pub effect_mode: EffectMode,
    pub descriptor_hash: DescriptorHash,
    pub result: ExecSummary,
    /// Audit-only timing; never an input to replay.
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecSummary {
    Ok { kind: String, bytes: usize },
    Error { kind: String },
}
