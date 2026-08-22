//! The scenario schema — versioned data, not code.
//!
//! A scenario is a list of **steps** (what the principals do) and a list of
//! **expectations** (what must be true of what was observed). Expectations are
//! target-neutral: the same file judges the weak reference gateway and ai2rules.
//! A target's *documented* failures live in that target's own documentation, not
//! here, or the oracle would be grading against the answer key.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Schema version of a scenario file. Bump on any incompatible field change.
pub const SCENARIO_VERSION: u32 = 1;

#[derive(Debug, Clone, Deserialize)]
pub struct Scenario {
    pub v: u32,
    pub id: String,
    pub title: String,
    /// The single governance question this scenario asks, in one sentence.
    pub question: String,
    /// Optional prose recorded verbatim in the report.
    #[serde(default)]
    pub notes: Option<String>,
    pub principals: Vec<PrincipalDecl>,
    pub steps: Vec<Step>,
    pub expect: Vec<Expectation>,
}

impl Scenario {
    pub fn validate(&self) -> Result<(), String> {
        if self.v != SCENARIO_VERSION {
            return Err(format!(
                "scenario {}: v must be {SCENARIO_VERSION}, found {}",
                self.id, self.v
            ));
        }
        if self.steps.is_empty() || self.expect.is_empty() {
            return Err(format!(
                "scenario {}: needs steps and expectations",
                self.id
            ));
        }
        for step in &self.steps {
            if self.principal(step.actor()).is_none() {
                return Err(format!(
                    "scenario {}: step {} acts as undeclared principal {:?}",
                    self.id,
                    step.id(),
                    step.actor()
                ));
            }
            if let Step::Invoke {
                handle: Some(handle),
                ..
            } = step
            {
                if !self.steps.iter().any(|s| s.id() == handle) {
                    return Err(format!(
                        "scenario {}: step {} takes its handle from unknown step {handle:?}",
                        self.id,
                        step.id()
                    ));
                }
            }
        }
        for expectation in &self.expect {
            for step in expectation.steps() {
                if !self.steps.iter().any(|s| s.id() == step) {
                    return Err(format!(
                        "scenario {}: expectation {} names unknown step {step:?}",
                        self.id,
                        expectation.id()
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn principal(&self, name: &str) -> Option<&PrincipalDecl> {
        self.principals.iter().find(|p| p.name == name)
    }
}

/// One principal a scenario acts as. `id` is the security identity the target
/// must bind authority to; `channel` is the manifest source channel it proposes
/// through.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PrincipalDecl {
    pub name: String,
    pub id: String,
    pub channel: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Step {
    /// Ask the target what tools this principal may see.
    Discover {
        id: String,
        #[serde(rename = "as")]
        actor: String,
    },
    /// Ask for one effect and have the operator answer "yes" once.
    Authorize {
        id: String,
        #[serde(rename = "as")]
        actor: String,
        tool: String,
        #[serde(default)]
        arguments: Value,
    },
    /// Attempt one effect, optionally presenting the handle another step obtained.
    Invoke {
        id: String,
        #[serde(rename = "as")]
        actor: String,
        tool: String,
        #[serde(default)]
        arguments: Value,
        /// Step id whose handle is presented with this call.
        #[serde(default)]
        handle: Option<String>,
    },
}

impl Step {
    pub fn id(&self) -> &str {
        match self {
            Step::Discover { id, .. } | Step::Authorize { id, .. } | Step::Invoke { id, .. } => id,
        }
    }

    pub fn actor(&self) -> &str {
        match self {
            Step::Discover { actor, .. }
            | Step::Authorize { actor, .. }
            | Step::Invoke { actor, .. } => actor,
        }
    }

    pub fn op(&self) -> &'static str {
        match self {
            Step::Discover { .. } => "discover",
            Step::Authorize { .. } => "authorize",
            Step::Invoke { .. } => "invoke",
        }
    }
}

/// One checkable claim about a run. Every variant carries its own `id` so a
/// failure names itself in the report.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "assert", rename_all = "snake_case")]
pub enum Expectation {
    /// The surface offered at `step` contains every name in `values`.
    VisibleIncludes {
        id: String,
        step: String,
        values: Vec<String>,
    },
    /// The surface offered at `step` contains none of `values`.
    VisibleExcludes {
        id: String,
        step: String,
        values: Vec<String>,
    },
    /// The two named steps were answered with different surface identities.
    SurfaceDiffers { id: String, steps: Vec<String> },
    /// The step was answered with exactly this verdict.
    Verdict {
        id: String,
        step: String,
        equals: String,
    },
    /// The step handed back an authorization handle.
    HandleIssued { id: String, step: String },
    /// The step did (or did not) reach the downstream effect target.
    EffectApplied {
        id: String,
        step: String,
        equals: bool,
    },
    /// Total downstream effects observed for the whole scenario.
    EffectCount { id: String, equals: u32 },
}

impl Expectation {
    pub fn id(&self) -> &str {
        match self {
            Expectation::VisibleIncludes { id, .. }
            | Expectation::VisibleExcludes { id, .. }
            | Expectation::SurfaceDiffers { id, .. }
            | Expectation::Verdict { id, .. }
            | Expectation::HandleIssued { id, .. }
            | Expectation::EffectApplied { id, .. }
            | Expectation::EffectCount { id, .. } => id,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Expectation::VisibleIncludes { .. } => "visible_includes",
            Expectation::VisibleExcludes { .. } => "visible_excludes",
            Expectation::SurfaceDiffers { .. } => "surface_differs",
            Expectation::Verdict { .. } => "verdict",
            Expectation::HandleIssued { .. } => "handle_issued",
            Expectation::EffectApplied { .. } => "effect_applied",
            Expectation::EffectCount { .. } => "effect_count",
        }
    }

    /// Step ids this expectation reads, for referential validation.
    pub fn steps(&self) -> Vec<&str> {
        match self {
            Expectation::VisibleIncludes { step, .. }
            | Expectation::VisibleExcludes { step, .. }
            | Expectation::Verdict { step, .. }
            | Expectation::HandleIssued { step, .. }
            | Expectation::EffectApplied { step, .. } => vec![step.as_str()],
            Expectation::SurfaceDiffers { steps, .. } => steps.iter().map(String::as_str).collect(),
            Expectation::EffectCount { .. } => Vec::new(),
        }
    }
}
