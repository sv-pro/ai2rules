//! `ExecutionSpec` assembly (E3, kernel-side).
//!
//! The kernel is the *only* producer of the one object that crosses into
//! execution (architecture §6 "ALLOW: build ExecutionSpec"). Given a sealed,
//! representable [`IntentIR`] and the chosen `EffectMode`, this lowers the
//! intent's parameters into a concrete [`Operation`] and stamps the runtime
//! policy (cwd, roots, env, network, timeout) supplied by the caller.
//!
//! It stays pure: everything environmental arrives in [`ExecEnv`]; nothing here
//! reads the filesystem, the process environment, or the clock.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use harness_types::{
    ActionName, ActionType, ArgSource, BackingIdentity, CompiledWorld, EffectMode, EnvPolicy,
    ExecutionSpec, FilesystemPolicy, NetworkPolicy, Operation, TraceId,
};
use serde_json::{json, Map, Value};

use crate::intent::IntentIR;

/// Runtime execution config the pure world cannot carry: where commands run,
/// which roots are reachable, which env vars survive, the egress policy, and the
/// default timeout. Supplied by the orchestrator/CLI at run time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecEnv {
    pub cwd: PathBuf,
    pub writable_roots: Vec<PathBuf>,
    pub readable_roots: Vec<PathBuf>,
    pub env_allowlist: Vec<String>,
    pub network: NetworkPolicy,
    pub default_timeout_ms: u64,
    /// Runtime values a scoped capability's `ContextRef` args resolve against.
    pub context: BTreeMap<String, String>,
}

impl Default for ExecEnv {
    fn default() -> Self {
        Self {
            cwd: PathBuf::from("."),
            writable_roots: Vec::new(),
            readable_roots: Vec::new(),
            env_allowlist: Vec::new(),
            network: NetworkPolicy::Disabled,
            default_timeout_ms: 30_000,
            context: BTreeMap::new(),
        }
    }
}

/// Why a spec could not be assembled from a representable intent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecError {
    /// The action's backing handler is not one E3 can lower (MCP/web/memory/pty
    /// arrive in later epics).
    UnsupportedBacking { action: ActionName, backing: String },
    /// A required argument was absent from the intent's params.
    MissingArgument { argument: String },
    /// An argument was present but malformed.
    BadArgument { detail: String },
}

impl std::fmt::Display for SpecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpecError::UnsupportedBacking { action, backing } => {
                write!(f, "no spec lowering for {action} (backing: {backing})")
            }
            SpecError::MissingArgument { argument } => {
                write!(f, "missing argument `{argument}`")
            }
            SpecError::BadArgument { detail } => write!(f, "bad argument: {detail}"),
        }
    }
}

impl std::error::Error for SpecError {}

/// Build the `ExecutionSpec` for an `ALLOW`ed intent. Pure in
/// `(world, intent, effect_mode, env, trace_id)`.
pub fn build_execution_spec(
    world: &CompiledWorld,
    intent: &IntentIR,
    effect_mode: EffectMode,
    env: &ExecEnv,
    trace_id: TraceId,
) -> Result<ExecutionSpec, SpecError> {
    let action = intent.action().clone();
    let descriptor = world
        .descriptor(&action)
        .ok_or_else(|| SpecError::BadArgument {
            detail: format!("no descriptor for {action}"),
        })?;
    let backing = descriptor.backing.clone();

    // Scoped capabilities narrow the actor's args: strip anything not declared
    // actor-input, inject literals, resolve context refs (invariant 12). A base
    // action keeps its params, but local-handler lowering may only read fields
    // explicitly declared by the sealed descriptor.
    let params = effective_params(world, intent, descriptor, env)?;

    let operation = match &backing {
        BackingIdentity::LocalHandler(handler) => {
            operation_for(&action, handler, &params.value, &params.contract)?
        }
        BackingIdentity::McpServer { server, tool } => Operation::Structured(json!({
            "server": server,
            "tool": tool,
            "input": params.value,
        })),
    };

    // Command-class actions honor the world's command timeout; everything else
    // falls back to the environment default.
    let timeout_ms = if matches!(intent.action_type(), ActionType::Command | ActionType::Pty) {
        world
            .budget()
            .command_timeout_ms
            .unwrap_or(env.default_timeout_ms)
    } else {
        env.default_timeout_ms
    };

    Ok(ExecutionSpec::new(
        action,
        operation,
        env.cwd.clone(),
        EnvPolicy {
            allowlist: env.env_allowlist.clone(),
        },
        timeout_ms,
        env.network.clone(),
        FilesystemPolicy {
            writable_roots: env.writable_roots.clone(),
            readable_roots: env.readable_roots.clone(),
        },
        intent.expected_descriptor_hash().clone(),
        effect_mode,
        trace_id,
    ))
}

