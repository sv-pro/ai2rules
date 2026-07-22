//! Pure governance **gate** — the host-neutral decision ABI (DECISIONS D24).
//!
//! [`gate`] maps a host-neutral [`GateRequest`] onto the kernel's neutral types,
//! runs [`world_kernel::decide`], and maps the `KernelOutcome` back to a
//! [`GateResponse`] — the verdict, the rule that fired, and the **post-call
//! monotonic taint** the host adapter must persist.
//!
//! It is **decision-only** (it never executes — on `ALLOW` the host runs its own
//! tool) and **pure** (no I/O, no LLM, no mutable state), so the *same* function
//! backs the `harness gate` subcommand and the WASM engine, the way [`preview`]
//! does for the authoring tool. Wire schema: `docs/harness-gate-abi.md`.
//!
//! [`preview`]: crate::preview

use harness_types::{
    ActionName, ActionType, CallId, CompiledWorld, ContentHash, Decision, ExecutionMode,
    Provenance, Provider, RootAccess, SessionId, SideEffectClass, SourceChannel, Taint,
    TaintContext, ToolCall,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use world_kernel::{decide, BudgetUsage, EvalContext, KernelOutcome};

/// The current ABI version. Bumped only on a breaking wire change (§8).
pub const ABI_VERSION: u32 = 1;

/// One proposed tool call to govern, in the manifest's vocabulary (§3).
#[derive(Debug, Clone, Deserialize)]
pub struct GateRequest {
    /// ABI version. Defaults to the current version when omitted.
    #[serde(default = "default_version")]
    pub v: u32,
    /// Action name **in the manifest's vocabulary** — the adapter has already
    /// mapped the host's tool name. Maps to `ToolCall.action_name`.
    pub tool: String,
    /// The proposed call's arguments. Maps to `ToolCall.arguments`.
    #[serde(default)]
    pub arguments: Value,
    /// The action's resolved **absolute** target path, for path-scoped (`roots`)
    /// governance. The *adapter* extracts it from the file-action arguments and does
    /// the I/O of absolutizing it, keeping the gate pure. Absent ⇒ no path scope for
    /// this call (structured file tools set it; Bash/etc. don't — Bash is undecidable
    /// and stays OS-sandbox territory).
    #[serde(default)]
    pub path: Option<String>,
    /// Carried, host-owned context (taint, mode, session).
    #[serde(default)]
    pub context: GateContext,
}

/// The host-owned context carried alongside a call (§3). Unknown fields are
/// ignored, so the wire format is forward-compatible.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct GateContext {
    /// Opaque host session id; the taint-sidecar key and trace correlator.
    #[serde(default)]
    pub session_id: String,
    /// `interactive` (default) | `background`. Drives ASK→DENY fail-closed.
    #[serde(default)]
    pub mode: Option<String>,
    /// Monotonic carried taint: `clean` (default) | `tainted`.
    #[serde(default)]
    pub taint: Option<String>,
    /// Provenance of this call's trigger (the proposing actor's trust). Defaults
    /// to `user_prompt`. The inbound taint *floor* is driven by `taint`, not this.
    #[serde(default)]
    pub source_channel: Option<String>,
    /// A granted approval token, when re-submitting a previously `ASK`ed call.
    #[serde(default)]
    pub approval_token: Option<String>,
}

/// The kernel's verdict for one call (§4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResponse {
    pub v: u32,
    /// `ABSENT` | `ALLOW` | `DENY` | `ASK` | `REPLAN`.
    pub decision: String,
    /// The **effective action** the kernel decided on — the request's `tool`
    /// after the world's `command_classes` classifiers ran (D36). Equal to the
    /// raw tool when no classifier matched. A backward-compatible v1 addition.
    pub action: String,
    /// The rule/invariant that fired, or `null` for a plain `ALLOW`.
    pub rule: Option<String>,
    /// Human-readable rationale, for the host UI / the trace.
    pub reason: String,
    pub context: GateResponseContext,
    /// Present only when `decision == ASK`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval: Option<GateApproval>,
    /// First 12 hex of the compiled manifest hash, for drift/trace correlation.
    pub manifest_hash: String,
}

/// Post-call state the adapter must persist for the next call (§4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResponseContext {
    /// Monotonic post-call taint to persist: `clean` | `tainted`.
    pub taint: String,
}

/// Approval handshake returned on `ASK` (§4). The token is a correlation id;
/// durable binding/validation is the host adapter's `ApprovalStore` (deferred).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateApproval {
    pub token: String,
    pub required: bool,
}

