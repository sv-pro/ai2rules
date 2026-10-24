//! `harness console` — the policy console (PLAN E15.7, DECISIONS D81): the
//! playground, connected to a live agent.
//!
//! It serves a local page for one project governed by `harness cc-hook`. The
//! page's switches and profiles rewrite the project's world file; cc-hook reads
//! that file on every call, so the agent behaves differently on its very next
//! tool call, with no restart. The hook's decision log (`cc-hook --log`) comes
//! back as the page's live feed.
//!
//! The page holds no governance logic (D18, D22). A switch names a group of
//! actions; this server sets those actions' `projected` and `approval_required`
//! on the real `WorldManifest`, renders it with the real serializer, compiles it
//! with the real compiler, and writes it only if it compiles. The kernel still
//! decides every call. Switching a group **off** removes it from the agent's world
//! (ABSENT, not DENY), which binds only when the hook runs with `--enforce-absent`.
//!
//! What no switch reaches: once a session has read from outside, nothing it does
//! may reach the network. The kernel enforces that floor whatever the manifest
//! says, so the page pins it, with the other kernel invariants a switch cannot
//! change, in a band above the policy and lights it when a verdict comes from it.
//!
//! The write is atomic (temp file + rename) because cc-hook fails open on a world
//! it cannot compile: a half-written file must never be visible to it.
//!
//! Endpoints (all JSON except `/`), guarded as `mcp-author`'s are (`local_page`):
//! - `GET  /`                 → the page, with this run's token
//! - `GET  /api/state`        → each switch's position, the profiles, the paths
//! - `POST /api/policy`       → `{switches: {id: "allow"|"ask"|"off"}}` or
//!   `{profile: id}` → applied, compiled, written; the new state
//! - `GET  /api/events?since=N` → decision-log lines from line N on

use std::collections::BTreeMap;
use std::io::{self, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};

use compiler::{compile, load_yaml};
use harness_types::WorldManifest;
use serde_json::{json, Value};

use crate::local_page::{guard, json_body, random_token, serve_one, write_atomically};
use crate::local_page::{Request, Response};

const PAGE: &str = include_str!("console.html");
/// Where the page's token goes; replaced on every `GET /`.
const TOKEN_SLOT: &str = "__HARNESS_CONSOLE_TOKEN__";
/// The header every POST must carry, holding this run's token.
const TOKEN_HEADER: &str = "x-harness-console";
/// At most this many log lines per `/api/events` answer; the page asks again.
const EVENTS_PER_CALL: usize = 500;

/// A switch: a group of actions the operator allows, makes ask, or turns off.
/// The action names are the ones the policy-console world declares
/// (`docs/demos/policy-console/world.yaml`); a world without any of them simply
/// does not offer that switch.
struct Switch {
    id: &'static str,
    label: &'static str,
    actions: &'static [&'static str],
}

const SWITCHES: &[Switch] = &[
    Switch {
        id: "web",
        label: "Read the web",
        actions: &["WebFetch", "WebSearch", "Bash_fetch", "PowerShell_fetch"],
    },
    Switch {
        id: "upload",
        label: "Send data out",
        actions: &["Bash_upload", "PowerShell_upload"],
    },
    Switch {
        id: "delete",
        label: "Delete files",
        actions: &["Bash_destructive", "PowerShell_destructive"],
    },
    Switch {
        id: "edit",
        label: "Edit project files",
        actions: &["Edit", "Write", "MultiEdit", "NotebookEdit"],
    },
    Switch {
        id: "scripts",
        label: "Run scripts",
        actions: &["Bash_script"],
    },
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Position {
    Allow,
    Ask,
    Off,
}

impl Position {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "allow" => Some(Position::Allow),
            "ask" => Some(Position::Ask),
            "off" => Some(Position::Off),
            _ => None,
        }
    }
    fn as_str(self) -> &'static str {
        match self {
            Position::Allow => "allow",
            Position::Ask => "ask",
            Position::Off => "off",
        }
    }
    fn word(self) -> &'static str {
        match self {
            Position::Allow => "Allow",
            Position::Ask => "Ask",
            Position::Off => "Off",
        }
    }
}

/// A profile: a named set of positions, in [`SWITCHES`] order.
struct Profile {
    id: &'static str,
    label: &'static str,
    blurb: &'static str,
    positions: [Position; 5],
}

