//! The argument-schema vocabulary the kernel enforces (DECISIONS D86).
//!
//! A base action's `schema` and `arg_constraints` are written in a small subset of
//! JSON Schema. Every keyword in them is either an *annotation*, which constrains
//! nothing and is read only by the model, or an *assertion* the kernel checks on
//! every call. A keyword that is neither is refused: the compiler will not build a
//! world that uses one, and the kernel denies a call against a descriptor that uses
//! one. A limit an author wrote is never skipped.
//!
//! This module holds the vocabulary and the shape check, so the compiler and the
//! kernel agree on one list. Evaluating arguments against it is the kernel's job
//! (`world_kernel::schema`).

use serde_json::Value;

/// Keywords that describe an argument without limiting it. `format` is one: in
/// JSON Schema 2020-12 it is an annotation unless a validator opts in, and the
/// kernel does not.
pub const ANNOTATION_KEYWORDS: &[&str] = &[
    "title",
    "description",
    "default",
    "examples",
    "format",
    "deprecated",
    "readOnly",
    "writeOnly",
    "$schema",
    "$id",
    "$comment",
];

/// Keywords the kernel enforces.
pub const ASSERTION_KEYWORDS: &[&str] = &[
    "type",
    "enum",
    "const",
    "minimum",
    "maximum",
    "exclusiveMinimum",
    "exclusiveMaximum",
    "minLength",
    "maxLength",
    "items",
    "minItems",
    "maxItems",
    "properties",
    "required",
    "additionalProperties",
];

/// The JSON types a `type` keyword may name.
pub const TYPE_NAMES: &[&str] = &[
    "string", "number", "integer", "boolean", "object", "array", "null",
];

/// Why an action's schema or constraints cannot be enforced as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaProblem {
    /// A keyword the kernel does not enforce. `at` locates the schema object that
    /// holds it, e.g. `arg_constraints.amount` or `schema.properties.tags.items`.
    Unsupported { at: String, keyword: String },
    /// A known keyword whose value has the wrong shape (`maxLength: -1`,
    /// `type: date`, `enum: []`, …).
    Malformed { at: String, detail: String },
}

impl std::fmt::Display for SchemaProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SchemaProblem::Unsupported { at, keyword } => write!(
                f,
                "`{keyword}` at {at} is not a keyword the kernel enforces"
            ),
            SchemaProblem::Malformed { at, detail } => write!(f, "{at}: {detail}"),
        }
    }
}

/// Check that every keyword in an action's `schema` and `arg_constraints` is one
/// the kernel enforces or a pure annotation, and that each is well formed.
///
/// `schema` may be `null` or `{}` (no schema). Its root, when present, describes
/// the arguments object, so its `type` may only be `object`. `arg_constraints`
/// may be `null`; otherwise it maps argument names to schemas of the same
/// vocabulary.
pub fn check_action_schema(schema: &Value, constraints: &Value) -> Result<(), SchemaProblem> {
    match schema {
        Value::Null => {}
        Value::Object(root) => {
            if let Some(ty) = root.get("type") {
                if ty != "object" {
                    return Err(malformed(
                        "schema",
                        "the root describes the arguments object, so its `type` must be `object`",
                    ));
                }
            }
            check_spec(schema, "schema")?;
        }
        _ => return Err(malformed("schema", "must be a mapping")),
    }
    match constraints {
        Value::Null => {}
        Value::Object(map) => {
            for (name, spec) in map {
                check_spec(spec, &format!("arg_constraints.{name}"))?;
            }
        }
        _ => {
            return Err(malformed(
                "arg_constraints",
                "must map argument names to constraints",
            ))
        }
    }
    Ok(())
}

fn malformed(at: &str, detail: impl Into<String>) -> SchemaProblem {
    SchemaProblem::Malformed {
        at: at.to_string(),
        detail: detail.into(),
    }
}

fn check_spec(spec: &Value, at: &str) -> Result<(), SchemaProblem> {
    let Some(spec) = spec.as_object() else {
        return Err(malformed(at, "must be a mapping of schema keywords"));
    };
    for (keyword, value) in spec {
        let keyword = keyword.as_str();
        if ANNOTATION_KEYWORDS.contains(&keyword) {
            continue;
        }
        let bad = |what: &str| malformed(at, format!("`{keyword}` must be {what}"));
        match keyword {
            "type" => {
                let names: Vec<&Value> = match value {
                    Value::Array(list) if !list.is_empty() => list.iter().collect(),
                    Value::Array(_) => return Err(bad("a type name or a non-empty list")),
                    other => vec![other],
                };
                for name in names {
                    if !name.as_str().is_some_and(|n| TYPE_NAMES.contains(&n)) {
                        return Err(bad(&format!("one of {}", TYPE_NAMES.join(", "))));
                    }
                }
            }
            "enum" => {
                // An empty list admits nothing: an authoring mistake, not a policy.
                if !value.as_array().is_some_and(|l| !l.is_empty()) {
                    return Err(bad("a non-empty list"));
                }
            }
            "const" => {}
            "minimum" | "maximum" | "exclusiveMinimum" | "exclusiveMaximum" => {
                if !value.is_number() {
                    return Err(bad("a number"));
                }
            }
            "minLength" | "maxLength" | "minItems" | "maxItems" => {
                if value.as_u64().is_none() {
                    return Err(bad("a non-negative integer"));
                }
            }
            "items" => check_spec(value, &format!("{at}.items"))?,
            "properties" => {
                let Some(props) = value.as_object() else {
                    return Err(bad("a mapping of argument names to schemas"));
                };
                for (name, prop) in props {
                    check_spec(prop, &format!("{at}.properties.{name}"))?;
                }
            }
            "required" => {
                if !value
                    .as_array()
                    .is_some_and(|l| l.iter().all(Value::is_string))
                {
                    return Err(bad("a list of argument names"));
                }
            }
            // Only the boolean form: a schema for extra keys would be a second
            // vocabulary to enforce.
            "additionalProperties" => {
                if !value.is_boolean() {
                    return Err(bad("true or false"));
                }
            }
            _ => {
                return Err(SchemaProblem::Unsupported {
                    at: at.to_string(),
                    keyword: keyword.to_string(),
                })
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn every_known_keyword_is_either_an_annotation_or_an_assertion() {
        for k in ANNOTATION_KEYWORDS {
            assert!(!ASSERTION_KEYWORDS.contains(k), "{k} is listed twice");
        }
    }

    #[test]
    fn unsupported_keywords_are_named_with_their_location() {
        let err = check_action_schema(
            &json!({"properties": {"tags": {"type": "array", "items": {"pattern": "^a"}}}}),
            &Value::Null,
        )
        .unwrap_err();
        assert_eq!(
            err,
            SchemaProblem::Unsupported {
                at: "schema.properties.tags.items".to_string(),
                keyword: "pattern".to_string()
            }
        );
    }
}
