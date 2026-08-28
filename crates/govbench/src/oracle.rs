//! The deterministic oracle.
//!
//! It judges observations against the scenario's own expectations, and it is the
//! only place in the runner that can say PASS. Two rules keep it honest:
//!
//! - It reads the target's **verdict** and the runner's **observed effect count**.
//!   A target that says DENY and then calls the upstream anyway fails, because
//!   both halves are checked.
//! - It has no target identity. The same code judges the reference gateway and
//!   ai2rules, so "the weak one is expected to fail here" cannot be encoded.

use std::collections::BTreeSet;

use crate::result::{CheckResult, Status, StepObservation};
use crate::scenario::{Expectation, Scenario};
use crate::target::Verdict;

pub fn judge(
    scenario: &Scenario,
    steps: &[StepObservation],
    effect_count: u32,
) -> Vec<CheckResult> {
    scenario
        .expect
        .iter()
        .map(|expectation| check(expectation, steps, effect_count))
        .chain(evidence_invariants(steps))
        .collect()
}

/// The evidence contract, checked on **every step of every run**, whatever the
/// scenario asked for.
///
/// Issue #64's rule is that PASS requires an observed decision, a downstream
/// effect count *and* evidence. Without this, a target could answer every
/// evidence field with `{}` and still pass all three scenarios — the decision
/// and the effect count would carry the whole result, and "evidence" would be a
/// field nobody read. These invariants are deliberately about the *shape* of
/// what a target must be able to say, never about a particular target's keys:
///
/// - a refusal that cannot name a rule is not a refusal, it is a shrug;
/// - a grant that cannot say what it covers has not shown it covers anything;
/// - a call presenting a handle must say what identity it checked;
/// - and what the ledger recorded must match the call the target was given, so a
///   target cannot answer about one call and execute another.
fn evidence_invariants(steps: &[StepObservation]) -> Vec<CheckResult> {
    let mut checks = Vec::new();
    for step in steps {
        let mut fail = |detail: String| {
            checks.push(CheckResult {
                id: format!("evidence:{}", step.id),
                assertion: "evidence",
                status: Status::Fail,
                detail,
            });
        };

        if step.evidence.as_object().is_none_or(|e| e.is_empty()) {
            fail(format!("{} produced no evidence at all", step.id));
            continue;
        }
        if matches!(
            step.verdict,
            Verdict::Deny | Verdict::Absent | Verdict::ErrorClosed | Verdict::ErrorOpen
        ) && step.rule.as_deref().unwrap_or_default().is_empty()
        {
            fail(format!(
                "{} answered {} without naming a rule",
                step.id,
                step.verdict.as_str()
            ));
            continue;
        }
        if step.op == "authorize"
            && step.handle.is_some()
            && step.grant_binding.as_deref().unwrap_or_default().is_empty()
        {
            fail(format!(
                "{} issued a handle without saying what the grant covers",
                step.id
            ));
            continue;
        }
        if step.op == "invoke" && step.handle.is_some() {
            if step
                .presented_binding
                .as_deref()
                .unwrap_or_default()
                .is_empty()
            {
                fail(format!(
                    "{} presented a handle but did not say what identity it checked",
                    step.id
                ));
                continue;
            }
            if step.verdict == Verdict::Deny
                && step.rejection.as_deref().unwrap_or_default().is_empty()
            {
                fail(format!(
                    "{} refused a presented authorization without a structured reason",
                    step.id
                ));
                continue;
            }
        }
        match (&step.effect, step.effect_applied) {
            (Some(effect), true) if Some(&effect.tool) != step.tool.as_ref() => {
                fail(format!(
                    "{} was asked for {:?} but the ledger recorded {:?}",
                    step.id, step.tool, effect.tool
                ));
                continue;
            }
            (None, true) => {
                fail(format!(
                    "{} reached the effect target with no ledger record",
                    step.id
                ));
                continue;
            }
            _ => {}
        }
        checks.push(CheckResult {
            id: format!("evidence:{}", step.id),
            assertion: "evidence",
            status: Status::Pass,
            detail: format!("{} carries the evidence its verdict requires", step.id),
        });
    }
    checks
}

/// A run passes only when every expectation holds.
pub fn outcome(checks: &[CheckResult]) -> Status {
    if checks.iter().all(|check| check.status == Status::Pass) {
        Status::Pass
    } else {
        Status::Fail
    }
}

