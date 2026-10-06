//! `harness mcp-author` — the authoring page for a scoped MCP surface (PLAN
//! E13.4c with E11.4, DECISIONS D78).
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

use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::hash::{BuildHasher, Hasher};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use compiler::{compile, load_yaml};
use harness_preview::{preview, GateUsage};
use harness_types::{ActionName, WorldManifest};
use serde_json::{json, Value};

use crate::mcp_gateway::{forward_target, govern, Upstream};

const PAGE: &str = include_str!("author.html");
/// Where the page's token goes; replaced on every `GET /`.
const TOKEN_SLOT: &str = "__HARNESS_AUTHOR_TOKEN__";
/// The header every POST must carry, holding this run's token.
const TOKEN_HEADER: &str = "x-harness-author";
/// Requests larger than this are refused rather than read.
const MAX_BODY: usize = 4 * 1024 * 1024;

struct Session {
    upstream: Upstream,
    tools: Value,
    world_path: PathBuf,
    token: String,
    port: u16,
}

struct Request {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: String,
}

struct Response {
    status: &'static str,
    content_type: &'static str,
    body: String,
}

impl Response {
    fn json(status: &'static str, body: Value) -> Self {
        Response {
            status,
            content_type: "application/json",
            body: body.to_string(),
        }
    }
    fn refused(why: &str) -> Self {
        Response::json("403 Forbidden", json!({ "ok": false, "error": why }))
    }
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
        let _ = handle(&mut session, stream);
    }
    0
}

fn handle(session: &mut Session, mut stream: TcpStream) -> io::Result<()> {
    // Connections are served one at a time, so a client that connects and stalls
    // must not hold the page hostage.
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let response = match read_request(&stream) {
        Ok(Some(req)) => route(session, &req),
        Ok(None) => Response::json(
            "413 Payload Too Large",
            json!({ "ok": false, "error": "request too large" }),
        ),
        Err(e) => return Err(e),
    };
    let head = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        response.status,
        response.content_type,
        response.body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(response.body.as_bytes())?;
    stream.flush()
}

/// `Ok(None)` when the body exceeds [`MAX_BODY`].
fn read_request(stream: &TcpStream) -> io::Result<Option<Request>> {
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

    let mut headers = HashMap::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }
    let length: usize = headers
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    if length > MAX_BODY {
        return Ok(None);
    }
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;
    Ok(Some(Request {
        method,
        path,
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    }))
}

fn route(session: &mut Session, req: &Request) -> Response {
    // DNS rebinding: a hostile page can resolve its own name to 127.0.0.1, but the
    // browser still sends that name as the Host.
    let own_hosts = [
        format!("127.0.0.1:{}", session.port),
        format!("localhost:{}", session.port),
    ];
    if !req
        .headers
        .get("host")
        .is_some_and(|h| own_hosts.iter().any(|o| o == h))
    {
        return Response::refused("unexpected Host header");
    }
    if req.method == "POST" {
        if let Some(origin) = req.headers.get("origin") {
            if !own_hosts.iter().any(|o| *origin == format!("http://{o}")) {
                return Response::refused("cross-origin request");
            }
        }
        if req.headers.get(TOKEN_HEADER) != Some(&session.token) {
            return Response::refused("missing or wrong authoring token");
        }
    }

    let body: Value = if req.method == "POST" {
        match serde_json::from_str(&req.body) {
            Ok(v) => v,
            Err(e) => {
                return Response::json(
                    "400 Bad Request",
                    json!({ "ok": false, "error": format!("malformed JSON: {e}") }),
                )
            }
        }
    } else {
        Value::Null
    };

    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/") => Response {
            status: "200 OK",
            content_type: "text/html; charset=utf-8",
            body: PAGE.replace(TOKEN_SLOT, &session.token),
        },
        ("GET", "/api/session") => Response::json("200 OK", session_info(session)),
        ("POST", "/api/render") => Response::json("200 OK", render(&body["manifest"])),
        ("POST", "/api/try") => Response::json("200 OK", try_call(session, &body)),
        ("POST", "/api/save") => Response::json("200 OK", save(session, &body["manifest"])),
        _ => Response::json(
            "404 Not Found",
            json!({ "ok": false, "error": "not found" }),
        ),
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
        "# World manifest written by `harness mcp-author` (DECISIONS D78).\n\
         # Serve it with: harness mcp-gateway --world <this file> -- <upstream command>\n{yaml}"
    );
    match write_atomically(&session.world_path, &text) {
        Ok(()) => json!({ "ok": true, "path": session.world_path.display().to_string() }),
        Err(e) => json!({ "ok": false, "error": format!("cannot write the world file: {e}") }),
    }
}

/// Replace `path` in one step, so a crash mid-write never leaves half a manifest.
fn write_atomically(path: &Path, text: &str) -> io::Result<()> {
    let dir = match path.parent() {
        Some(d) if !d.as_os_str().is_empty() => d,
        _ => Path::new("."),
    };
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(text.as_bytes())?;
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

/// 128 bits from the standard library's per-process random hash keys. This guards
/// a localhost page against other origins, not secrets at rest.
fn random_token() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    (0..2u8)
        .map(|i| {
            let mut h = RandomState::new().build_hasher();
            h.write_u8(i);
            h.write_u128(nanos);
            h.write_u32(std::process::id());
            format!("{:016x}", h.finish())
        })
        .collect()
}
