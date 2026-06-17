//! Run a command from pre-split argv with an isolated env and a hard timeout.
//!
//! Output is drained on background threads so a process that fills its pipe
//! buffer cannot deadlock the timeout poll. On timeout the *direct* child is
//! killed; killing an entire process group (kill-tree) is an OS-level concern
//! deferred to E8.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use harness_types::{ExecutionSpec, Operation};

use crate::handler::{ExecError, ExecOutput, Handler};

const POLL_INTERVAL: Duration = Duration::from_millis(5);

pub struct CommandHandler;

impl Handler for CommandHandler {
    fn execute(&self, spec: &ExecutionSpec) -> Result<ExecOutput, ExecError> {
        let argv = argv(spec)?;
        run_with_timeout(spec, argv)
    }

    fn simulate(&self, spec: &ExecutionSpec) -> Result<ExecOutput, ExecError> {
        let argv = argv(spec)?;
        Ok(ExecOutput::Simulated(format!(
            "would run `{}`",
            argv.join(" ")
        )))
    }
}

fn argv(spec: &ExecutionSpec) -> Result<&[String], ExecError> {
    match spec.operation() {
        Operation::Argv(argv) if !argv.is_empty() => Ok(argv),
        Operation::Argv(_) => Err(ExecError::BadOperation("empty argv".to_string())),
        Operation::Structured(_) => Err(ExecError::BadOperation(
            "expected argv, got a structured operation".to_string(),
        )),
    }
}

fn run_with_timeout(spec: &ExecutionSpec, argv: &[String]) -> Result<ExecOutput, ExecError> {
    let mut command = Command::new(&argv[0]);
    command
        .args(&argv[1..])
        .current_dir(spec.cwd())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Isolated environment: only the allowlisted variables survive.
    command.env_clear();
    for key in &spec.env().allowlist {
        if let Ok(value) = std::env::var(key) {
            command.env(key, value);
        }
    }

    let mut child = command.spawn().map_err(|e| ExecError::Io(e.to_string()))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let out_reader = std::thread::spawn(move || drain(stdout));
    let err_reader = std::thread::spawn(move || drain(stderr));

    let deadline = Instant::now() + Duration::from_millis(spec.timeout_ms());
    loop {
        match child.try_wait().map_err(|e| ExecError::Io(e.to_string()))? {
            Some(status) => {
                let stdout = out_reader.join().unwrap_or_default();
                let stderr = err_reader.join().unwrap_or_default();
                return Ok(ExecOutput::CommandResult {
                    exit_code: status.code().unwrap_or(-1),
                    stdout,
                    stderr,
                });
            }
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = out_reader.join();
                let _ = err_reader.join();
                return Err(ExecError::Timeout {
                    timeout_ms: spec.timeout_ms(),
                });
            }
            None => std::thread::sleep(POLL_INTERVAL),
        }
    }
}

fn drain<R: Read>(pipe: Option<R>) -> String {
    let mut buf = String::new();
    if let Some(mut pipe) = pipe {
        let _ = pipe.read_to_string(&mut buf);
    }
    buf
}