/// Govern one proposed call against a compiled world. Pure and deterministic in
/// `(world, req)` — the runtime half of the kernel, exposed as a value function.
pub fn gate(world: &CompiledWorld, req: &GateRequest) -> GateResponse {
    // The effective action: the world's own command classifiers run first
    // (D36), so classification is kernel data, identical for every host.
    let action = world.classify_command(&ActionName::new(&req.tool), &req.arguments);
    let inbound = parse_taint(req.context.taint.as_deref());
    let channel = parse_channel(req.context.source_channel.as_deref());
    let session = SessionId::new(&req.context.session_id);

    let call = ToolCall {
        action_name: action.clone(),
        arguments: req.arguments.clone(),
        provider: Provider::CliNative,
        call_id: CallId::new("gate"),
        source_perceptions: vec![],
        session_id: session.clone(),
    };
    let provenance = Provenance::from_channel(channel, session, ContentHash::new("gate"));
    let mode = parse_mode(req.context.mode.as_deref());
    let ctx = EvalContext {
        taint: TaintContext::from_taint(inbound),
        mode,
        usage: BudgetUsage::default(),
        approval_granted: req.context.approval_token.is_some(),
    };

    let (mut decision, mut rule) = match decide(world, &call, provenance, &ctx) {
        KernelOutcome::UnknownToOntology { .. } => {
            (Decision::Absent, "unknown_to_ontology".to_string())
        }
        KernelOutcome::NotRepresentable { decision, rule, .. } => (decision, rule),
        KernelOutcome::Evaluated { disposition, .. } => (disposition.decision, disposition.rule),
    };

    // Spatial scope (roots): when the world declares roots and the adapter resolved a
    // target `path` for a file action, decide by *where* it lands. Path-scope can only
    // TIGHTEN a kernel ALLOW (deny / ask / read-only) — never loosen a block. Bash and
    // other tools without a single resolvable path carry no `path`, so they are exempt.
    if decision == Decision::Allow {
        if let (Some(path), Some(at)) = (req.path.as_deref(), world.action_type(&action)) {
            if let Some(access) = world.classify_path(path) {
                let is_write = matches!(at, ActionType::Write | ActionType::Patch);
                match access {
                    RootAccess::Deny => {
                        decision = Decision::Deny;
                        rule = "path_scope_denied".to_string();
                    }
                    RootAccess::Read if is_write => {
                        decision = Decision::Deny;
                        rule = "path_scope_readonly".to_string();
                    }
                    RootAccess::Ask => {
                        // Fail closed in background (mirrors invariant 10 for approvals).
                        if matches!(mode, ExecutionMode::Background) {
                            decision = Decision::Deny;
                            rule = "path_scope_ask_background".to_string();
                        } else {
                            decision = Decision::Ask;
                            rule = "path_scope_ask".to_string();
                        }
                    }
                    RootAccess::Read | RootAccess::ReadWrite => {}
                }
            }
        }
    }

    // Post-call taint escalation: monotonic join of the carried taint with the
    // taint this action's *output* introduces. Only an executed (ALLOW) call
    // ingests anything — a blocked call brings in nothing (§4).
    let mut out_taint = inbound;
    if decision == Decision::Allow {
        if let Some(se) = world.side_effect(&action) {
            out_taint = out_taint.join(side_effect_taint(se));
        }
        // Path-aware read-taint: reading under a `taint_source` root taints the
        // session (restores the D25/D37-deferred read-taint, now declared per path).
        if let Some(path) = req.path.as_deref() {
            if world.path_taints(path) {
                out_taint = out_taint.join(Taint::Tainted);
            }
        }
    }

    let approval = (decision == Decision::Ask).then(|| GateApproval {
        token: format!("{}:{}", req.context.session_id, action.as_str()),
        required: true,
    });

    GateResponse {
        v: ABI_VERSION,
        decision: decision_str(decision).to_string(),
        action: action.as_str().to_string(),
        // A plain ALLOW surfaces no distinguishing rule (§4).
        rule: (decision != Decision::Allow).then(|| rule.clone()),
        reason: reason_for(decision, &rule).to_string(),
        context: GateResponseContext {
            taint: taint_str(out_taint).to_string(),
        },
        approval,
        manifest_hash: short_hash(world),
    }
}

fn default_version() -> u32 {
    ABI_VERSION
}

