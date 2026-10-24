//! Value limits in `arg_constraints` bind (DECISIONS D86). The world the AI2-55
//! report was written about — `amount: { maximum: 10000 }` — driven through the
//! real compiler and kernel.

use compiler::{compile, load_yaml};
use harness_types::{
    ActionName, BuildError, CallId, CompiledWorld, ContentHash, Decision, Provenance, Provider,
    SessionId, SourceChannel, TaintContext, ToolCall,
};
use serde_json::{json, Value};
use world_kernel::{decide, BudgetUsage, EvalContext, KernelOutcome};

const WORLD: &str = r#"
world_id: payments
capabilities:
  - { trust: Trusted, actions: [Mcp] }
base_actions:
  - name: pay
    action_type: Mcp
    side_effect: External
    backing: !McpServer { server: bank, tool: pay }
    schema:
      type: object
      required: [amount, payee]
      properties:
        amount: { type: number, description: "Amount in euros." }
        payee: { type: string }
        memo: { type: string, maxLength: 20 }
    arg_constraints:
      amount: { exclusiveMinimum: 0, maximum: 10000 }
scoped_capabilities:
  - name: pay_supplier
    base_action: pay
    args:
      payee: !Literal ACME
      amount: !ActorInput
"#;

fn world() -> CompiledWorld {
    compile(&load_yaml(WORLD).expect("manifest parses")).expect("manifest compiles")
}

fn decide_call(world: &CompiledWorld, action: &str, args: Value) -> KernelOutcome {
    let call = ToolCall {
        action_name: ActionName::new(action),
        arguments: args,
        provider: Provider::CliNative,
        call_id: CallId::new("c"),
        source_perceptions: vec![],
        session_id: SessionId::new("s"),
    };
    let provenance = Provenance::from_channel(
        SourceChannel::UserPrompt,
        SessionId::new("s"),
        ContentHash::new("h"),
    );
    let ctx = EvalContext {
        taint: TaintContext::clean(),
        mode: harness_types::ExecutionMode::Interactive,
        usage: BudgetUsage::default(),
        approval_granted: false,
    };
    decide(world, &call, provenance, &ctx)
}

fn schema_violation(outcome: KernelOutcome) -> String {
    match outcome {
        KernelOutcome::NotRepresentable {
            decision,
            rule,
            error: BuildError::SchemaViolation { detail, .. },
        } => {
            assert_eq!(decision, Decision::Deny);
            assert_eq!(rule, "schema_violation");
            detail
        }
        other => panic!("expected a schema violation, got {other:?}"),
    }
}

#[test]
fn a_payment_within_the_limit_is_allowed() {
    let outcome = decide_call(&world(), "pay", json!({"amount": 10000, "payee": "ACME"}));
    assert_eq!(outcome.decision(), Decision::Allow);
}

#[test]
fn a_payment_over_the_limit_is_denied() {
    let detail = schema_violation(decide_call(
        &world(),
        "pay",
        json!({"amount": 10000.01, "payee": "ACME"}),
    ));
    assert!(detail.contains("at most 10000"), "{detail}");
    schema_violation(decide_call(
        &world(),
        "pay",
        json!({"amount": 0, "payee": "ACME"}),
    ));
}

#[test]
fn a_limit_in_the_schema_binds_as_well() {
    let detail = schema_violation(decide_call(
        &world(),
        "pay",
        json!({"amount": 5, "payee": "ACME", "memo": "a memo longer than twenty"}),
    ));
    assert!(detail.contains("`memo`"), "{detail}");
}

#[test]
fn a_scoped_capability_inherits_its_base_actions_limit() {
    let world = world();
    assert_eq!(
        decide_call(&world, "pay_supplier", json!({"amount": 50})).decision(),
        Decision::Allow
    );
    schema_violation(decide_call(
        &world,
        "pay_supplier",
        json!({"amount": 20000}),
    ));
}
