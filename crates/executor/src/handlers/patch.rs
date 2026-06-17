//! Apply a workspace patch, constrained to writable roots.
//!
//! E3 models a patch as a structured full-file write (`{ path, contents }`).
//! Real unified-diff application needs a diff library that is not available
//! offline; it is deferred. The writable-root guard (invariant 8) is the point
//! that matters here and is fully enforced.

use std::path::PathBuf;

use harness_types::ExecutionSpec;

use crate::fs_guard;
use crate::handler::{ExecError, ExecOutput, Handler};
use crate::handlers::{str_field, structured};

pub struct PatchHandler;

impl Handler for PatchHandler {
    fn execute(&self, spec: &ExecutionSpec) -> Result<ExecOutput, ExecError> {
        let payload = structured(spec)?;
        let path = PathBuf::from(str_field(payload, "path")?);
        let contents = str_field(payload, "contents")?;
        let resolved = fs_guard::contained(spec.cwd(), &path, &spec.filesystem().writable_roots)
            .ok_or(ExecError::WriteOutsideRoots { path })?;
        std::fs::write(&resolved, contents).map_err(|e| ExecError::Io(e.to_string()))?;
        Ok(ExecOutput::PatchApplied { path: resolved })
    }

    fn simulate(&self, spec: &ExecutionSpec) -> Result<ExecOutput, ExecError> {
        let payload = structured(spec)?;
        let path = PathBuf::from(str_field(payload, "path")?);
        let contents = str_field(payload, "contents")?;
        // Confirm the write *would* be allowed, but touch nothing on disk.
        fs_guard::contained(spec.cwd(), &path, &spec.filesystem().writable_roots)
            .ok_or_else(|| ExecError::WriteOutsideRoots { path: path.clone() })?;
        Ok(ExecOutput::Simulated(format!(
            "would write {} bytes to {}",
            contents.len(),
            path.display()
        )))
    }
}
