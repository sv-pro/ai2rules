//! End-to-end test for `harness console` (DECISIONS D81, PLAN E15.7): the policy
//! console's server driven over real HTTP, with the real `harness cc-hook`
//! deciding calls against the world file the console writes. The point under
//! test is the promise the demo makes: a switch flipped on the page changes the
//! verdict of the agent's very next call, and the call shows up in the feed.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

struct Console {
    child: Child,
    port: u16,
    token: String,
}

impl Drop for Console {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A throwaway project governed by the policy-console demo world.
fn project(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "console-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(dir.join(".claude")).unwrap();
    std::fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/demos/policy-console/world.yaml"),
        world(&dir),
    )
    .unwrap();
    dir
}

fn world(project: &Path) -> PathBuf {
    project.join(".claude").join("cc-world.yaml")
}

fn log(project: &Path) -> PathBuf {
    project
        .join(".claude")
        .join("state")
        .join("decisions.jsonl")
}

fn start(project: &Path) -> Console {
    let mut child = Command::new(env!("CARGO_BIN_EXE_harness"))
        .args([
            "console",
            "--project",
            project.to_str().unwrap(),
            "--port",
            "0",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn console");
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
    let mut console = Console {
        child,
        port,
        token: String::new(),
    };
    let (status, page) = console.request("GET", "/", None, &[]);
    assert_eq!(status, 200);
    console.token = page
        .split(r#"name="harness-console-token" content=""#)
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("token in page")
        .to_string();
    assert_eq!(console.token.len(), 32);
    console
}

impl Console {
    /// One HTTP request. `headers` replace the defaults of the same name.
    fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
        headers: &[(&str, &str)],
    ) -> (u16, String) {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        let body = body.map(Value::to_string).unwrap_or_default();
        let mut head: Vec<(String, String)> = vec![
            ("Host".into(), format!("127.0.0.1:{}", self.port)),
            ("Content-Length".into(), body.len().to_string()),
            ("Connection".into(), "close".into()),
        ];
        if method == "POST" {
            head.push(("X-Harness-Console".into(), self.token.clone()));
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

    fn get(&self, path: &str) -> Value {
        let (status, text) = self.request("GET", path, None, &[]);
        assert_eq!(status, 200, "{path}: {text}");
        serde_json::from_str(&text).unwrap()
    }

    fn post(&self, path: &str, body: Value) -> Value {
        let (status, text) = self.request("POST", path, Some(&body), &[]);
        assert_eq!(status, 200, "{path}: {text}");
        serde_json::from_str(&text).unwrap()
    }
}

/// One call through the real hook, as the demo runs it: replace mode, ABSENT
/// enforced, decisions logged. Returns the hook's answer to the host.
fn hook(project: &Path, tool: &str, input: Value) -> Option<String> {
    hook_with(project, "demo", true, tool, input)
}

/// [`hook`] in a session of its own (taint is per session), with or without
/// `--enforce-absent`.
fn hook_with(
    project: &Path,
    session: &str,
    enforce_absent: bool,
    tool: &str,
    input: Value,
) -> Option<String> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_harness"))
        .args([
            "cc-hook",
            "--world",
            world(project).to_str().unwrap(),
            "--state",
            project.join(".claude").join("state").to_str().unwrap(),
            "--grant",
            "--log",
            log(project).to_str().unwrap(),
        ])
        .args(enforce_absent.then_some("--enforce-absent"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn cc-hook");
    let event = json!({"tool_name": tool, "tool_input": input, "session_id": session});
    child
        .stdin
        .take()
        .unwrap()
        .write_all(event.to_string().as_bytes())
        .unwrap();
    let out = String::from_utf8(child.wait_with_output().unwrap().stdout).unwrap();
    let line = out.lines().find(|l| l.contains("hookSpecificOutput"))?;
    let v: Value = serde_json::from_str(line).ok()?;
    v["hookSpecificOutput"]["permissionDecision"]
        .as_str()
        .map(String::from)
}

fn position(state: &Value, id: &str) -> String {
    state["switches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == id)
        .map(|s| s["position"].as_str().unwrap().to_string())
        .unwrap_or_else(|| panic!("no switch {id} in {state}"))
}

#[test]
fn requests_from_elsewhere_are_refused() {
    let dir = project("security");
    let before = std::fs::read_to_string(world(&dir)).unwrap();
    let console = start(&dir);
    let body = json!({ "switches": { "delete": "allow" } });

    // DNS rebinding: right address, someone else's name.
    let (status, _) = console.request("GET", "/api/state", None, &[("Host", "evil.example:80")]);
    assert_eq!(status, 403);
    // A POST without this run's token, with a wrong one, or from another origin.
    for headers in [
        [("X-Harness-Console", "")],
        [("X-Harness-Console", "guess")],
        [("Origin", "http://evil.example")],
    ] {
        let (status, _) = console.request("POST", "/api/policy", Some(&body), &headers);
        assert_eq!(status, 403, "{headers:?}");
    }
    assert_eq!(std::fs::read_to_string(world(&dir)).unwrap(), before);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn state_reads_the_switches_and_the_active_profile_from_the_world() {
    let dir = project("state");
    let console = start(&dir);
    let state = console.get("/api/state");
    assert_eq!(state["ok"], true, "{state}");
    assert_eq!(state["world_id"], "northwind-vendor-desk");
    let ids: Vec<&str> = state["switches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["web", "upload", "delete", "edit", "scripts"]);
    assert_eq!(position(&state, "web"), "allow");
    assert_eq!(position(&state, "upload"), "ask");
    let active: Vec<&str> = state["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["active"] == true)
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        active,
        ["developer"],
        "the shipped world is the Developer profile"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// The demo's promise, end to end: flip a switch, and the agent's very next call
/// gets a different verdict from the real hook, with both the change and the
/// call in the feed.
#[test]
fn a_switch_changes_the_very_next_verdict_and_both_reach_the_feed() {
    let dir = project("switch");
    let console = start(&dir);
    let delete = || hook(&dir, "Bash", json!({"command": "rm -rf archive"}));

    assert_eq!(delete().as_deref(), Some("ask"), "shipped: deleting asks");

    let r = console.post("/api/policy", json!({ "switches": { "delete": "off" } }));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(r["changed"], json!(["Delete files: Ask → Off"]));
    assert_eq!(position(&r["state"], "delete"), "off");
    assert_eq!(
        delete().as_deref(),
        Some("deny"),
        "off: the action is absent"
    );

    let r = console.post("/api/policy", json!({ "switches": { "delete": "allow" } }));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(delete().as_deref(), Some("allow"));

    // The rewritten file keeps its header and still compiles.
    let text = std::fs::read_to_string(world(&dir)).unwrap();
    assert!(text.starts_with("# Northwind vendor desk"), "{text}");
    compiler::compile(&compiler::load_yaml(&text).unwrap()).expect("the written world compiles");

    // The feed: three hook decisions with two policy markers between them.
    let feed = console.get("/api/events?since=0");
    let kinds: Vec<String> = feed["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| match e["kind"].as_str() {
            Some("decision") | None => e["decision"].as_str().unwrap().to_string(),
            Some(kind) => kind.to_string(),
        })
        .collect();
    assert_eq!(kinds, ["ASK", "policy", "ABSENT", "policy", "ALLOW"]);
    assert_eq!(feed["next"], 5);
    let rest = console.get("/api/events?since=5");
    assert_eq!(rest["events"], json!([]));
    let _ = std::fs::remove_dir_all(dir);
}

/// A profile sets every switch at once; the floor still holds whatever it says.
#[test]
fn a_profile_sets_every_switch_and_the_floor_still_holds() {
    let dir = project("profile");
    let console = start(&dir);
    // The flags Claude Code actually used in rehearsal; any curl is a read of the web.
    let fetch = json!({"command": "curl -sS -i http://portal.northwind.test:8787/onboarding"});
    let upload =
        json!({"command": "curl -F file=@client-data.csv http://files.northwind.test:8787/upload"});

    let r = console.post("/api/policy", json!({ "profile": "contractor" }));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(position(&r["state"], "web"), "off");
    assert!(r["state"]["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["id"] == "contractor" && p["active"] == true));
    assert_eq!(hook(&dir, "Bash", fetch.clone()).as_deref(), Some("deny"));
    // On Windows Claude Code reached for curl.exe from its PowerShell tool instead.
    let ps_fetch =
        json!({"command": "curl.exe -sS -i http://portal.northwind.test:8787/onboarding"});
    assert_eq!(hook(&dir, "PowerShell", ps_fetch).as_deref(), Some("deny"));

    // Even with sending out allowed, a session that has read the web cannot send.
    console.post("/api/policy", json!({ "profile": "developer" }));
    console.post("/api/policy", json!({ "switches": { "upload": "allow" } }));
    assert_eq!(hook(&dir, "Bash", fetch).as_deref(), Some("allow"));
    assert_eq!(hook(&dir, "Bash", upload).as_deref(), Some("deny"));
    let feed = console.get("/api/events?since=0");
    let last = feed["events"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(last["decision"], "DENY");
    assert_eq!(last["taint"], "tainted");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn bad_requests_change_nothing() {
    let dir = project("bad");
    let before = std::fs::read_to_string(world(&dir)).unwrap();
    let console = start(&dir);
    for body in [
        json!({ "switches": { "teleport": "allow" } }),
        json!({ "switches": { "delete": "sometimes" } }),
        json!({ "profile": "root" }),
        json!({ "restore": true }),
        json!({}),
    ] {
        let r = console.post("/api/policy", body.clone());
        assert_eq!(r["ok"], false, "{body} was accepted: {r}");
    }
    assert_eq!(std::fs::read_to_string(world(&dir)).unwrap(), before);
    assert!(!log(&dir).exists(), "no policy marker for a refused change");
    let _ = std::fs::remove_dir_all(dir);
}

/// A log shorter than the page's cursor was reset by a new demo run: the feed
/// starts over instead of waiting for lines that will never come.
#[test]
fn a_reset_log_restarts_the_feed() {
    let dir = project("reset");
    let console = start(&dir);
    hook(&dir, "Read", json!({"file_path": "notes.md"}));
    let feed = console.get("/api/events?since=40");
    assert_eq!(feed["reset"], true);
    assert_eq!(feed["events"].as_array().unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(dir);
}

/// The kernel's verdict on the last call in the log: what it decided, not what
/// the host was told (with `--grant`, ABSENT and DENY both reach it as `deny`).
fn last_decision(project: &Path) -> String {
    let text = std::fs::read_to_string(log(project)).unwrap();
    let line: Value = serde_json::from_str(text.lines().last().unwrap()).unwrap();
    line["decision"].as_str().unwrap_or("ALLOW").to_string()
}

/// What Stop has to take away: an edit, a delete, a command the world cannot
/// place, and a read of the web. Each round runs in its own session, so the
/// web read in one round does not taint the next.
fn effectful_verdicts(project: &Path, session: &str) -> Vec<String> {
    [
        ("Write", json!({"file_path": "notes.md", "content": "x"})),
        ("Bash", json!({"command": "rm -rf archive"})),
        ("Bash", json!({"command": "touch notes.md"})),
        (
            "WebFetch",
            json!({"url": "http://portal.northwind.test:8787/onboarding"}),
        ),
    ]
    .into_iter()
    .map(|(tool, input)| {
        hook_with(project, session, true, tool, input);
        last_decision(project)
    })
    .collect()
}

/// G7 (D85): Stop takes every effectful action away at the next call, says
/// "stopped" only once the hook has refused one, and Restore puts back exactly
/// what was there, with both moments in the log.
#[test]
fn stop_takes_every_effectful_action_away_and_restore_gives_it_back() {
    let dir = project("stop");
    let console = start(&dir);
    let developer = ["ALLOW", "ASK", "ASK", "ALLOW"];
    assert_eq!(effectful_verdicts(&dir, "before"), developer);

    let r = console.post("/api/policy", json!({ "profile": "stop" }));
    assert_eq!(r["ok"], true, "{r}");
    assert!(
        r["changed"]
            .as_array()
            .unwrap()
            .contains(&json!("Commands it cannot place: Ask → Off")),
        "{r}"
    );
    let stop = &r["state"]["stop"];
    assert_eq!(stop["status"], "pending", "no call refused yet: {stop}");
    assert_eq!(stop["restore_to"], "Developer");

    assert_eq!(
        effectful_verdicts(&dir, "stopped"),
        ["ABSENT", "ABSENT", "ABSENT", "ABSENT"]
    );
    // Read actions remain available. Process actions, including the shell's
    // read-looking commands, are stopped according to the manifest's effects.
    hook_with(
        &dir,
        "stopped",
        true,
        "Read",
        json!({"file_path": "notes.md"}),
    );
    assert_eq!(last_decision(&dir), "ALLOW");
    hook_with(&dir, "stopped", true, "Bash", json!({"command": "ls -la"}));
    assert_eq!(last_decision(&dir), "ABSENT");

    let stop = console.get("/api/state")["stop"].clone();
    assert_eq!(stop["status"], "stopped", "{stop}");
    assert!(stop["refused_ms"].as_u64().is_some(), "{stop}");
    assert_eq!(stop["enforced"], true);

    let r = console.post("/api/policy", json!({ "restore": true }));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(r["state"]["stop"]["status"], "off");
    assert_eq!(r["state"]["stop"]["restore_to"], Value::Null);
    let active: Vec<&Value> = r["state"]["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["active"] == true)
        .map(|p| &p["id"])
        .collect();
    assert_eq!(active, [&json!("developer")]);
    assert_eq!(effectful_verdicts(&dir, "restored"), developer);

    // Both moments are in the log, and the Stop marker keeps what it replaced.
    let feed = console.get("/api/events?since=0");
    let markers: Vec<&Value> = feed["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["kind"] == "policy")
        .collect();
    assert_eq!(markers.len(), 2, "{feed}");
    assert_eq!(
        markers[0]["headline"],
        "Stopped: every effectful action off"
    );
    assert_eq!(
        markers[0]["stop"]["before"]["Bash_destructive"]["approval_required"],
        true
    );
    assert_eq!(
        markers[0]["stop"]["before"]["Bash_unclassified"]["approval_required"],
        true
    );
    assert_eq!(markers[1]["headline"], "Restored: Developer");
    assert_eq!(markers[1]["restore"], true);
    assert!(markers[1]["ts_ms"].as_u64() >= markers[0]["ts_ms"].as_u64());

    // Restored once; there is nothing left to restore.
    let r = console.post("/api/policy", json!({ "restore": true }));
    assert_eq!(r["ok"], false, "{r}");
    let _ = std::fs::remove_dir_all(dir);
}

/// Without `--enforce-absent`, Off is left to the host's own prompt, so Stop must
/// not claim the agent is stopped: the hook script warns before any call, and the
/// first ABSENT the hook lets through proves it.
#[test]
fn stop_never_claims_stopped_when_the_hook_does_not_enforce_absent() {
    let dir = project("stop-unenforced");
    let shim = dir.join(".claude").join("hooks").join("world-gate.sh");
    std::fs::create_dir_all(shim.parent().unwrap()).unwrap();
    std::fs::write(
        &shim,
        "#!/usr/bin/env bash
exec \"$BIN\" cc-hook --grant --world \"$PD/.claude/cc-world.yaml\" --state \"$PD/.claude/state\"
",
    )
    .unwrap();
    let console = start(&dir);
    let stop = console.get("/api/state")["stop"].clone();
    assert_eq!(stop["status"], "off");
    assert_eq!(stop["enforced"], false, "{stop}");
    assert_eq!(stop["enforced_from"], "hook script");

    let r = console.post("/api/policy", json!({ "profile": "stop" }));
    assert_eq!(r["state"]["stop"]["status"], "not_binding", "{r}");

    // The call goes through to the host: the kernel says ABSENT, the hook is silent.
    let answer = hook_with(
        &dir,
        "demo",
        false,
        "Write",
        json!({"file_path": "notes.md", "content": "x"}),
    );
    assert_eq!(answer, None);
    assert_eq!(last_decision(&dir), "ABSENT");
    let stop = console.get("/api/state")["stop"].clone();
    assert_eq!(stop["status"], "not_binding", "{stop}");
    assert_eq!(stop["enforced_from"], "log");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn stop_covers_custom_effects_refuses_effectful_scopes_and_restores_exactly() {
    use harness_types::{ActionName, SideEffectClass};
    let dir = project("stop-custom");
    let mut manifest = compiler::load_yaml(&std::fs::read_to_string(world(&dir)).unwrap()).unwrap();
    let template = manifest
        .base_actions
        .iter()
        .find(|a| a.name.as_str() == "Write")
        .unwrap()
        .clone();
    for effect in [
        SideEffectClass::FilesystemWrite,
        SideEffectClass::PersistentWrite,
        SideEffectClass::Process,
        SideEffectClass::Network,
        SideEffectClass::Credential,
        SideEffectClass::Memory,
        SideEffectClass::External,
    ] {
        let mut action = template.clone();
        action.name = ActionName::new(format!("Custom{effect:?}"));
        action.side_effect = effect;
        manifest.base_actions.push(action);
    }
    let mut hidden = template;
    hidden.name = ActionName::new("HiddenWrite");
    hidden.projected = false;
    hidden.approval_required = true;
    manifest.base_actions.push(hidden);
    let mut pure = manifest
        .base_actions
        .iter()
        .find(|a| a.name.as_str() == "Read")
        .unwrap()
        .clone();
    pure.name = ActionName::new("Pure");
    pure.side_effect = SideEffectClass::None;
    manifest.base_actions.push(pure);
    for (name, base) in [("ScopedWrite", "HiddenWrite"), ("ScopedRead", "Read")] {
        manifest.scoped_capabilities.push(
            serde_json::from_value(json!({
                "name": name, "base_action": base, "args": {}, "description": "keep me on Restore"
            }))
            .unwrap(),
        );
    }
    compiler::compile(&manifest).unwrap();
    std::fs::write(world(&dir), serde_yaml::to_string(&manifest).unwrap()).unwrap();
    let console = start(&dir);
    let original_text = std::fs::read_to_string(world(&dir)).unwrap();
    let r = console.post("/api/policy", json!({"profile": "stop"}));
    assert_eq!(
        r["ok"], false,
        "effectful scopes must never survive a successful Stop: {r}"
    );
    assert!(r["error"].as_str().unwrap().contains("ScopedWrite"));
    assert_eq!(std::fs::read_to_string(world(&dir)).unwrap(), original_text);
    assert!(!log(&dir).exists(), "refused Stop must not write a marker");
    let state = console.get("/api/state");
    assert_eq!(state["stop"]["status"], "off");
    assert!(state["stop"]["unsupported"]
        .as_str()
        .unwrap()
        .contains("ScopedWrite"));
    // Read-only scopes are supported and stay unchanged, including definitions.
    manifest
        .scoped_capabilities
        .retain(|c| c.name.as_str() != "ScopedWrite");
    std::fs::write(world(&dir), serde_yaml::to_string(&manifest).unwrap()).unwrap();
    let r = console.post("/api/policy", json!({"profile": "stop"}));
    assert_eq!(r["ok"], true, "{r}");
    let stopped = compiler::load_yaml(&std::fs::read_to_string(world(&dir)).unwrap()).unwrap();
    let compiled = compiler::compile(&stopped).unwrap();
    for a in &manifest.base_actions {
        let safe = matches!(a.side_effect, SideEffectClass::None | SideEffectClass::Read);
        assert_eq!(
            compiled.is_projected(&a.name),
            safe && a.projected,
            "{} survives Stop",
            a.name
        );
    }
    assert!(!compiled.is_projected(&ActionName::new("ScopedWrite")));
    assert!(compiled.is_projected(&ActionName::new("ScopedRead")));
    for tool in ["CustomFilesystemWrite", "HiddenWrite"] {
        hook_with(
            &dir,
            "custom",
            true,
            tool,
            json!({"file_path": "notes.md", "content": "x"}),
        );
        assert_eq!(last_decision(&dir), "ABSENT", "{tool}");
    }
    assert_eq!(console.get("/api/state")["stop"]["status"], "stopped");
    // A second Stop and a server restart must not overwrite the original snapshot.
    assert_eq!(
        console.post("/api/policy", json!({"profile": "stop"}))["ok"],
        true
    );
    drop(console);
    let console = start(&dir);
    let r = console.post("/api/policy", json!({"restore": true}));
    assert_eq!(r["ok"], true, "{r}");
    let restored = compiler::load_yaml(&std::fs::read_to_string(world(&dir)).unwrap()).unwrap();
    assert_eq!(
        restored, manifest,
        "Restore must preserve hidden approval flags and scoped definitions"
    );
    drop(console);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn stop_detects_and_writes_effects_outside_unchanged_switches() {
    use harness_types::ActionName;
    let dir = project("stop-uncovered");
    let console = start(&dir);
    assert_eq!(
        console.post("/api/policy", json!({"profile": "stop"}))["ok"],
        true
    );
    hook_with(
        &dir,
        "evidence",
        true,
        "Write",
        json!({"file_path": "notes.md", "content": "x"}),
    );
    assert_eq!(console.get("/api/state")["stop"]["status"], "stopped");
    let mut changed = compiler::load_yaml(&std::fs::read_to_string(world(&dir)).unwrap()).unwrap();
    let mut custom = changed
        .base_actions
        .iter()
        .find(|a| a.name.as_str() == "Write")
        .unwrap()
        .clone();
    custom.name = ActionName::new("CustomWrite");
    custom.projected = true;
    changed.base_actions.push(custom);
    std::fs::write(world(&dir), serde_yaml::to_string(&changed).unwrap()).unwrap();
    let state = console.get("/api/state");
    assert_eq!(
        state["stop"]["status"], "off",
        "old refusal cannot prove a modified world is stopped"
    );
    assert!(!state["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["id"] == "stop" && p["active"] == true));
    let r = console.post("/api/policy", json!({"profile": "stop"}));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(
        r["state"]["stop"]["status"], "pending",
        "the new Stop needs fresh evidence"
    );
    hook_with(
        &dir,
        "new-stop",
        true,
        "CustomWrite",
        json!({"file_path": "notes.md", "content": "x"}),
    );
    assert_eq!(
        last_decision(&dir),
        "ABSENT",
        "must write even when no fixed switch changes"
    );
    assert_eq!(
        console.post("/api/policy", json!({"restore": true}))["ok"],
        true
    );
    assert_eq!(
        compiler::load_yaml(&std::fs::read_to_string(world(&dir)).unwrap()).unwrap(),
        changed
    );
    // A capability over a hidden effectful base must also invalidate Stop.
    assert_eq!(
        console.post("/api/policy", json!({"profile": "stop"}))["ok"],
        true
    );
    hook_with(
        &dir,
        "scope-evidence",
        true,
        "Write",
        json!({"file_path": "notes.md", "content": "x"}),
    );
    let mut with_scope =
        compiler::load_yaml(&std::fs::read_to_string(world(&dir)).unwrap()).unwrap();
    with_scope.scoped_capabilities.push(
        serde_json::from_value(json!({
            "name": "ScopedWrite", "base_action": "CustomWrite", "args": {}
        }))
        .unwrap(),
    );
    std::fs::write(world(&dir), serde_yaml::to_string(&with_scope).unwrap()).unwrap();
    let state = console.get("/api/state");
    assert_eq!(state["stop"]["status"], "off");
    assert!(state["stop"]["unsupported"]
        .as_str()
        .unwrap()
        .contains("ScopedWrite"));
    assert_eq!(
        console.post("/api/policy", json!({"profile": "stop"}))["ok"],
        false
    );
    drop(console);
    let _ = std::fs::remove_dir_all(dir);
}
