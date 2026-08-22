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
        .collect()
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
