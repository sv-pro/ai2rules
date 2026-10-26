//! Typed source/sink flow labels (AI2-70, D95).
//!
//! The binary `Taint` in `provenance.rs` is an *integrity* signal carried per
//! session: "did untrusted data reach this context?". It cannot say "this value
//! is private but came from a trusted place", so a world that reads a secret
//! must either taint the whole session (and lose every later benign write) or
//! let the secret leave through a permitted read. This module adds the second
//! axis — confidentiality — and makes both axes **per value**, carried by the
//! host and judged by the kernel against a per-action contract.
//!
//! Three rules, all pure:
//!
//! - **Unknown is explicit.** There is no `Default` for a label; "we do not know
//!   this value's label" is `Option::None`, and every consumer falls back to the
//!   conservative ambient label, never to Trusted/Public.
//! - **The only operation is `join`.** Any Untrusted input makes the result
//!   Untrusted; any Private input makes it Private. There is no declassify, no
//!   endorse, no clear. A label can only ever become more conservative.
//! - **The lattice is a product, not a total order.** `Trusted/Private` and
//!   `Untrusted/Public` are incomparable, so the types deliberately implement
//!   neither `Ord` nor `PartialOrd`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Integrity axis: did this value depend on anything untrusted?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Integrity {
    Trusted,
    Untrusted,
}

impl Integrity {
    /// Monotonic join: any Untrusted dependency makes the result Untrusted.
    pub fn join(self, other: Integrity) -> Integrity {
        match (self, other) {
            (Integrity::Trusted, Integrity::Trusted) => Integrity::Trusted,
            _ => Integrity::Untrusted,
        }
    }
}

/// Confidentiality axis: may this value be disclosed outside its source?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Confidentiality {
    Public,
    Private,
}

impl Confidentiality {
    /// Monotonic join: any Private dependency makes the result Private.
    pub fn join(self, other: Confidentiality) -> Confidentiality {
        match (self, other) {
            (Confidentiality::Public, Confidentiality::Public) => Confidentiality::Public,
            _ => Confidentiality::Private,
        }
    }
}

/// A value's label on both axes. No `Default`: an unknown label is
/// `Option::<FlowLabel>::None`, never an implicit Trusted/Public.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowLabel {
    pub integrity: Integrity,
    pub confidentiality: Confidentiality,
}

impl FlowLabel {
    /// The bottom of both axes — what a value the *user* typed into a trusted
    /// channel carries. Only a host may assert it; the kernel never assumes it.
    pub const TRUSTED_PUBLIC: FlowLabel = FlowLabel {
        integrity: Integrity::Trusted,
        confidentiality: Confidentiality::Public,
    };

    /// Pointwise monotonic join.
    pub fn join(self, other: FlowLabel) -> FlowLabel {
        FlowLabel {
            integrity: self.integrity.join(other.integrity),
            confidentiality: self.confidentiality.join(other.confidentiality),
        }
    }

    pub fn is_private(self) -> bool {
        self.confidentiality == Confidentiality::Private
    }

    pub fn is_untrusted(self) -> bool {
        self.integrity == Integrity::Untrusted
    }
}

/// Which axis a sink constraint rejected. Carried in `BuildError::FlowViolation`
/// so the refusal reason is stable and distinct per axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FlowAxis {
    Integrity,
    Confidentiality,
}

/// What a sink argument will accept. `integrity` is the required *minimum*
/// (`Trusted` means "no untrusted value here"); `confidentiality` is the allowed
/// *maximum* (`Public` means "no private value here"). A constraint that names
/// neither axis constrains nothing and is rejected by the compiler.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SinkConstraint {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integrity: Option<Integrity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidentiality: Option<Confidentiality>,
}

