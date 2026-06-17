//! Anthropic tool-call dialect ↔ neutral `ToolCall` (E5.2).
//!
//! Pure format translation — no policy. The Anthropic Messages API represents a
//! model's tool call as a `tool_use` content block (`{id, name, input}`), exposes
//! tools as `{name, description, input_schema}`, and takes results back as a
//! `tool_result` block (`{tool_use_id, content, is_error?}`). This module maps
//! those shapes to and from the harness's provider-neutral types.

use harness_types::{ActionName, CallId, Descriptor, Provider, SessionId, ToolCall};
use serde_json::{json, Value};

use crate::{AdapterError, ToolOutcome};

/// Parse an Anthropic `tool_use` block into a neutral [`ToolCall`].
pub fn tool_use_to_call(block: &Value, session: SessionId) -> Result<ToolCall, AdapterError> {
    let id = block
        .get("id")
        .and_then(Value::as_str)
        .ok_or(AdapterError::MissingField("id"))?;
    let name = block
        .get("name")
        .and_then(Value::as_str)
        .ok_or(AdapterError::MissingField("name"))?;
    let input = block.get("input").cloned().unwrap_or_else(|| json!({}));
    if !input.is_object() {
        return Err(AdapterError::BadType {
            field: "input",
            expected: "object",
        });
    }
    Ok(ToolCall {
        action_name: ActionName::new(name),
        arguments: input,
        provider: Provider::Anthropic,
        call_id: CallId::new(id),
        source_perceptions: Vec::new(),
        session_id: session,
    })
}

/// Build a `tool_use` block (the shape a model emits). Handy for scripted models
/// and tests that drive the adapter path.
pub fn tool_use_block(id: &str, name: &str, input: Value) -> Value {
    json!({ "type": "tool_use", "id": id, "name": name, "input": input })
}

/// Build the Anthropic `tools` array from the projected tool surface. Only the
/// actions handed in are exposed — the caller passes exactly the projected set.
pub fn tool_definitions(surface: &[(ActionName, &Descriptor)]) -> Value {
    let tools: Vec<Value> = surface
        .iter()
        .map(|(name, descriptor)| {
            let input_schema = if descriptor.schema.is_object() {
                descriptor.schema.clone()
            } else {
                json!({ "type": "object", "properties": {} })
            };
            json!({
                "name": name.as_str(),
                "description": describe(name, descriptor),
                "input_schema": input_schema,
            })
        })
        .collect();
    Value::Array(tools)
}

/// Format a neutral [`ToolOutcome`] as an Anthropic `tool_result` block.
pub fn format_tool_result(outcome: &ToolOutcome) -> Value {
    json!({
        "type": "tool_result",
        "tool_use_id": outcome.call_id.as_str(),
        "content": outcome.content,
        "is_error": outcome.is_error,
    })
}

fn describe(name: &ActionName, descriptor: &Descriptor) -> String {
    format!(
        "{} (side effect: {:?})",
        name.as_str(),
        descriptor.side_effect
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use harness_types::{BackingIdentity, SideEffectClass};

    fn descriptor(action: &str) -> Descriptor {
        Descriptor {
            action: ActionName::new(action),
            schema: Value::Null,
            arg_constraints: Value::Null,
            side_effect: SideEffectClass::Read,
            backing: BackingIdentity::LocalHandler(action.to_string()),
            metadata: Value::Null,
        }
    }

    #[test]
    fn parses_tool_use_block() {
        let block = tool_use_block("toolu_1", "read_workspace", json!({"path": "src/lib.rs"}));
        let call = tool_use_to_call(&block, SessionId::new("s")).unwrap();
        assert_eq!(call.action_name.as_str(), "read_workspace");
        assert_eq!(call.call_id.as_str(), "toolu_1");
        assert_eq!(call.provider, Provider::Anthropic);
        assert_eq!(call.arguments["path"], json!("src/lib.rs"));
    }

    #[test]
    fn missing_name_is_an_error() {
        let block = json!({"type": "tool_use", "id": "toolu_1", "input": {}});
        assert_eq!(
            tool_use_to_call(&block, SessionId::new("s")),
            Err(AdapterError::MissingField("name"))
        );
    }

    #[test]
    fn non_object_input_is_rejected() {
        let block = json!({"type": "tool_use", "id": "x", "name": "y", "input": "nope"});
        assert!(matches!(
            tool_use_to_call(&block, SessionId::new("s")),
            Err(AdapterError::BadType { field: "input", .. })
        ));
    }

    #[test]
    fn tool_definitions_lists_exactly_the_surface() {
        let read = descriptor("read_workspace");
        let patch = descriptor("apply_patch");
        let surface = vec![
            (ActionName::new("read_workspace"), &read),
            (ActionName::new("apply_patch"), &patch),
        ];
        let defs = tool_definitions(&surface);
        let arr = defs.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["name"], json!("read_workspace"));
        assert!(arr[0]["input_schema"].is_object());
    }

    #[test]
    fn formats_tool_result() {
        let outcome = ToolOutcome {
            call_id: CallId::new("toolu_1"),
            content: "denied".to_string(),
            is_error: true,
        };
        let block = format_tool_result(&outcome);
        assert_eq!(block["type"], json!("tool_result"));
        assert_eq!(block["tool_use_id"], json!("toolu_1"));
        assert_eq!(block["is_error"], json!(true));
    }
}