const PROFILES: &[Profile] = {
    use Position::{Allow, Ask, Off};
    &[
        Profile {
            id: "contractor",
            label: "Contractor",
            blurb: "Works on the files it is given. No web, no scripts, nothing leaves.",
            positions: [Off, Off, Off, Ask, Off],
        },
        Profile {
            id: "finance",
            label: "Finance",
            blurb: "Reads vendor portals and edits records. Sending out asks first.",
            positions: [Allow, Ask, Off, Allow, Off],
        },
        Profile {
            id: "developer",
            label: "Developer",
            blurb: "Full toolset. Deleting and sending out ask first.",
            positions: [Allow, Ask, Ask, Allow, Allow],
        },
    ]
};

struct Console {
    world_path: PathBuf,
    log_path: PathBuf,
    token: String,
    port: u16,
}

/// Start the console (blocks). `port` 0 picks a free port; the chosen URL is the
/// first line printed.
pub fn run(project: &Path, world: Option<&Path>, log: Option<&Path>, port: u16) -> i32 {
    let world_path = world
        .map(Path::to_path_buf)
        .unwrap_or_else(|| project.join(".claude").join("cc-world.yaml"));
    let log_path = log.map(Path::to_path_buf).unwrap_or_else(|| {
        project
            .join(".claude")
            .join("state")
            .join("decisions.jsonl")
    });
    if let Err(e) = load_world(&world_path) {
        eprintln!("console: {e}");
        return 2;
    }
    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("console: cannot listen on 127.0.0.1:{port}: {e}");
            return 2;
        }
    };
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(port);
    let console = Console {
        world_path,
        log_path,
        token: random_token(),
        port,
    };
    // Written, not printed: `println!` panics if stdout has gone away (piped into
    // `head`, or a test that reads only the URL), and the page must outlive that.
    let mut out = io::stdout();
    let _ = writeln!(
        out,
        "Policy console: http://127.0.0.1:{port}/  (Ctrl-C to stop)"
    );
    let _ = writeln!(out, "World: {}", console.world_path.display());
    let _ = writeln!(out, "Log:   {}", console.log_path.display());
    let _ = out.flush();
    for stream in listener.incoming().flatten() {
        // One bad connection shouldn't take down the console.
        let _ = serve_one(stream, |req| route(&console, req));
    }
    0
}

fn route(console: &Console, req: &Request) -> Response {
    if let Some(refusal) = guard(req, console.port, TOKEN_HEADER, &console.token) {
        return refusal;
    }
    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/") => Response::html(PAGE.replace(TOKEN_SLOT, &console.token)),
        ("GET", "/api/state") => Response::json("200 OK", state(console)),
        ("GET", "/api/events") => {
            let since = req
                .query_param("since")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            Response::json("200 OK", events(&console.log_path, since))
        }
        ("POST", "/api/policy") => match json_body(req) {
            Ok(body) => Response::json("200 OK", policy(console, &body)),
            Err(bad) => bad,
        },
        _ => Response::not_found(),
    }
}

fn load_world(path: &Path) -> Result<(String, WorldManifest), String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read the world file {}: {e}", path.display()))?;
    let manifest = load_yaml(&text)
        .map_err(|e| format!("the world file {} does not parse: {e}", path.display()))?;
    Ok((text, manifest))
}

/// Where a switch stands in `manifest`: `None` when the world declares none of
/// its actions, `Some(None)` when its actions disagree ("mixed").
fn position_of(manifest: &WorldManifest, switch: &Switch) -> Option<Option<Position>> {
    let mut seen = manifest
        .base_actions
        .iter()
        .filter(|a| switch.actions.contains(&a.name.as_str()))
        .map(|a| match (a.projected, a.approval_required) {
            (false, _) => Position::Off,
            (true, true) => Position::Ask,
            (true, false) => Position::Allow,
        });
    let first = seen.next()?;
    Some(seen.all(|p| p == first).then_some(first))
}

fn positions(manifest: &WorldManifest) -> BTreeMap<&'static str, Option<Option<Position>>> {
    SWITCHES
        .iter()
        .map(|s| (s.id, position_of(manifest, s)))
        .collect()
}

