//! Argument validation against a frozen `Descriptor` (E2.1, D86).
//!
//! This is deliberately *not* a full JSON Schema implementation — it validates a
//! small subset with the lean dependency set (serde/serde_json only). The subset
//! is fixed in `harness_types::arg_schema`: annotation keywords (`description`,
//! `format`, …) are read by the model only, and every other keyword is one this
//! module enforces:
//!
//! - `type` (one name or a list), `enum`, `const`;
//! - `minimum` / `maximum` / `exclusiveMinimum` / `exclusiveMaximum` on numbers;
//! - `minLength` / `maxLength` on strings, counted in characters;
//! - `items` / `minItems` / `maxItems` on arrays;
//! - `properties` / `required` / `additionalProperties` (boolean) on objects.
//!   Objects are closed by default: when a schema declares properties or
//!   required keys, any undeclared key is rejected unless
//!   `additionalProperties: true` is explicitly present.
//!
//! The same vocabulary applies to the root `schema` (the arguments object), to
//! each property, and to each `arg_constraints` entry. A keyword that only makes
//! sense for one JSON type (`maximum`, `maxLength`, …) refuses a value of another
//! type unless the schema's own `type` admits it, so a limit cannot be stepped
//! around by sending `"10001"` for a number.
//!
//! Anything outside the subset is refused, never skipped: the compiler will not
//! build such a world, and `validate` denies the call if a descriptor reaches it
//! anyway (a `CompiledWorld` can be assembled without the compiler).

use std::cmp::Ordering;
use std::collections::BTreeSet;

use harness_types::{check_action_schema, ActionName, ArgSource, BuildError, CompiledWorld};
use serde_json::{Map, Value};

/// Validate `args` against an action's model-facing `schema` and
/// `arg_constraints`. Returns `SchemaViolation` on the first failure.
pub fn validate(
    action: &ActionName,
    args: &Value,
    schema: &Value,
    constraints: &Value,
) -> Result<(), BuildError> {
    // Backstop (D86): a limit the kernel cannot evaluate denies the call rather
    // than letting it through unchecked.
    if let Err(problem) = check_action_schema(schema, constraints) {
        return Err(violation(
            action,
            format!(
                "the world limits this action in a way the kernel cannot check \
                 ({problem}), so the call is refused rather than let through unchecked"
            ),
        ));
    }

    // A call with no arguments is an empty arguments object.
    let empty = Value::Object(Map::new());
    let args = if args.is_null() { &empty } else { args };

    if let Some(root) = schema.as_object().filter(|s| !s.is_empty()) {
        let declared = declared_arg_names(schema, constraints);
        check(action, args, root, "", &declared)?;
    }
    if let (Some(constraints), Some(args_obj)) = (constraints.as_object(), args.as_object()) {
        for (name, spec) in constraints {
            let (Some(value), Some(spec)) = (args_obj.get(name), spec.as_object()) else {
                continue;
            };
            check(action, value, spec, name, &BTreeSet::new())?;
        }
    }
    Ok(())
}

fn violation(action: &ActionName, detail: impl Into<String>) -> BuildError {
    BuildError::SchemaViolation {
        action: action.clone(),
        detail: detail.into(),
    }
}

/// How a message names the value at `path`: the root is the arguments object.
fn shown(path: &str) -> String {
    if path.is_empty() {
        "arguments".to_string()
    } else {
        format!("argument `{path}`")
    }
}

fn child(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_string()
    } else {
        format!("{path}.{key}")
    }
}

/// Check `value` (found at `path`) against one schema object. `also_declared`
/// names keys an object may carry besides its own `properties` / `required`: at
/// the root, the `arg_constraints` names.
fn check(
    action: &ActionName,
    value: &Value,
    spec: &Map<String, Value>,
    path: &str,
    also_declared: &BTreeSet<String>,
) -> Result<(), BuildError> {
    if let Some(ty) = spec.get("type") {
        if !type_admits(ty, value) {
            return Err(violation(
                action,
                format!("{} must be {}", shown(path), type_label(ty)),
            ));
        }
    }
    if let Some(allowed) = spec.get("enum").and_then(Value::as_array) {
        if !allowed.iter().any(|a| a == value) {
            return Err(violation(
                action,
                format!("{} is not an allowed value", shown(path)),
            ));
        }
    }
    if let Some(constant) = spec.get("const") {
        if constant != value {
            return Err(violation(
                action,
                format!("{} must equal the fixed value", shown(path)),
            ));
        }
    }
    check_number(action, value, spec, path)?;
    check_string(action, value, spec, path)?;
    check_array(action, value, spec, path)?;
    check_object(action, value, spec, path, also_declared)
}

