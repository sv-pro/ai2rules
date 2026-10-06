//! Scoped MCP surfaces (DECISIONS D78): a world that hides its upstream tools,
//! re-exposes them under new names with renamed arguments, and limits what the
//! actor may supply. Driven through the real compiler and kernel.

use compiler::{compile, load_yaml};
use harness_types::{
    ActionName, BuildError, CallId, CompiledWorld, ContentHash, Decision, EffectMode, Operation,
    Provenance, Provider, SessionId, SourceChannel, TaintContext, ToolCall, TraceId,
};
use serde_json::{json, Value};
use world_kernel::schema::model_facing_schema;
use world_kernel::{
    build_execution_spec, decide, BudgetUsage, EvalContext, ExecEnv, KernelOutcome,
};

const WORLD: &str = r#"
world_id: my-jira
capabilities:
  - { trust: Trusted, actions: [Read] }
mcp_surface: { name: My_Jira, upstream: atlassian }
base_actions:
  - name: getJiraIssue
    action_type: Read
    side_effect: Read
    exposed: false
    backing: !McpServer { server: atlassian, tool: getJiraIssue }
    schema:
      type: object
      required: [cloudId, issueIdOrKey]
      properties:
        cloudId: { type: string }
        issueIdOrKey: { type: string, description: "Issue key" }
  - name: searchJiraIssuesUsingJql
    action_type: Read
    side_effect: Read
    exposed: false
    backing: !McpServer { server: atlassian, tool: searchJiraIssuesUsingJql }
    schema:
      type: object
      properties:
        cloudId: { type: string }
        jql: { type: string }
        maxResults: { type: integer }
scoped_capabilities:
  - name: get_platform_issue
    base_action: getJiraIssue
    args:
      cloudId: !Literal abc-123
      issueIdOrKey: !Input { as: issue, prefix: "PLAT-" }
  - name: get_listed_issue
    base_action: getJiraIssue
    args:
      cloudId: !Literal abc-123
      issueIdOrKey: !Input { as: issue, one_of: [PLAT-1, PLAT-2] }
  - name: search_issues
    base_action: searchJiraIssuesUsingJql
    args:
      cloudId: !Literal abc-123
      jql: !ActorInput
      maxResults: !Literal 20
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

/// The `{server, tool, input}` an allowed call lowers to.
fn lowered(world: &CompiledWorld, action: &str, args: Value) -> Value {
    let outcome = decide_call(world, action, args);
    let KernelOutcome::Evaluated {
        intent,
        disposition,
    } = outcome
    else {
        panic!("expected an evaluated intent, got {outcome:?}");
    };
    assert_eq!(disposition.decision, Decision::Allow);
    let spec = build_execution_spec(
        world,
        &intent,
        EffectMode::Simulate,
        &ExecEnv::default(),
        TraceId::new("t"),
    )
    .expect("spec assembles");
    match spec.operation() {
        Operation::Structured(v) => v.clone(),
        other => panic!("expected a structured MCP operation, got {other:?}"),
    }
}

fn assert_schema_violation(outcome: KernelOutcome) {
    match outcome {
        KernelOutcome::NotRepresentable {
            decision,
            rule,
            error: BuildError::SchemaViolation { .. },
        } => {
            assert_eq!(decision, Decision::Deny);
            assert_eq!(rule, "schema_violation");
        }
        other => panic!("expected a schema violation, got {other:?}"),
    }
}

#[test]
fn hidden_base_action_stays_in_the_ontology_but_is_absent() {
    let world = world();
    let name = ActionName::new("getJiraIssue");
    assert!(world.in_ontology(&name));
    assert!(!world.is_projected(&name));
    let outcome = decide_call(
        &world,
        "getJiraIssue",
        json!({ "cloudId": "abc-123", "issueIdOrKey": "SEC-1" }),
    );
    assert_eq!(outcome.decision(), Decision::Absent);
}

#[test]
fn renamed_input_lowers_to_the_upstream_name_with_literals_injected() {
    let op = lowered(&world(), "get_platform_issue", json!({ "issue": "PLAT-7" }));
    assert_eq!(
        op,
        json!({
            "server": "atlassian",
            "tool": "getJiraIssue",
            "input": { "cloudId": "abc-123", "issueIdOrKey": "PLAT-7" },
        })
    );
}

