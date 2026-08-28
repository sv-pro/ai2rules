//! The scripted client: walk a scenario's steps against one target.
//!
//! The runner is deterministic and knows nothing about governance. It performs
//! the steps in order, records what came back, and — crucially — reads the
//! downstream effect count from the mock upstream before and after every step, so
//! `effect_applied` is an observation rather than a claim.

use std::cell::RefCell;
use std::rc::Rc;

use serde_json::json;

use crate::oracle;
use crate::result::{RunResult, StepObservation};
use crate::scenario::{Scenario, Step};
use crate::target::{Call, Principal, Target};
use crate::upstream::Upstream;

/// Run one scenario against one target and judge it.
pub fn run_scenario(
    scenario: &Scenario,
    target: &mut dyn Target,
    upstream: &Rc<RefCell<Upstream>>,
) -> RunResult {
    let mut steps: Vec<StepObservation> = Vec::new();

    for step in &scenario.steps {
        let declared = scenario
            .principal(step.actor())
            .expect("validated at load time");
        let principal = Principal {
            id: declared.id.clone(),
            channel: declared.channel.clone(),
        };
        let before = upstream.borrow().effect_count();

        let mut observation = StepObservation {
            id: step.id().to_string(),
            op: step.op(),
            principal: declared.name.clone(),
            principal_id: declared.id.clone(),
            channel: declared.channel.clone(),
            tool: None,
            arguments: None,
            verdict: crate::target::Verdict::Unknown,
            rule: None,
            visible: None,
            surface_id: None,
            handle: None,
            grant_binding: None,
            presented_binding: None,
            rejection: None,
            effect_applied: false,
            effect: None,
            evidence: json!({}),
        };

        match step {
            Step::Discover { .. } => {
                let discovery = target.discover(&principal);
                observation.verdict = discovery.verdict;
                observation.visible = Some(discovery.visible);
                observation.surface_id = discovery.surface_id;
                observation.evidence = discovery.evidence;
            }
            Step::Authorize {
                tool, arguments, ..
            } => {
                let call = Call {
                    tool: tool.clone(),
                    arguments: arguments.clone(),
                };
                let authorization = target.authorize(&principal, &call);
                observation.tool = Some(call.tool);
                observation.arguments = Some(call.arguments);
                observation.verdict = authorization.verdict;
                observation.rule = authorization.rule;
                observation.handle = authorization.handle;
                observation.grant_binding = authorization.grant_binding;
                observation.evidence = authorization.evidence;
            }
            Step::Invoke {
                tool,
                arguments,
                handle,
                ..
            } => {
                let call = Call {
                    tool: tool.clone(),
                    arguments: arguments.clone(),
                };
                // The handle a previous step obtained, presented verbatim.
                let presented = handle.as_ref().and_then(|from| {
                    steps
                        .iter()
                        .find(|observed| &observed.id == from)
                        .and_then(|observed| observed.handle.clone())
                });
                let invocation = target.invoke(&principal, &call, presented.as_deref());
                observation.tool = Some(call.tool);
                observation.arguments = Some(call.arguments);
                observation.verdict = invocation.verdict;
                observation.rule = invocation.rule;
                observation.presented_binding = invocation.presented_binding;
                observation.rejection = invocation.rejection;
                observation.handle = presented;
                observation.evidence = invocation.evidence;
            }
        }

        // The independent half of the evidence: whatever the target claims, this
        // is the ledger's own record of what reached the upstream.
        let ledger = upstream.borrow();
        observation.effect_applied = ledger.effect_count() > before;
        observation.effect = ledger.effects().get(before).cloned();
        drop(ledger);
        steps.push(observation);
    }

    let effect_count = upstream.borrow().effect_count() as u32;
    let checks = oracle::judge(scenario, &steps, effect_count);
    let outcome = oracle::outcome(&checks);

    RunResult {
        scenario: scenario.id.clone(),
        scenario_version: scenario.v,
        title: scenario.title.clone(),
        question: scenario.question.clone(),
        target: target.id().to_string(),
        outcome,
        effect_count,
        steps,
        checks,
    }
}
