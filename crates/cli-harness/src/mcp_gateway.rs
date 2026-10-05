//! `harness mcp-gateway` — a **governed MCP stdio server** that fronts a real
//! upstream MCP server, deciding every call with the *real* `world-kernel` via
//! `gate()` (DECISIONS D33 / E16.B). This is the Rust MCP projection shim (E13.4),
//! brought forward.
//!
//! ```text
//!  host (Copilot / Claude Code)  ⇄ stdio ⇄  [ gateway: gate() ]  ⇄ stdio ⇄  upstream MCP
//! ```
//!
//! - `tools/list` → the upstream's real tools, dropping any **not in the projected
//!   surface** (ABSENT — never offered to the model), and re-issuing each survivor
//!   from the **world's** descriptor rather than the upstream's advertisement, so a
//!   drifted or hostile server cannot bolt an extra argument onto an allowed tool
//!   (finding #14).
//! - `tools/call` → `gate()` decides; the call is forwarded **only on ALLOW**;
//!   DENY / ABSENT / ASK come back as an MCP tool error.
//! - a world that declares an `mcp_surface` (D78) is served by its backings: each
//!   scoped capability is offered under its own name with the schema the model
//!   supplies, the kernel's lowered `ExecutionSpec` is what goes upstream (renamed
//!   arguments mapped back, literals injected, extras stripped), and the gateway
//!   announces the surface's name. A world without one is served as before.
//! - an upstream result that *demands* input instead of answering (MCP `2026-07-28`
//!   MRTR `input_required`) is **refused, not relayed** — the D49 interim deny.
//! - every decision is appended to an optional JSONL audit log.
//!
//! It is pure plumbing around the kernel — no policy logic lives here. MCP over
//! stdio is hand-rolled (newline-delimited JSON-RPC), so no async runtime / SDK.

use compiler::{compile, loader::load_yaml};
use harness_preview::{
    gate_and_lower, host_outcome, GateContext, GateRequest, GateResponse, GateUsage, HostOutcome,
    ABI_VERSION,
};
use harness_types::{ActionName, BackingIdentity, CompiledWorld, ExecutionSpec, Operation};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use world_kernel::schema::model_facing_schema;

const PROTOCOL_VERSION: &str = "2024-11-05";

/// A minimal MCP stdio **client** to the spawned upstream child process.
struct Upstream {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: i64,
}

impl Upstream {
    fn spawn(cmd: &[String]) -> std::io::Result<Self> {
        let mut child = Command::new(&cmd[0])
            .args(&cmd[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let stdin = child.stdin.take().expect("upstream stdin");
        let stdout = BufReader::new(child.stdout.take().expect("upstream stdout"));
        let mut up = Upstream {
            child,
            stdin,
            stdout,
            next_id: 1,
        };
        up.initialize()?;
        Ok(up)
    }

    fn rpc(&mut self, method: &str, params: Value) -> std::io::Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let req = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        writeln!(self.stdin, "{req}")?;
        self.stdin.flush()?;
        loop {
            let mut line = String::new();
            if self.stdout.read_line(&mut line)? == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "upstream closed the connection",
                ));
            }
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let v: Value = serde_json::from_str(line)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            // Ignore notifications / responses to other ids.
            if v.get("id").and_then(|i| i.as_i64()) == Some(id) {
                return Ok(v);
            }
        }
    }

    fn initialize(&mut self) -> std::io::Result<()> {
        self.rpc(
            "initialize",
            json!({"protocolVersion": PROTOCOL_VERSION, "capabilities": {},
                   "clientInfo": {"name": "harness-mcp-gateway", "version": "0.1.0"}}),
        )?;
        writeln!(
            self.stdin,
            "{}",
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
        )?;
        self.stdin.flush()?;
        Ok(())
    }

    /// The upstream's **whole** `tools/list` result, not just its `tools` array.
    /// The gateway shapes the array in place and passes every sibling field
    /// through: rebuilding the result would silently drop fields the gateway does
    /// not model (`_meta`, and since protocol version 2026-07-28 the required
    /// `ttlMs` / `cacheScope`). Shaping the surface is policy; discarding
    /// protocol fields is data loss.
    fn list_result(&mut self) -> std::io::Result<Value> {
        let resp = self.rpc("tools/list", json!({}))?;
        Ok(resp.get("result").cloned().unwrap_or_else(|| json!({})))
    }

    /// Returns the upstream's `result` object (already an MCP tool result), or an
    /// `isError` result wrapping a JSON-RPC error.
    ///
    /// `meta` is the inbound request's `params._meta`, forwarded verbatim. Since
    /// protocol version 2026-07-28 that field carries the client's protocol
    /// version, identity, capabilities, log level and OpenTelemetry trace context;
    /// a proxy that drops it makes the upstream request non-conformant and severs
    /// trace propagation. Forwarding it widens nothing: `_meta` is set by the
    /// *host*, not proposed by the model — the kernel still governs only the tool
    /// name and its arguments.
    fn call_tool(
        &mut self,
        name: &str,
        arguments: &Value,
        meta: Option<&Value>,
    ) -> std::io::Result<Value> {
        let mut params = json!({"name": name, "arguments": arguments});
        if let (Some(meta), Some(obj)) = (meta, params.as_object_mut()) {
            obj.insert("_meta".to_string(), meta.clone());
        }
        let resp = self.rpc("tools/call", params)?;
        if let Some(result) = resp.get("result") {
            Ok(result.clone())
        } else if let Some(err) = resp.get("error") {
            Ok(json!({"isError": true, "content": [{"type": "text", "text": err.to_string()}]}))
        } else {
            Ok(json!({"content": []}))
        }
    }
}

