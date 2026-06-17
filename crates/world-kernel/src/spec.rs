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

use std::path::PathBuf;

use harness_types::{
    ActionName, ActionType, BackingIdentity, CompiledWorld, EffectMode, EnvPolicy, ExecutionSpec,
    FilesystemPolicy, NetworkPolicy, Operation, TraceId,
};
use serde_json::Value;

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

    let operation = match &backing {
        BackingIdentity::LocalHandler(handler) => operation_for(&action, handler, intent.params())?,
        BackingIdentity::McpServer { server, tool } => {
            return Err(SpecError::UnsupportedBacking {
                action,
                backing: format!("mcp:{server}/{tool}"),
            })
        }
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
        other => Err(SpecError::UnsupportedBacking {
            action: action.clone(),
            backing: format!("local:{other}"),
        }),
    }
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
