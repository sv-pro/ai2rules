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

use std::collections::BTreeMap;

use harness_types::{
    ActionName, ArgRole, ArgSource, BuildError, DescriptorHash, SideEffectClass, Taint,
    TaintContext,
};

use crate::taint::externally_effectful;

/// The taint the floor should judge this intent by (PACT L2, arXiv:2605.11039).
///
/// The floor rule ([`check_taint`]) is unchanged; this only computes a *more
/// precise input* for it. When an action declares no argument roles, the result
/// is the ambient scalar — byte-for-byte today's behavior (L0/L1). When it does
/// declare roles, the floor considers only the taint bound to **authority-bearing**
/// arguments (`Target`/`Command`/`Credential`); tainted *content* flowing to a
/// clean destination no longer trips the floor (Theorem 3 / 4). Two fail-closed
/// rules keep it sound:
///
///   * an authority-bearing argument whose per-argument taint is *unknown* falls
///     back to the ambient scalar (never less conservative than today);
///   * a `Literal`-pinned scoped-capability argument is design-time-clean and is
///     skipped (it cannot carry actor-supplied taint).
///
/// L2 only *refines* — it never lifts the ambient floor for an action it has no
/// role information about.
pub fn effective_floor_taint(
    roles: &BTreeMap<String, ArgRole>,
    literal_args: &BTreeMap<String, ArgSource>,
    ctx: &TaintContext,
) -> Taint {
    if roles.is_empty() {
        return ctx.taint(); // L0/L1: no argument contract → ambient floor, unchanged.
    }
    let mut effective = Taint::Clean;
    for (arg, role) in roles {
        if !role.is_authority_bearing() {
            continue; // content / selector / control never trips the floor
        }
        if matches!(literal_args.get(arg), Some(ArgSource::Literal(_))) {
            continue; // design-time literal → clean by construction
        }
        // Known per-argument taint refines; unknown fails closed to ambient.
        let arg_taint = ctx.arg_taint(arg).unwrap_or_else(|| ctx.taint());
        effective = effective.join(arg_taint);
    }
    effective
}

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

    fn roles(pairs: &[(&str, ArgRole)]) -> BTreeMap<String, ArgRole> {
        pairs.iter().map(|(k, r)| (k.to_string(), *r)).collect()
    }

    // ── PACT L2: effective_floor_taint ──

    #[test]
    fn no_roles_falls_back_to_ambient_scalar() {
        // The backward-compatibility contract: an action with no argument roles
        // is judged by the ambient scalar, exactly as before L2 existed.
        let ctx = TaintContext::from_taint(Taint::Tainted);
        let eff = effective_floor_taint(&BTreeMap::new(), &BTreeMap::new(), &ctx);
        assert_eq!(eff, Taint::Tainted);
    }

    #[test]
    fn clean_target_in_tainted_session_is_recovered() {
        // The Theorem-3 witness: ambient session is tainted, but the value bound
        // to the authority-bearing `url` is clean. L2 judges the argument, not the
        // ambient context → the floor sees Clean → the benign fetch is allowed.
        let ctx = TaintContext::from_taint(Taint::Tainted)
            .with_arg_taint([("url".to_string(), Taint::Clean)]);
        let eff =
            effective_floor_taint(&roles(&[("url", ArgRole::Target)]), &BTreeMap::new(), &ctx);
        assert_eq!(eff, Taint::Clean);
        assert!(check_taint(&ActionName::new("fetch_web"), eff, SideEffectClass::Network).is_ok());
    }

    #[test]
    fn tainted_target_still_blocks() {
        // No false negative: when the url itself is tainted-derived, L2 blocks
        // just as the flat floor did.
        let ctx = TaintContext::from_taint(Taint::Tainted)
            .with_arg_taint([("url".to_string(), Taint::Tainted)]);
        let eff =
            effective_floor_taint(&roles(&[("url", ArgRole::Target)]), &BTreeMap::new(), &ctx);
        assert_eq!(eff, Taint::Tainted);
        assert!(check_taint(&ActionName::new("fetch_web"), eff, SideEffectClass::Network).is_err());
    }

    #[test]
    fn tainted_content_does_not_trip_the_floor() {
        // The utility recovery: a tainted *content* argument alongside a clean
        // authority-bearing one does not trip the floor.
        let ctx = TaintContext::from_taint(Taint::Tainted).with_arg_taint([
            ("to".to_string(), Taint::Clean),
            ("body".to_string(), Taint::Tainted),
        ]);
        let eff = effective_floor_taint(
            &roles(&[("to", ArgRole::Target), ("body", ArgRole::Content)]),
            &BTreeMap::new(),
            &ctx,
        );
        assert_eq!(eff, Taint::Clean);
    }

    #[test]
    fn unknown_authority_arg_fails_closed_to_ambient() {
        // Fail-closed: an authority-bearing argument whose per-argument taint is
        // unknown must not be assumed clean — it falls back to the ambient scalar.
        let ctx = TaintContext::from_taint(Taint::Tainted); // no arg_taint provided
        let eff =
            effective_floor_taint(&roles(&[("url", ArgRole::Target)]), &BTreeMap::new(), &ctx);
        assert_eq!(eff, Taint::Tainted);
    }

    #[test]
    fn literal_pinned_authority_arg_is_clean_by_construction() {
        // A scoped-capability argument pinned to a Literal is design-time-clean;
        // even an unknown/tainted ambient does not taint it.
        let literals: BTreeMap<String, ArgSource> = [(
            "endpoint".to_string(),
            ArgSource::Literal("https://docs".into()),
        )]
        .into_iter()
        .collect();
        let ctx = TaintContext::from_taint(Taint::Tainted);
        let eff = effective_floor_taint(&roles(&[("endpoint", ArgRole::Target)]), &literals, &ctx);
        assert_eq!(eff, Taint::Clean);
    }

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
