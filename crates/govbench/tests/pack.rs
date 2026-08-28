//! The pack, run in CI.
//!
//! These tests are the standing guarantee behind the committed report: ai2rules
//! holds every line the scenarios draw, and the reference gateway's three
//! defects are still *detected* rather than quietly fixed. A benchmark whose
//! baseline stops failing has stopped measuring anything, so that direction is
//! asserted as carefully as the other.
//!
//! They run the linked transport, which needs no built binary. The wire
//! transport and the linked/wire parity check are exercised by
//! `scripts/run-governance-bench.sh`.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use govbench::result::{RunResult, Status};
use govbench::run::run_scenario;
use govbench::scenario::SCENARIO_VERSION;
use govbench::targets::{Ai2rules, Transport, WeakGateway};
use govbench::upstream::Upstream;
use govbench::Pack;

fn pack_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/benchmarks/mcp-governance/pack")
}

fn pack() -> Pack {
    Pack::load(pack_path()).expect("the committed pack loads and validates")
}

fn run_all(target: &str) -> Vec<RunResult> {
    let pack = pack();
    pack.scenarios
        .iter()
        .map(|scenario| {
            let upstream = Rc::new(RefCell::new(Upstream::new(pack.tools.clone())));
            if target == WeakGateway::ID {
                let mut gateway = WeakGateway::new(upstream.clone(), pack.weak_policy.clone());
                run_scenario(scenario, &mut gateway, &upstream)
            } else {
                let mut ai2rules = Ai2rules::new(
                    upstream.clone(),
                    pack.world.clone(),
                    &pack.world_path,
                    Transport::Linked,
                )
                .expect("the ai2rules target starts");
                run_scenario(scenario, &mut ai2rules, &upstream)
            }
        })
        .collect()
}

fn run<'a>(runs: &'a [RunResult], scenario: &str) -> &'a RunResult {
    runs.iter()
        .find(|run| run.scenario == scenario)
        .unwrap_or_else(|| panic!("no run for {scenario}"))
}

fn rule(run: &RunResult, step: &str) -> String {
    run.steps
        .iter()
        .find(|observed| observed.id == step)
        .unwrap_or_else(|| panic!("no step {step}"))
        .rule
        .clone()
        .unwrap_or_default()
}

#[test]
fn the_pack_is_three_versioned_scenarios_over_one_real_world() {
    let pack = pack();
    let ids: Vec<&str> = pack.scenarios.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "discovery-cache-isolation",
            "approval-substitution-and-replay",
            "cross-principal-handle-reuse"
        ]
    );
    assert!(pack.scenarios.iter().all(|s| s.v == SCENARIO_VERSION));
    // The world is compiled by the real compiler, so it has a real identity.
    assert_eq!(pack.world.world_id().as_str(), "mcp-governance-bench");
    assert!(!pack.world.manifest_hash().as_str().is_empty());
    // The upstream advertises one tool the world never declares.
    assert!(pack.tools.iter().any(|tool| tool.name == "admin_purge_all"));
}

