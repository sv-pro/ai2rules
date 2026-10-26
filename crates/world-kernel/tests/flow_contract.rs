//! Typed source/sink flow contracts (AI2-70, D95) driven through the real
//! compiler and kernel: the slice world `read_document → summarize → publish`.
//!
//! What this pins: a contracted action with no host flow context is refused
//! (`missing_flow_context`); a Private value cannot enter a Public sink even
//! from a trusted caller (`confidentiality_violation`), and that refusal is
//! reported before the taint floor and cannot be overridden by approval; a
//! transform with a Private parent yields a Private result floor; actions
//! without a contract are untouched.

use compiler::{compile, load_yaml};
use harness_types::{
    ActionName, BuildError, CallId, CompiledWorld, Confidentiality, ContentHash, Decision,
    FlowAxis, FlowContext, FlowLabel, Integrity, Provenance, Provider, SessionId, SourceChannel,
    Taint, TaintContext, ToolCall,
};
use serde_json::{json, Value};
use world_kernel::{decide, BudgetUsage, EvalContext, IRBuilder, KernelOutcome};

const WORLD: &str = r#"
world_id: flow-slice
capabilities:
  - { trust: Trusted, actions: [Read, Mcp] }
base_actions:
  - name: read_document
    action_type: Read
    side_effect: Read
    schema:
      type: object
      required: [name]
      properties:
        name: { type: string }
    flow_contract:
      output: { integrity: Trusted, confidentiality: Private }
  - name: summarize_fixture
    action_type: Read
    side_effect: None
    schema:
      type: object
      required: [body]
      properties:
        body: { type: string }
    flow_contract:
      inherits: [body]
  - name: publish
    action_type: Mcp
    side_effect: External
    projected: false
    approval_required: true
    backing: !McpServer { server: reports, tool: publish }
    arg_roles: { to: Target, body: Content }
    schema:
      type: object
      required: [to, body]
      properties:
        to: { type: string }
        body: { type: string }
    flow_contract:
      sinks:
        body: { confidentiality: Public }
  - name: note
    action_type: Read
    side_effect: None
    schema:
      type: object
      properties:
        text: { type: string }
scoped_capabilities:
  - name: publish_report
    base_action: publish
    args:
      to: !Input { one_of: ["https://reports.example/weekly"] }
      body: !ActorInput
"#;

const TP: FlowLabel = FlowLabel::TRUSTED_PUBLIC;
const TPRIV: FlowLabel = FlowLabel {
    integrity: Integrity::Trusted,
    confidentiality: Confidentiality::Private,
};
const UP: FlowLabel = FlowLabel {
    integrity: Integrity::Untrusted,
    confidentiality: Confidentiality::Public,
};

fn world() -> CompiledWorld {
    compile(&load_yaml(WORLD).expect("manifest parses")).expect("manifest compiles")
}

fn call(action: &str, args: Value) -> ToolCall {
    ToolCall {
        action_name: ActionName::new(action),
        arguments: args,
        provider: Provider::CliNative,
        call_id: CallId::new("c"),
        source_perceptions: vec![],
        session_id: SessionId::new("s"),
    }
}

fn provenance() -> Provenance {
    Provenance::from_channel(
        SourceChannel::UserPrompt,
        SessionId::new("s"),
        ContentHash::new("h"),
    )
}

fn flow(ambient: FlowLabel, args: &[(&str, FlowLabel)]) -> FlowContext {
    FlowContext::new(ambient).with_arg_labels(args.iter().map(|(k, l)| (k.to_string(), *l)))
}

fn ctx(taint: TaintContext, approval_granted: bool) -> EvalContext {
    EvalContext {
        taint,
        mode: harness_types::ExecutionMode::Interactive,
        usage: BudgetUsage::default(),
        approval_granted,
    }
}

fn publish_args() -> Value {
    json!({"to": "https://reports.example/weekly", "body": "the summary"})
}

fn not_representable(outcome: KernelOutcome) -> (Decision, String, BuildError) {
    match outcome {
        KernelOutcome::NotRepresentable {
            decision,
            rule,
            error,
        } => (decision, rule, error),
        other => panic!("expected NotRepresentable, got {other:?}"),
    }
}

#[test]
fn contracted_action_without_flow_context_is_denied_and_uncontracted_needs_none() {
    let w = world();
    for action in ["read_document", "publish_report"] {
        let args = if action == "read_document" {
            json!({"name": "q3"})
        } else {
            publish_args()
        };
        let (decision, rule, error) = not_representable(decide(
            &w,
            &call(action, args),
            provenance(),
            &ctx(TaintContext::clean(), false),
        ));
        assert_eq!(decision, Decision::Deny, "{action}");
        assert_eq!(rule, "missing_flow_context", "{action}");
        assert!(matches!(error, BuildError::MissingFlowContext { .. }));
    }
    // `note` declares no contract: no flow context needed, result label unknown.
    let ir = IRBuilder::new(&w)
        .build(
            &call("note", json!({"text": "x"})),
            provenance(),
            &TaintContext::clean(),
        )
        .expect("uncontracted action builds without flow context");
    assert_eq!(ir.result_label(), None);
}

