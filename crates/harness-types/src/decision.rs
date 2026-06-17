//! Policy outcomes and effect/execution modes (architecture §5).

use serde::{Deserialize, Serialize};

/// The policy outcome for a present action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Decision {
    /// Unavailable in this world/context.
    Absent,
    /// May run, paired with an `EffectMode`.
    Allow,
    /// Visible/present but blocked by policy.
    Deny,
    /// Requires human approval.
    Ask,
    /// Over budget or too broad; planner should choose a smaller/safer path.
    Replan,
}

/// How an allowed action interacts with reality. Orthogonal to `Decision`:
/// the pair is `Allow + Simulate`, never `Decision::Simulate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EffectMode {
    Execute,
    Simulate,
    Proxy,
    Sanitize,
    Truncate,
    Defer,
}

/// Whether a human is available to approve. Distinct from `EffectMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExecutionMode {
    Interactive,
    Background,
}

/// The full result of policy evaluation: a decision, an effect mode when the
/// decision is `Allow`, and the rule that produced it (for audit/replay).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Disposition {
    pub decision: Decision,
    pub effect_mode: Option<EffectMode>,
    pub rule: String,
}

impl Disposition {
    pub fn allow(effect_mode: EffectMode, rule: impl Into<String>) -> Self {
        Self {
            decision: Decision::Allow,
            effect_mode: Some(effect_mode),
            rule: rule.into(),
        }
    }

    pub fn of(decision: Decision, rule: impl Into<String>) -> Self {
        Self {
            decision,
            effect_mode: None,
            rule: rule.into(),
        }
    }
}