/// Lower intent params into a concrete operation, keyed by the backing handler.
fn operation_for(
    action: &ActionName,
    handler: &str,
    params: &Value,
    contract: &ParamContract,
) -> Result<Operation, SpecError> {
    match handler {
        "read_workspace" => Ok(Operation::Structured(
            serde_json::json!({ "path": str_param(params, contract, "path")? }),
        )),
        "apply_patch" => Ok(Operation::Structured(serde_json::json!({
            "path": str_param(params, contract, "path")?,
            "contents": str_param(params, contract, "contents")?,
        }))),
        "run_command" => Ok(Operation::Argv(argv_from(params, contract)?)),
        "fetch_web" => Ok(Operation::Structured(
            serde_json::json!({ "url": str_param(params, contract, "url")? }),
        )),
        other => Err(SpecError::UnsupportedBacking {
            action: action.clone(),
            backing: format!("local:{other}"),
        }),
    }
}

struct EffectiveParams {
    value: Value,
    contract: ParamContract,
}

struct ParamContract {
    declared: BTreeSet<String>,
}

impl ParamContract {
    fn for_descriptor(descriptor: &harness_types::Descriptor) -> Self {
        Self {
            declared: crate::schema::declared_arg_names(
                &descriptor.schema,
                &descriptor.arg_constraints,
            ),
        }
    }

    fn for_scoped_args(args: &BTreeMap<String, ArgSource>) -> Self {
        Self {
            declared: args.keys().cloned().collect(),
        }
    }

    fn require(&self, key: &str) -> Result<(), SpecError> {
        if self.declared.contains(key) {
            Ok(())
        } else {
            Err(SpecError::BadArgument {
                detail: format!("handler requested undeclared argument `{key}`"),
            })
        }
    }
}

/// The call a host must actually make for an admitted intent (D80).
///
/// The kernel decides on the *proposed* call, but a scoped capability runs with
/// different arguments: locked and unknown actor arguments are stripped,
/// literals injected, context refs resolved (invariant 12). A host that executes
/// the proposal instead of this — the wire gate is decision-only, so every
/// out-of-process host does — silently drops the scoping: the model's `repo`
/// reaches the upstream in place of the manifest's literal.
///
/// Pure in `(world, intent)`. Context refs cannot be resolved here (they are
/// runtime values); they are left out of `arguments` and listed in
/// `context_refs` for the host to fill from its own trusted context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveCall {
    /// The base action the call lowers to (itself, for a base action).
    pub base_action: ActionName,
    /// What executes it: a local handler or an MCP server's tool.
    pub backing: BackingIdentity,
    /// Arguments to execute with: actor input that survived scoping, plus literals.
    pub arguments: Value,
    /// Argument name → context key, for `ContextRef` arguments.
    pub context_refs: BTreeMap<String, String>,
    /// Actor-supplied argument names the scoping dropped. Empty for a clean call;
    /// non-empty means something tried to set what the world fixes — worth
    /// recording as a signal, not just discarding.
    pub stripped: Vec<String>,
}

