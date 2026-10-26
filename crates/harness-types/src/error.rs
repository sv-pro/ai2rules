//! Failure / outcome taxonomy (architecture §5 IntentIR construction failures).

use serde::{Deserialize, Serialize};

use crate::action::ActionType;
use crate::flow::FlowAxis;
use crate::ids::{ActionName, DescriptorHash};
use crate::provenance::TrustLevel;

/// Reasons an `IntentIR` cannot be built — the representability stage.
///
/// `UnknownToOntology` and `Absent` are the two levels of absence; the rest are
/// structural failures surfaced to the model as `DENY`-class feedback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildError {
    /// Action is unknown to the entire compiled ontology.
    UnknownToOntology {
        action: ActionName,
    },
    /// Action exists in the ontology but is not projected into this world.
    Absent {
        action: ActionName,
    },
    SchemaViolation {
        action: ActionName,
        detail: String,
    },
    CapabilityViolation {
        trust: TrustLevel,
        action_type: ActionType,
    },
    InvariantViolation {
        law: String,
        detail: String,
    },
    DescriptorDrift {
        action: ActionName,
        expected: DescriptorHash,
        actual: DescriptorHash,
    },
    TaintViolation {
        detail: String,
    },
    ApprovalRequired {
        action: ActionName,
    },
    BudgetExceeded {
        detail: String,
    },
    /// A flow-contracted action was built without host-owned flow context
    /// (AI2-70). Fails closed: the kernel never guesses a label.
    MissingFlowContext {
        action: ActionName,
    },
    /// A labelled value may not flow into this argument's declared sink
    /// (AI2-70). `axis` keeps the two refusal reasons distinct.
    FlowViolation {
        action: ActionName,
        argument: String,
        axis: FlowAxis,
        detail: String,
    },
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::UnknownToOntology { action } => {
                write!(f, "action {action} is unknown to the ontology")
            }
            BuildError::Absent { action } => {
                write!(f, "action {action} is absent from this world")
            }
            BuildError::SchemaViolation { action, detail } => {
                write!(f, "schema violation for {action}: {detail}")
            }
            BuildError::CapabilityViolation { trust, action_type } => {
                write!(f, "trust {trust:?} lacks capability for {action_type:?}")
            }
            BuildError::InvariantViolation { law, detail } => {
                write!(f, "invariant {law} violated: {detail}")
            }
            BuildError::DescriptorDrift {
                action,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "descriptor drift for {action}: expected {expected}, got {actual}"
                )
            }
            BuildError::TaintViolation { detail } => write!(f, "taint violation: {detail}"),
            BuildError::ApprovalRequired { action } => {
                write!(f, "action {action} requires approval")
            }
            BuildError::BudgetExceeded { detail } => write!(f, "budget exceeded: {detail}"),
            BuildError::MissingFlowContext { action } => write!(
                f,
                "flow context missing for {action}: the action declares a flow contract"
            ),
            BuildError::FlowViolation {
                action,
                argument,
                axis,
                detail,
            } => write!(
                f,
                "{axis:?} flow violation for {action}.{argument}: {detail}"
            ),
        }
    }
}

impl std::error::Error for BuildError {}
