//! Run a command from pre-split argv with an isolated env and a hard timeout.
//!
//! Output is drained on background threads so a process that fills its pipe
//! buffer cannot deadlock the timeout poll. On timeout the *direct* child is
//! killed; killing an entire process group (kill-tree) is an OS-level concern
//! deferred to E8.
//!
//! **Confinement (finding #10 / D47).** The spec carries a `NetworkPolicy` and a
//! `FilesystemPolicy`, but this handler cannot enforce either on a *subprocess* —
//! a child can open sockets and write anywhere the host user can, and nothing
//! here (short of an OS sandbox) constrains it. That enforcement is E8's job
//! (isolated FS roots, network-off-by-default, kill-tree — all `[ ]`). Rather
//! than run a command while silently ignoring the policy it was handed, a
//! `CommandHandler` must be told its posture: it **fails closed** on `Execute`
//! unless the caller has explicitly accepted unconfined execution
//! (`CommandHandler::unconfined()`). `Simulate` is always safe — it spawns
//! nothing. When E8 lands, an active sandbox becomes the third posture and the
//! `unconfined` acknowledgment can retire.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use harness_types::{ExecutionSpec, Operation};

use crate::handler::{ExecError, ExecOutput, Handler};

const POLL_INTERVAL: Duration = Duration::from_millis(5);

/// Whether an OS-level sandbox confines a command's subprocess. The executor
/// cannot itself enforce a subprocess's network/filesystem policy; see the
/// module docs and D47.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Confinement {
    /// No sandbox, and the caller has not accepted unconfined execution:
    /// `Execute` fails closed with `SandboxRequired`. The safe default.
    #[default]
    Required,
    /// The caller explicitly accepts that the subprocess runs with the host's
    /// authority (network + filesystem policy unenforced). `Execute` proceeds;
    /// the acknowledgment is on the record instead of silent. Retired by E8.
    Unconfined,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CommandHandler {
    confinement: Confinement,
}

impl CommandHandler {
    /// A fail-closed handler: `Execute` is refused (`SandboxRequired`) because no
    /// OS sandbox enforces the subprocess's network/filesystem policy (D47).
    pub fn new() -> Self {
        Self::default()
    }

    /// Opt into running commands **unconfined** — an explicit, audited
    /// acknowledgment that the subprocess runs with host authority, used until
    /// the E8 OS sandbox exists (D47).
    pub fn unconfined() -> Self {
        Self {
            confinement: Confinement::Unconfined,
        }
    }
}

impl Handler for CommandHandler {
    fn execute(&self, spec: &ExecutionSpec) -> Result<ExecOutput, ExecError> {
        // Fail closed: without a sandbox (or an explicit unconfined opt-in) the
        // handler cannot honor the spec's network/filesystem policy, so it must
        // not spawn the subprocess at all (D47). Checked before argv parsing so
        // nothing about the command runs.
        if self.confinement == Confinement::Required {
            return Err(ExecError::SandboxRequired {
                action: spec.action().clone(),
            });
        }
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

    // Put the child in its own process group so a timeout can kill everything it
    // spawned, not just the process we hold a handle to (finding #11). With
    // `process_group(0)` the child becomes group leader and its pgid equals its
    // pid, so the group is exactly this command's descendants.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
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
                // Kill the whole group, not just the direct child. Two things were
                // wrong with `child.kill()` alone (finding #11): a timed-out command
                // could leave `sleep 300 &` running indefinitely, and any surviving
                // descendant still holding the inherited stdout/stderr pipes meant
                // the reader threads never saw EOF — so `out_reader.join()` below
                // could block forever, hanging the executor on the very path meant
                // to bound it.
                kill_tree(&mut child);
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

/// Kill a timed-out command and everything it spawned.
///
/// Unix: signal the process group created at spawn. The safe `nix` wrapper is
/// used rather than a raw `libc::killpg` so the workspace stays free of `unsafe`.
/// `SIGKILL` rather than `SIGTERM` because this path has already waited out the
/// command's whole timeout budget — there is no grace period left to offer.
#[cfg(unix)]
fn kill_tree(child: &mut std::process::Child) {
    use nix::sys::signal::{killpg, Signal};
    use nix::unistd::Pid;

    let pgid = Pid::from_raw(child.id() as i32);
    // Best effort: the group may already be gone, which is the outcome we wanted.
    let _ = killpg(pgid, Signal::SIGKILL);
    // Still reap the direct child, and cover the case where the group call failed.
    let _ = child.kill();
}

/// Non-Unix fallback: kill only the direct child.
///
/// Windows needs a Job Object to bound a process tree and the executor does not
/// create one yet, so a timed-out command may still leave descendants there. The
/// gap is real and stated rather than hidden.
#[cfg(not(unix))]
fn kill_tree(child: &mut std::process::Child) {
    let _ = child.kill();
}

fn drain<R: Read>(pipe: Option<R>) -> String {
    let mut buf = String::new();
    if let Some(mut pipe) = pipe {
        let _ = pipe.read_to_string(&mut buf);
    }
    buf
}