/// Whether the type-specific `keywords` present in `spec` apply to `value`.
/// They do when `value` is of the kind they constrain. When it is not, they are
/// skipped only if the schema's own `type` admitted the value (an author who
/// wrote `type: [integer, "null"]` allowed `null`); otherwise the value is
/// refused, because a limit that a value of the wrong type could skip is a limit
/// that does not bind.
fn applies(
    action: &ActionName,
    value: &Value,
    spec: &Map<String, Value>,
    path: &str,
    keywords: &[&str],
    is_kind: fn(&Value) -> bool,
    kind: &str,
) -> Result<bool, BuildError> {
    let Some(keyword) = keywords.iter().find(|k| spec.contains_key(**k)) else {
        return Ok(false);
    };
    if is_kind(value) {
        return Ok(true);
    }
    if spec.contains_key("type") {
        return Ok(false);
    }
    Err(violation(
        action,
        format!(
            "{} must be {kind} to be checked against `{keyword}`",
            shown(path)
        ),
    ))
}

fn check_number(
    action: &ActionName,
    value: &Value,
    spec: &Map<String, Value>,
    path: &str,
) -> Result<(), BuildError> {
    const BOUNDS: &[&str] = &["minimum", "maximum", "exclusiveMinimum", "exclusiveMaximum"];
    if !applies(
        action,
        value,
        spec,
        path,
        BOUNDS,
        Value::is_number,
        "a number",
    )? {
        return Ok(());
    }
    // (keyword, how the value must compare with the bound, phrase for the message)
    type Bound = (&'static str, fn(Ordering) -> bool, &'static str);
    let rules: [Bound; 4] = [
        ("minimum", |o| o != Ordering::Less, "at least"),
        ("maximum", |o| o != Ordering::Greater, "at most"),
        (
            "exclusiveMinimum",
            |o| o == Ordering::Greater,
            "greater than",
        ),
        ("exclusiveMaximum", |o| o == Ordering::Less, "less than"),
    ];
    for (keyword, holds, phrase) in rules {
        let Some(bound) = spec.get(keyword) else {
            continue;
        };
        if !compare(value, bound).is_some_and(holds) {
            return Err(violation(
                action,
                format!("{} must be {phrase} {bound}", shown(path)),
            ));
        }
    }
    Ok(())
}

/// Compare two JSON numbers, exactly when both are integers (an `f64` cannot
/// hold every `u64`), as floats otherwise.
fn compare(a: &Value, b: &Value) -> Option<Ordering> {
    let int = |v: &Value| {
        v.as_i64()
            .map(i128::from)
            .or_else(|| v.as_u64().map(i128::from))
    };
    match (int(a), int(b)) {
        (Some(x), Some(y)) => Some(x.cmp(&y)),
        _ => a.as_f64()?.partial_cmp(&b.as_f64()?),
    }
}

fn check_string(
    action: &ActionName,
    value: &Value,
    spec: &Map<String, Value>,
    path: &str,
) -> Result<(), BuildError> {
    const LENGTHS: &[&str] = &["minLength", "maxLength"];
    if !applies(
        action,
        value,
        spec,
        path,
        LENGTHS,
        Value::is_string,
        "a string",
    )? {
        return Ok(());
    }
    // JSON Schema counts characters (code points), not bytes.
    let len = value.as_str().map_or(0, |s| s.chars().count()) as u64;
    check_count(
        action,
        spec,
        path,
        len,
        ("minLength", "maxLength"),
        "characters long",
    )
}