#[test]
fn private_body_cannot_be_published_even_by_a_trusted_caller() {
    let w = world();
    let taint = TaintContext::clean().with_flow(flow(TP, &[("body", TPRIV)]));
    let (decision, rule, error) = not_representable(decide(
        &w,
        &call("publish_report", publish_args()),
        provenance(),
        &ctx(taint, false),
    ));
    assert_eq!(decision, Decision::Deny);
    assert_eq!(rule, "confidentiality_violation");
    match error {
        BuildError::FlowViolation { argument, axis, .. } => {
            assert_eq!(argument, "body");
            assert_eq!(axis, FlowAxis::Confidentiality);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn public_body_passes_flow_admission_and_reaches_approval() {
    let w = world();
    let taint = TaintContext::clean().with_flow(flow(TP, &[("body", TP)]));
    match decide(
        &w,
        &call("publish_report", publish_args()),
        provenance(),
        &ctx(taint.clone(), false),
    ) {
        KernelOutcome::Evaluated { disposition, .. } => {
            assert_eq!(disposition.decision, Decision::Ask)
        }
        other => panic!("{other:?}"),
    }
    match decide(
        &w,
        &call("publish_report", publish_args()),
        provenance(),
        &ctx(taint, true),
    ) {
        KernelOutcome::Evaluated { disposition, .. } => {
            assert_eq!(disposition.decision, Decision::Allow)
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn unlabelled_body_falls_back_to_the_ambient_label() {
    let w = world();
    let private_ambient = TaintContext::clean().with_flow(flow(TPRIV, &[]));
    let (_, rule, _) = not_representable(decide(
        &w,
        &call("publish_report", publish_args()),
        provenance(),
        &ctx(private_ambient, false),
    ));
    assert_eq!(rule, "confidentiality_violation");
    let public_ambient = TaintContext::clean().with_flow(flow(TP, &[]));
    assert!(matches!(
        decide(
            &w,
            &call("publish_report", publish_args()),
            provenance(),
            &ctx(public_ambient, false),
        ),
        KernelOutcome::Evaluated { .. }
    ));
}

#[test]
fn flow_admission_runs_before_the_taint_floor() {
    // Tainted session AND a private body: the floor would refuse this call
    // too, but the disclosure is the reason reported.
    let w = world();
    let taint = TaintContext::from_taint(Taint::Tainted).with_flow(flow(TP, &[("body", TPRIV)]));
    let (_, rule, error) = not_representable(decide(
        &w,
        &call("publish_report", publish_args()),
        provenance(),
        &ctx(taint, false),
    ));
    assert_eq!(rule, "confidentiality_violation");
    assert!(matches!(error, BuildError::FlowViolation { .. }));
}

#[test]
fn untrusted_destination_is_refused_by_the_existing_floor() {
    // `to` has no integrity sink arm; its Untrusted label reaches the floor
    // through `with_flow`'s derived per-argument taint (decision E).
    let w = world();
    let taint = TaintContext::clean().with_flow(flow(TP, &[("to", UP), ("body", TP)]));
    let (_, rule, error) = not_representable(decide(
        &w,
        &call("publish_report", publish_args()),
        provenance(),
        &ctx(taint, false),
    ));
    assert_eq!(rule, "taint_invariant");
    assert!(matches!(error, BuildError::TaintViolation { .. }));
}

#[test]
fn approval_cannot_override_a_flow_deny() {
    let w = world();
    let taint = TaintContext::clean().with_flow(flow(TP, &[("body", TPRIV)]));
    let (decision, rule, _) = not_representable(decide(
        &w,
        &call("publish_report", publish_args()),
        provenance(),
        &ctx(taint, true),
    ));
    assert_eq!(decision, Decision::Deny);
    assert_eq!(rule, "confidentiality_violation");
}

#[test]
fn result_label_floor_follows_the_contract() {
    let w = world();
    let b = IRBuilder::new(&w);
    // A source: its declared output, whatever the ambient.
    let ir = b
        .build(
            &call("read_document", json!({"name": "q3"})),
            provenance(),
            &TaintContext::clean().with_flow(flow(TP, &[])),
        )
        .unwrap();
    assert_eq!(ir.result_label(), Some(TPRIV));
    // A transform: a Private parent stays Private.
    let ir = b
        .build(
            &call("summarize_fixture", json!({"body": "secret"})),
            provenance(),
            &TaintContext::clean().with_flow(flow(TP, &[("body", TPRIV)])),
        )
        .unwrap();
    assert_eq!(ir.result_label(), Some(TPRIV));
    // A sink-only action: the result label is unknown.
    let ir = b
        .build(
            &call("publish_report", publish_args()),
            provenance(),
            &TaintContext::clean().with_flow(flow(TP, &[("body", TP)])),
        )
        .unwrap();
    assert_eq!(ir.result_label(), None);
}

#[test]
fn decide_is_deterministic_with_flow_context() {
    let w = world();
    let taint = TaintContext::clean().with_flow(flow(TP, &[("body", TPRIV)]));
    let run = || {
        format!(
            "{:?}",
            decide(
                &w,
                &call("publish_report", publish_args()),
                provenance(),
                &ctx(taint.clone(), false),
            )
        )
    };
    assert_eq!(run(), run());
}

// ---- binding provenance (PR #169 review) ----
//
// A label describes the value the actor supplied under a name. A value the host
// fills from its own context (`ContextRef`), or an argument a scoped capability
// does not list, is a different value — even when an actor argument travels under
// the same name, renamed onto it or stripped during lowering. Such a value is
// judged by the ambient, never by that actor label.

const CONTEXT_WORLD: &str = r#"
world_id: context-flow-review
capabilities:
  - { trust: Trusted, actions: [Read, Mcp] }
base_actions:
  - name: publish
    action_type: Mcp
    side_effect: External
    projected: false
    backing: !McpServer { server: reports, tool: publish }
    schema:
      type: object
      properties: { body: { type: string }, subject: { type: string } }
    flow_contract:
      sinks: { body: { confidentiality: Public } }
  - name: digest
    action_type: Read
    side_effect: None
    projected: false
    schema:
      type: object
      properties: { body: { type: string }, subject: { type: string } }
    flow_contract:
      inherits: [body]
scoped_capabilities:
  # The reviewer's manifest: the actor's `body` is the subject.
  - name: report
    base_action: publish
    args:
      body: !ContextRef secret
      subject: !Input { as: body }
  # An actor `body` sent alongside, and stripped during lowering.
  - name: report_stripped
    base_action: publish
    args:
      body: !ContextRef secret
      subject: !ActorInput
  # The same rename collision on a transform's inherited argument.
  - name: digest_context
    base_action: digest
    args:
      body: !ContextRef secret
      subject: !Input { as: body }
"#;

fn context_world() -> CompiledWorld {
    compile(&load_yaml(CONTEXT_WORLD).expect("manifest parses")).expect("manifest compiles")
}

#[test]
fn an_actor_label_never_describes_a_context_value_under_the_same_name() {
    let w = context_world();
    // Private ambient (the host holds the secret it will fill in); the actor's
    // own `body` — which lowers to `subject` — is correctly labelled Public.
    let taint = TaintContext::clean().with_flow(flow(TPRIV, &[("body", TP)]));
    let (decision, rule, error) = not_representable(decide(
        &w,
        &call("report", json!({ "body": "public subject" })),
        provenance(),
        &ctx(taint, false),
    ));
    assert_eq!(decision, Decision::Deny);
    assert_eq!(rule, "confidentiality_violation");
    assert!(matches!(
        error,
        BuildError::FlowViolation {
            axis: FlowAxis::Confidentiality,
            ..
        }
    ));
}

#[test]
fn a_stripped_actor_argument_lends_its_label_to_nothing() {
    let w = context_world();
    let taint = TaintContext::clean().with_flow(flow(TPRIV, &[("body", TP), ("subject", TP)]));
    let (_, rule, _) = not_representable(decide(
        &w,
        &call(
            "report_stripped",
            json!({ "subject": "public subject", "body": "public, and stripped" }),
        ),
        provenance(),
        &ctx(taint, false),
    ));
    assert_eq!(rule, "confidentiality_violation");
}

#[test]
fn a_context_value_takes_the_ambient_label_whatever_the_actor_sends() {
    // With a Public ambient the context value is admitted — the host vouches for
    // its own context through the ambient — and an actor label cannot make it
    // stricter or looser.
    let w = context_world();
    for args in [vec![], vec![("body", TP)], vec![("body", TPRIV)]] {
        let taint = TaintContext::clean().with_flow(flow(TP, &args));
        assert!(
            matches!(
                decide(
                    &w,
                    &call("report", json!({ "body": "public subject" })),
                    provenance(),
                    &ctx(taint, false),
                ),
                KernelOutcome::Evaluated { .. }
            ),
            "{args:?}"
        );
    }
}

#[test]
fn an_inherited_context_value_keeps_the_ambient_label() {
    let w = context_world();
    let ir = IRBuilder::new(&w)
        .build(
            &call("digest_context", json!({ "body": "public subject" })),
            provenance(),
            &TaintContext::clean().with_flow(flow(TPRIV, &[("body", TP)])),
        )
        .expect("a transform builds");
    assert_eq!(ir.result_label(), Some(TPRIV));
}
