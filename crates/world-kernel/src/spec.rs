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

use std::collections::BTreeMap;
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
    let backing = world
        .descriptor(&action)
        .map(|d| d.backing.clone())
        .ok_or_else(|| SpecError::BadArgument {
            detail: format!("no descriptor for {action}"),
        })?;

    // Scoped capabilities narrow the actor's args: strip anything not declared
    // actor-input, inject literals, resolve context refs (invariant 12). A base
    // action passes its params through unchanged.
    let params = effective_params(world, &action, intent.params(), env)?;

    let operation = match &backing {
        BackingIdentity::LocalHandler(handler) => operation_for(&action, handler, &params)?,
        BackingIdentity::McpServer { server, tool } => Operation::Structured(json!({
            "server": server,
            "tool": tool,
            "input": params,
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
) -> Result<Operation, SpecError> {
    match handler {
        "read_workspace" => Ok(Operation::Structured(
            serde_json::json!({ "path": str_param(params, "path")? }),
        )),
        "apply_patch" => Ok(Operation::Structured(serde_json::json!({
            "path": str_param(params, "path")?,
            "contents": str_param(params, "contents")?,
        }))),
        "run_command" => Ok(Operation::Argv(argv_from(params)?)),
        "fetch_web" => Ok(Operation::Structured(
            serde_json::json!({ "url": str_param(params, "url")? }),
        )),
        other => Err(SpecError::UnsupportedBacking {
            action: action.clone(),
            backing: format!("local:{other}"),
        }),
    }
}

/// Resolve the effective params a scoped capability runs with: keep only
/// declared actor-input args (stripping locked/unknown ones), inject literals,
/// and resolve context refs. A non-scoped action passes its params through.
fn effective_params(
    world: &CompiledWorld,
    action: &ActionName,
    actor_params: &Value,
    env: &ExecEnv,
) -> Result<Value, SpecError> {
    let cap = match world.scoped_capability(action) {
        None => return Ok(actor_params.clone()),
        Some(cap) => cap,
    };
    let actor = actor_params.as_object();
    let mut out = Map::new();
    for (name, source) in &cap.args {
        match source {
            // Copy only what the actor is allowed to set; anything else they
            // sent is never read here, so it is stripped.
            ArgSource::ActorInput => {
                if let Some(value) = actor.and_then(|o| o.get(name)) {
                    out.insert(name.clone(), value.clone());
                }
            }
            ArgSource::Literal(value) => {
                out.insert(name.clone(), Value::String(value.clone()));
            }
            ArgSource::ContextRef(key) => {
                let value = env
                    .context
                    .get(key)
                    .ok_or_else(|| SpecError::MissingArgument {
                        argument: format!("context:{key}"),
                    })?;
                out.insert(name.clone(), Value::String(value.clone()));
            }
        }
    }
    Ok(Value::Object(out))
}

fn str_param(params: &Value, key: &str) -> Result<String, SpecError> {
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
fn argv_from(params: &Value) -> Result<Vec<String>, SpecError> {
    if let Some(array) = params.get("argv").and_then(Value::as_array) {
        let argv = string_array(array);
        if argv.is_empty() {
            return Err(SpecError::BadArgument {
                detail: "empty argv".to_string(),
            });
        }
        return Ok(argv);
    }

    let command = str_param(params, "command")?;
    let mut argv = shell_words::split(&command).map_err(|e| SpecError::BadArgument {
        detail: e.to_string(),
    })?;
    if let Some(args) = params.get("args").and_then(Value::as_array) {
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
        CallId, ContentHash, Provenance, Provider, SessionId, SourceChannel, TaintContext, ToolCall,
    };
    use serde_json::json;

    fn spec(action: &str, args: Value) -> ExecutionSpec {
        let world = compile_default();
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
        let ir = IRBuilder::new(&world)
            .build(&call, prov, &TaintContext::clean())
            .expect("intent builds");
        build_execution_spec(
            &world,
            &ir,
            EffectMode::Simulate,
            &ExecEnv::default(),
            TraceId::new("t"),
        )
        .expect("spec assembles")
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
    fn base_action_params_pass_through() {
        let s = spec("read_workspace", json!({ "path": "a", "extra": 1 }));
        // Not a scoped cap → params pass through to the read op as-is.
        assert!(matches!(s.operation(), Operation::Structured(_)));
    }
}
