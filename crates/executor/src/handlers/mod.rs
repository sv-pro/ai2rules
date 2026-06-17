//! The three core handlers (E3.3) plus shared operation-parsing helpers.

mod command;
mod patch;
mod read;

pub use command::CommandHandler;
pub use patch::PatchHandler;
pub use read::ReadHandler;

use harness_types::{ExecutionSpec, Operation};
use serde_json::Value;

use crate::handler::ExecError;

/// Borrow the structured operation payload, or fail if the spec carries argv.
pub(crate) fn structured(spec: &ExecutionSpec) -> Result<&Value, ExecError> {
    match spec.operation() {
        Operation::Structured(value) => Ok(value),
        Operation::Argv(_) => Err(ExecError::BadOperation(
            "expected a structured operation, got argv".to_string(),
        )),
    }
}

/// Read a required string field from a structured payload.
pub(crate) fn str_field(value: &Value, key: &str) -> Result<String, ExecError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ExecError::BadOperation(format!("missing string field `{key}`")))
}
