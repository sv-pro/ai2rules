//! The ai2rules target: this repository's kernel, reached through its shipped
//! surfaces.
//!
//! Three surfaces, and nothing else:
//!
//! | Question | Surface |
//! |---|---|
//! | which tools exist for this principal | `harness project` — the discovery projection ABI (D72) |
//! | may this exact call proceed | `harness gate` — the host-neutral gate ABI (D24) |
//! | is this human "yes" the one being spent | `trace_store::ApprovalStore` — the durable effect-bound authorization instance (D73) |
//!
//! The first two are wire operations and are driven either **linked** (calling
//! `harness_preview::{project, gate}` in process) or over the **wire** (spawning
//! the `harness` binary). Both transports are exercised and their observations
//! compared, because a benchmark that only proves the library right proves
//! nothing about the product.
//!
//! The third is a linked library call, and that is a finding this pack records
//! rather than hides: `harness gate` deliberately has no verifier or store access
//! (`docs/harness-gate-abi.md` §3), so the trusted boundary that consumes one
//! exact authorization before an effect is wiring every host must supply today.
//! ai2rules ships the store and the binding; it does not yet ship that boundary
//! as a command.
//!
//! ## What this adapter is not allowed to do
//!
//! It translates and it plumbs. It has no scenario identity, no case table, and
//! no rule of its own: every decision below is a verdict from `gate`, a
//! visibility answer from `project`, or an [`AuthorizationRejection`] from the
//! store. The one judgement it makes — *an `ASK` needs a consumed authorization
//! before an effect* — is the boundary contract the ABI documents, applied
//! identically to every call.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};
use std::rc::Rc;

use compiler::sha256_hex;
use harness_preview::{gate, project, GateRequest, GateResponse};
use harness_types::{
    ActionName, ApprovalState, ApprovalToken, AuthorizationInstanceId, CompiledWorld, ContentHash,
    EffectMode, PrincipalId, Provenance, SessionId, SourceChannel,
};
use serde_json::{json, Value};
use trace_store::{effect_binding, effect_resource, ApprovalStore, ConsumeOutcome, EffectBinding};

use crate::target::{Authorization, Call, Discovery, Invocation, Principal, Target, Verdict};
use crate::upstream::Upstream;

/// How the two wire operations are reached.
#[derive(Debug, Clone)]
pub enum Transport {
    /// Call `harness_preview::{project, gate}` in process.
    Linked,
    /// Spawn the `harness` binary at this path, one process per operation.
    Wire(PathBuf),
}

impl Transport {
    pub fn label(&self) -> &'static str {
        match self {
            Transport::Linked => "linked",
            Transport::Wire(_) => "wire",
        }
    }
}

/// Milliseconds of validity given to each authorization. Long enough that the
/// pack never depends on wall-clock timing; expiry has its own scenario to earn.
const AUTHORIZATION_TTL_MS: u64 = 3_600_000;

pub struct Ai2rules {
    upstream: Rc<RefCell<Upstream>>,
    world: CompiledWorld,
    world_path: PathBuf,
    transport: Transport,
    authorizations: ApprovalStore,
    /// Keeps the authorization log alive for the lifetime of the target.
    _dir: tempfile::TempDir,
    session: String,
    now_ms: u64,
    issued: u64,
    /// Memoised projection answers. The key carries the proposing channel,
    /// because the projection request does: a surface is an answer to *who is
    /// asking*, so it cannot be keyed by the upstream alone.
    surface_cache: BTreeMap<String, (Vec<String>, String)>,
}

impl Ai2rules {
    pub const ID: &'static str = "ai2rules-reference-host";

