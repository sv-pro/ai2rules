//! Pure design-time **preview** over the real compiler + kernel (E11 / E14).
//!
//! Given a draft `WorldManifest` (YAML), [`preview`] compiles it and reports the
//! projected tool surface plus a clean-vs-tainted decision matrix per action —
//! the *exact* governance the harness would apply, with no logic reimplemented.
//!
//! This is the single source of truth shared by two front ends so they can never
//! drift (see DECISIONS D18/D22):
//! - `harness serve` (the native HTTP authoring tool, E11), and
//! - `harness-wasm` (the in-browser engine behind the visualization suite, E14/E15).
//!
//! It is pure: no I/O, no threads — safe to compile to `wasm32`.
//!
//! Alongside the design-time [`preview`], [`gate`] is the *runtime* half: it
//! governs one concrete proposed call (the host-neutral `harness gate` ABI,
//! DECISIONS D24), and [`project`] is its discovery sibling (D72): it shapes the
//! tool surface a host is about to offer one proposing channel. All three run
//! the real kernel and share this crate, so native, WASM, and the per-host
//! adapters can never diverge.

pub mod gate;
pub mod host;
pub mod project;

pub use gate::{
    gate, GateApproval, GateContext, GateRequest, GateResponse, GateUsage, ABI_VERSION,
};
pub use host::{host_outcome, BlockKind, HostOutcome};
pub use project::{project, PROJECTION_VERSION};

use compiler::{compile, loader::load_yaml};
use harness_types::{
    ActionName, ArgSource, CallId, CompiledWorld, ContentHash, ExecutionMode, Provenance, Provider,
    SessionId, SourceChannel, Taint, TaintContext, ToolCall,
};
use serde_json::{json, Value};
use world_kernel::schema::model_facing_schema;
use world_kernel::{decide, BudgetUsage, EvalContext, KernelOutcome};

/// Compile a draft manifest and report the projected surface + decision matrix.
///
/// Returns `{ ok: false, error }` on a parse/compile failure, else
/// `{ ok: true, world_id, manifest_hash, surface[], decisions[] }`.
pub fn preview(yaml: &str) -> Value {
    let manifest = match load_yaml(yaml) {
        Ok(m) => m,
        Err(e) => return json!({ "ok": false, "error": format!("parse error: {e}") }),
    };
    let world = match compile(&manifest) {
        Ok(w) => w,
        Err(e) => return json!({ "ok": false, "error": format!("compile error: {e}") }),
    };

    let mut actions: Vec<&ActionName> = world.projected_actions().collect();
    actions.sort();

    let mut surface = Vec::new();
    let mut decisions = Vec::new();
    for action in actions {
        let scoped = world.scoped_capability(action);
        surface.push(json!({
            "name": action.as_str(),
            "kind": if scoped.is_some() { "scoped" } else { "base" },
            "action_type": format!("{:?}", world.action_type(action)),
            "side_effect": format!("{:?}", world.side_effect(action)),
            "args": scoped.map(|c| {
                c.args.iter().map(|(k, v)| (k.clone(), describe_arg(v))).collect::<serde_json::Map<_, _>>()
            }),
            // What the model is shown (D78): renamed, with locked args left out.
            "input_schema": model_facing_schema(&world, action),
        }));
        decisions.push(json!({
            "action": action.as_str(),
            "clean": verdict(&world, action, Taint::Clean),
            "tainted": verdict(&world, action, Taint::Tainted),
        }));
    }

    let hash = world.manifest_hash().as_str();
    json!({
        "ok": true,
        "world_id": world.world_id().as_str(),
        "manifest_hash": &hash[..hash.len().min(12)],
        "surface": surface,
        "decisions": decisions,
    })
}

fn describe_arg(source: &ArgSource) -> Value {
    match source {
        ArgSource::ActorInput => json!("actor-input"),
        ArgSource::Input(rule) => {
            let mut text = "actor-input".to_string();
            if let Some(name) = &rule.exposed_as {
                text.push_str(&format!(" as {name}"));
            }
            if !rule.one_of.is_empty() {
                let values: Vec<String> = rule.one_of.iter().map(plain).collect();
                text.push_str(&format!(", one of [{}]", values.join(", ")));
            }
            if let Some(prefix) = &rule.prefix {
                text.push_str(&format!(", starts with {prefix}"));
            }
            json!(text)
        }
        ArgSource::Literal(v) => json!(format!("literal: {}", plain(v))),
        ArgSource::ContextRef(k) => json!(format!("context: {k}")),
    }
}

