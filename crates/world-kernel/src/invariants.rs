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
    ActionName, ArgRole, ArgSource, BuildError, DescriptorHash, FlowContext, FlowContract,
    FlowLabel, SideEffectClass, Taint, TaintContext,
};

use crate::taint::externally_effectful;

/// Where the value a base-action argument carries at execution comes from, and
/// so which per-argument label or taint may describe it.
enum Binding<'a> {
    /// The actor supplies it, under this name (a D79 rename included).
    Actor(&'a str),
    /// A design-time literal: clean by construction, never actor-influenced.
    Literal,
    /// Anything else: a context ref the host fills from its own state, or an
    /// argument a scoped capability does not list. No actor-supplied label or
    /// taint describes it — an actor argument that happens to share its name is
    /// a *different* value, renamed onto that name or stripped during lowering —
    /// so it is judged by the ambient (PR #169 review: an actor's Public label
    /// on a renamed `subject` admitted a private `ContextRef` body).
    Ambient,
}

/// How `arg` is bound. `scoped` is the scoped capability's argument map, `None`
/// for a plain action, whose every argument the actor supplies under its own name.
fn binding<'a>(arg: &'a str, scoped: Option<&'a BTreeMap<String, ArgSource>>) -> Binding<'a> {
    let Some(args) = scoped else {
        return Binding::Actor(arg);
    };
    match args.get(arg) {
        Some(ArgSource::Literal(_)) => Binding::Literal,
        Some(source) => source
            .actor_name(arg)
            .map_or(Binding::Ambient, Binding::Actor),
        None => Binding::Ambient,
    }
}

/// Flow admission (AI2-70, D95): every sink the contract declares must admit
/// the label of the value bound to it. Runs *before* [`check_taint`] and is
/// independent of it — a confidentiality refusal is a disclosure, not an
/// integrity gap, and is reported as such even when the floor would also
/// refuse the call.
///
/// Fail-closed rules:
///
///   * a contracted action built without host flow context is refused outright
///     (`MissingFlowContext`): the kernel never guesses a label, and a source
///     with no sinks still needs the explicit ambient to label its result;
///   * an argument whose label the host did not bind falls back to the ambient
///     label, never to Trusted/Public;
///   * a `Literal`-pinned scoped-capability argument is design-time data and is
///     skipped, exactly as in [`effective_floor_taint`].
///
/// Sinks are judged in `BTreeMap` order, each on confidentiality first, so the
/// refusal is deterministic and the two reasons never mask each other.
pub fn check_flow(
    action: &ActionName,
    contract: &FlowContract,
    scoped: Option<&BTreeMap<String, ArgSource>>,
    flow: Option<&FlowContext>,
) -> Result<(), BuildError> {
    let Some(flow) = flow else {
        return Err(BuildError::MissingFlowContext {
            action: action.clone(),
        });
    };
    for (arg, sink) in &contract.sinks {
        let (key, label) = match binding(arg, scoped) {
            Binding::Literal => continue,
            Binding::Actor(key) => (key, flow.arg_label(key).unwrap_or(flow.ambient())),
            Binding::Ambient => (arg.as_str(), flow.ambient()),
        };
        if let Err(axis) = sink.admits(label) {
            return Err(BuildError::FlowViolation {
                action: action.clone(),
                argument: key.to_string(),
                axis,
                detail: format!(
                    "a {:?}/{:?} value may not flow into sink {key} of {action}",
                    label.integrity, label.confidentiality
                ),
            });
        }
    }
    Ok(())
}

