//! The `Handler` trait and the executor's result / error types (E3.1, E3.3,
//! E3.6).
//!
//! A handler knows how to perform one kind of operation behind the boundary. It
//! holds no policy state and never decides whether it *should* run — by the time
//! a handler is called, the kernel has already produced an `ExecutionSpec`.

use std::path::PathBuf;

use harness_types::{ActionName, DescriptorHash, EffectMode, ExecutionSpec};

/// The result of running an `ExecutionSpec`. Always returned wrapped in a
/// `TaintedValue` by [`crate::Executor::run`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecOutput {
    FileContents(String),
    CommandResult {
        exit_code: i32,
        stdout: String,
        stderr: String,
    },
    PatchApplied {
        path: PathBuf,
    },
    /// A result from an external channel (MCP tool / web fetch). `source` is a
    /// short origin tag like `mcp:<server>/<tool>` or `web:<url>`.
    External {
        source: String,
        content: String,
    },
    /// A synthetic result produced under `EffectMode::Simulate` — no real side
    /// effect happened.
    Simulated(String),
}

/// Why an `ExecutionSpec` could not be run. The executor evaluates no policy, so
/// these are mechanical failures (missing registration, drift, path escape, I/O,
/// unsupported mode), not policy decisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecError {
    /// The action has no handler in this executor's closed registry (invariant 16).
    Unregistered { action: ActionName },
    /// The spec's descriptor hash no longer matches the registered one
    /// (invariant 11) — blocked before any handler runs.
    DescriptorDrift {
        action: ActionName,
        expected: DescriptorHash,
        actual: DescriptorHash,
    },
    /// A write target resolved outside every writable root (invariant 8).
    WriteOutsideRoots { path: PathBuf },
    /// A read target resolved outside every readable root.
    ReadOutsideRoots { path: PathBuf },
    /// A subprocess outlived its timeout and was killed.
    Timeout { timeout_ms: u64 },
    /// The effect mode is not implemented in this epic (Proxy/Sanitize/Defer).
    UnsupportedEffectMode(EffectMode),
    /// The operation payload did not match what the handler expected.
    BadOperation(String),
    /// An underlying I/O failure.
    Io(String),
}

impl std::fmt::Display for ExecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecError::Unregistered { action } => {
                write!(f, "no registered handler for action {action}")
            }
            ExecError::DescriptorDrift {
                action,
                expected,
                actual,
            } => write!(
                f,
                "descriptor drift for {action}: expected {expected}, registered {actual}"
            ),
            ExecError::WriteOutsideRoots { path } => {
                write!(f, "write outside writable roots: {}", path.display())
            }
            ExecError::ReadOutsideRoots { path } => {
                write!(f, "read outside readable roots: {}", path.display())
            }
            ExecError::Timeout { timeout_ms } => {
                write!(f, "command exceeded timeout of {timeout_ms} ms")
            }
            ExecError::UnsupportedEffectMode(mode) => {
                write!(f, "effect mode {mode:?} is not supported yet")
            }
            ExecError::BadOperation(detail) => write!(f, "bad operation: {detail}"),
            ExecError::Io(detail) => write!(f, "io error: {detail}"),
        }
    }
}

impl std::error::Error for ExecError {}

/// One execution capability. Implementations are policy-free: they trust that a
/// valid `ExecutionSpec` means the kernel already authorized the action.
pub trait Handler: Send + Sync {
    /// Perform the real operation (`EffectMode::Execute`).
    fn execute(&self, spec: &ExecutionSpec) -> Result<ExecOutput, ExecError>;

    /// Produce a synthetic result with **no real side effect**
    /// (`EffectMode::Simulate`). Path/argument checks may still run so the
    /// simulation faithfully reports what *would* happen.
    fn simulate(&self, spec: &ExecutionSpec) -> Result<ExecOutput, ExecError>;
}