#[test]
fn a_value_outside_the_prefix_is_denied() {
    let world = world();
    assert_schema_violation(decide_call(
        &world,
        "get_platform_issue",
        json!({ "issue": "SEC-1" }),
    ));
    // A prefix only ever matches a string.
    assert_schema_violation(decide_call(
        &world,
        "get_platform_issue",
        json!({ "issue": 7 }),
    ));
}

#[test]
fn only_listed_values_are_allowed() {
    let world = world();
    let op = lowered(&world, "get_listed_issue", json!({ "issue": "PLAT-2" }));
    assert_eq!(op["input"]["issueIdOrKey"], json!("PLAT-2"));
    assert_schema_violation(decide_call(
        &world,
        "get_listed_issue",
        json!({ "issue": "PLAT-3" }),
    ));
}

#[test]
fn the_upstream_argument_name_is_not_a_back_door() {
    let world = world();
    // Supplied only under the base name: not read, so the required argument is
    // missing and the call is refused rather than run without its limit.
    assert_schema_violation(decide_call(
        &world,
        "get_platform_issue",
        json!({ "issueIdOrKey": "SEC-1" }),
    ));
    // Supplied under both names: the base-name copy is stripped, never forwarded.
    let op = lowered(
        &world,
        "get_platform_issue",
        json!({ "issue": "PLAT-1", "issueIdOrKey": "SEC-1" }),
    );
    assert_eq!(op["input"]["issueIdOrKey"], json!("PLAT-1"));
}

#[test]
fn a_locked_argument_cannot_be_overridden() {
    let op = lowered(
        &world(),
        "get_platform_issue",
        json!({ "issue": "PLAT-1", "cloudId": "attacker-cloud" }),
    );
    assert_eq!(op["input"]["cloudId"], json!("abc-123"));
}

#[test]
fn literals_keep_their_json_type() {
    let op = lowered(
        &world(),
        "search_issues",
        json!({ "jql": "project = PLAT" }),
    );
    assert_eq!(op["input"]["maxResults"], json!(20));
    assert_eq!(op["input"]["jql"], json!("project = PLAT"));
}

#[test]
fn the_model_sees_renamed_args_and_their_limits_only() {
    let world = world();
    let listed = model_facing_schema(&world, &ActionName::new("get_listed_issue")).unwrap();
    assert_eq!(
        listed,
        json!({
            "type": "object",
            "properties": {
                "issue": {
                    "type": "string",
                    "description": "Issue key",
                    "enum": ["PLAT-1", "PLAT-2"],
                },
            },
            "required": ["issue"],
            "additionalProperties": false,
        })
    );
    let prefixed = model_facing_schema(&world, &ActionName::new("get_platform_issue")).unwrap();
    assert_eq!(prefixed["properties"]["issue"]["pattern"], json!("^PLAT-"));

    let search = model_facing_schema(&world, &ActionName::new("search_issues")).unwrap();
    let props = search["properties"].as_object().unwrap();
    assert_eq!(props.keys().collect::<Vec<_>>(), vec!["jql"]);
    assert!(search.get("required").is_none());

    // A base action is shown exactly as its descriptor declares it.
    let base = model_facing_schema(&world, &ActionName::new("getJiraIssue")).unwrap();
    assert_eq!(
        base,
        world
            .descriptor(&ActionName::new("getJiraIssue"))
            .unwrap()
            .schema
    );
}

#[test]
fn a_prefix_with_regex_syntax_is_advertised_literally() {
    let yaml = WORLD.replace(r#"prefix: "PLAT-""#, r#"prefix: "a.b(c""#);
    let world = compile(&load_yaml(&yaml).unwrap()).unwrap();
    let schema = model_facing_schema(&world, &ActionName::new("get_platform_issue")).unwrap();
    assert_eq!(schema["properties"]["issue"]["pattern"], json!(r"^a\.b\(c"));
}

#[test]
fn the_world_carries_its_mcp_surface() {
    let world = world();
    let surface = world.mcp_surface().expect("surface declared");
    assert_eq!(surface.name, "My_Jira");
    assert_eq!(surface.upstream, "atlassian");
}
