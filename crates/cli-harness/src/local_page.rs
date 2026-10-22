//! The plumbing shared by the local pages that can change something on disk:
//! `harness mcp-author` (D79) and `harness console` (D81).
//!
//! Both serve a page on 127.0.0.1 that writes a world file, so both need the same
//! guards, and keeping one copy keeps them from drifting apart:
//!
//! - a request whose `Host` is not this server is refused (DNS rebinding: a
//!   hostile page can resolve its own name to 127.0.0.1, but the browser still
//!   sends that name as the Host);
//! - every POST needs this run's token in a custom header. The token is embedded
//!   in the page, which another origin cannot read, and a custom header cannot be
//!   sent cross-origin without a CORS preflight these servers never approve;
//! - a POST from another `Origin` is refused outright;
//! - files are replaced atomically, so a reader never sees half a manifest.
//!
//! Connections are served one at a time. This is a page for one operator, not a
//! web server.

use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::hash::{BuildHasher, Hasher};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

/// Requests larger than this are refused rather than read.
const MAX_BODY: usize = 4 * 1024 * 1024;

pub(crate) struct Request {
    pub method: String,
    pub path: String,
    /// The raw query string, without the `?`.
    pub query: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

impl Request {
    /// One query parameter, undecoded: the pages only send numbers and slugs.
    pub fn query_param(&self, name: &str) -> Option<&str> {
        self.query
            .split('&')
            .filter_map(|kv| kv.split_once('='))
            .find(|(k, _)| *k == name)
            .map(|(_, v)| v)
    }
}

pub(crate) struct Response {
    pub status: &'static str,
    pub content_type: &'static str,
    pub body: String,
}

impl Response {
    pub fn json(status: &'static str, body: Value) -> Self {
        Response {
            status,
            content_type: "application/json",
            body: body.to_string(),
        }
    }
    pub fn html(body: String) -> Self {
        Response {
            status: "200 OK",
            content_type: "text/html; charset=utf-8",
            body,
        }
    }
    pub fn refused(why: &str) -> Self {
        Response::json("403 Forbidden", json!({ "ok": false, "error": why }))
    }
    pub fn not_found() -> Self {
        Response::json(
            "404 Not Found",
            json!({ "ok": false, "error": "not found" }),
        )
    }
}

/// Read one request and answer it with `route`. A client that connects and
/// stalls must not hold the page hostage, so reads time out.
pub(crate) fn serve_one(
    mut stream: TcpStream,
    route: impl FnOnce(&Request) -> Response,
) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let response = match read_request(&stream)? {
        Some(req) => route(&req),
        None => Response::json(
            "413 Payload Too Large",
            json!({ "ok": false, "error": "request too large" }),
        ),
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
    let target = parts.next().unwrap_or("");
    let (path, query) = target.split_once('?').unwrap_or((target, ""));

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
        path: path.to_string(),
        query: query.to_string(),
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    }))
}

/// The guards every request passes before routing: this server's own `Host`,
/// and for a POST the same origin and this run's token in `token_header`.
/// `Some(refusal)` when the request must not be served.
pub(crate) fn guard(req: &Request, port: u16, token_header: &str, token: &str) -> Option<Response> {
    let own_hosts = [format!("127.0.0.1:{port}"), format!("localhost:{port}")];
    if !req
        .headers
        .get("host")
        .is_some_and(|h| own_hosts.iter().any(|o| o == h))
    {
        return Some(Response::refused("unexpected Host header"));
    }
    if req.method == "POST" {
        if let Some(origin) = req.headers.get("origin") {
            if !own_hosts.iter().any(|o| *origin == format!("http://{o}")) {
                return Some(Response::refused("cross-origin request"));
            }
        }
        if req.headers.get(token_header).map(String::as_str) != Some(token) {
            return Some(Response::refused("missing or wrong page token"));
        }
    }
    None
}

/// A POST body as JSON, or the 400 to answer with.
pub(crate) fn json_body(req: &Request) -> Result<Value, Response> {
    serde_json::from_str(&req.body).map_err(|e| {
        Response::json(
            "400 Bad Request",
            json!({ "ok": false, "error": format!("malformed JSON: {e}") }),
        )
    })
}

/// Replace `path` in one step, so a crash mid-write never leaves half a file and
/// a concurrent reader sees either the old content or the new, never a mix.
pub(crate) fn write_atomically(path: &Path, text: &str) -> io::Result<()> {
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
pub(crate) fn random_token() -> String {
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
