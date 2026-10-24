//! Pure discovery **projection** — the sibling of [`gate`](crate::gate) (D72).
//!
//! A host supplies the exact tool schemas its registry is about to expose. The
//! compiled world selects which *names* exist for the proposing channel; the
//! host stays the owner of operational schemas. The response binds the exact
//! visible schema array to a deterministic hash and the same manifest identity
//! `gate` returns.
//!
//! The logic lives here, beside `gate`, for the same reason `gate` does: the
//! `harness project` wire operation, an in-process Rust host, and the WASM
//! build must not be able to answer a discovery question differently.

use compiler::sha256_hex;
use harness_types::{ActionName, CompiledWorld};
use serde_json::{json, Value};
use std::collections::HashSet;

/// Wire version of the projection request/response envelope.
pub const PROJECTION_VERSION: u64 = 1;

/// Shape one offered host surface from the compiled world's projected names.
pub fn project(world: &CompiledWorld, request: &Value) -> Result<Value, String> {
    if request.get("v").and_then(Value::as_u64) != Some(PROJECTION_VERSION) {
        return Err(format!("v must be {PROJECTION_VERSION}"));
    }
    let offered = request
        .get("tools")
        .and_then(Value::as_array)
        .ok_or_else(|| "tools must be an array".to_string())?;
    let source = request
        .get("context")
        .and_then(|context| context.get("source_channel"))
        .and_then(Value::as_str)
        .ok_or_else(|| "context.source_channel must be a string".to_string())?;
    let channel = world
        .channel_policy(source)
        .ok_or_else(|| format!("context.source_channel {source:?} is not declared by the world"))?;
    let mut seen = HashSet::new();
    let mut visible = Vec::new();
    let mut absent = Vec::new();
    for schema in offered {
        let name = schema
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .ok_or_else(|| "every tool must have a non-empty string name".to_string())?;
        if !seen.insert(name.to_string()) {
            return Err(format!("duplicate tool name {name:?}"));
        }
        let action = ActionName::new(name);
        let visible_to_source = world
            .action_type(&action)
            .map(|kind| world.can_perform(channel.trust, kind))
            .unwrap_or(false);
        if world.is_projected(&action) && visible_to_source {
            visible.push(schema.clone());
        } else {
            absent.push(name.to_string());
        }
    }
    let canonical = serde_json::to_vec(&visible)
        .map_err(|e| format!("cannot canonicalize visible schemas: {e}"))?;
    Ok(json!({
        "v": PROJECTION_VERSION,
        "tools": visible,
        "absent": absent,
        "manifest_hash": world.manifest_hash().as_str().chars().take(12).collect::<String>(),
        "schema_hash": sha256_hex(&canonical),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use compiler::{compile, loader::load_yaml};

    fn world() -> CompiledWorld {
        let manifest = load_yaml(
            "world_id: projection-test\nchannels:\n  - { name: user_cli, trust: Trusted, taint: false }\n  - { name: web_fetch, trust: Untrusted, taint: true }\ncapabilities:\n  - { trust: Trusted, actions: [Read, Write] }\n  - { trust: Untrusted, actions: [Read] }\nbase_actions:\n  - { name: read, action_type: Read, side_effect: Read }\n  - { name: write, action_type: Write, side_effect: FilesystemWrite }\n",
        )
        .unwrap();
        compile(&manifest).unwrap()
    }

    #[test]
    fn filters_absent_without_rewriting_host_schema() {
        let read = json!({"name":"read","description":"host-owned","parameters":{"type":"object"}});
        let ghost = json!({"name":"ghost","parameters":{"type":"object"}});
        let response = project(
            &world(),
            &json!({
                "v": 1,
                "context": {"source_channel": "user_cli"},
                "tools": [read.clone(), ghost]
            }),
        )
        .unwrap();
        assert_eq!(response["tools"], json!([read]));
        assert_eq!(response["absent"], json!(["ghost"]));
        assert_eq!(response["manifest_hash"].as_str().unwrap().len(), 12);
        assert_eq!(response["schema_hash"].as_str().unwrap().len(), 64);
    }

    #[test]
    fn rejects_ambiguous_or_malformed_surfaces() {
        let duplicate = json!({
            "v": 1,
            "context": {"source_channel": "user_cli"},
            "tools": [{"name":"read"},{"name":"read"}]
        });
        assert!(project(&world(), &duplicate)
            .unwrap_err()
            .contains("duplicate"));
        assert!(project(
            &world(),
            &json!({"v":2,"context":{"source_channel":"user_cli"},"tools":[]})
        )
        .is_err());
        assert!(project(
            &world(),
            &json!({"v":1,"context":{"source_channel":"user_cli"},"tools":[{}]})
        )
        .is_err());
    }

    #[test]
    fn source_capabilities_shape_discovery_like_invocation() {
        let offered = json!({
            "v": 1,
            "context": {"source_channel": "web_fetch"},
            "tools": [{"name":"read"},{"name":"write"}]
        });
        let response = project(&world(), &offered).unwrap();
        assert_eq!(response["tools"], json!([{"name":"read"}]));
        assert_eq!(response["absent"], json!(["write"]));
    }
}
