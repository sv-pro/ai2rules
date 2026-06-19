//! E11 — World Authoring Tool: a local browser editor for world manifests.
//!
//! `harness serve` runs a tiny std-only HTTP server (no async, no extra deps)
//! that hosts a single embedded HTML+JS page and a JSON API backed by the **real**
//! compiler and kernel — so the live preview is 100% faithful to what the harness
//! would actually do (no reimplemented governance logic; see DECISIONS D18).
//!
//! Endpoints:
//! - `GET  /`                   → the authoring page
//! - `GET  /api/world/default`  → the bundled default manifest (seeds the editor)
//! - `POST /api/preview {yaml}` → compile a draft manifest and return the
//!   projected tool surface + a decision matrix (clean vs tainted) per action

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};

use compiler::{compile, default_world_yaml, loader::load_yaml};
use harness_types::{
    ActionName, ArgSource, CallId, CompiledWorld, ContentHash, ExecutionMode, Provenance, Provider,
    SessionId, SourceChannel, Taint, TaintContext, ToolCall,
};
use serde_json::{json, Value};
use world_kernel::{decide, BudgetUsage, EvalContext, KernelOutcome};

const INDEX_HTML: &str = include_str!("ui.html");

/// Start the authoring server (blocks). Connections are handled sequentially —
/// this is a single-user, localhost design-time tool.
pub fn run(port: u16) -> io::Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    println!("World Authoring Tool: http://127.0.0.1:{port}  (Ctrl-C to stop)");
    for stream in listener.incoming().flatten() {
        // One bad connection shouldn't take down the tool.
        let _ = handle(stream);
    }
    Ok(())
}

fn handle(mut stream: TcpStream) -> io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);

    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts
        .next()
        .unwrap_or("")
        .split('?')
        .next()
        .unwrap_or("")
        .to_string();

    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        let lower = line.to_ascii_lowercase();
        if let Some(v) = lower.strip_prefix("content-length:") {
            content_length = v.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body)?;
    }
    let body = String::from_utf8_lossy(&body).into_owned();

    let (status, content_type, payload) = route(&method, &path, &body);
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        payload.len()
    );
    stream.write_all(response.as_bytes())?;
    stream.write_all(payload.as_bytes())?;
    stream.flush()
}

fn route(method: &str, path: &str, body: &str) -> (&'static str, &'static str, String) {
    match (method, path) {
        ("GET", "/") => ("200 OK", "text/html; charset=utf-8", INDEX_HTML.to_string()),
        ("GET", "/api/world/default") => (
            "200 OK",
            "application/json",
            json!({ "yaml": default_world_yaml() }).to_string(),
        ),
        ("POST", "/api/preview") => {
            let yaml = serde_json::from_str::<Value>(body)
                .ok()
                .and_then(|v| v.get("yaml").and_then(Value::as_str).map(str::to_string))
                .unwrap_or_default();
            ("200 OK", "application/json", preview(&yaml).to_string())
        }
        _ => ("404 Not Found", "text/plain", "not found".to_string()),
    }
}

/// Compile a draft manifest and report the projected surface + decision matrix.
/// Pure (no I/O) — the HTTP layer is a thin wrapper over this.
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
