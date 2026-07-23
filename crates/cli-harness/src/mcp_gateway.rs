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
//!   surface** (ABSENT — never offered to the model).
//! - `tools/call` → `gate()` decides; the call is forwarded **only on ALLOW**;
//!   DENY / ABSENT / ASK come back as an MCP tool error.
//! - every decision is appended to an optional JSONL audit log.
//!
//! It is pure plumbing around the kernel — no policy logic lives here. MCP over
//! stdio is hand-rolled (newline-delimited JSON-RPC), so no async runtime / SDK.

use compiler::{compile, loader::load_yaml};
use harness_preview::{
    gate, host_outcome, GateContext, GateRequest, GateResponse, HostOutcome, ABI_VERSION,
};
use harness_types::{
    ActionName, ApprovalToken, ApprovalTokenId, CompiledWorld, ContentHash, EffectMode, Provenance,
    SessionId, SourceChannel,
};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use trace_store::{params_hash, ApprovalStore};

/// The gateway forwards approved calls as real upstream calls; the binding's
/// effect mode is fixed so mint and the retry check agree.
const GATEWAY_EFFECT_MODE: EffectMode = EffectMode::Execute;

/// Deterministic provenance for an approval binding — mirrors how `gate()` derives
/// provenance for the decision (same channel/session/hash), so the token records
/// the real evaluation context and is computed identically at mint and at retry.
fn binding_provenance(source: &str) -> Provenance {
    let channel = match source {
        "workspace_file" => SourceChannel::WorkspaceFile,
        "shell_output" => SourceChannel::ShellOutput,
        "mcp_output" => SourceChannel::McpOutput,
        "web" => SourceChannel::Web,
        "memory" => SourceChannel::Memory,
        "generated" => SourceChannel::Generated,
        _ => SourceChannel::UserPrompt,
    };
    Provenance::from_channel(
        channel,
        SessionId::new("mcp-gateway"),
        ContentHash::new("gate"),
    )
}

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

    fn list_tools(&mut self) -> std::io::Result<Vec<Value>> {
        let resp = self.rpc("tools/list", json!({}))?;
        Ok(resp
            .get("result")
            .and_then(|r| r.get("tools"))
            .and_then(|t| t.as_array())
            .cloned()
            .unwrap_or_default())
    }

    /// Returns the upstream's `result` object (already an MCP tool result), or an
    /// `isError` result wrapping a JSON-RPC error.
    fn call_tool(&mut self, name: &str, arguments: &Value) -> std::io::Result<Value> {
        let resp = self.rpc("tools/call", json!({"name": name, "arguments": arguments}))?;
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
) -> GateResponse {
    let req = GateRequest {
        v: ABI_VERSION,
        tool: tool.to_string(),
        arguments: args.clone(),
        path: None, // MCP tools are not file paths — path scope does not apply here
        context: GateContext {
            session_id: "mcp-gateway".to_string(),
            mode: Some(mode.to_string()),
            taint: tainted.then(|| "tainted".to_string()),
            source_channel: Some(source.to_string()),
            approval_token: None,
        },
    };
    gate(world, &req)
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

fn rpc_result(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    world_path: &Path,
    upstream: &[String],
    source: &str,
    initial_taint: bool,
    mode: &str,
    audit_path: Option<&Path>,
    approvals_path: Option<&Path>,
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
    let projected: HashSet<String> = world
        .projected_actions()
        .map(|a| a.as_str().to_string())
        .collect();

    // Ensure the approvals store's directory exists so the first mint can append.
    if let Some(apath) = approvals_path {
        if let Some(parent) = apath.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
    }

    let mut up = match Upstream::spawn(upstream) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("mcp-gateway: cannot start upstream {upstream:?}: {e}");
            return 2;
        }
    };

    // Monotonic session taint: starts at the inbound floor, only ever rises.
    let mut session_taint = initial_taint;

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
                "serverInfo": {"name": "harness-mcp-gateway", "version": "0.1.0"},
                "capabilities": {"tools": {}}
            }),
            "tools/list" => {
                let tools = match up.list_tools() {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("[mcp-gateway] upstream tools/list failed: {e}");
                        Vec::new()
                    }
                };
                let advertised = tools.len();
                let exposed: Vec<Value> = tools
                    .into_iter()
                    .filter(|t| {
                        t.get("name")
                            .and_then(|n| n.as_str())
                            .map(|n| projected.contains(n))
                            .unwrap_or(false)
                    })
                    .collect();
                // Surface-shaping ratio — the visible governability story: how much
                // of the upstream's advertised surface the world actually exposes.
                eprintln!(
                    "[mcp-gateway] surface shaped: upstream advertised {advertised} tools, \
                     exposing {} ({} ABSENT)",
                    exposed.len(),
                    advertised.saturating_sub(exposed.len())
                );
                json!({"tools": exposed})
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
                let verdict = govern(&world, &name, &args, source, session_taint, mode);
                audit(
                    audit_path,
                    json!({"tool": name, "action": verdict.action,
                           "decision": verdict.decision,
                           "rule": verdict.rule.clone().unwrap_or_default(),
                           "manifest_hash": verdict.manifest_hash, "mode": mode,
                           "source": source, "taint_in": session_taint}),
                );
                // The gateway is fail-closed by design: an unevaluated or
                // non-ALLOW call is never forwarded upstream. ABSENT / ASK /
                // DENY / REPLAN stay distinguishable via the label prefix —
                // MCP's only structural channel is the isError text.
                match host_outcome(&verdict) {
                    HostOutcome::Proceed => {
                        // Monotonic escalation from the call's post-call taint.
                        if verdict.context.taint == "tainted" {
                            session_taint = true;
                        }
                        match up.call_tool(&name, &args) {
                            Ok(r) => r,
                            Err(e) => json!({"isError": true,
                                "content": [{"type": "text", "text": format!("upstream error: {e}")}]}),
                        }
                    }
                    HostOutcome::NeedsApproval { reason } => match approvals_path {
                        // No approval channel wired → ASK is a non-forwarded block
                        // (an automated agent cannot self-approve).
                        None => json!({"isError": true,
                            "content": [{"type": "text", "text": format!("ASK: {reason}")}]}),
                        // Out-of-band approval (E18.2): re-open the store each time so a
                        // human's `harness approvals approve` (a separate process) is seen.
                        Some(apath) => {
                            let action = ActionName::new(&verdict.action);
                            let dhash = world.descriptor_hash(&action).cloned().unwrap_or_default();
                            let prov = binding_provenance(source);
                            let wid = world.world_id().clone();
                            match ApprovalStore::open(apath) {
                                Err(e) => json!({"isError": true, "content": [{"type": "text",
                                    "text": format!("ASK: {reason}; approval store error: {e}")}]}),
                                Ok(mut store) => {
                                    if let Some(tid) = store.granted_token_id(
                                        &action,
                                        &args,
                                        &wid,
                                        &dhash,
                                        &prov,
                                        GATEWAY_EFFECT_MODE,
                                    ) {
                                        // Approved out of band → forward exactly once.
                                        if verdict.context.taint == "tainted" {
                                            session_taint = true;
                                        }
                                        let forwarded = up.call_tool(&name, &args);
                                        // Single-use: consume the approval once the RPC
                                        // reached upstream (a transport failure leaves it
                                        // approved so the agent can retry).
                                        if forwarded.is_ok() {
                                            let _ = store.mark_executed(&tid);
                                        }
                                        match forwarded {
                                            Ok(r) => r,
                                            Err(e) => {
                                                json!({"isError": true, "content": [{"type": "text",
                                                "text": format!("upstream error: {e}")}]})
                                            }
                                        }
                                    } else {
                                        // Not yet approved: reuse an outstanding request
                                        // for this exact call, or mint a fresh one.
                                        let existing = store.pending_token_id(
                                            &action,
                                            &args,
                                            &wid,
                                            &dhash,
                                            &prov,
                                            GATEWAY_EFFECT_MODE,
                                        );
                                        let minted = match existing {
                                            Some(tid) => Ok(tid),
                                            None => {
                                                let ts = SystemTime::now()
                                                    .duration_since(UNIX_EPOCH)
                                                    .map(|d| d.as_nanos())
                                                    .unwrap_or(0);
                                                store.mint(ApprovalToken::pending(
                                                    ApprovalTokenId::new(format!("appr-{ts:x}")),
                                                    action.clone(),
                                                    params_hash(&args),
                                                    wid.clone(),
                                                    dhash.clone(),
                                                    prov.clone(),
                                                    GATEWAY_EFFECT_MODE,
                                                ))
                                            }
                                        };
                                        match minted {
                                            Ok(tid) => {
                                                json!({"isError": true, "content": [{"type": "text",
                                                "text": format!("ASK: {reason}; approval pending — a human must run `harness approvals approve {}`, then retry", tid.as_str())}]})
                                            }
                                            Err(e) => {
                                                json!({"isError": true, "content": [{"type": "text",
                                                "text": format!("ASK: {reason}; could not record approval request: {e}")}]})
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
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