    pub fn new(
        upstream: Rc<RefCell<Upstream>>,
        world: CompiledWorld,
        world_path: &Path,
        transport: Transport,
    ) -> std::io::Result<Self> {
        let dir = tempfile::tempdir()?;
        let authorizations = ApprovalStore::open(dir.path().join("authorizations.jsonl"))?;
        Ok(Self {
            upstream,
            world,
            world_path: world_path.to_path_buf(),
            transport,
            authorizations,
            _dir: dir,
            session: "govbench".to_string(),
            // A fixed clock: the pack is deterministic, and every authorization
            // it mints is valid for the whole run.
            now_ms: 1_000_000,
            issued: 0,
            surface_cache: BTreeMap::new(),
        })
    }

    // ---- the two wire operations -------------------------------------------

    fn project_surface(&self, request: &Value) -> Result<Value, String> {
        match &self.transport {
            Transport::Linked => project(&self.world, request),
            Transport::Wire(binary) => self.run_wire(binary, "project", request),
        }
    }

    fn gate_call(&self, request: &Value) -> Result<GateResponse, String> {
        match &self.transport {
            Transport::Linked => {
                let parsed: GateRequest = serde_json::from_value(request.clone())
                    .map_err(|e| format!("malformed gate request: {e}"))?;
                Ok(gate(&self.world, &parsed))
            }
            Transport::Wire(binary) => {
                let value = self.run_wire(binary, "gate", request)?;
                serde_json::from_value(value).map_err(|e| format!("malformed gate response: {e}"))
            }
        }
    }

