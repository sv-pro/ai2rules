//! The narrow interface every benchmark target implements.
//!
//! Three operations, no scenario identity, no pass/fail vocabulary. A target
//! cannot know which case it is running, so it cannot special-case one — which is
//! the only structural reason to believe a passing result.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The outcome vocabulary, kept distinct on purpose.
///
/// Collapsing these is how governance benchmarks flatter their subjects:
/// "not allowed" hides whether a tool was never offered (`ABSENT`), was refused
/// (`DENY`), needed a human (`ASK`), or whether the governor simply broke — and
/// if it broke, whether it broke *closed* or *open*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Verdict {
    /// The action does not exist for this principal; it was never offered.
    Absent,
    /// Permitted.
    Allow,
    /// Refused by a rule.
    Deny,
    /// A human decision is required first.
    Ask,
    /// The governor could not answer and the call was refused.
    ErrorClosed,
    /// The governor could not answer and the call proceeded anyway.
    ErrorOpen,
    /// No comparable observation (the target does not implement this operation).
    Unknown,
}

impl Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Absent => "ABSENT",
            Verdict::Allow => "ALLOW",
            Verdict::Deny => "DENY",
            Verdict::Ask => "ASK",
            Verdict::ErrorClosed => "ERROR_CLOSED",
            Verdict::ErrorOpen => "ERROR_OPEN",
            Verdict::Unknown => "UNKNOWN",
        }
    }

    pub fn parse(name: &str) -> Option<Verdict> {
        Some(match name {
            "ABSENT" => Verdict::Absent,
            "ALLOW" => Verdict::Allow,
            "DENY" => Verdict::Deny,
            "ASK" => Verdict::Ask,
            "ERROR_CLOSED" => Verdict::ErrorClosed,
            "ERROR_OPEN" => Verdict::ErrorOpen,
            "UNKNOWN" => Verdict::Unknown,
            _ => return None,
        })
    }
}

/// Who is acting.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Principal {
    /// Security identity — what authority must be bound to.
    pub id: String,
    /// Manifest source channel this principal proposes through.
    pub channel: String,
}

/// One proposed call.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Call {
    pub tool: String,
    pub arguments: Value,
}

/// What a target answered a discovery request with.
#[derive(Debug, Clone, Serialize)]
pub struct Discovery {
    pub verdict: Verdict,
    /// Tool names offered to this principal, in the order offered.
    pub visible: Vec<String>,
    /// An identity for the exact surface returned, if the target binds one.
    /// Two principals answered from one cached surface share it.
    pub surface_id: Option<String>,
    pub evidence: Value,
}

/// What a target answered an authorization request with.
#[derive(Debug, Clone, Serialize)]
pub struct Authorization {
    pub verdict: Verdict,
    pub rule: Option<String>,
    /// The handle the principal now holds, if one was issued.
    pub handle: Option<String>,
    /// **The identity of what this grant covers** — whatever the target will
    /// actually check when the handle comes back. This is a required piece of
    /// evidence, not a description: a target that cannot say what its grant is
    /// bound to has not shown that it bound it to anything, and the oracle
    /// fails the step. Targets that bind little report little, honestly (a tool
    /// name is a legitimate answer; it is simply a weak one, and
    /// `binding_distinguishes` is what exposes that).
    pub grant_binding: Option<String>,
    pub evidence: Value,
}

/// What a target did with a proposed call.
#[derive(Debug, Clone, Serialize)]
pub struct Invocation {
    pub verdict: Verdict,
    pub rule: Option<String>,
    /// The identity the target checked **for this presented call**, in the same
    /// vocabulary as [`Authorization::grant_binding`]. Required whenever a
    /// handle was presented. Comparing this against the grant's binding is how
    /// "bound to the exact effect" becomes a mechanical check rather than a
    /// claim in a README.
    pub presented_binding: Option<String>,
    /// The structured reason a presented authorization was refused — for
    /// ai2rules this is a `trace_store::AuthorizationRejection` variant.
    /// Required on a refusal, so "DENY" can never stand as its own explanation.
    pub rejection: Option<String>,
    pub evidence: Value,
}

/// A benchmark target: something that fronts the upstream and decides.
pub trait Target {
    fn id(&self) -> &str;
    fn description(&self) -> &str;
    /// Target-identifying facts recorded in the results file (versions, hashes).
    fn metadata(&self) -> Value;

    fn discover(&mut self, principal: &Principal) -> Discovery;
    fn authorize(&mut self, principal: &Principal, call: &Call) -> Authorization;
    fn invoke(&mut self, principal: &Principal, call: &Call, handle: Option<&str>) -> Invocation;
}
