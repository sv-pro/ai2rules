//! Read a workspace file, constrained to readable roots.

use std::path::PathBuf;

use harness_types::ExecutionSpec;

use crate::fs_guard;
use crate::handler::{ExecError, ExecOutput, Handler};
use crate::handlers::{str_field, structured};

pub struct ReadHandler;

impl Handler for ReadHandler {
    fn execute(&self, spec: &ExecutionSpec) -> Result<ExecOutput, ExecError> {
        let path = PathBuf::from(str_field(structured(spec)?, "path")?);
        let resolved = fs_guard::contained(spec.cwd(), &path, &readable_roots(spec))
            .ok_or(ExecError::ReadOutsideRoots { path })?;
        let contents =
            std::fs::read_to_string(&resolved).map_err(|e| ExecError::Io(e.to_string()))?;
        Ok(ExecOutput::FileContents(contents))
    }

    fn simulate(&self, spec: &ExecutionSpec) -> Result<ExecOutput, ExecError> {
        let path = str_field(structured(spec)?, "path")?;
        Ok(ExecOutput::Simulated(format!("would read {path}")))
    }
}

/// Reads are allowed from anywhere the spec lets the agent read *or* write.
fn readable_roots(spec: &ExecutionSpec) -> Vec<PathBuf> {
    let fs = spec.filesystem();
    fs.readable_roots
        .iter()
        .chain(fs.writable_roots.iter())
        .cloned()
        .collect()
}