fn check_array(
    action: &ActionName,
    value: &Value,
    spec: &Map<String, Value>,
    path: &str,
) -> Result<(), BuildError> {
    const ARRAY: &[&str] = &["items", "minItems", "maxItems"];
    if !applies(action, value, spec, path, ARRAY, Value::is_array, "a list")? {
        return Ok(());
    }
    let items = value.as_array().map(Vec::as_slice).unwrap_or_default();
    let count = items.len() as u64;
    check_count(
        action,
        spec,
        path,
        count,
        ("minItems", "maxItems"),
        "items long",
    )?;
    if let Some(item_spec) = spec.get("items").and_then(Value::as_object) {
        for (i, item) in items.iter().enumerate() {
            let at = format!("{path}[{i}]");
            check(action, item, item_spec, &at, &BTreeSet::new())?;
        }
    }
    Ok(())
}

/// Check a length against the `(min, max)` keywords that bound it.
fn check_count(
    action: &ActionName,
    spec: &Map<String, Value>,
    path: &str,
    count: u64,
    (min_keyword, max_keyword): (&str, &str),
    unit: &str,
) -> Result<(), BuildError> {
    if let Some(min) = spec.get(min_keyword).and_then(Value::as_u64) {
        if count < min {
            return Err(violation(
                action,
                format!("{} must be at least {min} {unit}", shown(path)),
            ));
        }
    }
    if let Some(max) = spec.get(max_keyword).and_then(Value::as_u64) {
        if count > max {
            return Err(violation(
                action,
                format!("{} must be at most {max} {unit}", shown(path)),
            ));
        }
    }
    Ok(())
}

fn check_object(
    action: &ActionName,
    value: &Value,
    spec: &Map<String, Value>,
    path: &str,
    also_declared: &BTreeSet<String>,
) -> Result<(), BuildError> {
    const OBJECT: &[&str] = &["properties", "required", "additionalProperties"];
    if !applies(
        action,
        value,
        spec,
        path,
        OBJECT,
        Value::is_object,
        "an object",
    )? {
        return Ok(());
    }
    let Some(obj) = value.as_object() else {
        return Ok(());
    };

    let required: Vec<&str> = spec
        .get("required")
        .and_then(Value::as_array)
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    for key in &required {
        if !obj.contains_key(*key) {
            return Err(violation(
                action,
                format!("missing required argument `{}`", child(path, key)),
            ));
        }
    }

    let props = spec.get("properties").and_then(Value::as_object);
    for (name, prop) in props.into_iter().flatten() {
        let (Some(arg), Some(prop)) = (obj.get(name), prop.as_object()) else {
            continue;
        };
        check(action, arg, prop, &child(path, name), &BTreeSet::new())?;
    }

    // Closed by default: once keys are declared, an undeclared one is refused
    // unless the schema says `additionalProperties: true`.
    if matches!(spec.get("additionalProperties"), Some(Value::Bool(true))) {
        return Ok(());
    }
    let mut declared: BTreeSet<&str> = also_declared.iter().map(String::as_str).collect();
    declared.extend(props.into_iter().flat_map(|p| p.keys().map(String::as_str)));
    declared.extend(required.iter().copied());
    let closed = !declared.is_empty()
        || matches!(spec.get("additionalProperties"), Some(Value::Bool(false)));
    if closed {
        if let Some(key) = obj.keys().find(|k| !declared.contains(k.as_str())) {
            return Err(violation(
                action,
                format!("undeclared argument `{}` is not allowed", child(path, key)),
            ));
        }
    }
    Ok(())
}

fn type_admits(ty: &Value, value: &Value) -> bool {
    match ty {
        Value::String(name) => type_matches(name, value),
        Value::Array(names) => names
            .iter()
            .filter_map(Value::as_str)
            .any(|n| type_matches(n, value)),
        _ => false,
    }
}

