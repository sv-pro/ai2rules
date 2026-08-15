//! Minimal argument validation against a frozen `Descriptor` (E2.1).
//!
//! This is deliberately *not* a full JSON Schema implementation — it validates
//! what the default CLI world needs while keeping the lean dependency set
//! (serde/serde_json only). It supports:
//!
//! - an empty / `Null` schema (the default world's base actions) → always valid;
//! - object schemas with `required` (keys must be present) and `properties`
//!   carrying a `type` keyword (declared args must match that JSON type);
//! - closed object schemas by default: when an object schema declares
//!   properties/required keys, any undeclared input key is rejected unless
//!   `additionalProperties: true` is explicitly present;
//! - `arg_constraints` entries with `enum` / `const` (value must be a member /
//!   equal).
//!
//! Anything outside that subset passes — full Draft validation is later
//! hardening, not something the current ontology exercises.

use harness_types::{ActionName, BuildError};
use serde_json::Value;

/// Validate `args` against an action's model-facing `schema` and
/// `arg_constraints`. Returns `SchemaViolation` on the first failure.
pub fn validate(
    action: &ActionName,
    args: &Value,
    schema: &Value,
    constraints: &Value,
) -> Result<(), BuildError> {
    validate_schema(action, args, schema)?;
    validate_declared_properties(action, args, schema, constraints)?;
    validate_constraints(action, args, constraints)?;
    Ok(())
}

fn violation(action: &ActionName, detail: impl Into<String>) -> BuildError {
    BuildError::SchemaViolation {
        action: action.clone(),
        detail: detail.into(),
    }
}

fn validate_schema(action: &ActionName, args: &Value, schema: &Value) -> Result<(), BuildError> {
    // No schema (Null or non-object) or an empty object: nothing to enforce.
    let Some(schema) = schema.as_object() else {
        return Ok(());
    };
    if schema.is_empty() {
        return Ok(());
    }

    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        let args_obj = args.as_object();
        for key in required.iter().filter_map(Value::as_str) {
            let present = args_obj.is_some_and(|o| o.contains_key(key));
            if !present {
                return Err(violation(
                    action,
                    format!("missing required argument `{key}`"),
                ));
            }
        }
    }

    if let Some(props) = schema.get("properties").and_then(Value::as_object) {
        if let Some(args_obj) = args.as_object() {
            for (name, spec) in props {
                let (Some(value), Some(ty)) =
                    (args_obj.get(name), spec.get("type").and_then(Value::as_str))
                else {
                    continue;
                };
                if !type_matches(ty, value) {
                    return Err(violation(action, format!("argument `{name}` must be {ty}")));
                }
            }
        }
    }

    Ok(())
}

fn validate_declared_properties(
    action: &ActionName,
    args: &Value,
    schema: &Value,
    constraints: &Value,
) -> Result<(), BuildError> {
    let Some(args_obj) = args.as_object() else {
        return Ok(());
    };
    let Some(schema_obj) = schema.as_object() else {
        return Ok(());
    };
    if schema_obj.is_empty()
        || matches!(
            schema_obj.get("additionalProperties"),
            Some(Value::Bool(true))
        )
    {
        return Ok(());
    }

    let declared = declared_arg_names(schema, constraints);
    if declared.is_empty() {
        return Ok(());
    }
    for key in args_obj.keys() {
        if !declared.contains(key) {
            return Err(violation(
                action,
                format!("undeclared argument `{key}` is not allowed"),
            ));
        }
    }
    Ok(())
}

fn validate_constraints(
    action: &ActionName,
    args: &Value,
    constraints: &Value,
) -> Result<(), BuildError> {
    let (Some(constraints), Some(args_obj)) = (constraints.as_object(), args.as_object()) else {
        return Ok(());
    };
    for (name, spec) in constraints {
        let Some(value) = args_obj.get(name) else {
            continue;
        };
        if let Some(allowed) = spec.get("enum").and_then(Value::as_array) {
            if !allowed.iter().any(|a| a == value) {
                return Err(violation(
                    action,
                    format!("argument `{name}` is not an allowed value"),
                ));
            }
        }
        if let Some(constant) = spec.get("const") {
            if constant != value {
                return Err(violation(
                    action,
                    format!("argument `{name}` must equal the fixed value"),
                ));
            }
        }
    }
    Ok(())
}

