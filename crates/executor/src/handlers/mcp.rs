//! MCP dispatch handler (E7.1). Reads the structured `{server, tool, input}`
//! operation the kernel lowered, calls the transport, and returns the result as
//! a (tainted) `External` output. Descriptor drift is caught by the executor's
//! registry before this runs (E7.2 / invariant 11); this handler enforces no
//! policy.

use harness_types::ExecutionSpec;
use serde_json::json;

use crate::handler::{ExecError, ExecOutput, Handler};
use crate::handlers::{str_field, structured};
use crate::transport::McpTransport;

pub struct McpHandler {
    transport: Box<dyn McpTransport>,
}

impl McpHandler {
    pub fn new(transport: Box<dyn McpTransport>) -> Self {
        Self { transport }
    }
}

impl Handler for McpHandler {
    fn execute(&self, spec: &ExecutionSpec) -> Result<ExecOutput, ExecError> {
        let payload = structured(spec)?;
        let server = str_field(payload, "server")?;
        let tool = str_field(payload, "tool")?;
        let input = payload.get("input").cloned().unwrap_or_else(|| json!({}));
        let result = self
            .transport
            .call(&server, &tool, &input)
            .map_err(ExecError::Io)?;
        Ok(ExecOutput::External {
            source: format!("mcp:{server}/{tool}"),
            content: result.to_string(),
        })
    }

    fn simulate(&self, spec: &ExecutionSpec) -> Result<ExecOutput, ExecError> {
        let payload = structured(spec)?;
        let server = str_field(payload, "server")?;
        let tool = str_field(payload, "tool")?;
        Ok(ExecOutput::Simulated(format!(
            "would call mcp {server}/{tool}"
        )))
    }
}