fn parse_taint(s: Option<&str>) -> Taint {
    match s {
        Some("tainted") => Taint::Tainted,
        _ => Taint::Clean,
    }
}

fn parse_mode(s: Option<&str>) -> ExecutionMode {
    match s {
        Some("background") => ExecutionMode::Background,
        _ => ExecutionMode::Interactive,
    }
}

/// Map the wire `source_channel` to a kernel channel. An absent or unrecognized
/// value defaults to `UserPrompt` (the proposing actor); the security-critical
/// inbound floor is carried by `taint`, not this field.
fn parse_channel(s: Option<&str>) -> SourceChannel {
    match s {
        Some("workspace_file") => SourceChannel::WorkspaceFile,
        Some("shell_output") => SourceChannel::ShellOutput,
        Some("mcp_output") => SourceChannel::McpOutput,
        Some("web") => SourceChannel::Web,
        Some("memory") => SourceChannel::Memory,
        Some("generated") => SourceChannel::Generated,
        _ => SourceChannel::UserPrompt,
    }
}

/// The taint an action's *output* introduces, by side-effect class. v1 policy:
/// network/external/memory ingress brings in untrusted data; pure effects and
/// local reads do not. The finer, path-sensitive taint-source policy is the
/// deferred manifest-schema item (`docs/harness-gate-abi.md` §7).
fn side_effect_taint(se: SideEffectClass) -> Taint {
    match se {
        SideEffectClass::Network | SideEffectClass::External | SideEffectClass::Memory => {
            Taint::Tainted
        }
        _ => Taint::Clean,
    }
}

fn decision_str(d: Decision) -> &'static str {
    match d {
        Decision::Absent => "ABSENT",
        Decision::Allow => "ALLOW",
        Decision::Deny => "DENY",
        Decision::Ask => "ASK",
        Decision::Replan => "REPLAN",
    }
}

fn taint_str(t: Taint) -> &'static str {
    match t {
        Taint::Clean => "clean",
        Taint::Tainted => "tainted",
    }
}

fn reason_for(d: Decision, rule: &str) -> &'static str {
    match (d, rule) {
        (Decision::Absent, "unknown_to_ontology") => "action is not in this world's ontology",
        (Decision::Absent, "capability") => "the actor's trust/capability cannot see this action",
        (Decision::Absent, _) => "action is not projected into this world",
        (Decision::Deny, "taint_invariant") => {
            "tainted context cannot reach an externally-effectful action"
        }
        (Decision::Deny, "schema_violation") => "arguments violate the action's schema",
        (Decision::Deny, "descriptor_drift") => "the action descriptor changed since projection",
        (Decision::Deny, "path_scope_denied") => "the target path is outside the allowed roots",
        (Decision::Deny, "path_scope_readonly") => {
            "the target path is read-only under the roots policy"
        }
        (Decision::Deny, "path_scope_ask_background") => {
            "the target path needs approval, unavailable in background"
        }
        (Decision::Deny, _) => "policy blocked a visible action",
        (Decision::Ask, _) => "human approval is required before this action",
        (Decision::Replan, _) => "over budget or too broad; propose a smaller step",
        (Decision::Allow, _) => "permitted; the host may execute this action",
    }
}