impl SinkConstraint {
    /// Does a value with `label` satisfy this sink? Confidentiality is judged
    /// first: a private value is refused as a disclosure before anything is said
    /// about its integrity, so the two reasons never mask each other.
    pub fn admits(&self, label: FlowLabel) -> Result<(), FlowAxis> {
        if self.confidentiality == Some(Confidentiality::Public) && label.is_private() {
            return Err(FlowAxis::Confidentiality);
        }
        if self.integrity == Some(Integrity::Trusted) && label.is_untrusted() {
            return Err(FlowAxis::Integrity);
        }
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.integrity.is_none() && self.confidentiality.is_none()
    }
}

/// A base action's design-time flow contract. Opt-in: an action without one is
/// judged exactly as before this module existed, and a manifest that declares
/// none serializes without the field, so its hash is unchanged.
///
/// - `output`: a floor on the label of the whole result (the *source* side). The
///   host may only raise it, never lower it.
/// - `inherits`: arguments whose labels are joined into the result (a transform).
/// - `sinks`: per-argument constraints the lowered bindings must satisfy.
///
/// A contract with neither `output` nor `inherits` declares no result label;
/// the kernel then reports `None` (unknown), never an implicit Trusted/Public.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowContract {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<FlowLabel>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inherits: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sinks: BTreeMap<String, SinkConstraint>,
}

impl FlowContract {
    /// Does this contract say anything about the label of its result?
    pub fn declares_result(&self) -> bool {
        self.output.is_some() || !self.inherits.is_empty()
    }

    pub fn is_empty(&self) -> bool {
        !self.declares_result() && self.sinks.is_empty()
    }
}

/// The host-owned, per-call flow context: a conservative ambient label plus the
/// labels of the actual values bound to arguments. Mirrors the shape of
/// `TaintContext`'s per-argument taint. The kernel reads it; only the host
/// writes it — model arguments and remote metadata are data, not labels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowContext {
    ambient: FlowLabel,
    arg_labels: BTreeMap<String, FlowLabel>,
}

impl FlowContext {
    pub fn new(ambient: FlowLabel) -> Self {
        Self {
            ambient,
            arg_labels: BTreeMap::new(),
        }
    }

    /// Attach the labels the host bound to argument values. Builder-style.
    pub fn with_arg_labels(
        mut self,
        labels: impl IntoIterator<Item = (String, FlowLabel)>,
    ) -> Self {
        self.arg_labels = labels.into_iter().collect();
        self
    }

    /// The conservative label every unlabelled argument falls back to.
    pub fn ambient(&self) -> FlowLabel {
        self.ambient
    }

    /// The label of the value bound to `arg`, if the host tracked it.
    /// `None` means unknown — callers must fall back to `ambient()`.
    pub fn arg_label(&self, arg: &str) -> Option<FlowLabel> {
        self.arg_labels.get(arg).copied()
    }

