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

use compiler::{compile, loader::load_yaml};
use harness_types::{
    ActionName, ArgSource, CallId, CompiledWorld, ContentHash, ExecutionMode, Provenance, Provider,
    SessionId, SourceChannel, Taint, TaintContext, ToolCall,
};
use serde_json::{json, Value};
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
        ArgSource::Literal(v) => json!(format!("literal: {v}")),
        ArgSource::ContextRef(k) => json!(format!("context: {k}")),
    }
}

/// The kernel's verdict for a projected action under a trusted, interactive
/// context with the given inbound taint and empty arguments.
fn verdict(world: &CompiledWorld, action: &ActionName, taint: Taint) -> Value {
    let call = ToolCall {
        action_name: action.clone(),
        arguments: json!({}),
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
    fn invalid_manifest_reports_error() {
        let out = preview("world_id: \"\"\nbase_actions: []");
        assert_eq!(out["ok"], json!(false));
        assert!(out["error"].as_str().unwrap().contains("error"));
    }
}
