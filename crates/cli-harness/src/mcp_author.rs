//! `harness mcp-author` — the authoring page for a scoped MCP surface (PLAN
//! E13.4c with E11.4, DECISIONS D79).
//!
//! It spawns the upstream MCP server once, lists its tools, and serves a local page
//! where an operator names the surface, picks the tools that exist, renames them,
//! and fixes or limits their arguments. Every choice lands in a `WorldManifest`
//! that `harness mcp-gateway --world` then serves.
//!
//! The page holds no governance logic and no YAML writer (D18). It sends the
//! manifest as JSON; this server parses it into the real `WorldManifest` type,
//! renders it with the real serializer, compiles it with the real compiler, and
//! answers "Try it" through the same gate, lowering and forwarding path the gateway
//! uses (`mcp_gateway::govern` / `forward_target`). What the page shows is what the
//! gateway will do.
//!
//! Because the page can call the real upstream and write a file, the server is
//! stricter than `harness serve`: it binds to 127.0.0.1 only, refuses a request
//! whose `Host` is not this server (DNS rebinding), and requires a per-run token on
//! every POST. The token is embedded in the page, which another origin cannot read,
//! and travels in a custom header, which another origin cannot send without a CORS
//! preflight this server never approves. It writes only the `--world` path given on
//! the command line, and "Try it" calls the upstream only when asked to send.
//!
//! Endpoints (all JSON except `/`):
//! - `GET  /`              → the page, with this run's token
//! - `GET  /api/session`   → the upstream's tools, and the world file if it exists
//! - `POST /api/render`    → `{manifest}` → its YAML, or why it does not compile,
//!   plus the projected surface and decision matrix
//! - `POST /api/try`       → `{manifest, tool, arguments, taint, send}` → the verdict,
//!   what would be (or was) sent upstream, and the upstream's answer when sent
//! - `POST /api/save`      → `{manifest}` → writes the `--world` file

use std::io::{self, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};

use compiler::{compile, load_yaml};
use harness_preview::{preview, GateUsage};
use harness_types::{ActionName, WorldManifest};
use serde_json::{json, Value};

use crate::local_page::{guard, json_body, random_token, serve_one, write_atomically};
use crate::local_page::{Request, Response};
use crate::mcp_gateway::{forward_target, govern, Upstream};

const PAGE: &str = include_str!("author.html");
/// Where the page's token goes; replaced on every `GET /`.
const TOKEN_SLOT: &str = "__HARNESS_AUTHOR_TOKEN__";
/// The header every POST must carry, holding this run's token.
const TOKEN_HEADER: &str = "x-harness-author";

struct Session {
    upstream: Upstream,
    tools: Value,
    world_path: PathBuf,
    token: String,
    port: u16,
}

/// Start the authoring page (blocks). `port` 0 picks a free port; the chosen URL
/// is the first line printed.
pub fn run(world_path: &Path, port: u16, upstream: &[String]) -> i32 {
    let mut up = match Upstream::spawn(upstream) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("mcp-author: cannot start upstream {upstream:?}: {e}");
            return 2;
        }
    };
    let tools = match up.list_result() {
        Ok(r) => r.get("tools").cloned().unwrap_or_else(|| json!([])),
        Err(e) => {
            eprintln!("mcp-author: upstream tools/list failed: {e}");
            return 2;
        }
    };
    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("mcp-author: cannot listen on 127.0.0.1:{port}: {e}");
            return 2;
        }
    };
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(port);
    let mut session = Session {
        upstream: up,
        tools,
        world_path: world_path.to_path_buf(),
        token: random_token(),
        port,
    };
    // Written, not printed: `println!` panics if stdout has gone away (piped into
    // `head`, or a test that reads only the URL), and the page must outlive that.
    let mut out = io::stdout();
    let _ = writeln!(
        out,
        "MCP authoring page: http://127.0.0.1:{port}/  (Ctrl-C to stop)"
    );
    let _ = writeln!(out, "Saving to {}", world_path.display());
    let _ = out.flush();
    for stream in listener.incoming().flatten() {
        // One bad connection shouldn't take down the tool.
        let _ = serve_one(stream, |req| route(&mut session, req));
    }
    0
}

fn route(session: &mut Session, req: &Request) -> Response {
    if let Some(refusal) = guard(req, session.port, TOKEN_HEADER, &session.token) {
        return refusal;
    }
    let body: Value = if req.method == "POST" {
        match json_body(req) {
            Ok(v) => v,
            Err(bad) => return bad,
        }
    } else {
        Value::Null
    };

    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/") => Response::html(PAGE.replace(TOKEN_SLOT, &session.token)),
        ("GET", "/api/session") => Response::json("200 OK", session_info(session)),
        ("POST", "/api/render") => Response::json("200 OK", render(&body["manifest"])),
        ("POST", "/api/try") => Response::json("200 OK", try_call(session, &body)),
        ("POST", "/api/save") => Response::json("200 OK", save(session, &body["manifest"])),
        _ => Response::not_found(),
    }
}

