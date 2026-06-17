//! # executor
//!
//! The execution boundary. It accepts **only** an `ExecutionSpec`, looks the
//! action up in a local *closed* registry, verifies the descriptor hash, applies
//! the `EffectMode`, and returns a `TaintedValue`. It evaluates no policy and
//! holds no policy state — by the time a spec arrives, the kernel has already
//! authorized it.
//!
//! Invariants enforced here: only `ExecutionSpec` crosses in (5); unregistered
//! actions are refused (16); descriptor drift blocks before any handler (11);
//! writes cannot escape writable roots (8); `Simulate` has no side effect (13).

mod fs_guard;
mod handler;
mod handlers;

pub use handler::{ExecError, ExecOutput, Handler};
pub use handlers::{CommandHandler, PatchHandler, ReadHandler};

use std::collections::BTreeMap;

use harness_types::{ActionName, DescriptorHash, EffectMode, ExecutionSpec, TaintedValue};

/// A registry entry: the handler plus the descriptor hash it was registered
/// with (the identity used for drift detection).
struct RegisteredHandler {
    descriptor_hash: DescriptorHash,
    handler: Box<dyn Handler>,
}

/// The closed execution registry. Construct it with [`Executor::builder`].
#[derive(Default)]
pub struct Executor {
    registry: BTreeMap<ActionName, RegisteredHandler>,
}

/// Incremental builder for [`Executor`]. The caller wires action → (hash,
/// handler) entries — typically from a compiled world's descriptor hashes — so
/// the executor never needs to depend on the kernel or compiler.
#[derive(Default)]
pub struct ExecutorBuilder {
    registry: BTreeMap<ActionName, RegisteredHandler>,
}

impl Executor {
    pub fn builder() -> ExecutorBuilder {
        ExecutorBuilder::default()
    }

    /// Run a validated spec. The only entry point across the boundary.
    pub fn run(&self, spec: &ExecutionSpec) -> Result<TaintedValue<ExecOutput>, ExecError> {
        // 1. Closed registry: an unregistered action does not run (invariant 16).
        let entry = self
            .registry
            .get(spec.action())
            .ok_or_else(|| ExecError::Unregistered {
                action: spec.action().clone(),
            })?;

        // 2. Descriptor drift blocks before the handler (invariant 11).
        if entry.descriptor_hash != *spec.expected_descriptor_hash() {
            return Err(ExecError::DescriptorDrift {
                action: spec.action().clone(),
                expected: spec.expected_descriptor_hash().clone(),
                actual: entry.descriptor_hash.clone(),
            });
        }

        // 3. Apply the effect mode.
        let output = match spec.effect_mode() {
            EffectMode::Execute => entry.handler.execute(spec)?,
            EffectMode::Simulate => entry.handler.simulate(spec)?, // invariant 13
            EffectMode::Truncate => truncate(entry.handler.execute(spec)?),
            mode => return Err(ExecError::UnsupportedEffectMode(mode)),
        };

        // Execution results come from tainted-by-default channels (workspace
        // files, shell output); there is no untainted execution result.
        Ok(TaintedValue::tainted(output))
    }
}

impl ExecutorBuilder {
    /// Register a handler for an action under the descriptor hash it must match.
    pub fn register(
        mut self,
        action: ActionName,
        descriptor_hash: DescriptorHash,
        handler: Box<dyn Handler>,
    ) -> Self {
        self.registry.insert(
            action,
            RegisteredHandler {
                descriptor_hash,
                handler,
            },
        );
        self
    }

    pub fn build(self) -> Executor {
        Executor {
            registry: self.registry,
        }
    }
}

const TRUNCATE_CAP: usize = 64 * 1024;

fn truncate(output: ExecOutput) -> ExecOutput {
    match output {
        ExecOutput::FileContents(s) => ExecOutput::FileContents(cap(s)),
        ExecOutput::CommandResult {
            exit_code,
            stdout,
            stderr,
        } => ExecOutput::CommandResult {
            exit_code,
            stdout: cap(stdout),
            stderr: cap(stderr),
        },
        other => other,
    }
}