#[test]
fn ai2rules_holds_every_line_the_scenarios_draw() {
    for run in run_all(Ai2rules::ID) {
        assert_eq!(
            run.outcome,
            Status::Pass,
            "{} failed: {:?}",
            run.scenario,
            run.checks
                .iter()
                .filter(|c| c.status == Status::Fail)
                .map(|c| format!("{}: {}", c.id, c.detail))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn one_human_yes_is_exactly_one_downstream_effect() {
    let runs = run_all(Ai2rules::ID);
    assert_eq!(run(&runs, "discovery-cache-isolation").effect_count, 0);
    assert_eq!(
        run(&runs, "approval-substitution-and-replay").effect_count,
        1
    );
    assert_eq!(run(&runs, "cross-principal-handle-reuse").effect_count, 1);
}

#[test]
fn every_refusal_names_the_binding_that_caught_it() {
    let runs = run_all(Ai2rules::ID);
    let substitution = run(&runs, "approval-substitution-and-replay");
    assert_eq!(
        rule(substitution, "mutated"),
        "authorization_effect_mismatch"
    );
    assert_eq!(rule(substitution, "exact"), "authorization_consumed");
    assert_eq!(rule(substitution, "replay"), "authorization_exhausted");

    let reuse = run(&runs, "cross-principal-handle-reuse");
    assert_eq!(rule(reuse, "reuse"), "authorization_principal_mismatch");
    assert_eq!(rule(reuse, "owner_uses"), "authorization_consumed");
}

#[test]
fn the_projected_surface_is_an_answer_to_who_asked() {
    let runs = run_all(Ai2rules::ID);
    let discovery = run(&runs, "discovery-cache-isolation");
    let warm = &discovery.steps[0];
    let probe = &discovery.steps[1];
    assert_eq!(
        warm.visible.as_deref().map(<[String]>::len),
        Some(5),
        "the operator sees every declared tool"
    );
    assert_eq!(
        probe.visible.as_deref(),
        Some(["ticket_get".to_string(), "ticket_search".to_string()].as_slice()),
        "the model-proposed channel sees only what its trust can perform"
    );
    assert_ne!(
        warm.surface_id, probe.surface_id,
        "two capability sets must not share one surface identity"
    );
}

#[test]
fn the_weak_baseline_still_fails_the_three_defects_it_was_built_to_fail() {
    let runs = run_all(WeakGateway::ID);
    for run in &runs {
        assert_eq!(
            run.outcome,
            Status::Fail,
            "{} passed; the reference gateway's defect is no longer detected",
            run.scenario
        );
    }

    // Defect 1: the first caller's surface is served to the second.
    let discovery = run(&runs, "discovery-cache-isolation");
    assert_failed(
        discovery,
        "restricted_principal_never_sees_privileged_surface",
    );
    assert_failed(discovery, "the_two_principals_get_distinct_surfaces");

    // Defect 2: a bearer grant naming a tool survives argument substitution and
    // is never spent, so one "yes" becomes three effects.
    let substitution = run(&runs, "approval-substitution-and-replay");
    assert_failed(substitution, "substituted_arguments_reach_nothing");
    assert_failed(substitution, "the_replay_reaches_nothing");
    assert_eq!(substitution.effect_count, 3);

    // Defect 3: the grant is bound to no principal.
    let reuse = run(&runs, "cross-principal-handle-reuse");
    assert_failed(reuse, "the_reuse_reaches_nothing");
    assert_eq!(reuse.effect_count, 2);
}

#[test]
fn the_weak_baseline_is_not_a_strawman() {
    // Its policy is applied correctly wherever the cache and the bearer grant are
    // not in the way: the undeclared tool never appears, and a lower-privileged
    // principal that asks first is not handed the privileged surface.
    let pack = pack();
    let upstream = Rc::new(RefCell::new(Upstream::new(pack.tools.clone())));
    let mut gateway = WeakGateway::new(upstream.clone(), pack.weak_policy.clone());
    let restricted = govbench::target::Principal {
        id: "assistant-a".to_string(),
        channel: "generated".to_string(),
    };
    let privileged = govbench::target::Principal {
        id: "operator-a".to_string(),
        channel: "user_cli".to_string(),
    };
    let first = govbench::target::Target::discover(&mut gateway, &restricted);
    assert_eq!(first.visible, vec!["ticket_get", "ticket_search"]);
    let second = govbench::target::Target::discover(&mut gateway, &privileged);
    assert_eq!(
        second.visible, first.visible,
        "the same cache defect, now costing the privileged principal its own tools"
    );
}

/// A target that decides correctly and says nothing about why.
///
/// This exists to prove the evidence contract bites. Before it, `oracle.rs`
/// examined verdicts, visibility, handles and effect counts but never evidence,
/// so a target could answer `{}` everywhere and still pass all three scenarios —
/// which is not what issue #64 asks for (PASS requires an observed decision,
/// a downstream effect count **and** evidence).
struct SilentTarget {
    inner: Ai2rules,
}

impl govbench::target::Target for SilentTarget {
    fn id(&self) -> &str {
        "silent"
    }
    fn description(&self) -> &str {
        "answers exactly like ai2rules, and shows nothing"
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::json!({})
    }
    fn discover(&mut self, principal: &govbench::target::Principal) -> govbench::target::Discovery {
        let mut out = self.inner.discover(principal);
        out.evidence = serde_json::json!({});
        out
    }
    fn authorize(
        &mut self,
        principal: &govbench::target::Principal,
        call: &govbench::target::Call,
    ) -> govbench::target::Authorization {
        let mut out = self.inner.authorize(principal, call);
        out.evidence = serde_json::json!({});
        out.grant_binding = None;
        out
    }
    fn invoke(
        &mut self,
        principal: &govbench::target::Principal,
        call: &govbench::target::Call,
        handle: Option<&str>,
    ) -> govbench::target::Invocation {
        let mut out = self.inner.invoke(principal, call, handle);
        out.evidence = serde_json::json!({});
        out.presented_binding = None;
        out.rejection = None;
        out
    }
}

#[test]
fn assert_contrast_refuses_to_pass_without_running_the_contrast() {
    use govbench::accept::contrast;
    use govbench::result::{BenchResult, PackIdentity, RESULT_VERSION};

    let pack = pack();
    let scenarios: Vec<String> = pack.scenarios.iter().map(|s| s.id.clone()).collect();
    let half = BenchResult {
        v: RESULT_VERSION,
        pack: PackIdentity {
            path: "pack".to_string(),
            world_id: pack.world.world_id().as_str().to_string(),
            manifest_hash: pack.world.manifest_hash().as_str().to_string(),
            scenarios: scenarios.clone(),
        },
        targets: Vec::new(),
        // Only ai2rules ran. Every run in it passes, so judging the runs that
        // exist would call this a held contrast — an assertion about a
        // comparison, passed without the comparison.
        runs: run_all(Ai2rules::ID),
        transport_parity: None,
    };
    let refused = contrast(&half, WeakGateway::ID, Ai2rules::ID)
        .expect_err("a half-run matrix must not satisfy --assert-contrast");
    for scenario in &scenarios {
        assert!(
            refused.contains(scenario.as_str()),
            "the refusal should name the missing cell {scenario}: {refused}"
        );
    }

    // The whole matrix, and it is satisfied.
    let mut runs = run_all(WeakGateway::ID);
    runs.extend(run_all(Ai2rules::ID));
    let full = BenchResult { runs, ..half };
    contrast(&full, WeakGateway::ID, Ai2rules::ID).expect("the full matrix holds the contrast");
}

#[test]
fn a_correct_verdict_without_evidence_is_not_a_pass() {
    let pack = pack();
    let scenario = pack
        .scenarios
        .iter()
        .find(|s| s.id == "approval-substitution-and-replay")
        .expect("the approval scenario");
    let upstream = Rc::new(RefCell::new(Upstream::new(pack.tools.clone())));
    let inner = Ai2rules::new(
        upstream.clone(),
        pack.world.clone(),
        &pack.world_path,
        Transport::Linked,
    )
    .expect("the ai2rules target starts");
    let mut silent = SilentTarget { inner };
    let run = run_scenario(scenario, &mut silent, &upstream);

    // It decides exactly as ai2rules does, and applies exactly one effect...
    assert_eq!(run.effect_count, 1);
    // ...and still fails, on evidence alone.
    assert_eq!(run.outcome, Status::Fail);
    let failed: Vec<&str> = run
        .checks
        .iter()
        .filter(|c| c.status == Status::Fail)
        .map(|c| c.id.as_str())
        .collect();
    assert!(
        failed.iter().all(|id| id.starts_with("evidence:")
            || *id == "the_grant_is_bound_to_more_than_the_tool_name"),
        "every failure should be an evidence failure, got {failed:?}"
    );
    assert!(
        failed.iter().any(|id| id.starts_with("evidence:")),
        "the evidence invariant must fire, got {failed:?}"
    );
}

fn assert_failed(run: &RunResult, check: &str) {
    let found = run
        .checks
        .iter()
        .find(|c| c.id == check)
        .unwrap_or_else(|| panic!("{} has no check {check}", run.scenario));
    assert_eq!(
        found.status,
        Status::Fail,
        "{}/{check} passed: {}",
        run.scenario,
        found.detail
    );
}