/// A JSON value as an author would write it: strings without their quotes.
fn plain(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// The kernel's verdict for a projected action under a trusted, interactive
/// context with the given inbound taint and minimal sample arguments.
fn verdict(world: &CompiledWorld, action: &ActionName, taint: Taint) -> Value {
    let call = ToolCall {
        action_name: action.clone(),
        arguments: sample_arguments(world, action),
        provider: Provider::CliNative,
        call_id: CallId::new("preview"),
        source_perceptions: vec![],
        session_id: SessionId::new("wat"),
    };
    let provenance = Provenance::from_channel(
        SourceChannel::UserPrompt,
        SessionId::new("wat"),
        ContentHash::new("wat"),
    );
    let ctx = EvalContext {
        taint: TaintContext::from_taint(taint),
        mode: ExecutionMode::Interactive,
        usage: BudgetUsage::default(),
        approval_granted: false,
    };
    let (decision, rule) = match decide(world, &call, provenance, &ctx) {
        KernelOutcome::UnknownToOntology { .. } => {
            ("UNKNOWN".to_string(), "unknown_to_ontology".to_string())
        }
        KernelOutcome::NotRepresentable { decision, rule, .. } => (format!("{decision:?}"), rule),
        KernelOutcome::Evaluated { disposition, .. } => {
            (format!("{:?}", disposition.decision), disposition.rule)
        }
    };
    json!({ "decision": decision, "rule": rule })
}

fn sample_arguments(world: &CompiledWorld, action: &ActionName) -> Value {
    if let Some(cap) = world.scoped_capability(action) {
        return Value::Object(
            cap.args
                .iter()
                .filter_map(|(name, source)| {
                    let actor_name = source.actor_name(name)?;
                    // A sample the input rule admits, so the matrix shows the
                    // verdict for a well-formed call rather than a refusal.
                    let value = match source {
                        ArgSource::Input(rule) => match (rule.one_of.first(), &rule.prefix) {
                            (Some(first), _) => first.clone(),
                            (None, Some(prefix)) => json!(format!("{prefix}1")),
                            (None, None) => sample_value(actor_name, None),
                        },
                        _ => sample_value(name, None),
                    };
                    Some((actor_name.to_string(), value))
                })
                .collect(),
        );
    }
    let Some(descriptor) = world.descriptor(action) else {
        return json!({});
    };
    let Some(props) = descriptor
        .schema
        .get("properties")
        .and_then(Value::as_object)
    else {
        return json!({});
    };
    Value::Object(
        props
            .iter()
            .map(|(name, spec)| {
                (
                    name.clone(),
                    sample_value(name, spec.get("type").and_then(Value::as_str)),
                )
            })
            .collect(),
    )
}

fn sample_value(name: &str, ty: Option<&str>) -> Value {
    match name {
        "path" => json!("Cargo.toml"),
        "url" => json!("https://docs.example/guide"),
        "command" => json!("echo ok"),
        "content" | "contents" | "value" => json!("x"),
        "query" => json!("guide"),
        "key" => json!("k"),
        _ => match ty {
            Some("number") => json!(1.0),
            Some("integer") => json!(1),
            Some("boolean") => json!(true),
            Some("array") => json!([]),
            Some("object") => json!({}),
            Some("null") => Value::Null,
            _ => json!("x"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use compiler::default_world_yaml;

    #[test]
    fn preview_of_default_world_lists_surface_and_decisions() {
        let out = preview(default_world_yaml());
        assert_eq!(out["ok"], json!(true));
        let names: Vec<&str> = out["surface"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"read_workspace"));
        assert!(names.contains(&"read_repo_file")); // a scoped cap is shown too
                                                    // run_tests is a scoped cap with a locked literal command.
        let run_tests = out["surface"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["name"] == json!("run_tests"))
            .unwrap();
        assert_eq!(run_tests["kind"], json!("scoped"));
        assert_eq!(run_tests["args"]["command"], json!("literal: pytest"));
    }

    #[test]
    fn decision_matrix_shows_taint_floor() {
        let out = preview(default_world_yaml());
        let decisions = out["decisions"].as_array().unwrap();
        let fetch = decisions
            .iter()
            .find(|d| d["action"] == json!("fetch_web"))
            .unwrap();
        // Clean fetch is allowed; tainted fetch is denied by the taint floor.
        assert_eq!(fetch["clean"]["decision"], json!("Allow"));
        assert_eq!(fetch["tainted"]["decision"], json!("Deny"));
        // start_pty asks for approval regardless of taint.
        let pty = decisions
            .iter()
            .find(|d| d["action"] == json!("start_pty"))
            .unwrap();
        assert_eq!(pty["clean"]["decision"], json!("Ask"));
    }

    #[test]
    fn scoped_surface_previews_renamed_limited_inputs() {
        let yaml = r#"
world_id: my-jira
capabilities:
  - { trust: Trusted, actions: [Read] }
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
        issueIdOrKey: { type: string }
scoped_capabilities:
  - name: get_issue
    base_action: getJiraIssue
    args:
      cloudId: !Literal abc-123
      issueIdOrKey: !Input { as: issue, prefix: "PLAT-" }
"#;
        let out = preview(yaml);
        assert_eq!(out["ok"], json!(true), "{out}");
        let surface = out["surface"].as_array().unwrap();
        // The hidden backing is not on the surface; the scoped tool is.
        assert_eq!(surface.len(), 1);
        let tool = &surface[0];
        assert_eq!(tool["name"], json!("get_issue"));
        assert_eq!(tool["args"]["cloudId"], json!("literal: abc-123"));
        assert_eq!(
            tool["args"]["issueIdOrKey"],
            json!("actor-input as issue, starts with PLAT-")
        );
        assert_eq!(tool["input_schema"]["required"], json!(["issue"]));
        // The matrix samples a value the rule admits, so it shows the real verdict.
        let decision = &out["decisions"][0];
        assert_eq!(decision["clean"]["decision"], json!("Allow"));
    }

    #[test]
    fn invalid_manifest_reports_error() {
        let out = preview("world_id: \"\"\nbase_actions: []");
        assert_eq!(out["ok"], json!(false));
        assert!(out["error"].as_str().unwrap().contains("error"));
    }
}
