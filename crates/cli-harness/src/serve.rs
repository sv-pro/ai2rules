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

use compiler::default_world_yaml;
use harness_preview::preview;
use serde_json::{json, Value};

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

// The pure `preview(yaml) -> Value` logic lives in the `harness-preview` crate so
// the native authoring tool (here) and the WASM engine (E14) share one
// implementation and can never drift (DECISIONS D22).