    pub fn arg_labels(&self) -> &BTreeMap<String, FlowLabel> {
        &self.arg_labels
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provenance::{Taint, TaintContext};

    const TP: FlowLabel = FlowLabel::TRUSTED_PUBLIC;
    const TPRIV: FlowLabel = FlowLabel {
        integrity: Integrity::Trusted,
        confidentiality: Confidentiality::Private,
    };
    const UP: FlowLabel = FlowLabel {
        integrity: Integrity::Untrusted,
        confidentiality: Confidentiality::Public,
    };
    const UPRIV: FlowLabel = FlowLabel {
        integrity: Integrity::Untrusted,
        confidentiality: Confidentiality::Private,
    };

    #[test]
    fn joins_are_monotonic_commutative_and_idempotent() {
        for a in [TP, TPRIV, UP, UPRIV] {
            assert_eq!(a.join(a), a, "idempotent");
            for b in [TP, TPRIV, UP, UPRIV] {
                let j = a.join(b);
                assert_eq!(j, b.join(a), "commutative");
                // Never less conservative than either input.
                assert!(j.is_private() || !(a.is_private() || b.is_private()));
                assert!(j.is_untrusted() || !(a.is_untrusted() || b.is_untrusted()));
            }
        }
        assert_eq!(TP.join(UPRIV), UPRIV);
        assert_eq!(TPRIV.join(UP), UPRIV);
    }

    #[test]
    fn sink_judges_confidentiality_before_integrity() {
        let both = SinkConstraint {
            integrity: Some(Integrity::Trusted),
            confidentiality: Some(Confidentiality::Public),
        };
        assert_eq!(both.admits(TP), Ok(()));
        assert_eq!(both.admits(TPRIV), Err(FlowAxis::Confidentiality));
        assert_eq!(both.admits(UP), Err(FlowAxis::Integrity));
        // Both violated: the disclosure is reported, not the integrity gap.
        assert_eq!(both.admits(UPRIV), Err(FlowAxis::Confidentiality));
        // A sink that names no axis admits everything (and is a compile error).
        assert!(SinkConstraint::default().is_empty());
        assert_eq!(SinkConstraint::default().admits(UPRIV), Ok(()));
    }

    #[test]
    fn an_absent_contract_serializes_to_nothing() {
        // The hash-stability prerequisite: every field is skipped when absent.
        assert_eq!(
            serde_json::to_string(&FlowContract::default()).unwrap(),
            "{}"
        );
        assert!(FlowContract::default().is_empty());
        let c: FlowContract = serde_json::from_str("{}").unwrap();
        assert_eq!(c, FlowContract::default());
    }

    #[test]
    fn contract_round_trips_and_rejects_unknown_fields() {
        let src = r#"{
            "output": { "integrity": "Trusted", "confidentiality": "Private" },
            "inherits": ["body"],
            "sinks": {
                "to": { "integrity": "Trusted" },
                "body": { "confidentiality": "Public" }
            }
        }"#;
        let c: FlowContract = serde_json::from_str(src).unwrap();
        assert_eq!(c.output, Some(TPRIV));
        assert!(c.declares_result());
        assert_eq!(
            c.sinks["body"].admits(TPRIV),
            Err(FlowAxis::Confidentiality)
        );
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(serde_json::from_str::<FlowContract>(&json).unwrap(), c);
        assert!(serde_json::from_str::<FlowContract>(r#"{"outputs": {}}"#).is_err());
        assert!(serde_json::from_str::<FlowLabel>(r#"{"integrity": "Trusted"}"#).is_err());
    }

    #[test]
    fn with_flow_derives_integrity_taint_and_never_lowers_it() {
        // Ambient Untrusted taints the session.
        let ctx = TaintContext::clean().with_flow(FlowContext::new(UP));
        assert_eq!(ctx.taint(), Taint::Tainted);
        // Ambient Trusted does not clean an already-tainted session.
        let ctx = TaintContext::from_taint(Taint::Tainted).with_flow(FlowContext::new(TP));
        assert_eq!(ctx.taint(), Taint::Tainted);
        // Per-argument labels: Untrusted => Tainted; Trusted => Clean only where
        // the caller said nothing; a caller-set Tainted is never lowered.
        let flow = FlowContext::new(TP).with_arg_labels([
            ("to".to_string(), UP),
            ("body".to_string(), TP),
            ("note".to_string(), TP),
        ]);
        let ctx = TaintContext::clean()
            .with_arg_taint([("note".to_string(), Taint::Tainted)])
            .with_flow(flow);
        assert_eq!(ctx.arg_taint("to"), Some(Taint::Tainted));
        assert_eq!(ctx.arg_taint("body"), Some(Taint::Clean));
        assert_eq!(ctx.arg_taint("note"), Some(Taint::Tainted));
        assert_eq!(ctx.arg_taint("other"), None);
        assert_eq!(ctx.flow().map(|f| f.ambient()), Some(TP));
        // A context built without flow reports none.
        assert!(TaintContext::clean().flow().is_none());
    }

    #[test]
    fn unknown_argument_label_is_none_not_public() {
        let flow = FlowContext::new(TPRIV);
        assert_eq!(flow.arg_label("anything"), None);
        assert_eq!(flow.arg_label("anything").unwrap_or(flow.ambient()), TPRIV);
    }
}
