//! Hard invariants — the non-overridable "physics" floor (architecture §6, E2.3).
//!
//! These run inside `IRBuilder::build`, *before* any manifest policy, and cannot
//! be relaxed by a manifest or by human approval. A violation means the intent
//! is not representable at all: no `IntentIR` is sealed, and the failure is
//! surfaced with the structural `BuildError` vocabulary.
//!
//! The floor is **code, not manifest-driven** — a manifest can only add taint
//! policy on top (evaluated in [`crate::disposition`]), never weaken this. The
//! default world's `transition_policies` happen to coincide with the floor;
//! that overlap is harmless, and the floor still holds if a manifest omits them.

use harness_types::{ActionName, BuildError, DescriptorHash, SideEffectClass, Taint};

use crate::taint::externally_effectful;

/// The hard taint invariant: a `Tainted` value cannot drive an externally
/// effectful action (network egress, external side effect, credential access,
/// durable memory write, or persistent write). This is the structural floor
/// behind acceptance invariant 7.
pub fn check_taint(
    action: &ActionName,
    taint: Taint,
    side_effect: SideEffectClass,
) -> Result<(), BuildError> {
    if taint.is_tainted() && externally_effectful(side_effect) {
        return Err(BuildError::TaintViolation {
            detail: format!("tainted value cannot drive {side_effect:?} action {action}"),
        });
    }
    Ok(())
}

/// Descriptor drift: the descriptor hash recorded when an intent was built must
/// still match the world's current hash for the action. Drift blocks before the
/// handler runs (acceptance invariant 11).
///
/// At pure build time within a single world the two hashes are equal by
/// construction (there is no separate "current" yet), so `IRBuilder` does not
/// call this. It is the gate for the cross-world / external-descriptor cases —
/// re-evaluating an old intent against a recompiled world (E6.4) and MCP
/// descriptor registration (E7) — and is unit-tested here so the contract is
/// pinned now.
pub fn check_descriptor_drift(
    action: &ActionName,
    expected: &DescriptorHash,
    current: &DescriptorHash,
) -> Result<(), BuildError> {
    if expected != current {
        return Err(BuildError::DescriptorDrift {
            action: action.clone(),
            expected: expected.clone(),
            actual: current.clone(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tainted_external_action_violates_floor() {
        let err = check_taint(
            &ActionName::new("fetch_web"),
            Taint::Tainted,
            SideEffectClass::Network,
        )
        .unwrap_err();
        assert!(matches!(err, BuildError::TaintViolation { .. }));
    }

    #[test]
    fn clean_external_action_passes() {
        assert!(check_taint(
            &ActionName::new("fetch_web"),
            Taint::Clean,
            SideEffectClass::Network,
        )
        .is_ok());
    }

    #[test]
    fn tainted_local_action_passes() {
        assert!(check_taint(
            &ActionName::new("read_workspace"),
            Taint::Tainted,
            SideEffectClass::Read,
        )
        .is_ok());
    }

    #[test]
    fn matching_descriptor_hash_passes() {
        let h = DescriptorHash::new("abc");
        assert!(check_descriptor_drift(&ActionName::new("x"), &h, &h).is_ok());
    }

    #[test]
    fn drifted_descriptor_hash_is_rejected() {
        let err = check_descriptor_drift(
            &ActionName::new("x"),
            &DescriptorHash::new("old"),
            &DescriptorHash::new("new"),
        )
        .unwrap_err();
        assert!(matches!(err, BuildError::DescriptorDrift { .. }));
    }
}