fn short_hash(world: &CompiledWorld) -> String {
    let h = world.manifest_hash().as_str();
    h[..h.len().min(12)].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use compiler::compile_default;

    /// Build a request for `tool` with the given carried taint, interactive,
    /// user-prompt provenance, empty args.
    fn req(tool: &str, taint: &str) -> GateRequest {
        GateRequest {
            v: ABI_VERSION,
            tool: tool.to_string(),
            arguments: serde_json::json!({}),
            path: None,
            context: GateContext {
                session_id: "s1".to_string(),
                mode: None,
                taint: Some(taint.to_string()),
                source_channel: None,
                approval_token: None,
            },
        }
    }

    #[test]
    fn clean_read_is_allowed_and_does_not_taint() {
        let world = compile_default();
        let res = gate(&world, &req("read_workspace", "clean"));
        assert_eq!(res.decision, "ALLOW");
        assert_eq!(res.rule, None); // a plain ALLOW surfaces no rule
        assert_eq!(res.context.taint, "clean"); // Read does not ingest untrusted data
        assert!(res.approval.is_none());
        assert_eq!(res.v, ABI_VERSION);
        assert!(!res.manifest_hash.is_empty());
    }

    #[test]
    fn clean_fetch_web_is_allowed_but_escalates_taint() {
        // The core loop: a web fetch is allowed when clean, and its untrusted
        // output taints the session for the *next* call.
        let world = compile_default();
        let res = gate(&world, &req("fetch_web", "clean"));
        assert_eq!(res.decision, "ALLOW");
        assert_eq!(res.context.taint, "tainted");
    }

    #[test]
    fn tainted_fetch_web_is_denied_by_the_taint_floor() {
        let world = compile_default();
        let res = gate(&world, &req("fetch_web", "tainted"));
        assert_eq!(res.decision, "DENY");
        assert_eq!(res.rule.as_deref(), Some("taint_invariant"));
        assert_eq!(res.context.taint, "tainted"); // stays tainted (monotonic)
    }

    #[test]
    fn unknown_tool_is_absent_not_denied() {
        let world = compile_default();
        let res = gate(&world, &req("send_email", "clean"));
        assert_eq!(res.decision, "ABSENT");
        assert_eq!(res.rule.as_deref(), Some("unknown_to_ontology"));
    }

    #[test]
    fn approval_required_action_asks_and_returns_a_token() {
        let world = compile_default();
        let res = gate(&world, &req("start_pty", "clean"));
        assert_eq!(res.decision, "ASK");
        let approval = res.approval.expect("ASK must carry an approval handshake");
        assert!(approval.required);
        assert_eq!(approval.token, "s1:start_pty");
    }

    #[test]
    fn approval_required_in_background_fails_closed_to_deny() {
        let world = compile_default();
        let mut r = req("start_pty", "clean");
        r.context.mode = Some("background".to_string());
        let res = gate(&world, &r);
        assert_eq!(res.decision, "DENY");
        assert!(res.approval.is_none());
    }

    #[test]
    fn granted_approval_token_allows() {
        let world = compile_default();
        let mut r = req("start_pty", "clean");
        r.context.approval_token = Some("s1:start_pty".to_string());
        let res = gate(&world, &r);
        assert_eq!(res.decision, "ALLOW");
    }

    #[test]
    fn untrusted_source_channel_makes_a_write_absent_by_capability() {
        // An untrusted actor (web channel) may only Read; a write is ABSENT.
        let world = compile_default();
        let mut r = req("write_workspace", "clean");
        r.context.source_channel = Some("web".to_string());
        let res = gate(&world, &r);
        assert_eq!(res.decision, "ABSENT");
        assert_eq!(res.rule.as_deref(), Some("capability"));
    }

    #[test]
    fn deserializes_from_wire_json_with_defaults() {
        // Minimal request: only `tool`. Everything else defaults.
        let r: GateRequest = serde_json::from_str(r#"{"tool":"read_workspace"}"#).unwrap();
        assert_eq!(r.v, ABI_VERSION);
        assert_eq!(r.tool, "read_workspace");
        assert!(r.context.taint.is_none());

        let world = compile_default();
        let res = gate(&world, &r);
        assert_eq!(res.decision, "ALLOW");
    }

    #[test]
    fn unknown_wire_fields_are_ignored() {
        let r: GateRequest =
            serde_json::from_str(r#"{"tool":"read_workspace","future_field":42}"#).unwrap();
        assert_eq!(r.tool, "read_workspace");
    }

    #[test]
    fn response_serializes_to_documented_shape() {
        let world = compile_default();
        let res = gate(&world, &req("fetch_web", "tainted"));
        let v = serde_json::to_value(&res).unwrap();
        assert_eq!(v["decision"], "DENY");
        assert_eq!(v["rule"], "taint_invariant");
        assert_eq!(v["context"]["taint"], "tainted");
        assert!(v.get("approval").is_none()); // omitted unless ASK
    }

    /// An inline world with manifest-declared command classifiers (D36) — the
    /// canonical bash-shape spec previously duplicated in the Claude Code and
    /// OpenCode adapters. The kernel now owns these golden vectors.
    fn classified_world() -> harness_types::CompiledWorld {
        let yaml = r#"
world_id: classified-test
capabilities:
  - { trust: Trusted, actions: [Read, Write, Patch, Command, Pty, Mcp, Web, Memory] }
base_actions:
  - { name: bash, action_type: Command, side_effect: Process }
  - { name: bash_network, action_type: Command, side_effect: Network }
  - { name: bash_destructive, action_type: Command, side_effect: Process, approval_required: true }
command_classes:
  - action: bash
    arg: command
    classes:
      - { to: bash_network, patterns: ["curl ", "wget ", "nc ", "ncat ", "telnet ", "ssh ", "scp ", "sftp "] }
      - { to: bash_destructive, patterns: ["rm -rf", "rm -fr", "sudo ", "mkfs", "dd if=", ":(){"] }
transition_policies:
  - { from_taint: Tainted, side_effect: Network, decision: Deny, rule: no_tainted_network }
"#;
        compiler::compile(&compiler::loader::load_yaml(yaml).unwrap()).unwrap()
    }

    fn bash_req(cmd: &str, taint: &str) -> GateRequest {
        let mut r = req("bash", taint);
        r.arguments = serde_json::json!({ "command": cmd });
        r
    }

    #[test]
    fn egress_commands_classify_as_network() {
        let world = classified_world();
        for cmd in [
            "curl http://x",
            "wget http://x",
            "nc -l 9000",
            "ncat host 1",
            "telnet host 23",
            "ssh host",
            "scp a b",
            "sftp host",
            "ls && curl http://x", // chained, at a boundary
        ] {
            let res = gate(&world, &bash_req(cmd, "clean"));
            assert_eq!(res.action, "bash_network", "{cmd}");
        }
    }

    #[test]
    fn destructive_commands_classify_as_destructive_and_ask() {
        let world = classified_world();
        for cmd in [
            "rm -rf build",
            "rm -fr build",
            "sudo systemctl restart x",
            "mkfs.ext4 /dev/sda",
            "dd if=/dev/zero of=/dev/sda",
            ":(){ :|:& };:",
        ] {
            let res = gate(&world, &bash_req(cmd, "clean"));
            assert_eq!(res.action, "bash_destructive", "{cmd}");
            assert_eq!(res.decision, "ASK", "{cmd}");
        }
    }

    #[test]
    fn ordinary_commands_classify_as_plain_bash() {
        let world = classified_world();
        for cmd in ["ls -la", "git status", "echo hi", "cargo test"] {
            let res = gate(&world, &bash_req(cmd, "clean"));
            assert_eq!(res.action, "bash", "{cmd}");
            assert_eq!(res.decision, "ALLOW", "{cmd}");
        }
    }

    #[test]
    fn substrings_of_larger_words_do_not_false_match() {
        // The regression this guards: naive substring matching flagged these as
        // egress ("nc " inside "jsonc "/"sync ") or destructive ("rm -rf" inside
        // "warm -rf").
        let world = classified_world();
        for cmd in [
            "cat app.jsonc 2>/dev/null",
            "git sync origin",
            "mycurl http://x",
            "warm -rf cache",
            "echo unscp",
        ] {
            let res = gate(&world, &bash_req(cmd, "clean"));
            assert_eq!(res.action, "bash", "{cmd}");
        }
    }

    #[test]
    fn tainted_classified_egress_is_denied_by_the_taint_floor() {
        let world = classified_world();
        let res = gate(&world, &bash_req("ls && curl http://exfil", "tainted"));
        assert_eq!(res.action, "bash_network");
        assert_eq!(res.decision, "DENY");
        assert_eq!(res.rule.as_deref(), Some("taint_invariant"));
    }

    #[test]
    fn effective_action_is_used_in_the_approval_token() {
        let world = classified_world();
        let res = gate(&world, &bash_req("rm -rf /tmp/x", "clean"));
        let approval = res.approval.expect("ASK carries an approval handshake");
        assert_eq!(approval.token, "s1:bash_destructive");
    }

    #[test]
    fn unclassified_worlds_pass_the_raw_action_through() {
        // The default world declares no command_classes: the response's
        // effective action is the raw tool, for every tool.
        let world = compile_default();
        for tool in ["read_workspace", "fetch_web", "no_such_tool"] {
            let res = gate(&world, &req(tool, "clean"));
            assert_eq!(res.action, tool);
        }
    }

    #[test]
    fn gate_is_deterministic() {
        let world = compile_default();
        for tool in ["read_workspace", "fetch_web", "start_pty", "send_email"] {
            for taint in ["clean", "tainted"] {
                let r = req(tool, taint);
                let a = serde_json::to_value(gate(&world, &r)).unwrap();
                let b = serde_json::to_value(gate(&world, &r)).unwrap();
                assert_eq!(a, b, "gate must be deterministic for {tool}/{taint}");
            }
        }
    }

    // ---- roots / path-scope (spatial confinement) ----

    /// A world with `roots` (absolute paths, so no env resolution is needed).
    fn roots_world() -> harness_types::CompiledWorld {
        let yaml = r#"
world_id: roots-test
capabilities:
  - { trust: Trusted, actions: [Read, Write, Patch, Command, Web] }
base_actions:
  - { name: read_file,  action_type: Read,  side_effect: Read }
  - { name: write_file, action_type: Write, side_effect: FilesystemWrite }
roots:
  default: Ask
  rules:
    - { path: "/work",        access: ReadWrite }
    - { path: "/work/inbox",  access: Read, taint_source: true }
    - { path: "/etc",         access: Read }
    - { path: "/etc/shadow",  access: Deny, class: Secret }
    - { path: "/home/u/.ssh", access: Deny, class: Credential }
"#;
        compiler::compile(&compiler::loader::load_yaml(yaml).unwrap()).unwrap()
    }

    fn path_req(tool: &str, path: &str, taint: &str) -> GateRequest {
        let mut r = req(tool, taint);
        r.path = Some(path.to_string());
        r
    }

    #[test]
    fn in_root_write_is_allowed() {
        let res = gate(
            &roots_world(),
            &path_req("write_file", "/work/src/x.rs", "clean"),
        );
        assert_eq!(res.decision, "ALLOW");
    }

    #[test]
    fn out_of_root_defaults_to_ask() {
        let res = gate(&roots_world(), &path_req("write_file", "/tmp/x", "clean"));
        assert_eq!(res.decision, "ASK");
        assert_eq!(res.rule.as_deref(), Some("path_scope_ask"));
    }

    #[test]
    fn read_only_root_allows_read_denies_write() {
        let w = roots_world();
        assert_eq!(
            gate(&w, &path_req("read_file", "/etc/passwd", "clean")).decision,
            "ALLOW"
        );
        let write = gate(&w, &path_req("write_file", "/etc/passwd", "clean"));
        assert_eq!(write.decision, "DENY");
        assert_eq!(write.rule.as_deref(), Some("path_scope_readonly"));
    }

    #[test]
    fn deny_rule_shadows_a_broader_allow_the_etc_shadow_probe() {
        // The discovery's probe: /etc is Read, but /etc/shadow is a longer Deny rule.
        let res = gate(
            &roots_world(),
            &path_req("read_file", "/etc/shadow", "clean"),
        );
        assert_eq!(res.decision, "DENY");
        assert_eq!(res.rule.as_deref(), Some("path_scope_denied"));
    }

    #[test]
    fn write_to_ssh_is_denied_the_grant_mode_blast_radius_fix() {
        let res = gate(
            &roots_world(),
            &path_req("write_file", "/home/u/.ssh/authorized_keys", "clean"),
        );
        assert_eq!(res.decision, "DENY");
        assert_eq!(res.rule.as_deref(), Some("path_scope_denied"));
    }

    #[test]
    fn taint_source_root_taints_a_read() {
        let res = gate(
            &roots_world(),
            &path_req("read_file", "/work/inbox/msg.txt", "clean"),
        );
        assert_eq!(res.decision, "ALLOW");
        assert_eq!(res.context.taint, "tainted"); // read-taint restored, declared per path
    }

    #[test]
    fn ordinary_in_root_read_does_not_taint() {
        // /work is ReadWrite but not a taint_source, so a read stays clean.
        let res = gate(
            &roots_world(),
            &path_req("read_file", "/work/src/x.rs", "clean"),
        );
        assert_eq!(res.decision, "ALLOW");
        assert_eq!(res.context.taint, "clean");
    }

    #[test]
    fn path_scope_ask_fails_closed_in_background() {
        let mut r = path_req("write_file", "/tmp/x", "clean");
        r.context.mode = Some("background".to_string());
        let res = gate(&roots_world(), &r);
        assert_eq!(res.decision, "DENY");
        assert_eq!(res.rule.as_deref(), Some("path_scope_ask_background"));
    }

    #[test]
    fn no_resolved_path_is_exempt_the_bash_analog() {
        // A file action with no adapter-resolved path (path=None) is unaffected by
        // roots — the Bash-exemption analog (Bash carries no single resolvable path).
        let res = gate(&roots_world(), &req("read_file", "clean"));
        assert_eq!(res.decision, "ALLOW");
    }

    #[test]
    fn roots_absent_means_no_path_scope() {
        // The default world declares no roots: a resolved path never changes a verdict.
        let res = gate(
            &compile_default(),
            &path_req("read_workspace", "/etc/shadow", "clean"),
        );
        assert_eq!(res.decision, "ALLOW"); // classify_path returns None -> unaffected
    }
}