fn type_matches(expected: &str, value: &Value) -> bool {
    match expected {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.is_i64() || value.is_u64(),
        "boolean" => value.is_boolean(),
        "object" => value.is_object(),
        "array" => value.is_array(),
        "null" => value.is_null(),
        // Unknown type keyword: do not fail on something we don't model.
        _ => true,
    }
}

pub(crate) fn declared_arg_names(
    schema: &Value,
    constraints: &Value,
) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    if let Some(props) = schema.get("properties").and_then(Value::as_object) {
        out.extend(props.keys().cloned());
    }
    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        out.extend(
            required
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string),
        );
    }
    if let Some(constraints) = constraints.as_object() {
        out.extend(constraints.keys().cloned());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn act() -> ActionName {
        ActionName::new("read_repo_file")
    }

    #[test]
    fn empty_schema_accepts_anything() {
        assert!(validate(&act(), &json!({"anything": 1}), &Value::Null, &Value::Null).is_ok());
        assert!(validate(&act(), &json!({}), &json!({}), &json!({})).is_ok());
    }

    #[test]
    fn missing_required_argument_is_rejected() {
        let schema = json!({"type": "object", "required": ["path"]});
        let err = validate(&act(), &json!({}), &schema, &Value::Null).unwrap_err();
        assert!(matches!(err, BuildError::SchemaViolation { .. }));
    }

    #[test]
    fn present_required_argument_passes() {
        let schema = json!({"type": "object", "required": ["path"]});
        assert!(validate(
            &act(),
            &json!({"path": "src/lib.rs"}),
            &schema,
            &Value::Null
        )
        .is_ok());
    }

    #[test]
    fn wrong_property_type_is_rejected() {
        let schema = json!({"properties": {"path": {"type": "string"}}});
        let err = validate(&act(), &json!({"path": 7}), &schema, &Value::Null).unwrap_err();
        assert!(matches!(err, BuildError::SchemaViolation { .. }));
    }

    #[test]
    fn undeclared_properties_are_rejected_by_default() {
        let schema = json!({"properties": {"command": {"type": "string"}}});
        let err = validate(
            &act(),
            &json!({"command": "echo safe", "argv": ["sh", "-c", "echo pwned"]}),
            &schema,
            &Value::Null,
        )
        .unwrap_err();
        assert!(matches!(err, BuildError::SchemaViolation { .. }));
    }

    #[test]
    fn additional_properties_can_be_explicitly_allowed() {
        let schema = json!({
            "properties": {"command": {"type": "string"}},
            "additionalProperties": true
        });
        assert!(validate(
            &act(),
            &json!({"command": "echo safe", "argv": ["echo", "ok"]}),
            &schema,
            &Value::Null,
        )
        .is_ok());
    }

    #[test]
    fn enum_constraint_is_enforced() {
        let constraints = json!({"mode": {"enum": ["r", "w"]}});
        assert!(validate(&act(), &json!({"mode": "r"}), &Value::Null, &constraints).is_ok());
        let err = validate(&act(), &json!({"mode": "x"}), &Value::Null, &constraints).unwrap_err();
        assert!(matches!(err, BuildError::SchemaViolation { .. }));
    }

    #[test]
    fn const_constraint_is_enforced() {
        let constraints = json!({"command": {"const": "pytest"}});
        assert!(validate(
            &act(),
            &json!({"command": "pytest"}),
            &Value::Null,
            &constraints
        )
        .is_ok());
        let err = validate(
            &act(),
            &json!({"command": "rm"}),
            &Value::Null,
            &constraints,
        )
        .unwrap_err();
        assert!(matches!(err, BuildError::SchemaViolation { .. }));
    }
}
