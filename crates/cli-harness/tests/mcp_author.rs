//! End-to-end test for `harness mcp-author` (DECISIONS D78, PLAN E13.4c): the
//! authoring page's server, driven over real HTTP against `harness mock-jira
//! --rovo --echo`, and the file it saves served by the real `mcp-gateway`.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

struct Author {
    child: Child,
    port: u16,
    token: String,
}

impl Drop for Author {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn scratch_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "mcp-author-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn start(world: &Path) -> Author {
    let bin = env!("CARGO_BIN_EXE_harness");
    let mut child = Command::new(bin)
        .args([
            "mcp-author",
            "--world",
            world.to_str().unwrap(),
            "--port",
            "0",
            "--",
            bin,
            "mock-jira",
            "--rovo",
            "--echo",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn mcp-author");
    let mut first = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut first)
        .unwrap();
    let port: u16 = first
        .split("http://127.0.0.1:")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .and_then(|p| p.parse().ok())
        .unwrap_or_else(|| panic!("no URL in {first:?}"));
    let mut author = Author {
        child,
        port,
        token: String::new(),
    };
    let (status, page) = author.request("GET", "/", None, &[]);
    assert_eq!(status, 200);
    author.token = page
        .split(r#"name="harness-author-token" content=""#)
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("token in page")
        .to_string();
    assert_eq!(author.token.len(), 32);
    author
}

impl Author {
    /// One HTTP request. `headers` replace the defaults of the same name.
    fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
        headers: &[(&str, &str)],
    ) -> (u16, String) {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        let host = format!("127.0.0.1:{}", self.port);
        let body = body.map(Value::to_string).unwrap_or_default();
        let mut head: Vec<(String, String)> = vec![
            ("Host".into(), host),
            ("Content-Length".into(), body.len().to_string()),
            ("Connection".into(), "close".into()),
        ];
        if method == "POST" {
            head.push(("X-Harness-Author".into(), self.token.clone()));
        }
        for (name, value) in headers {
            head.retain(|(n, _)| !n.eq_ignore_ascii_case(name));
            if !value.is_empty() {
                head.push((name.to_string(), value.to_string()));
            }
        }
        let mut req = format!("{method} {path} HTTP/1.1\r\n");
        for (n, v) in head {
            req.push_str(&format!("{n}: {v}\r\n"));
        }
        req.push_str("\r\n");
        req.push_str(&body);
        stream.write_all(req.as_bytes()).unwrap();
        let mut raw = String::new();
        stream.read_to_string(&mut raw).unwrap();
        let (status_line, rest) = raw.split_once("\r\n").unwrap();
        let status = status_line
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        let body = rest.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or("");
        (status, body.to_string())
    }

    fn post(&self, path: &str, body: Value) -> Value {
        let (status, text) = self.request("POST", path, Some(&body), &[]);
        assert_eq!(status, 200, "{path}: {text}");
        serde_json::from_str(&text).unwrap()
    }
}

/// What the page sends: the My_Jira world, as JSON.
fn manifest() -> Value {
    let yaml = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/my-jira.world.yaml"),
    )
    .unwrap();
    let parsed: harness_types::WorldManifest = serde_yaml::from_str(&yaml).unwrap();
    serde_json::to_value(parsed).unwrap()
}