fn session_info(session: &Session) -> Value {
    let (world, load_error) = match std::fs::read_to_string(&session.world_path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => (Value::Null, Value::Null),
        Err(e) => (
            Value::Null,
            json!(format!("cannot read the world file: {e}")),
        ),
        Ok(text) => match load_yaml(&text) {
            Ok(m) => (serde_json::to_value(m).unwrap_or(Value::Null), Value::Null),
            Err(e) => (
                Value::Null,
                json!(format!("the world file does not parse: {e}")),
            ),
        },
    };
    let stem = session
        .world_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("world")
        .trim_end_matches(".world");
    json!({
        "upstream": {
            "server_info": session.upstream.server_info,
            "tools": session.tools,
        },
        "world": world,
        "load_error": load_error,
        "world_path": session.world_path.display().to_string(),
        "world_stem": stem,
    })
}

/// Parse the page's manifest through the real type and render it as YAML. The
/// rendering is reloaded and must equal what was sent, so a serializer quirk can
/// never save a file that means something else.
fn to_yaml(manifest: &Value) -> Result<(WorldManifest, String), String> {
    let parsed: WorldManifest =
        serde_json::from_value(manifest.clone()).map_err(|e| format!("not a manifest: {e}"))?;
    let yaml = serde_yaml::to_string(&parsed).map_err(|e| format!("cannot render: {e}"))?;
    let reloaded = load_yaml(&yaml).map_err(|e| format!("rendered YAML does not reload: {e}"))?;
    if reloaded != parsed {
        return Err("rendered YAML does not reload to the same manifest".to_string());
    }
    Ok((parsed, yaml))
}

fn render(manifest: &Value) -> Value {
    match to_yaml(manifest) {
        Err(e) => json!({ "ok": false, "error": e }),
        Ok((parsed, yaml)) => match compile(&parsed) {
            Err(e) => json!({ "ok": false, "yaml": yaml, "error": format!("compile error: {e}") }),
            Ok(_) => json!({ "ok": true, "yaml": yaml, "preview": preview(&yaml) }),
        },
    }
}

fn try_call(session: &mut Session, body: &Value) -> Value {
    let (parsed, _) = match to_yaml(&body["manifest"]) {
        Ok(v) => v,
        Err(e) => return json!({ "ok": false, "error": e }),
    };
    let world = match compile(&parsed) {
        Ok(w) => w,
        Err(e) => return json!({ "ok": false, "error": format!("compile error: {e}") }),
    };
    let tool = body["tool"].as_str().unwrap_or_default();
    let arguments = body.get("arguments").cloned().unwrap_or_else(|| json!({}));
    let tainted = body["taint"].as_str() == Some("tainted");
    let send = body["send"].as_bool().unwrap_or(false);

    let (verdict, lowered) = govern(
        &world,
        tool,
        &arguments,
        "cli",
        tainted,
        "interactive",
        GateUsage::default(),
    );
    let verdict_json = serde_json::to_value(&verdict).unwrap_or(Value::Null);
    if verdict.decision != "ALLOW" {
        return json!({ "ok": true, "verdict": verdict_json });
    }
    let (upstream_tool, input) = match forward_target(
        &world,
        tool,
        &ActionName::new(&verdict.action),
        &arguments,
        lowered,
    ) {
        Ok(t) => t,
        Err(why) => return json!({ "ok": true, "verdict": verdict_json, "refused": why }),
    };
    let forwarded = json!({ "tool": upstream_tool, "arguments": input });
    if !send {
        return json!({ "ok": true, "verdict": verdict_json, "forwarded": forwarded, "sent": false });
    }
    let result = match session.upstream.call_tool(&upstream_tool, &input, None) {
        Ok(r) => r,
        Err(e) => {
            json!({ "isError": true, "content": [{"type": "text", "text": format!("upstream error: {e}")}] })
        }
    };
    json!({
        "ok": true,
        "verdict": verdict_json,
        "forwarded": forwarded,
        "sent": true,
        "result": result,
    })
}

fn save(session: &Session, manifest: &Value) -> Value {
    let (parsed, yaml) = match to_yaml(manifest) {
        Ok(v) => v,
        Err(e) => return json!({ "ok": false, "error": e }),
    };
    if let Err(e) = compile(&parsed) {
        return json!({ "ok": false, "error": format!("not saved — compile error: {e}") });
    }
    // The upstream command line is deliberately not recorded: it can carry
    // credentials, and the file is meant to be committed.
    let text = format!(
        "# World manifest written by `harness mcp-author` (DECISIONS D79).\n\
         # Serve it with: harness mcp-gateway --world <this file> -- <upstream command>\n{yaml}"
    );
    match write_atomically(&session.world_path, &text) {
        Ok(()) => json!({ "ok": true, "path": session.world_path.display().to_string() }),
        Err(e) => json!({ "ok": false, "error": format!("cannot write the world file: {e}") }),
    }
}