/// Lower an admitted intent to the call the host must execute (D80). See
/// [`EffectiveCall`]. `None` only if the world has no descriptor for the
/// intent's action, which a sealed intent rules out.
pub fn effective_call(world: &CompiledWorld, intent: &IntentIR) -> Option<EffectiveCall> {
    let action = intent.action();
    let descriptor = world.descriptor(action)?;
    let actor_params = intent.params();
    let Some(cap) = world.scoped_capability(action) else {
        return Some(EffectiveCall {
            base_action: action.clone(),
            backing: descriptor.backing.clone(),
            arguments: actor_params.clone(),
            context_refs: BTreeMap::new(),
            stripped: Vec::new(),
        });
    };
    let actor = actor_params.as_object();
    let mut arguments = Map::new();
    let mut context_refs = BTreeMap::new();
    for (name, source) in &cap.args {
        match source {
            // Copy only what the actor is allowed to set; anything else they
            // sent is never read here, so it is stripped.
            ArgSource::ActorInput => {
                if let Some(value) = actor.and_then(|o| o.get(name)) {
                    arguments.insert(name.clone(), value.clone());
                }
            }
            ArgSource::Literal(value) => {
                arguments.insert(name.clone(), Value::String(value.clone()));
            }
            ArgSource::ContextRef(key) => {
                context_refs.insert(name.clone(), key.clone());
            }
        }
    }
    let stripped = actor
        .map(|o| {
            o.keys()
                .filter(|k| !matches!(cap.args.get(*k), Some(ArgSource::ActorInput)))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    Some(EffectiveCall {
        base_action: cap.base_action.clone(),
        backing: descriptor.backing.clone(),
        arguments: Value::Object(arguments),
        context_refs,
        stripped,
    })
}

/// Resolve the effective params a scoped capability runs with: the
/// [`effective_call`] lowering, with context refs resolved from `env`. A
/// non-scoped action passes its params through.
fn effective_params(
    world: &CompiledWorld,
    intent: &IntentIR,
    descriptor: &harness_types::Descriptor,
    env: &ExecEnv,
) -> Result<EffectiveParams, SpecError> {
    let action = intent.action();
    let lowered = effective_call(world, intent).ok_or_else(|| SpecError::BadArgument {
        detail: format!("no descriptor for {action}"),
    })?;
    let Some(cap) = world.scoped_capability(action) else {
        return Ok(EffectiveParams {
            value: lowered.arguments,
            contract: ParamContract::for_descriptor(descriptor),
        });
    };
    let mut value = lowered.arguments;
    for (name, key) in &lowered.context_refs {
        let resolved = env
            .context
            .get(key)
            .ok_or_else(|| SpecError::MissingArgument {
                argument: format!("context:{key}"),
            })?;
        if let Some(obj) = value.as_object_mut() {
            obj.insert(name.clone(), Value::String(resolved.clone()));
        }
    }
    Ok(EffectiveParams {
        value,
        contract: ParamContract::for_scoped_args(&cap.args),
    })
}

fn str_param(params: &Value, contract: &ParamContract, key: &str) -> Result<String, SpecError> {
    contract.require(key)?;
    params
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| SpecError::MissingArgument {
            argument: key.to_string(),
        })
}

/// Accept either a pre-split `argv` array or a `command` string (split with
/// shell-words) plus optional `args`.
fn argv_from(params: &Value, contract: &ParamContract) -> Result<Vec<String>, SpecError> {
    if let Some(value) = params.get("argv") {
        contract.require("argv")?;
        let array = value.as_array().ok_or_else(|| SpecError::BadArgument {
            detail: "argument `argv` must be array".to_string(),
        })?;
        let argv = string_array(array);
        if argv.is_empty() {
            return Err(SpecError::BadArgument {
                detail: "empty argv".to_string(),
            });
        }
        return Ok(argv);
    }

    let command = str_param(params, contract, "command")?;
    let mut argv = shell_words::split(&command).map_err(|e| SpecError::BadArgument {
        detail: e.to_string(),
    })?;
    if let Some(value) = params.get("args") {
        contract.require("args")?;
        let args = value.as_array().ok_or_else(|| SpecError::BadArgument {
            detail: "argument `args` must be array".to_string(),
        })?;
        argv.extend(string_array(args));
    }
    if argv.is_empty() {
        return Err(SpecError::BadArgument {
            detail: "empty command".to_string(),
        });
    }
    Ok(argv)
}

fn string_array(values: &[Value]) -> Vec<String> {
    values
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::IRBuilder;
    use compiler::compile_default;
    use harness_types::{
        BuildError, CallId, ContentHash, Provenance, Provider, SessionId, SourceChannel,
        TaintContext, ToolCall,
    };
    use serde_json::json;

    fn build_ir_with_world(
        world: CompiledWorld,
        action: &str,
        args: Value,
    ) -> Result<(CompiledWorld, IntentIR), BuildError> {
        let call = ToolCall {
            action_name: ActionName::new(action),
            arguments: args,
            provider: Provider::CliNative,
            call_id: CallId::new("c"),
            source_perceptions: vec![],
            session_id: SessionId::new("s"),
        };
        let prov = Provenance::from_channel(
            SourceChannel::UserPrompt,
            SessionId::new("s"),
            ContentHash::new("h"),
        );
        let ir = IRBuilder::new(&world).build(&call, prov, &TaintContext::clean())?;
        Ok((world, ir))
    }

    fn try_spec_with_world(
        world: CompiledWorld,
        action: &str,
        args: Value,
    ) -> Result<ExecutionSpec, BuildError> {
        let (world, ir) = build_ir_with_world(world, action, args)?;
        build_execution_spec(
            &world,
            &ir,
            EffectMode::Simulate,
            &ExecEnv::default(),
            TraceId::new("t"),
        )
        .map_err(|e| BuildError::InvariantViolation {
            law: "spec_lowering".to_string(),
            detail: e.to_string(),
        })
    }

    fn try_spec(action: &str, args: Value) -> Result<ExecutionSpec, BuildError> {
        try_spec_with_world(compile_default(), action, args)
    }

    fn spec(action: &str, args: Value) -> ExecutionSpec {
        try_spec(action, args).expect("spec assembles")
    }

    #[test]
    fn run_tests_strips_locked_args_and_injects_literal() {
        // Invariant 12: the actor's `command` is locked (literal `pytest`); a
        // malicious override and an undeclared `path` are both stripped.
        let s = spec("run_tests", json!({ "command": "rm -rf /", "path": "x" }));
        match s.operation() {
            Operation::Argv(argv) => assert_eq!(argv, &vec!["pytest".to_string()]),
            other => panic!("expected argv, got {other:?}"),
        }
    }

    #[test]
    fn read_repo_file_keeps_only_actor_input() {
        let s = spec(
            "read_repo_file",
            json!({ "path": "src/lib.rs", "evil": "drop tables" }),
        );
        match s.operation() {
            Operation::Structured(v) => {
                assert_eq!(v["path"], json!("src/lib.rs"));
                assert!(v.get("evil").is_none(), "undeclared arg must be stripped");
            }
            other => panic!("expected structured, got {other:?}"),
        }
    }

    #[test]
    fn base_action_declared_params_lower_to_operation() {
        let s = spec("read_workspace", json!({ "path": "a" }));
        assert!(matches!(s.operation(), Operation::Structured(_)));
    }

    #[test]
    fn run_command_hidden_argv_is_rejected_by_descriptor() {
        let err = try_spec(
            "run_command",
            json!({ "command": "echo safe", "argv": ["sh", "-c", "echo pwned"] }),
        )
        .unwrap_err();
        assert!(matches!(err, BuildError::SchemaViolation { .. }));
    }

    #[test]
    fn apply_patch_hidden_execution_fields_are_rejected_by_descriptor() {
        let err = try_spec(
            "apply_patch",
            json!({
                "patch": "benign modeled patch",
                "path": "target.txt",
                "contents": "hidden write"
            }),
        )
        .unwrap_err();
        assert!(matches!(err, BuildError::SchemaViolation { .. }));
    }

    #[test]
    fn local_handler_cannot_consume_additional_properties() {
        let yaml = r#"
world_id: spec-contract
capabilities:
  - { trust: Trusted, actions: [Patch] }
base_actions:
  - name: apply_patch
    action_type: Patch
    side_effect: FilesystemWrite
    schema:
      type: object
      additionalProperties: true
      properties:
        patch: { type: string }
"#;
        let manifest = compiler::load_yaml(yaml).expect("manifest parses");
        let world = compiler::compile(&manifest).expect("manifest compiles");
        let err = try_spec_with_world(
            world,
            "apply_patch",
            json!({
                "patch": "modeled",
                "path": "target.txt",
                "contents": "hidden write"
            }),
        )
        .unwrap_err();
        assert!(matches!(err, BuildError::InvariantViolation { .. }));
    }

    // ---- D80: the lowered call a decision-only host must execute ----

    #[test]
    fn effective_call_strips_injects_and_reports() {
        let (world, ir) = build_ir_with_world(
            compile_default(),
            "run_tests",
            json!({ "command": "rm -rf /", "path": "x" }),
        )
        .unwrap();
        let call = effective_call(&world, &ir).unwrap();
        assert_eq!(call.base_action, ActionName::new("run_command"));
        assert_eq!(call.arguments, json!({ "command": "pytest" }));
        assert_eq!(
            call.stripped,
            vec!["command".to_string(), "path".to_string()]
        );
        assert!(call.context_refs.is_empty());
    }

    #[test]
    fn effective_call_of_a_base_action_is_the_call_itself() {
        let args = json!({ "path": "src/lib.rs" });
        let (world, ir) =
            build_ir_with_world(compile_default(), "read_workspace", args.clone()).unwrap();
        let call = effective_call(&world, &ir).unwrap();
        assert_eq!(call.base_action, ActionName::new("read_workspace"));
        assert_eq!(call.arguments, args);
        assert!(call.stripped.is_empty());
    }

    #[test]
    fn effective_call_carries_the_mcp_backing_and_lists_context_refs() {
        let world = compiler::compile(
            &compiler::load_yaml(
                r#"
world_id: w
capabilities:
  - { trust: Trusted, actions: [Mcp] }
base_actions:
  - name: create_pull_request
    action_type: Mcp
    side_effect: External
    backing: !McpServer { server: codehost, tool: create_pull_request }
    schema:
      type: object
      properties: { repo: { type: string }, base: { type: string }, title: { type: string } }
scoped_capabilities:
  - name: open_change_pr
    base_action: create_pull_request
    args: { repo: !Literal "acme/svc", base: !ContextRef change.base, title: ActorInput }
"#,
            )
            .unwrap(),
        )
        .unwrap();
        let (world, ir) = build_ir_with_world(
            world,
            "open_change_pr",
            json!({ "title": "t", "repo": "attacker/fork" }),
        )
        .unwrap();
        let call = effective_call(&world, &ir).unwrap();
        assert_eq!(
            call.backing,
            BackingIdentity::McpServer {
                server: "codehost".to_string(),
                tool: "create_pull_request".to_string()
            }
        );
        assert_eq!(call.arguments, json!({ "repo": "acme/svc", "title": "t" }));
        assert_eq!(
            call.context_refs.get("base").map(String::as_str),
            Some("change.base")
        );
        assert_eq!(call.stripped, vec!["repo".to_string()]);
    }
}