fn state(console: &Console) -> Value {
    let manifest = match load_world(&console.world_path) {
        Ok((_, m)) => m,
        Err(e) => return json!({ "ok": false, "error": e }),
    };
    let current = positions(&manifest);
    let switches: Vec<Value> = SWITCHES
        .iter()
        .filter_map(|s| {
            let at = current.get(s.id).copied().flatten()?;
            Some(json!({
                "id": s.id,
                "label": s.label,
                "position": at.map(Position::as_str).unwrap_or("mixed"),
                "actions": manifest
                    .base_actions
                    .iter()
                    .filter(|a| s.actions.contains(&a.name.as_str()))
                    .map(|a| a.name.as_str())
                    .collect::<Vec<_>>(),
            }))
        })
        .collect();
    let profiles: Vec<Value> = PROFILES
        .iter()
        .map(|p| {
            let matches = SWITCHES.iter().zip(p.positions).all(|(s, want)| {
                matches!(current.get(s.id), Some(Some(Some(at))) if *at == want)
                    || matches!(current.get(s.id), Some(None))
            });
            json!({
                "id": p.id,
                "label": p.label,
                "blurb": p.blurb,
                "active": matches,
                "positions": SWITCHES
                    .iter()
                    .zip(p.positions)
                    .map(|(s, at)| (s.id.to_string(), json!(at.as_str())))
                    .collect::<serde_json::Map<_, _>>(),
            })
        })
        .collect();
    json!({
        "ok": true,
        "world_id": manifest.world_id.as_str(),
        "world_path": console.world_path.display().to_string(),
        "log_path": console.log_path.display().to_string(),
        "switches": switches,
        "profiles": profiles,
    })
}

/// Apply `{switches}` or `{profile}`, compile, write atomically, and record what
/// changed in the decision log so the feed shows cause next to effect.
fn policy(console: &Console, body: &Value) -> Value {
    let (text, mut manifest) = match load_world(&console.world_path) {
        Ok(v) => v,
        Err(e) => return json!({ "ok": false, "error": e }),
    };
    let profile = match body["profile"].as_str() {
        Some(id) => match PROFILES.iter().find(|p| p.id == id) {
            Some(p) => Some(p),
            None => return json!({ "ok": false, "error": format!("no profile {id:?}") }),
        },
        None => None,
    };
    let mut wanted: Vec<(&Switch, Position)> = Vec::new();
    if let Some(p) = profile {
        wanted.extend(SWITCHES.iter().zip(p.positions));
    } else if let Some(map) = body["switches"].as_object() {
        for (id, at) in map {
            let Some(switch) = SWITCHES.iter().find(|s| s.id == id) else {
                return json!({ "ok": false, "error": format!("no switch {id:?}") });
            };
            let Some(at) = at.as_str().and_then(Position::parse) else {
                return json!({ "ok": false, "error": format!("{id}: say allow, ask or off") });
            };
            wanted.push((switch, at));
        }
    } else {
        return json!({ "ok": false, "error": "send {switches: {...}} or {profile: id}" });
    }

    let before = positions(&manifest);
    let mut changes = Vec::new();
    for (switch, at) in &wanted {
        let Some(was) = before.get(switch.id).copied().flatten() else {
            continue; // this world does not offer the switch
        };
        if was == Some(*at) {
            continue;
        }
        for action in manifest
            .base_actions
            .iter_mut()
            .filter(|a| switch.actions.contains(&a.name.as_str()))
        {
            action.projected = *at != Position::Off;
            action.approval_required = *at == Position::Ask;
        }
        let was = was.map(Position::word).unwrap_or("Mixed");
        changes.push(format!("{}: {was} → {}", switch.label, at.word()));
    }
    if changes.is_empty() {
        return json!({ "ok": true, "changed": [], "state": state(console) });
    }

    let yaml = match render(&manifest) {
        Ok(y) => y,
        Err(e) => return json!({ "ok": false, "error": e }),
    };
    if let Err(e) = compile(&manifest) {
        return json!({ "ok": false, "error": format!("not written — compile error: {e}") });
    }
    let written = format!("{}{yaml}", leading_comments(&text));
    if let Err(e) = write_atomically(&console.world_path, &written) {
        return json!({ "ok": false, "error": format!("cannot write the world file: {e}") });
    }
    let headline = profile.map(|p| format!("Profile: {}", p.label));
    record_policy(&console.log_path, headline.as_deref(), &changes);
    json!({ "ok": true, "changed": changes, "state": state(console) })
}

