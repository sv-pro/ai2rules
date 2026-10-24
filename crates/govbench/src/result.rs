//! The result and evidence schema.
//!
//! One JSON document per run of the pack. It is written to be diffed and cited:
//! every step records who acted, what verdict came back, whether a downstream
//! effect actually happened, and the target's own evidence for the verdict.
//!
//! There is no aggregate score, deliberately. A number would let a target trade a
//! held line against a lost one, which is exactly the trade governance cannot
//! make.

use serde::Serialize;
use serde_json::Value;

use crate::target::Verdict;
use crate::upstream::AppliedEffect;

/// Result-schema version. Bump on any incompatible field change.
pub const RESULT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize)]
pub struct BenchResult {
    pub v: u32,
    pub pack: PackIdentity,
    pub targets: Vec<TargetIdentity>,
    pub runs: Vec<RunResult>,
    /// Cross-transport agreement for the ai2rules target, when both were run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport_parity: Option<TransportParity>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PackIdentity {
    pub path: String,
    pub world_id: String,
    pub manifest_hash: String,
    pub scenarios: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TargetIdentity {
    pub id: String,
    pub description: String,
    pub metadata: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Status {
    Pass,
    Fail,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Pass => "PASS",
            Status::Fail => "FAIL",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RunResult {
    pub scenario: String,
    pub scenario_version: u32,
    pub title: String,
    pub question: String,
    pub target: String,
    pub outcome: Status,
    pub effect_count: u32,
    pub steps: Vec<StepObservation>,
    pub checks: Vec<CheckResult>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StepObservation {
    pub id: String,
    pub op: &'static str,
    /// The scenario's name for the acting principal.
    pub principal: String,
    /// The security identity the target was given.
    pub principal_id: String,
    pub channel: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Value>,
    pub verdict: Verdict,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surface_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handle: Option<String>,
    /// What the target says its grant covers (`authorize`), and what it checked
    /// for this presented call (`invoke`). Required evidence — see
    /// [`crate::target::Authorization::grant_binding`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grant_binding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presented_binding: Option<String>,
    /// The structured reason a presented authorization was refused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection: Option<String>,
    /// Observed by the runner from the upstream's effect ledger — never reported
    /// by the target.
    pub effect_applied: bool,
    /// The ledger's own record of what reached the upstream, when one did. This
    /// is the independent half of the evidence: the target says what it decided,
    /// this says what actually happened, and the oracle checks they agree.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effect: Option<AppliedEffect>,
    pub evidence: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckResult {
    pub id: String,
    pub assertion: &'static str,
    pub status: Status,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransportParity {
    pub status: Status,
    pub compared: Vec<String>,
    pub detail: String,
}
