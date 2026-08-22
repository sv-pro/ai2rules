//! The acceptance check: is the contrast this pack promises actually held?
//!
//! It lives outside the oracle on purpose. The oracle judges observations
//! against a scenario's expectations and must never know which target produced
//! them; *this* is the layer that knows a benchmark needs two targets and that
//! one of them is supposed to fail. Keeping them apart is what stops "expected
//! to fail here" from leaking into the judging.

use crate::result::{BenchResult, Status};

/// The acceptance check, stated at the call site rather than in the oracle: the
/// weak baseline's intentional failures are all detected, and ai2rules produces
/// the expected outcome everywhere.
///
/// It first insists the contrast was actually *run*. Judging only the runs that
/// happen to exist would let `--target ai2rules --assert-contrast` succeed
/// having never executed the baseline — an assertion about a comparison, passed
/// without the comparison. Every `scenario × target` cell must be present
/// exactly once.
pub fn contrast(result: &BenchResult, weak_id: &str, ai2rules_id: &str) -> Result<(), String> {
    let mut problems = Vec::new();
    for scenario in &result.pack.scenarios {
        for target in [weak_id, ai2rules_id] {
            let runs = result
                .runs
                .iter()
                .filter(|run| &run.scenario == scenario && run.target == target)
                .count();
            if runs != 1 {
                problems.push(format!(
                    "`{target}` × `{scenario}`: {runs} runs, expected exactly 1 \
                     (--assert-contrast requires the whole matrix; use --target all)"
                ));
            }
        }
    }
    if !problems.is_empty() {
        return Err(problems.join("; "));
    }
    for run in &result.runs {
        match (run.target.as_str(), run.outcome) {
            (target, Status::Pass) if target == weak_id => problems.push(format!(
                "`{}` passed `{}`; the reference gateway's defect there is no longer detected",
                run.target, run.scenario
            )),
            (target, Status::Fail) if target == ai2rules_id => {
                problems.push(format!("`{}` failed `{}`", run.target, run.scenario))
            }
            _ => {}
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("; "))
    }
}