    /// One `harness <op> --world <manifest>` process: request on stdin, response
    /// on stdout. Exit ≠ 0 is a broken governor, never a verdict.
    fn run_wire(&self, binary: &Path, op: &str, request: &Value) -> Result<Value, String> {
        let mut child = ProcessCommand::new(binary)
            .arg(op)
            .arg("--world")
            .arg(&self.world_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("cannot spawn {} {op}: {e}", binary.display()))?;
        child
            .stdin
            .as_mut()
            .ok_or_else(|| "no stdin on harness process".to_string())?
            .write_all(request.to_string().as_bytes())
            .map_err(|e| format!("cannot write {op} request: {e}"))?;
        let output = child
            .wait_with_output()
            .map_err(|e| format!("cannot read {op} response: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "harness {op} exited {}: {}",
                output.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        serde_json::from_slice(&output.stdout).map_err(|e| format!("malformed {op} response: {e}"))
    }

    // ---- translation --------------------------------------------------------

    fn gate_request(&self, principal: &Principal, call: &Call) -> Value {
        json!({
            "v": 1,
            "tool": call.tool,
            "arguments": call.arguments,
            "context": {
                "session_id": self.session,
                "mode": "interactive",
                "taint": "clean",
                "source_channel": principal.channel,
            }
        })
    }

    /// The provenance every call in this session carries: the proposing channel,
    /// the world's declared trust for it, and the session. It is deliberately
    /// *not* derived from the arguments — an argument change must show up as an
    /// effect mismatch, not as a provenance mismatch.
    fn provenance(&self, principal: &Principal) -> Option<Provenance> {
        let policy = self.world.channel_policy(&principal.channel)?;
        let channel = SourceChannel::from_name(&principal.channel)?;
        Some(Provenance::from_channel_with_trust(
            channel,
            policy.trust,
            SessionId::new(&self.session),
            ContentHash::new(format!("session:{}", self.session)),
        ))
    }

    /// The exact identity of one proposed effect, built from the call as
    /// presented. Everything an approval is bound to comes from here.
    fn binding(&self, principal: &Principal, call: &Call) -> Result<EffectBinding, String> {
        let action = ActionName::new(&call.tool);
        let descriptor = self
            .world
            .descriptor_hash(&action)
            .ok_or_else(|| format!("world declares no schema for {}", call.tool))?
            .clone();
        let provenance = self
            .provenance(principal)
            .ok_or_else(|| format!("world declares no channel {}", principal.channel))?;
        Ok(effect_binding(
            PrincipalId::new(&principal.id),
            action,
            &call.arguments,
            effect_resource(&call.arguments),
            self.world.world_id().clone(),
            self.world.manifest_hash().clone(),
            descriptor,
            provenance,
            EffectMode::Execute,
        ))
    }

    fn verdict_of(decision: &str) -> Verdict {
        match decision {
            "ALLOW" => Verdict::Allow,
            "ASK" => Verdict::Ask,
            "DENY" => Verdict::Deny,
            "ABSENT" => Verdict::Absent,
            // REPLAN is "not as proposed": no effect may follow this call, so it
            // lands with DENY here and keeps its own rule in the evidence.
            "REPLAN" => Verdict::Deny,
            _ => Verdict::Unknown,
        }
    }

    fn apply(&mut self, principal: &Principal, call: &Call) -> Value {
        self.upstream
            .borrow_mut()
            .call(&call.tool, &principal.id, &call.arguments)
    }
}

impl Target for Ai2rules {
    fn id(&self) -> &str {
        Self::ID
    }

    fn description(&self) -> &str {
        "ai2rules kernel components (`harness project` D72, `harness gate` D24, the durable \
         authorization store D73) composed with this benchmark's reference trusted-host \
         integration — the consume-then-invoke boundary ai2rules does not yet ship as a command"
    }

    fn metadata(&self) -> Value {
        json!({
            "kind": "ai2rules-reference-host",
            "transport": self.transport.label(),
            "world_id": self.world.world_id().as_str(),
            "manifest_hash": self.world.manifest_hash().as_str(),
            "version": env!("CARGO_PKG_VERSION"),
            // What this target actually is, so a reader of results.json never has
            // to take the name on trust. A result here is a statement about the
            // composition, not about a shipped ai2rules command.
            "composition": {
                "shipped_by_ai2rules": [
                    "harness project — discovery projection ABI (D72)",
                    "harness gate — host-neutral gate ABI (D24)",
                    "trace_store::ApprovalStore — durable effect-bound authorization (D73)",
                ],
                "supplied_by_this_benchmark": [
                    "the trusted host boundary: consume one exact authorization, \
                     then invoke the upstream — see crates/govbench/src/targets/ai2rules.rs",
                ],
                "proves": "ai2rules components composed with the reference trusted-host \
                           integration hold these lines",
                "does_not_prove": "that a shipped ai2rules command holds them (PLAN.md E18.10)",
            },
        })
    }

    fn discover(&mut self, principal: &Principal) -> Discovery {
        let offered = self.upstream.borrow().tools_list();
        let request = json!({
            "v": 1,
            "context": {"source_channel": principal.channel},
            "tools": offered,
        });
        let offered_hash = sha256_hex(request["tools"].to_string().as_bytes());
        // The cache key is the projection request's identity. The channel is part
        // of it because the request says so — this is the same memoisation the
        // reference gateway wanted, keyed by what the answer actually depends on.
        let key = format!(
            "{}|{}|{}",
            self.world.manifest_hash().as_str(),
            principal.channel,
            &offered_hash[..16]
        );
        if let Some((visible, surface_id)) = self.surface_cache.get(&key) {
            return Discovery {
                verdict: Verdict::Allow,
                visible: visible.clone(),
                surface_id: Some(surface_id.clone()),
                evidence: json!({"cache_key": key, "cache": "hit"}),
            };
        }
        let response = match self.project_surface(&request) {
            Ok(response) => response,
            Err(error) => {
                // A projection that cannot be computed offers nothing.
                return Discovery {
                    verdict: Verdict::ErrorClosed,
                    visible: Vec::new(),
                    surface_id: None,
                    evidence: json!({"error": error, "transport": self.transport.label()}),
                };
            }
        };
        let visible: Vec<String> = response["tools"]
            .as_array()
            .map(|tools| {
                tools
                    .iter()
                    .filter_map(|tool| tool.get("name").and_then(Value::as_str))
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();
        let surface_id = response["schema_hash"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        self.surface_cache
            .insert(key.clone(), (visible.clone(), surface_id.clone()));
        Discovery {
            verdict: Verdict::Allow,
            visible,
            surface_id: Some(surface_id),
            evidence: json!({
                "surface": "harness project",
                "transport": self.transport.label(),
                "cache_key": key,
                "cache": "miss",
                "absent": response["absent"].clone(),
                "manifest_hash": response["manifest_hash"].clone(),
            }),
        }
    }

    fn authorize(&mut self, principal: &Principal, call: &Call) -> Authorization {
        let request = self.gate_request(principal, call);
        let response = match self.gate_call(&request) {
            Ok(response) => response,
            Err(error) => {
                return Authorization {
                    verdict: Verdict::ErrorClosed,
                    rule: Some("gate_unavailable".to_string()),
                    handle: None,
                    grant_binding: None,
                    evidence: json!({"error": error, "transport": self.transport.label()}),
                }
            }
        };
        let verdict = Self::verdict_of(&response.decision);
        if verdict != Verdict::Ask {
            return Authorization {
                verdict,
                rule: response.rule.clone(),
                handle: None,
                grant_binding: None,
                evidence: json!({
                    "surface": "harness gate",
                    "transport": self.transport.label(),
                    "decision": response.decision,
                    "reason": response.reason,
                }),
            };
        }
        // The human says yes, once, to this exact effect.
        let binding = match self.binding(principal, call) {
            Ok(binding) => binding,
            Err(error) => {
                return Authorization {
                    verdict: Verdict::ErrorClosed,
                    rule: Some("unbindable_effect".to_string()),
                    handle: None,
                    grant_binding: None,
                    evidence: json!({"error": error}),
                }
            }
        };
        self.issued += 1;
        let id = AuthorizationInstanceId::new(format!("auth-{}", self.issued));
        let token = ApprovalToken::pending(
            id.clone(),
            binding.principal.clone(),
            binding.action.clone(),
            binding.params_hash.clone(),
            binding.canonical_effect_hash.clone(),
            binding.resource.clone(),
            binding.world_id.clone(),
            binding.manifest_hash.clone(),
            binding.descriptor_hash.clone(),
            binding.provenance.clone(),
            binding.effect_mode,
            self.now_ms + AUTHORIZATION_TTL_MS,
        );
        if let Err(error) = self
            .authorizations
            .mint(token)
            .and_then(|id| self.authorizations.approve(&id))
        {
            return Authorization {
                verdict: Verdict::ErrorClosed,
                rule: Some("authorization_store_unavailable".to_string()),
                handle: None,
                grant_binding: None,
                evidence: json!({"error": error.to_string()}),
            };
        }
        Authorization {
            verdict: Verdict::Ask,
            rule: response.rule.clone(),
            handle: Some(id.as_str().to_string()),
            // The canonical effect hash covers principal, action, complete
            // arguments, resource, both epochs, provenance and effect mode — so
            // any change to the approved call changes this string.
            grant_binding: Some(binding_identity(&binding)),
            evidence: json!({
                "surface": "harness gate",
                "transport": self.transport.label(),
                "decision": response.decision,
                "correlation_token": response.approval.as_ref().map(|a| a.token.clone()),
                "authorization": {
                    "id": id.as_str(),
                    "bound_to": [
                        "principal", "action", "arguments", "resource",
                        "world_epoch", "schema_epoch", "provenance", "effect_mode",
                    ],
                    "remaining_uses": 1,
                    "canonical_effect_hash": binding.canonical_effect_hash.as_str(),
                },
            }),
        }
    }

    fn invoke(&mut self, principal: &Principal, call: &Call, handle: Option<&str>) -> Invocation {
        let request = self.gate_request(principal, call);
        let response = match self.gate_call(&request) {
            Ok(response) => response,
            Err(error) => {
                return Invocation {
                    verdict: Verdict::ErrorClosed,
                    rule: Some("gate_unavailable".to_string()),
                    presented_binding: None,
                    rejection: Some("gate_unavailable".to_string()),
                    evidence: json!({"error": error, "transport": self.transport.label()}),
                }
            }
        };
        let verdict = Self::verdict_of(&response.decision);
        let mut evidence = json!({
            "surface": "harness gate",
            "transport": self.transport.label(),
            "decision": response.decision,
            "rule": response.rule,
            "reason": response.reason,
            "manifest_hash": response.manifest_hash,
        });
        match verdict {
            Verdict::Allow => {
                evidence["upstream_result"] = self.apply(principal, call);
                Invocation {
                    verdict: Verdict::Allow,
                    rule: response.rule,
                    presented_binding: None,
                    rejection: None,
                    evidence,
                }
            }
            Verdict::Ask => {
                let Some(handle) = handle else {
                    evidence["authorization"] = json!("none presented");
                    return Invocation {
                        verdict: Verdict::Deny,
                        rule: Some("approval_required".to_string()),
                        presented_binding: None,
                        rejection: Some("no_authorization_presented".to_string()),
                        evidence,
                    };
                };
                let binding = match self.binding(principal, call) {
                    Ok(binding) => binding,
                    Err(error) => {
                        evidence["error"] = json!(error);
                        return Invocation {
                            verdict: Verdict::ErrorClosed,
                            rule: Some("unbindable_effect".to_string()),
                            presented_binding: None,
                            rejection: Some("unbindable_effect".to_string()),
                            evidence,
                        };
                    }
                };
                let id = AuthorizationInstanceId::new(handle);
                let outcome = self.authorizations.consume(&id, &binding, self.now_ms);
                evidence["authorization"] = json!({
                    "presented_handle": handle,
                    "presented_effect_hash": binding.canonical_effect_hash.as_str(),
                    "presented_principal": binding.principal.as_str(),
                    "presented_resource": binding.resource,
                });
                match outcome {
                    Ok(ConsumeOutcome::Consumed(consumed)) => {
                        evidence["upstream_result"] = self.apply(principal, call);
                        let _ = self.authorizations.mark_executed(&consumed);
                        Invocation {
                            verdict: Verdict::Allow,
                            rule: Some("authorization_consumed".to_string()),
                            presented_binding: Some(binding_identity(&binding)),
                            rejection: None,
                            evidence,
                        }
                    }
                    Ok(ConsumeOutcome::Rejected(reason)) => {
                        let label = serde_json::to_value(reason)
                            .ok()
                            .and_then(|value| value.as_str().map(String::from))
                            .unwrap_or_else(|| "rejected".to_string());
                        Invocation {
                            verdict: Verdict::Deny,
                            rule: Some(format!("authorization_{label}")),
                            presented_binding: Some(binding_identity(&binding)),
                            rejection: Some(label),
                            evidence,
                        }
                    }
                    Err(error) => {
                        evidence["error"] = json!(error.to_string());
                        Invocation {
                            verdict: Verdict::ErrorClosed,
                            rule: Some("unknown_authorization".to_string()),
                            presented_binding: Some(binding_identity(&binding)),
                            rejection: Some("unknown_authorization".to_string()),
                            evidence,
                        }
                    }
                }
            }
            other => Invocation {
                verdict: other,
                rule: response.rule.clone(),
                presented_binding: None,
                rejection: response.rule,
                evidence,
            },
        }
    }
}

/// The identity an authorization is bound to and compared on, in the form the
/// benchmark's evidence contract expects. The canonical effect hash covers
/// principal, action, complete normalized arguments, resource, world and schema
/// epochs, provenance and effect mode (D73), so any change to any of those is a
/// different string here.
fn binding_identity(binding: &EffectBinding) -> String {
    format!("effect:{}", binding.canonical_effect_hash.as_str())
}

/// Exposed for the pack's own tests: an authorization is single-use, so its state
/// after a consumed call is terminal.
pub fn is_spent(store: &ApprovalStore, handle: &str) -> bool {
    store
        .token(&AuthorizationInstanceId::new(handle))
        .map(|token| {
            token.remaining_uses == 0
                || matches!(
                    token.state,
                    ApprovalState::Consumed | ApprovalState::Executed
                )
        })
        .unwrap_or(false)
}