impl Drop for Upstream {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// `source` is the proposer's channel (its *trust*); `tainted` is the carried,
/// monotonic session taint (the inbound floor). They're independent dimensions.
/// `mode` is threaded into every request so ASK fails closed in background.
fn govern(
    world: &CompiledWorld,
    tool: &str,
    args: &Value,
    source: &str,
    tainted: bool,
    mode: &str,
    usage: GateUsage,
) -> (GateResponse, Option<Result<ExecutionSpec, String>>) {
    let req = GateRequest {
        v: ABI_VERSION,
        tool: tool.to_string(),
        arguments: args.clone(),
        path: None, // MCP tools are not file paths — path scope does not apply here
        context: GateContext {
            session_id: "mcp-gateway".to_string(),
            mode: Some(mode.to_string()),
            taint: Some(if tainted { "tainted" } else { "clean" }.to_string()),
            source_channel: Some(source.to_string()),
            approval_token: None,
            // The gateway is one long-lived process, so it carries budget counters in
            // memory rather than through a sidecar (finding #16).
            usage: Some(usage),
        },
    };
    gate_and_lower(world, &req)
}

fn audit(path: Option<&Path>, entry: Value) {
    let Some(path) = path else { return };
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let mut record = entry;
    if let Some(obj) = record.as_object_mut() {
        obj.insert("ts_ms".to_string(), json!(ts));
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{record}");
    }
}

/// The upstream tool that serves a world action, or `None` when this gateway does
/// not serve it.
///
/// With an `mcp_surface` (D78) the backing is authoritative: an action is served
/// when its `McpServer` backing names the surface's upstream, under the backing's
/// tool name — so several scoped capabilities can wrap one upstream tool under
/// names of their own. Without one, the world names upstream tools directly by
/// action name, as every world did before D78; their backings were never read
/// here, and some name tools the upstream does not have.
fn served_tool(world: &CompiledWorld, action: &ActionName) -> Option<String> {
    let Some(surface) = world.mcp_surface() else {
        return Some(action.as_str().to_string());
    };
    match &world.descriptor(action)?.backing {
        BackingIdentity::McpServer { server, tool } if *server == surface.upstream => {
            Some(tool.clone())
        }
        _ => None,
    }
}

/// The surface the model is offered: every projected action whose serving
/// upstream tool the upstream actually advertises, in the upstream's order.
/// Returns the tools, how many had an upstream schema differing from the world's,
/// and how many upstream tools serve at least one of them (the rest are ABSENT).
fn surface_tools(world: &CompiledWorld, upstream: &[Value]) -> (Vec<Value>, usize, usize) {
    let mut by_tool: BTreeMap<String, Vec<&ActionName>> = BTreeMap::new();
    for action in world.projected_actions() {
        if let Some(tool) = served_tool(world, action) {
            by_tool.entry(tool).or_default().push(action);
        }
    }
    let mut exposed = Vec::new();
    let mut rewritten = 0usize;
    let mut served = 0usize;
    for offered in upstream {
        let Some(actions) = offered
            .get("name")
            .and_then(Value::as_str)
            .and_then(|n| by_tool.get(n))
        else {
            continue;
        };
        let before = exposed.len();
        for action in actions {
            if let Some((tool, changed)) = project_tool(world, action, offered) {
                rewritten += usize::from(changed);
                exposed.push(tool);
            }
        }
        served += usize::from(exposed.len() > before);
    }
    (exposed, rewritten, served)
}

/// Re-issue an offered tool from the **world's** descriptor instead of the
/// upstream's advertisement (finding #14). The upstream names the tool; the *world*
/// says what its arguments are — so a malicious or drifted server that bolts an
/// extra argument onto an allowed tool cannot get that argument in front of the
/// model. Returns the projected tool and whether the upstream's schema differed.
///
/// A scoped capability (D78) is offered under its own name, with the schema the
/// model supplies — renamed arguments, locked ones left out, limits stated — and,
/// when the world gives one, its own description.
///
/// Fail-closed: a tool the world cannot describe is dropped rather than passed
/// through, even though `projected` should already have excluded it.
///
/// Otherwise the `description` is still the upstream's. The manifest has no
/// description for a base action, and sending a bare name would leave the model
/// unable to use the tool at all. That leaves **prose-level** tool poisoning
/// unaddressed for base actions — a different vector from this finding (which is
/// about arguments), and the reason MCP `2026-07-28` says annotations from an
/// untrusted server should be treated as untrusted. Tracked separately; see the PR
/// for finding #14.
fn project_tool(
    world: &CompiledWorld,
    action: &ActionName,
    upstream: &Value,
) -> Option<(Value, bool)> {
    let descriptor = world.descriptor(action)?;
    let schema = model_facing_schema(world, action)?;
    let changed = upstream.get("inputSchema") != Some(&descriptor.schema);

    let mut tool = json!({ "name": action.as_str(), "inputSchema": schema });
    let description = world
        .scoped_capability(action)
        .and_then(|c| c.description.as_ref())
        .map(|d| Value::from(d.as_str()))
        .or_else(|| upstream.get("description").cloned());
    if let (Some(desc), Some(obj)) = (description, tool.as_object_mut()) {
        obj.insert("description".to_string(), desc);
    }
    Some((tool, changed))
}

/// What to send upstream for an allowed call: the tool name and its arguments.
///
/// With an `mcp_surface` (D78) this is the kernel's `ExecutionSpec` — the
/// backing tool, with renamed arguments mapped back, literals injected and
/// extras stripped — never the call as the model proposed it. A call the kernel
/// allowed but could not lower is refused. Without a surface the call is
/// forwarded as proposed, as before D78: the world names upstream tools directly
/// and declares no scoping the gateway would have to apply.
fn forward_target(
    world: &CompiledWorld,
    action: &ActionName,
    args: &Value,
    lowered: Option<Result<ExecutionSpec, String>>,
) -> Result<(String, Value), String> {
    let tool = served_tool(world, action)
        .ok_or_else(|| format!("`{action}` is not served by this gateway's upstream"))?;
    if world.mcp_surface().is_none() {
        return Ok((tool, args.clone()));
    }
    let spec = match lowered {
        Some(Ok(spec)) => spec,
        Some(Err(e)) => return Err(format!("the kernel could not lower the call: {e}")),
        None => return Err("the kernel produced no execution spec".to_string()),
    };
    match spec.operation() {
        Operation::Structured(op) if op.get("tool").and_then(Value::as_str) == Some(&tool) => {
            Ok((tool, op.get("input").cloned().unwrap_or_else(|| json!({}))))
        }
        _ => Err("the execution spec is not a call to this gateway's upstream".to_string()),
    }
}

/// Does this upstream result *demand* something rather than answer (MCP
/// `2026-07-28` MRTR)? Checks both the declared `resultType` and a bare
/// `inputRequests`, so a server that half-implements the shape is still caught.
fn demands_input(result: &Value) -> bool {
    result.get("resultType").and_then(|v| v.as_str()) == Some("input_required")
        || result.get("inputRequests").is_some()
}

fn rpc_result(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

pub fn run(
    world_path: &Path,
    upstream: &[String],
    source: &str,
    initial_taint: bool,
    mode: &str,
    audit_path: Option<&Path>,
) -> i32 {
    let content = match std::fs::read_to_string(world_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "mcp-gateway: cannot read world {}: {e}",
                world_path.display()
            );
            return 2;
        }
    };
    let manifest = match load_yaml(&content) {
        Ok(m) => m,
        Err(e) => {
            eprintln!(
                "mcp-gateway: cannot parse world {}: {e}",
                world_path.display()
            );
            return 2;
        }
    };
    let world = match compile(&manifest) {
        Ok(w) => w,
        Err(e) => {
            eprintln!(
                "mcp-gateway: cannot compile world {}: {e}",
                world_path.display()
            );
            return 2;
        }
    };
    let server_name = world
        .mcp_surface()
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "harness-mcp-gateway".to_string());

    let mut up = match Upstream::spawn(upstream) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("mcp-gateway: cannot start upstream {upstream:?}: {e}");
            return 2;
        }
    };

    // Monotonic session taint: starts at the inbound floor, only ever rises.
    let mut session_taint = initial_taint;
    let mut session_usage = GateUsage::default();

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[mcp-gateway] malformed JSON: {e}");
                continue;
            }
        };
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let id = req.get("id").cloned().unwrap_or(Value::Null);

        if method == "notifications/initialized" {
            continue;
        }

        let result: Value = match method {
            "initialize" => json!({
                "protocolVersion": PROTOCOL_VERSION,
                "serverInfo": {"name": server_name, "version": "0.1.0"},
                "capabilities": {"tools": {}}
            }),
            "tools/list" => {
                let mut result = match up.list_result() {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("[mcp-gateway] upstream tools/list failed: {e}");
                        json!({})
                    }
                };
                let tools: Vec<Value> = result
                    .get("tools")
                    .and_then(|t| t.as_array())
                    .cloned()
                    .unwrap_or_default();
                let advertised = tools.len();
                let (exposed, rewritten, served) = surface_tools(&world, &tools);
                // Surface-shaping ratio — the visible governability story: how much
                // of the upstream's advertised surface the world actually exposes.
                eprintln!(
                    "[mcp-gateway] surface shaped: upstream advertised {advertised} tools, \
                     exposing {} ({} ABSENT)",
                    exposed.len(),
                    advertised.saturating_sub(served)
                );
                if rewritten > 0 {
                    // Loud on purpose: the upstream described an allowed tool
                    // differently than the world does. Benign drift and an attempted
                    // schema poisoning look identical here, so say it either way.
                    eprintln!(
                        "[mcp-gateway] schema projected: {rewritten} tool(s) advertised an \
                         inputSchema differing from the world's — the world's was sent"
                    );
                }
                // Replace only the `tools` array; every sibling field the upstream
                // sent rides through untouched.
                match result.as_object_mut() {
                    Some(obj) => {
                        obj.insert("tools".to_string(), json!(exposed));
                        result
                    }
                    None => json!({"tools": exposed}),
                }
            }
            "tools/call" => {
                let params = req.get("params").cloned().unwrap_or_else(|| json!({}));
                let name = params
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .to_string();
                let args = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                let carried_usage = session_usage;
                let (verdict, lowered) = govern(
                    &world,
                    &name,
                    &args,
                    source,
                    session_taint,
                    mode,
                    session_usage,
                );
                session_usage = verdict.context.usage;
                let action = verdict.action.clone();
                let manifest_hash = verdict.manifest_hash.clone();
                audit(
                    audit_path,
                    json!({"tool": name, "action": action, "stage": "call",
                           "decision": verdict.decision,
                           "rule": verdict.rule.clone().unwrap_or_default(),
                           "manifest_hash": manifest_hash, "mode": mode,
                           "source": source, "taint_in": session_taint}),
                );
                // The gateway is fail-closed by design: an unevaluated or
                // non-ALLOW call is never forwarded upstream. ABSENT / ASK /
                // DENY / REPLAN stay distinguishable via the label prefix —
                // MCP's only structural channel is the isError text.
                match host_outcome(&verdict) {
                    HostOutcome::Proceed => {
                        let target =
                            forward_target(&world, &ActionName::new(&action), &args, lowered);
                        match target {
                            Err(why) => {
                                // Nothing runs, so nothing is ingested or spent: the
                                // gate's post-call taint and budget charge assumed
                                // the call would execute.
                                session_usage = carried_usage;
                                audit(
                                    audit_path,
                                    json!({"tool": name, "action": action, "stage": "forward",
                                           "decision": "DENY", "rule": "not_forwardable",
                                           "manifest_hash": manifest_hash, "mode": mode,
                                           "source": source, "taint_in": session_taint}),
                                );
                                eprintln!("[mcp-gateway] refused to forward `{name}`: {why}");
                                json!({"isError": true, "content": [{"type": "text",
                                    "text": format!("REFUSED (gateway): {why}")}]})
                            }
                            Ok((upstream_tool, upstream_args)) => {
                                // Monotonic escalation from the call's post-call taint.
                                if verdict.context.taint == "tainted" {
                                    session_taint = true;
                                }
                                match up.call_tool(
                                    &upstream_tool,
                                    &upstream_args,
                                    params.get("_meta"),
                                ) {
                                    Ok(r) if demands_input(&r) => {
                                        // D49 interim deny (issue #40). The upstream answered
                                        // with a *demand* rather than a result. Note what this
                                        // does and does not do: the call already happened, so
                                        // this protects the host from the demand, not the
                                        // upstream from the call.
                                        audit(
                                            audit_path,
                                            json!({"tool": name, "action": action, "stage": "result",
                                           "decision": "DENY",
                                           "rule": "mrtr_input_required_interim",
                                           "manifest_hash": manifest_hash, "mode": mode,
                                           "source": source, "taint_in": session_taint}),
                                        );
                                        eprintln!(
                                            "[mcp-gateway] refused an input_required result from \
                                     `{name}` (D49 interim deny)"
                                        );
                                        json!({"isError": true, "content": [{"type": "text", "text":
                                    "REFUSED (gateway, interim): the upstream answered with an MCP \
                                     `input_required` result — a demand that the host gather input \
                                     (an elicitation or an LLM completion) and resend. The kernel has \
                                     no verdict shape for a demand, so the gateway refuses to relay \
                                     one rather than pass it to the model unexamined. See D49 and \
                                     sv-pro/ai2rules#40."}]})
                                    }
                                    Ok(r) => r,
                                    Err(e) => json!({"isError": true,
                                "content": [{"type": "text", "text": format!("upstream error: {e}")}]}),
                                }
                            }
                        }
                    }
                    HostOutcome::NeedsApproval { reason } => json!({"isError": true,
                        "content": [{"type": "text", "text": format!("ASK: {reason}")}]}),
                    HostOutcome::Block { kind, reason } => json!({"isError": true,
                        "content": [{"type": "text",
                                     "text": format!("{}: {reason}", kind.label())}]}),
                }
            }
            other => {
                let err = json!({"jsonrpc": "2.0", "id": id,
                    "error": {"code": -32601, "message": format!("Method not found: {other}")}});
                let _ = writeln!(out, "{err}");
                let _ = out.flush();
                continue;
            }
        };

        let _ = writeln!(out, "{}", rpc_result(id, result));
        let _ = out.flush();
    }
    0
}
