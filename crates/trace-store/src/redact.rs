//! Redaction before write (E4.2, invariant 15).
//!
//! Secrets must never reach the audit log. This walks a JSON value and masks any
//! string/value whose key — or dotted path, e.g. `env.GITHUB_TOKEN` — matches a
//! manifest redaction pattern. Masking keeps the key present and the value a
//! string, so it does not change a decision's representability for the default
//! world (an `enum`/`const` constraint on a secret is a noted edge case).
//!
//! Patterns use a minimal `*` wildcard (any run, including `/`), so `**/.env`
//! degrades sensibly. Full glob / value-pattern semantics are deferred.

use serde_json::{Map, Value};

pub const REDACTED: &str = "[REDACTED]";

/// Return a copy of `value` with matching fields masked.
pub fn redact(value: &Value, patterns: &[String]) -> Value {
    redact_at(value, "", patterns)
}

fn redact_at(value: &Value, path: &str, patterns: &[String]) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = Map::with_capacity(map.len());
            for (key, child) in map {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                if matches_any(patterns, key) || matches_any(patterns, &child_path) {
                    out.insert(key.clone(), Value::String(REDACTED.to_string()));
                } else {
                    out.insert(key.clone(), redact_at(child, &child_path, patterns));
                }
            }
            Value::Object(out)
        }
        Value::Array(items) => {
            Value::Array(items.iter().map(|v| redact_at(v, path, patterns)).collect())
        }
        other => other.clone(),
    }
}

fn matches_any(patterns: &[String], text: &str) -> bool {
    patterns.iter().any(|p| glob_match(p, text))
}

/// Minimal wildcard match: `*` matches any run of characters (including empty
/// and `/`). Everything else matches literally.
fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut mark) = (None, 0usize);
    while ti < t.len() {
        if pi < p.len() && p[pi] == t[ti] {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn glob_basics() {
        assert!(glob_match("env.*_TOKEN", "env.GITHUB_TOKEN"));
        assert!(glob_match("env.*_KEY", "env.AWS_SECRET_KEY"));
        assert!(glob_match("**/.env", "config/sub/.env"));
        assert!(!glob_match("env.*_TOKEN", "env.GITHUB_USER"));
        assert!(glob_match("path", "path"));
    }

    #[test]
    fn redacts_matching_nested_key() {
        let patterns = vec!["env.*_TOKEN".to_string()];
        let value = json!({
            "path": "src/lib.rs",
            "env": { "GITHUB_TOKEN": "ghp_supersecret", "USER": "dev" }
        });
        let out = redact(&value, &patterns);
        assert_eq!(out["env"]["GITHUB_TOKEN"], json!(REDACTED));
        assert_eq!(out["env"]["USER"], json!("dev"));
        assert_eq!(out["path"], json!("src/lib.rs"));
        // The secret must not survive anywhere in the serialized form.
        assert!(!serde_json::to_string(&out)
            .unwrap()
            .contains("ghp_supersecret"));
    }

    #[test]
    fn no_patterns_is_identity() {
        let value = json!({"a": 1, "b": {"c": "x"}});
        assert_eq!(redact(&value, &[]), value);
    }
}