/// The floor on the label of this action's result (AI2-70, D95): the declared
/// `output` joined with the labels of every `inherits` argument (ambient
/// fallback, `Literal` skipped). `None` when the contract says nothing about
/// its result — an explicit unknown, never an implicit Trusted/Public. The
/// host may only ever raise this floor when it binds the real value.
pub fn result_label_floor(
    contract: &FlowContract,
    scoped: Option<&BTreeMap<String, ArgSource>>,
    flow: &FlowContext,
) -> Option<FlowLabel> {
    if !contract.declares_result() {
        return None;
    }
    let inherited = contract
        .inherits
        .iter()
        .filter_map(|arg| match binding(arg, scoped) {
            Binding::Literal => None,
            Binding::Actor(key) => Some(flow.arg_label(key).unwrap_or(flow.ambient())),
            Binding::Ambient => Some(flow.ambient()),
        });
    let mut label: Option<FlowLabel> = contract.output;
    for next in inherited {
        label = Some(label.map_or(next, |l| l.join(next)));
    }
    label
}

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
    scoped: Option<&BTreeMap<String, ArgSource>>,
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
        // Known per-argument taint refines; unknown fails closed to ambient. A
        // renamed input (D79) carries its taint under the name the actor used; a
        // context ref or an unlisted argument is never described by an actor's
        // taint, whatever name that argument was sent under.
        let arg_taint = match binding(arg, scoped) {
            Binding::Literal => continue, // design-time literal → clean by construction
            Binding::Actor(key) => ctx.arg_taint(key).unwrap_or_else(|| ctx.taint()),
            Binding::Ambient => ctx.taint(),
        };
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
    use harness_types::InputRule;

    fn roles(pairs: &[(&str, ArgRole)]) -> BTreeMap<String, ArgRole> {
        pairs.iter().map(|(k, r)| (k.to_string(), *r)).collect()
    }

    // ── PACT L2: effective_floor_taint ──

    #[test]
    fn no_roles_falls_back_to_ambient_scalar() {
        // The backward-compatibility contract: an action with no argument roles
        // is judged by the ambient scalar, exactly as before L2 existed.
        let ctx = TaintContext::from_taint(Taint::Tainted);
        let eff = effective_floor_taint(&BTreeMap::new(), None, &ctx);
        assert_eq!(eff, Taint::Tainted);
    }

    #[test]
    fn clean_target_in_tainted_session_is_recovered() {
        // The Theorem-3 witness: ambient session is tainted, but the value bound
        // to the authority-bearing `url` is clean. L2 judges the argument, not the
        // ambient context → the floor sees Clean → the benign fetch is allowed.
        let ctx = TaintContext::from_taint(Taint::Tainted)
            .with_arg_taint([("url".to_string(), Taint::Clean)]);
        let eff = effective_floor_taint(&roles(&[("url", ArgRole::Target)]), None, &ctx);
        assert_eq!(eff, Taint::Clean);
        assert!(check_taint(&ActionName::new("fetch_web"), eff, SideEffectClass::Network).is_ok());
    }

    #[test]
    fn tainted_target_still_blocks() {
        // No false negative: when the url itself is tainted-derived, L2 blocks
        // just as the flat floor did.
        let ctx = TaintContext::from_taint(Taint::Tainted)
            .with_arg_taint([("url".to_string(), Taint::Tainted)]);
        let eff = effective_floor_taint(&roles(&[("url", ArgRole::Target)]), None, &ctx);
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
            None,
            &ctx,
        );
        assert_eq!(eff, Taint::Clean);
    }

    #[test]
    fn unknown_authority_arg_fails_closed_to_ambient() {
        // Fail-closed: an authority-bearing argument whose per-argument taint is
        // unknown must not be assumed clean — it falls back to the ambient scalar.
        let ctx = TaintContext::from_taint(Taint::Tainted); // no arg_taint provided
        let eff = effective_floor_taint(&roles(&[("url", ArgRole::Target)]), None, &ctx);
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
        let eff = effective_floor_taint(
            &roles(&[("endpoint", ArgRole::Target)]),
            Some(&literals),
            &ctx,
        );
        assert_eq!(eff, Taint::Clean);
    }

    #[test]
    fn renamed_input_is_judged_by_the_taint_of_the_name_the_actor_used() {
        // D79: the actor supplies `issue`, the role is declared on the base name
        // `key`. The floor must read the taint recorded under `issue`, or a tainted
        // renamed argument would be judged by whatever `key` happens to carry.
        let inputs: BTreeMap<String, ArgSource> = [(
            "key".to_string(),
            ArgSource::Input(InputRule {
                exposed_as: Some("issue".to_string()),
                ..InputRule::default()
            }),
        )]
        .into_iter()
        .collect();
        let ctx = TaintContext::from_taint(Taint::Clean).with_arg_taint([
            ("issue".to_string(), Taint::Tainted),
            ("key".to_string(), Taint::Clean),
        ]);
        let eff = effective_floor_taint(&roles(&[("key", ArgRole::Target)]), Some(&inputs), &ctx);
        assert_eq!(eff, Taint::Tainted);
    }

    #[test]
    fn a_context_bound_authority_arg_is_judged_by_the_ambient() {
        // PR #169 review, the floor's half: `to` is filled from host context and
        // `note` is exposed to the actor under the name `to`. The actor's clean
        // `to` describes the note, not the destination, so the destination is
        // judged by the tainted ambient. The same holds for an actor `to` that is
        // simply stripped during lowering.
        let renamed: BTreeMap<String, ArgSource> = [
            ("to".to_string(), ArgSource::ContextRef("dest".to_string())),
            (
                "note".to_string(),
                ArgSource::Input(InputRule {
                    exposed_as: Some("to".to_string()),
                    ..InputRule::default()
                }),
            ),
        ]
        .into_iter()
        .collect();
        let stripped: BTreeMap<String, ArgSource> =
            [("to".to_string(), ArgSource::ContextRef("dest".to_string()))]
                .into_iter()
                .collect();
        let ctx = TaintContext::from_taint(Taint::Tainted)
            .with_arg_taint([("to".to_string(), Taint::Clean)]);
        for scoped in [&renamed, &stripped] {
            let eff = effective_floor_taint(&roles(&[("to", ArgRole::Target)]), Some(scoped), &ctx);
            assert_eq!(eff, Taint::Tainted);
        }
        // An argument the capability does not list is never described by an
        // actor's taint either.
        let unlisted: BTreeMap<String, ArgSource> = BTreeMap::new();
        let eff = effective_floor_taint(&roles(&[("to", ArgRole::Target)]), Some(&unlisted), &ctx);
        assert_eq!(eff, Taint::Tainted);
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

    // ── Flow admission (AI2-70, D95) ──

    use harness_types::{Confidentiality, FlowAxis, Integrity, SinkConstraint};

    const TP: FlowLabel = FlowLabel::TRUSTED_PUBLIC;
    const TPRIV: FlowLabel = FlowLabel {
        integrity: Integrity::Trusted,
        confidentiality: Confidentiality::Private,
    };
    const UP: FlowLabel = FlowLabel {
        integrity: Integrity::Untrusted,
        confidentiality: Confidentiality::Public,
    };

    fn public_body_sink() -> FlowContract {
        FlowContract {
            sinks: [(
                "body".to_string(),
                SinkConstraint {
                    confidentiality: Some(Confidentiality::Public),
                    ..Default::default()
                },
            )]
            .into_iter()
            .collect(),
            ..Default::default()
        }
    }

    fn flow(ambient: FlowLabel, args: &[(&str, FlowLabel)]) -> FlowContext {
        FlowContext::new(ambient).with_arg_labels(args.iter().map(|(k, l)| (k.to_string(), *l)))
    }

    #[test]
    fn contracted_action_without_flow_context_fails_closed() {
        let err =
            check_flow(&ActionName::new("publish"), &public_body_sink(), None, None).unwrap_err();
        assert!(matches!(err, BuildError::MissingFlowContext { .. }));
        // Even a contract with no sinks needs the context: its result label
        // depends on the ambient.
        let source = FlowContract {
            output: Some(TPRIV),
            ..Default::default()
        };
        assert!(check_flow(&ActionName::new("read"), &source, None, None).is_err());
    }

    #[test]
    fn private_value_cannot_enter_a_public_sink() {
        let ctx = flow(TP, &[("body", TPRIV)]);
        let err = check_flow(
            &ActionName::new("publish"),
            &public_body_sink(),
            None,
            Some(&ctx),
        )
        .unwrap_err();
        match err {
            BuildError::FlowViolation { argument, axis, .. } => {
                assert_eq!(argument, "body");
                assert_eq!(axis, FlowAxis::Confidentiality);
            }
            other => panic!("{other:?}"),
        }
        let ctx = flow(TP, &[("body", TP)]);
        assert!(check_flow(
            &ActionName::new("publish"),
            &public_body_sink(),
            None,
            Some(&ctx)
        )
        .is_ok());
    }

    #[test]
    fn unlabelled_argument_falls_back_to_the_ambient_label() {
        // Ambient Private, nothing bound for `body`: refused.
        let ctx = flow(TPRIV, &[]);
        assert!(check_flow(
            &ActionName::new("publish"),
            &public_body_sink(),
            None,
            Some(&ctx)
        )
        .is_err());
        // Ambient Public: admitted.
        let ctx = flow(TP, &[]);
        assert!(check_flow(
            &ActionName::new("publish"),
            &public_body_sink(),
            None,
            Some(&ctx)
        )
        .is_ok());
    }

    #[test]
    fn integrity_sink_on_a_content_argument_is_enforced() {
        // `arg_roles` cannot say "this content must be trusted"; a sink can.
        let contract = FlowContract {
            sinks: [(
                "body".to_string(),
                SinkConstraint {
                    integrity: Some(Integrity::Trusted),
                    confidentiality: Some(Confidentiality::Public),
                },
            )]
            .into_iter()
            .collect(),
            ..Default::default()
        };
        let ctx = flow(TP, &[("body", UP)]);
        match check_flow(&ActionName::new("publish"), &contract, None, Some(&ctx)) {
            Err(BuildError::FlowViolation { axis, .. }) => assert_eq!(axis, FlowAxis::Integrity),
            other => panic!("{other:?}"),
        }
        // Both axes violated: confidentiality is reported first.
        let both = FlowLabel {
            integrity: Integrity::Untrusted,
            confidentiality: Confidentiality::Private,
        };
        let ctx = flow(TP, &[("body", both)]);
        match check_flow(&ActionName::new("publish"), &contract, None, Some(&ctx)) {
            Err(BuildError::FlowViolation { axis, .. }) => {
                assert_eq!(axis, FlowAxis::Confidentiality)
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn literal_pinned_sink_is_skipped_and_renamed_input_judged_under_actor_name() {
        let args: BTreeMap<String, ArgSource> = [
            (
                "to".to_string(),
                ArgSource::Literal("https://reports.example".into()),
            ),
            (
                "body".to_string(),
                ArgSource::Input(InputRule {
                    exposed_as: Some("text".to_string()),
                    ..InputRule::default()
                }),
            ),
        ]
        .into_iter()
        .collect();
        let contract = FlowContract {
            sinks: [
                (
                    "to".to_string(),
                    SinkConstraint {
                        integrity: Some(Integrity::Trusted),
                        ..Default::default()
                    },
                ),
                (
                    "body".to_string(),
                    SinkConstraint {
                        confidentiality: Some(Confidentiality::Public),
                        ..Default::default()
                    },
                ),
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        };
        // Ambient Untrusted would refuse `to` — but it is a Literal, so skipped.
        // `body` is labelled Public under the base name, Private under the actor
        // name; the actor name wins.
        let ctx = flow(UP, &[("body", TP), ("text", TPRIV)]);
        match check_flow(
            &ActionName::new("publish_report"),
            &contract,
            Some(&args),
            Some(&ctx),
        ) {
            Err(BuildError::FlowViolation { argument, axis, .. }) => {
                assert_eq!(argument, "text");
                assert_eq!(axis, FlowAxis::Confidentiality);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn result_label_floor_joins_output_and_inherited_arguments() {
        let source = FlowContract {
            output: Some(TPRIV),
            ..Default::default()
        };
        let ctx = flow(TP, &[]);
        assert_eq!(result_label_floor(&source, None, &ctx), Some(TPRIV));

        // A transform inherits: a Private parent stays Private; an Untrusted
        // parent makes the result Untrusted; an unlabelled parent is ambient.
        let transform = FlowContract {
            inherits: vec!["body".to_string()],
            ..Default::default()
        };
        let ctx = flow(TP, &[("body", TPRIV)]);
        assert_eq!(result_label_floor(&transform, None, &ctx), Some(TPRIV));
        let ctx = flow(UP, &[]);
        assert_eq!(result_label_floor(&transform, None, &ctx), Some(UP));
        let both = FlowContract {
            output: Some(TPRIV),
            inherits: vec!["body".to_string()],
            ..Default::default()
        };
        let ctx = flow(TP, &[("body", UP)]);
        assert_eq!(
            result_label_floor(&both, None, &ctx),
            Some(FlowLabel {
                integrity: Integrity::Untrusted,
                confidentiality: Confidentiality::Private
            })
        );
        // Sinks only: the result label is unknown, not Trusted/Public.
        let ctx = flow(TP, &[]);
        assert_eq!(result_label_floor(&public_body_sink(), None, &ctx), None);
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