/// Render through the real serializer and reload: a serializer quirk must never
/// write a file that means something else.
fn render(manifest: &WorldManifest) -> Result<String, String> {
    let yaml = serde_yaml::to_string(manifest).map_err(|e| format!("cannot render: {e}"))?;
    let reloaded = load_yaml(&yaml).map_err(|e| format!("rendered YAML does not reload: {e}"))?;
    if &reloaded != manifest {
        return Err("rendered YAML does not reload to the same manifest".to_string());
    }
    Ok(yaml)
}

/// The comment block at the top of the file, kept across rewrites: it is the only
/// prose a reader of the world file gets, and the serializer would drop it.
fn leading_comments(text: &str) -> String {
    text.lines()
        .take_while(|l| l.starts_with('#') || l.trim().is_empty())
        .map(|l| format!("{l}\n"))
        .collect()
}

/// Append a policy marker to the decision log. Like the hook's own lines it is
/// observability: the world file is already written, whatever happens here.
fn record_policy(log: &Path, headline: Option<&str>, changes: &[String]) {
    let ts_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let line = json!({
        "ts_ms": ts_ms,
        "kind": "policy",
        "headline": headline,
        "changes": changes,
    });
    if let Some(dir) = log.parent().filter(|d| !d.as_os_str().is_empty()) {
        let _ = std::fs::create_dir_all(dir);
    }
    let written = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .and_then(|mut f| writeln!(f, "{line}"));
    if let Err(e) = written {
        eprintln!("console: decision log {} not written: {e}", log.display());
    }
}

/// Log lines from line `since` on. A log shorter than `since` was reset (a new
/// run of the demo), so the answer starts over and says so.
fn events(log: &Path, since: usize) -> Value {
    let text = match std::fs::read_to_string(log) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return json!({ "ok": false, "error": format!("cannot read the log: {e}") }),
    };
    let lines: Vec<&str> = text.lines().collect();
    let (from, reset) = if since > lines.len() {
        (0, true)
    } else {
        (since, false)
    };
    let to = (from + EVENTS_PER_CALL).min(lines.len());
    // Outcome lines (D84) say how an already-shown call ended; the feed counts
    // verdicts, so they would read as extra ALLOWs.
    let events: Vec<Value> = lines[from..to]
        .iter()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|ev| ev["kind"] != "outcome")
        .collect();
    json!({ "ok": true, "next": to, "reset": reset, "events": events })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo_world() -> WorldManifest {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/demos/policy-console/world.yaml");
        load_world(&path).expect("the demo world loads").1
    }

    #[test]
    fn the_demo_world_offers_every_switch_with_a_clear_position() {
        let at = positions(&demo_world());
        for s in SWITCHES {
            assert!(
                matches!(at.get(s.id), Some(Some(Some(_)))),
                "{} is missing or mixed in the demo world",
                s.id
            );
        }
    }

    #[test]
    fn every_profile_compiles_and_reads_back_as_itself() {
        for p in PROFILES {
            let mut m = demo_world();
            for (s, at) in SWITCHES.iter().zip(p.positions) {
                for a in m
                    .base_actions
                    .iter_mut()
                    .filter(|a| s.actions.contains(&a.name.as_str()))
                {
                    a.projected = at != Position::Off;
                    a.approval_required = at == Position::Ask;
                }
            }
            compile(&m).unwrap_or_else(|e| panic!("{} does not compile: {e}", p.id));
            render(&m).unwrap_or_else(|e| panic!("{} does not render: {e}", p.id));
            let back = positions(&m);
            for (s, at) in SWITCHES.iter().zip(p.positions) {
                assert_eq!(back.get(s.id), Some(&Some(Some(at))), "{} / {}", p.id, s.id);
            }
        }
    }

    #[test]
    fn leading_comments_keep_the_header_and_stop_at_the_first_key() {
        let text = "# one\n#\n\n# two\nworld_id: w\n# not header\n";
        assert_eq!(leading_comments(text), "# one\n#\n\n# two\n");
    }
}