#[test]
fn requests_from_elsewhere_are_refused() {
    let dir = scratch_dir("security");
    let author = start(&dir.join("my.world.yaml"));
    let body = json!({ "manifest": manifest() });

    // DNS rebinding: right address, someone else's name.
    let (status, _) = author.request("GET", "/", None, &[("Host", "evil.example:80")]);
    assert_eq!(status, 403);
    // A POST without this run's token, or with a wrong one.
    let (status, _) = author.request(
        "POST",
        "/api/save",
        Some(&body),
        &[("X-Harness-Author", "")],
    );
    assert_eq!(status, 403);
    let (status, _) = author.request(
        "POST",
        "/api/save",
        Some(&body),
        &[("X-Harness-Author", "guess")],
    );
    assert_eq!(status, 403);
    // The token without the page's origin.
    let (status, _) = author.request(
        "POST",
        "/api/save",
        Some(&body),
        &[("Origin", "http://evil.example")],
    );
    assert_eq!(status, 403);
    assert!(!dir.join("my.world.yaml").exists(), "nothing was written");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn session_lists_the_upstream_and_starts_without_a_world() {
    let dir = scratch_dir("session");
    let author = start(&dir.join("my.world.yaml"));
    let (status, text) = author.request("GET", "/api/session", None, &[]);
    assert_eq!(status, 200);
    let s: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(s["world"], Value::Null);
    assert_eq!(s["world_stem"], json!("my"));
    assert_eq!(
        s["upstream"]["server_info"]["name"],
        json!("mock-jira-rovo")
    );
    let names: Vec<&str> = s["upstream"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"getJiraIssue"));
    assert!(names.contains(&"transitionJiraIssue"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn render_returns_yaml_and_the_surface_or_why_not() {
    let dir = scratch_dir("render");
    let author = start(&dir.join("my.world.yaml"));
    let r = author.post("/api/render", json!({ "manifest": manifest() }));
    assert_eq!(r["ok"], json!(true), "{r}");
    let yaml = r["yaml"].as_str().unwrap();
    assert!(yaml.contains("!Input"), "{yaml}");
    let names: Vec<&str> = r["preview"]["surface"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"get_platform_issue"));
    assert!(!names.contains(&"getJiraIssue"));

    // A manifest that does not compile says why, and is never "ok".
    let mut broken = manifest();
    broken["mcp_surface"]["upstream"] = json!("nowhere");
    let r = author.post("/api/render", json!({ "manifest": broken }));
    assert_eq!(r["ok"], json!(false));
    assert!(r["error"].as_str().unwrap().contains("nowhere"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn try_checks_without_sending_and_sends_only_when_asked() {
    let dir = scratch_dir("try");
    let author = start(&dir.join("my.world.yaml"));
    let try_call = |tool: &str, arguments: Value, send: bool| {
        author.post(
            "/api/try",
            json!({ "manifest": manifest(), "tool": tool, "arguments": arguments,
                    "taint": "clean", "send": send }),
        )
    };

    let checked = try_call("get_platform_issue", json!({ "issue": "PLAT-7" }), false);
    assert_eq!(checked["verdict"]["decision"], json!("ALLOW"));
    assert_eq!(checked["sent"], json!(false));
    assert!(checked.get("result").is_none());
    assert_eq!(
        checked["forwarded"],
        json!({ "tool": "getJiraIssue",
                "arguments": { "cloudId": "abc-123", "issueIdOrKey": "PLAT-7" } })
    );

    let sent = try_call("get_platform_issue", json!({ "issue": "PLAT-7" }), true);
    assert_eq!(sent["sent"], json!(true));
    let echoed: Value =
        serde_json::from_str(sent["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(echoed["received_tool"], json!("getJiraIssue"));
    assert_eq!(echoed["received_arguments"]["cloudId"], json!("abc-123"));

    let denied = try_call("get_platform_issue", json!({ "issue": "SEC-1" }), true);
    assert_eq!(denied["verdict"]["decision"], json!("DENY"));
    assert!(denied.get("forwarded").is_none() && denied.get("result").is_none());

    let refused = try_call(
        "getConfluencePage",
        json!({ "cloudId": "c", "pageId": "1" }),
        true,
    );
    assert_eq!(refused["verdict"]["decision"], json!("ALLOW"));
    assert!(refused["refused"].as_str().unwrap().contains("not served"));
    assert!(refused.get("result").is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn save_writes_a_world_the_gateway_serves() {
    let dir = scratch_dir("save");
    let world = dir.join("my.world.yaml");
    let author = start(&world);

    // A manifest that does not compile is not written.
    let mut broken = manifest();
    broken["scoped_capabilities"][0]["name"] = json!("searchJiraIssuesUsingJql");
    let r = author.post("/api/save", json!({ "manifest": broken }));
    assert_eq!(r["ok"], json!(false));
    assert!(!world.exists());

    let r = author.post("/api/save", json!({ "manifest": manifest() }));
    assert_eq!(r["ok"], json!(true), "{r}");
    let text = std::fs::read_to_string(&world).unwrap();
    assert!(text.starts_with("# World manifest written by `harness mcp-author`"));
    let saved = compiler::load_yaml(&text).expect("saved file loads");
    assert_eq!(serde_json::to_value(&saved).unwrap(), manifest());

    // Reopening the page finds what was saved.
    let (_, session) = author.request("GET", "/api/session", None, &[]);
    let session: Value = serde_json::from_str(&session).unwrap();
    assert_eq!(session["world"], manifest());

    // And the gateway serves it.
    let bin = env!("CARGO_BIN_EXE_harness");
    let mut gateway = Command::new(bin)
        .args(["mcp-gateway", "--world", world.to_str().unwrap(), "--", bin])
        .args(["mock-jira", "--rovo", "--echo"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    {
        let mut stdin = gateway.stdin.take().unwrap();
        writeln!(
            stdin,
            "{}",
            json!({"jsonrpc":"2.0","id":1,"method":"initialize"})
        )
        .unwrap();
        writeln!(
            stdin,
            "{}",
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})
        )
        .unwrap();
    }
    let mut out = String::new();
    gateway
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut out)
        .unwrap();
    let _ = gateway.wait();
    let lines: Vec<Value> = out
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines[0]["result"]["serverInfo"]["name"], json!("My_Jira"));
    let names: Vec<&str> = lines[1]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"get_platform_issue"), "{names:?}");
    let _ = std::fs::remove_dir_all(dir);
}