fn check(expectation: &Expectation, steps: &[StepObservation], effect_count: u32) -> CheckResult {
    let (status, detail) = match expectation {
        Expectation::VisibleIncludes { step, values, .. } => match visible(steps, step) {
            Err(detail) => (Status::Fail, detail),
            Ok(visible) => {
                let missing: Vec<&String> = values
                    .iter()
                    .filter(|name| !visible.contains(name))
                    .collect();
                if missing.is_empty() {
                    (Status::Pass, format!("{step} offered all of {values:?}"))
                } else {
                    (
                        Status::Fail,
                        format!("{step} did not offer {missing:?}; offered {visible:?}"),
                    )
                }
            }
        },
        Expectation::VisibleExcludes { step, values, .. } => match visible(steps, step) {
            Err(detail) => (Status::Fail, detail),
            Ok(visible) => {
                let leaked: Vec<&String> = values
                    .iter()
                    .filter(|name| visible.contains(name))
                    .collect();
                if leaked.is_empty() {
                    (
                        Status::Pass,
                        format!("{step} withheld every one of {values:?}"),
                    )
                } else {
                    (
                        Status::Fail,
                        format!("{step} offered {leaked:?}, which it must not see"),
                    )
                }
            }
        },
        Expectation::SurfaceDiffers { steps: named, .. } => {
            let mut ids = BTreeSet::new();
            let mut missing = Vec::new();
            for name in named {
                match steps.iter().find(|observed| &observed.id == name) {
                    Some(observed) => {
                        ids.insert(observed.surface_id.clone().unwrap_or_default());
                    }
                    None => missing.push(name.clone()),
                }
            }
            if !missing.is_empty() {
                (Status::Fail, format!("no observation for {missing:?}"))
            } else if ids.len() == named.len() {
                (
                    Status::Pass,
                    format!("{named:?} were answered with distinct surfaces"),
                )
            } else {
                (
                    Status::Fail,
                    format!("{named:?} shared a surface identity: {ids:?}"),
                )
            }
        }
        Expectation::BindingDistinguishes { steps: named, .. } => {
            let mut bindings = BTreeSet::new();
            let mut missing = Vec::new();
            for name in named {
                match steps.iter().find(|observed| &observed.id == name) {
                    // A step binds through whichever field its operation uses:
                    // a grant declares what it covers, a call declares what it
                    // checked. Both are the same vocabulary by contract.
                    Some(observed) => {
                        let binding = observed
                            .grant_binding
                            .clone()
                            .or_else(|| observed.presented_binding.clone());
                        match binding {
                            Some(binding) => {
                                bindings.insert(binding);
                            }
                            None => missing.push(name.clone()),
                        }
                    }
                    None => missing.push(name.clone()),
                }
            }
            if !missing.is_empty() {
                (Status::Fail, format!("no binding recorded for {missing:?}"))
            } else if bindings.len() == named.len() {
                (
                    Status::Pass,
                    format!("{named:?} were checked against distinct identities"),
                )
            } else {
                (
                    Status::Fail,
                    format!("{named:?} were checked against the same identity: {bindings:?}"),
                )
            }
        }
        Expectation::Verdict { step, equals, .. } => {
            let Some(observed) = steps.iter().find(|observed| &observed.id == step) else {
                return failed(expectation, format!("no observation for step {step}"));
            };
            let Some(wanted) = Verdict::parse(equals) else {
                return failed(expectation, format!("unknown verdict {equals:?}"));
            };
            if observed.verdict == wanted {
                (Status::Pass, format!("{step} answered {}", wanted.as_str()))
            } else {
                (
                    Status::Fail,
                    format!(
                        "{step} answered {} ({}), expected {}",
                        observed.verdict.as_str(),
                        observed.rule.as_deref().unwrap_or("no rule"),
                        wanted.as_str()
                    ),
                )
            }
        }
        Expectation::HandleIssued { step, .. } => {
            match steps.iter().find(|observed| &observed.id == step) {
                None => (Status::Fail, format!("no observation for step {step}")),
                Some(observed) if observed.handle.is_some() => {
                    (Status::Pass, format!("{step} issued a handle"))
                }
                Some(_) => (Status::Fail, format!("{step} issued no handle")),
            }
        }
        Expectation::EffectApplied { step, equals, .. } => {
            match steps.iter().find(|observed| &observed.id == step) {
                None => (Status::Fail, format!("no observation for step {step}")),
                Some(observed) if observed.effect_applied == *equals => (
                    Status::Pass,
                    format!(
                        "{step} {} the downstream effect target",
                        if *equals { "reached" } else { "did not reach" }
                    ),
                ),
                Some(observed) => (
                    Status::Fail,
                    format!(
                        "{step} answered {} and {} the downstream effect target",
                        observed.verdict.as_str(),
                        if observed.effect_applied {
                            "reached"
                        } else {
                            "did not reach"
                        }
                    ),
                ),
            }
        }
        Expectation::EffectCount { equals, .. } => {
            if effect_count == *equals {
                (Status::Pass, effects(effect_count))
            } else {
                (
                    Status::Fail,
                    format!("{}, expected {equals}", effects(effect_count)),
                )
            }
        }
    };
    CheckResult {
        id: expectation.id().to_string(),
        assertion: expectation.kind(),
        status,
        detail,
    }
}

fn failed(expectation: &Expectation, detail: String) -> CheckResult {
    CheckResult {
        id: expectation.id().to_string(),
        assertion: expectation.kind(),
        status: Status::Fail,
        detail,
    }
}

fn effects(count: u32) -> String {
    format!(
        "{count} downstream effect{}",
        if count == 1 { "" } else { "s" }
    )
}

fn visible<'a>(steps: &'a [StepObservation], step: &str) -> Result<&'a Vec<String>, String> {
    let observed = steps
        .iter()
        .find(|observed| observed.id == step)
        .ok_or_else(|| format!("no observation for step {step}"))?;
    observed
        .visible
        .as_ref()
        .ok_or_else(|| format!("step {step} produced no surface"))
}