fn type_label(ty: &Value) -> String {
    match ty {
        Value::Array(names) => names
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(" or "),
        other => other.as_str().unwrap_or_default().to_string(),
    }
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
        // `check_action_schema` refuses any other name before we get here; a name
        // the kernel does not model admits nothing.
        _ => false,
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

/// The argument schema the model is shown for `action` (D79), or `None` when the
/// world does not describe it.
///
/// A base action shows its descriptor schema unchanged. A scoped capability shows
/// only what the actor supplies: locked and context arguments are left out, each
/// argument appears under the name the actor uses, and an input rule's limits are
/// stated as `enum` (a list) and `pattern` (a prefix) so the model can see them
/// before it calls. The kernel enforces the same rules at the gate; this is the
/// advertisement, not the check.
pub fn model_facing_schema(world: &CompiledWorld, action: &ActionName) -> Option<Value> {
    let descriptor = world.descriptor(action)?;
    let Some(cap) = world.scoped_capability(action) else {
        return Some(descriptor.schema.clone());
    };
    let base_props = descriptor
        .schema
        .get("properties")
        .and_then(Value::as_object);
    let base_required: Vec<&str> = descriptor
        .schema
        .get("required")
        .and_then(Value::as_array)
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();

    let mut properties = Map::new();
    let mut required = Vec::new();
    for (name, source) in &cap.args {
        let Some(actor_name) = source.actor_name(name) else {
            continue;
        };
        let mut spec = base_props
            .and_then(|p| p.get(name))
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        if let ArgSource::Input(rule) = source {
            if !rule.one_of.is_empty() {
                spec.insert("enum".to_string(), Value::Array(rule.one_of.clone()));
            }
            if let Some(prefix) = &rule.prefix {
                spec.insert("type".to_string(), Value::from("string"));
                spec.insert(
                    "pattern".to_string(),
                    Value::from(format!("^{}", escape_regex(prefix))),
                );
            }
        }
        properties.insert(actor_name.to_string(), Value::Object(spec));
        if base_required.contains(&name.as_str()) {
            required.push(Value::from(actor_name));
        }
    }

    let mut schema = Map::new();
    schema.insert("type".to_string(), Value::from("object"));
    schema.insert("properties".to_string(), Value::Object(properties));
    if !required.is_empty() {
        schema.insert("required".to_string(), Value::Array(required));
    }
    // Anything else the actor sends is stripped before execution (invariant 12),
    // so the honest advertisement is a closed object.
    schema.insert("additionalProperties".to_string(), Value::Bool(false));
    Some(Value::Object(schema))
}

/// Escape `text` so it matches literally inside an ECMA/JSON-Schema regex. Only
/// the syntax characters: `\-` and `\/` are invalid escapes in Unicode mode, which
/// JSON Schema validators commonly use, and a Jira key like `PLAT-` has a `-`.
fn escape_regex(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if "\\^$.|?*+()[]{}".contains(c) {
            out.push('\\');
        }
        out.push(c);
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

    // --- The enforced vocabulary (D86): each keyword's pass and fail ---

    /// Validate `value` as the argument `x` constrained by `constraint`.
    fn constrained(constraint: Value, value: Value) -> Result<(), BuildError> {
        validate(
            &act(),
            &json!({ "x": value }),
            &Value::Null,
            &json!({ "x": constraint }),
        )
    }

    fn refused(constraint: Value, value: Value) -> String {
        match constrained(constraint.clone(), value.clone()) {
            Err(BuildError::SchemaViolation { detail, .. }) => detail,
            other => panic!("{constraint} admitted {value}: {other:?}"),
        }
    }

    #[test]
    fn each_supported_keyword_admits_and_refuses() {
        let cases = [
            (json!({"type": "integer"}), json!(3), json!(3.5)),
            (json!({"type": ["string", "null"]}), json!(null), json!(1)),
            (json!({"enum": ["r", "w"]}), json!("r"), json!("x")),
            (json!({"const": "pytest"}), json!("pytest"), json!("rm")),
            (json!({"minimum": 1}), json!(1), json!(0)),
            (json!({"maximum": 10000}), json!(10000), json!(10001)),
            (json!({"maximum": 10000}), json!(9999.5), json!(10000.01)),
            (json!({"exclusiveMinimum": 0}), json!(0.1), json!(0)),
            (json!({"exclusiveMaximum": 100}), json!(99), json!(100)),
            (json!({"minLength": 2}), json!("ab"), json!("a")),
            (json!({"maxLength": 3}), json!("abc"), json!("abcd")),
            (json!({"minItems": 1}), json!(["a"]), json!([])),
            (
                json!({"maxItems": 2}),
                json!(["a", "b"]),
                json!(["a", "b", "c"]),
            ),
            (
                json!({"items": {"type": "string", "maxLength": 2}}),
                json!(["ab", "c"]),
                json!(["ab", "cde"]),
            ),
            (
                json!({"required": ["text"]}),
                json!({"text": "hi"}),
                json!({}),
            ),
            (
                json!({"properties": {"n": {"maximum": 5}}}),
                json!({"n": 5}),
                json!({"n": 6}),
            ),
            (
                json!({"properties": {"n": {}}, "additionalProperties": false}),
                json!({"n": 1}),
                json!({"n": 1, "m": 2}),
            ),
        ];
        for (constraint, admitted, rejected) in cases {
            assert!(
                constrained(constraint.clone(), admitted.clone()).is_ok(),
                "{constraint} refused {admitted}"
            );
            refused(constraint, rejected);
        }
    }

    #[test]
    fn annotations_constrain_nothing() {
        let constraint = json!({
            "description": "amount in cents", "title": "Amount", "format": "int64",
            "default": 1, "examples": [1], "$comment": "note"
        });
        assert!(constrained(constraint, json!("anything at all")).is_ok());
    }

    #[test]
    fn messages_say_which_limit_was_crossed() {
        assert!(refused(json!({"maximum": 10000}), json!(10001)).contains("at most 10000"));
        assert!(refused(json!({"maxLength": 3}), json!("abcd")).contains("at most 3 characters"));
        let nested = refused(
            json!({"items": {"properties": {"n": {"maximum": 1}}}}),
            json!([{"n": 1}, {"n": 2}]),
        );
        assert!(nested.contains("`x[1].n`"), "{nested}");
    }

    #[test]
    fn integer_bounds_compare_exactly_beyond_f64_precision() {
        // 2^53 + 1 rounds to 2^53 as an f64; the bound must still hold.
        let bound = json!({"maximum": 9_007_199_254_740_992_u64});
        assert!(constrained(bound.clone(), json!(9_007_199_254_740_992_u64)).is_ok());
        refused(bound, json!(9_007_199_254_740_993_u64));
    }

    #[test]
    fn a_limit_cannot_be_stepped_around_with_another_type() {
        // No `type` in the constraint: a string is not a number under the limit.
        let detail = refused(json!({"maximum": 10000}), json!("10001"));
        assert!(detail.contains("must be a number"), "{detail}");
        refused(json!({"maxLength": 3}), json!(12345));
        refused(json!({"maxItems": 1}), json!("not a list"));
        // A `type` that admits the other type is the author's explicit choice.
        assert!(constrained(
            json!({"type": ["number", "null"], "maximum": 1}),
            json!(null)
        )
        .is_ok());
    }

    #[test]
    fn schema_properties_carry_the_same_vocabulary() {
        let schema = json!({
            "type": "object",
            "properties": {"amount": {"type": "number", "maximum": 10000}}
        });
        assert!(validate(&act(), &json!({"amount": 10}), &schema, &Value::Null).is_ok());
        let err = validate(&act(), &json!({"amount": 10001}), &schema, &Value::Null);
        assert!(matches!(err, Err(BuildError::SchemaViolation { .. })));
    }

    #[test]
    fn an_unsupported_keyword_denies_the_call_as_a_backstop() {
        // The compiler refuses this world; a descriptor assembled without it must
        // still not let the call through with the limit unchecked.
        let detail = refused(json!({"pattern": "^[0-9]+$"}), json!("123"));
        assert!(detail.contains("`pattern`"), "{detail}");
        let schema = json!({"properties": {"x": {"type": "date"}}});
        let err = validate(&act(), &json!({"x": "2026-10-23"}), &schema, &Value::Null);
        assert!(matches!(err, Err(BuildError::SchemaViolation { .. })));
        // It denies even when the constrained argument is not supplied.
        let err = validate(
            &act(),
            &json!({}),
            &Value::Null,
            &json!({"x": {"multipleOf": 5}}),
        );
        assert!(matches!(err, Err(BuildError::SchemaViolation { .. })));
    }

    #[test]
    fn a_call_without_arguments_is_an_empty_object() {
        let schema = json!({"type": "object", "properties": {}});
        assert!(validate(&act(), &Value::Null, &schema, &Value::Null).is_ok());
        let schema = json!({"type": "object", "required": ["path"]});
        assert!(validate(&act(), &Value::Null, &schema, &Value::Null).is_err());
    }
}