fn cap(s: String) -> String {
    if s.chars().count() > TRUNCATE_CAP {
        s.chars().take(TRUNCATE_CAP).collect()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harness_types::{EnvPolicy, FilesystemPolicy, NetworkPolicy, Operation, TraceId};
    use serde_json::json;
    use std::path::{Path, PathBuf};

    const HASH: &str = "desc-hash";

    #[allow(clippy::too_many_arguments)]
    fn spec(
        action: &str,
        operation: Operation,
        cwd: &Path,
        writable: Vec<PathBuf>,
        readable: Vec<PathBuf>,
        hash: &str,
        effect: EffectMode,
        timeout_ms: u64,
    ) -> ExecutionSpec {
        ExecutionSpec::new(
            ActionName::new(action),
            operation,
            cwd.to_path_buf(),
            EnvPolicy::default(),
            timeout_ms,
            NetworkPolicy::Disabled,
            FilesystemPolicy {
                writable_roots: writable,
                readable_roots: readable,
            },
            DescriptorHash::new(hash),
            effect,
            TraceId::new("t"),
        )
    }

    fn read_executor() -> Executor {
        Executor::builder()
            .register(
                ActionName::new("read_workspace"),
                DescriptorHash::new(HASH),
                Box::new(ReadHandler),
            )
            .build()
    }

    #[test]
    fn unregistered_action_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let s = spec(
            "send_email",
            Operation::Structured(json!({"path": "x"})),
            dir.path(),
            vec![],
            vec![],
            HASH,
            EffectMode::Execute,
            1_000,
        );
        assert!(matches!(
            read_executor().run(&s),
            Err(ExecError::Unregistered { .. })
        ));
    }

    #[test]
    fn descriptor_drift_blocks_before_handler() {
        let dir = tempfile::tempdir().unwrap();
        let s = spec(
            "read_workspace",
            Operation::Structured(json!({"path": "x"})),
            dir.path(),
            vec![],
            vec![dir.path().to_path_buf()],
            "a-different-hash",
            EffectMode::Execute,
            1_000,
        );
        assert!(matches!(
            read_executor().run(&s),
            Err(ExecError::DescriptorDrift { .. })
        ));
    }

    #[test]
    fn write_outside_writable_roots_is_denied() {
        let dir = tempfile::tempdir().unwrap();
        let exec = Executor::builder()
            .register(
                ActionName::new("apply_patch"),
                DescriptorHash::new(HASH),
                Box::new(PatchHandler),
            )
            .build();
        // Writable root is the tempdir, but the target is /tmp (outside it).
        let s = spec(
            "apply_patch",
            Operation::Structured(json!({"path": "/tmp/escape.txt", "contents": "x"})),
            dir.path(),
            vec![dir.path().to_path_buf()],
            vec![],
            HASH,
            EffectMode::Execute,
            1_000,
        );
        assert!(matches!(
            exec.run(&s),
            Err(ExecError::WriteOutsideRoots { .. })
        ));
    }

    #[test]
    fn simulated_write_has_no_side_effect() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("created.txt");
        let exec = Executor::builder()
            .register(
                ActionName::new("apply_patch"),
                DescriptorHash::new(HASH),
                Box::new(PatchHandler),
            )
            .build();
        let s = spec(
            "apply_patch",
            Operation::Structured(json!({"path": target.to_str().unwrap(), "contents": "data"})),
            dir.path(),
            vec![dir.path().to_path_buf()],
            vec![],
            HASH,
            EffectMode::Simulate,
            1_000,
        );
        let out = exec.run(&s).unwrap();
        assert!(matches!(out.value, ExecOutput::Simulated(_)));
        assert!(!target.exists(), "SIMULATE must not write to disk");
    }

    #[test]
    fn execute_writes_then_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("note.txt");
        let exec = Executor::builder()
            .register(
                ActionName::new("apply_patch"),
                DescriptorHash::new(HASH),
                Box::new(PatchHandler),
            )
            .register(
                ActionName::new("read_workspace"),
                DescriptorHash::new(HASH),
                Box::new(ReadHandler),
            )
            .build();

        let write = spec(
            "apply_patch",
            Operation::Structured(json!({"path": target.to_str().unwrap(), "contents": "hello"})),
            dir.path(),
            vec![dir.path().to_path_buf()],
            vec![],
            HASH,
            EffectMode::Execute,
            1_000,
        );
        exec.run(&write).unwrap();
        assert!(target.exists());

        let read = spec(
            "read_workspace",
            Operation::Structured(json!({"path": target.to_str().unwrap()})),
            dir.path(),
            vec![],
            vec![dir.path().to_path_buf()],
            HASH,
            EffectMode::Execute,
            1_000,
        );
        let out = exec.run(&read).unwrap();
        assert_eq!(out.value, ExecOutput::FileContents("hello".to_string()));
        assert!(out.taint.is_tainted());
    }

    #[test]
    fn command_runs_and_reports_exit_code() {
        let dir = tempfile::tempdir().unwrap();
        let exec = Executor::builder()
            .register(
                ActionName::new("run_command"),
                DescriptorHash::new(HASH),
                Box::new(CommandHandler),
            )
            .build();
        let s = spec(
            "run_command",
            Operation::Argv(vec!["echo".into(), "hi".into()]),
            dir.path(),
            vec![],
            vec![],
            HASH,
            EffectMode::Execute,
            5_000,
        );
        match exec.run(&s).unwrap().value {
            ExecOutput::CommandResult {
                exit_code, stdout, ..
            } => {
                assert_eq!(exit_code, 0);
                assert!(stdout.contains("hi"));
            }
            other => panic!("expected CommandResult, got {other:?}"),
        }
    }

    #[test]
    fn command_past_timeout_is_killed() {
        let dir = tempfile::tempdir().unwrap();
        let exec = Executor::builder()
            .register(
                ActionName::new("run_command"),
                DescriptorHash::new(HASH),
                Box::new(CommandHandler),
            )
            .build();
        let s = spec(
            "run_command",
            Operation::Argv(vec!["sleep".into(), "5".into()]),
            dir.path(),
            vec![],
            vec![],
            HASH,
            EffectMode::Execute,
            50,
        );
        assert!(matches!(exec.run(&s), Err(ExecError::Timeout { .. })));
    }

    #[test]
    fn unsupported_effect_mode_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let s = spec(
            "read_workspace",
            Operation::Structured(json!({"path": "x"})),
            dir.path(),
            vec![],
            vec![dir.path().to_path_buf()],
            HASH,
            EffectMode::Proxy,
            1_000,
        );
        assert!(matches!(
            read_executor().run(&s),
            Err(ExecError::UnsupportedEffectMode(EffectMode::Proxy))
        ));
    }

    #[test]
    fn truncate_caps_long_output() {
        let long = "a".repeat(TRUNCATE_CAP + 100);
        match truncate(ExecOutput::FileContents(long)) {
            ExecOutput::FileContents(s) => assert_eq!(s.chars().count(), TRUNCATE_CAP),
            other => panic!("unexpected {other:?}"),
        }
    }
}
