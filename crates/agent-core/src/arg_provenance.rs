//! Per-argument taint producer (PACT §3.3–3.4, arXiv:2605.11039).
//!
//! The kernel's L2 floor ([`world_kernel::invariants::effective_floor_taint`])
//! consumes a per-argument taint map, but something has to *produce* it from the
//! actual data flow. This module is that producer — and it is deliberately the
//! **deterministic, no-LLM** half of PACT's inference pipeline. PACT's §3.4 uses
//! "exact structural matching, role-aware heuristics, and an LLM classifier for
//! remaining ambiguous arguments"; we take the exact structural matching and
//! **refuse the classifier** (THESIS §3: no LLM in the trust path). The cost of
//! refusing it is precision, not soundness — anything we cannot prove falls
//! through to the ambient floor.
//!
//! Two rules, both fail-closed:
//!
//!   * an argument is **Tainted** only when its value is provably derived from a
//!     tainted source (verbatim-contained in a tainted output);
//!   * an argument is **Clean** only when *every* string in it is provably from a
//!     clean source (the trusted user request, or a clean prior output);
//!   * everything else is **omitted** — the kernel then falls back to the ambient
//!     scalar, exactly today's conservative behavior.
//!
//! So this can only ever *recover* a benign call whose authority-bearing argument
//! is verbatim from a trusted source. It never asserts clean-origin for a value
//! the model might have paraphrased out of tainted content — that is precisely
//! the case PACT hands to its LLM classifier and we hand to the ambient floor.

use std::collections::BTreeMap;

use harness_types::Taint;
use serde_json::Value;

/// Values shorter than this are not treated as provenance evidence: a bare `"1"`
/// or `"id"` coincidentally appearing in some text is not proof of origin.
const MIN_MATCH_LEN: usize = 4;

/// Compute a per-argument taint map for a proposed call's `arguments` object,
/// given the tainted and clean source corpora accumulated so far this session.
///
/// Only arguments with a *provable* verdict appear in the result; the rest are
/// omitted so the kernel's L2 check fails closed to the ambient floor.
pub fn arg_taint(
    arguments: &Value,
    tainted_sources: &[String],
    clean_sources: &[String],
) -> BTreeMap<String, Taint> {
    let mut out = BTreeMap::new();
    let Value::Object(map) = arguments else {
        return out;
    };
    for (name, value) in map {
        if let Some(taint) = classify(value, tainted_sources, clean_sources) {
            out.insert(name.clone(), taint);
        }
    }
    out
}

/// Classify one argument value. `None` means "cannot prove" → the caller omits
/// it and the kernel uses the ambient scalar.
fn classify(value: &Value, tainted: &[String], clean: &[String]) -> Option<Taint> {
    let leaves = string_leaves(value);
    if leaves.is_empty() {
        return None;
    }
    // Tainted wins (monotonic): any leaf provably from a tainted source.
    if leaves.iter().any(|s| corpus_contains(tainted, s)) {
        return Some(Taint::Tainted);
    }
    // Clean requires *every* leaf to be provably from a clean source.
    if leaves.iter().all(|s| corpus_contains(clean, s)) {
        return Some(Taint::Clean);
    }
    None
}

/// Verbatim containment of `needle` in any corpus entry, ignoring trivially short
/// needles. Substring, not equality: a URL embedded in a longer prompt or
/// document still counts as derived from it.
fn corpus_contains(corpus: &[String], needle: &str) -> bool {
    needle.len() >= MIN_MATCH_LEN && corpus.iter().any(|hay| hay.contains(needle))
}

/// Collect the string leaves of a JSON value (recursing into arrays/objects).
fn string_leaves(value: &Value) -> Vec<&str> {
    let mut acc = Vec::new();
    collect(value, &mut acc);
    acc
}

fn collect<'v>(value: &'v Value, acc: &mut Vec<&'v str>) {
    match value {
        Value::String(s) => acc.push(s.as_str()),
        Value::Array(items) => items.iter().for_each(|v| collect(v, acc)),
        Value::Object(map) => map.values().for_each(|v| collect(v, acc)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const GUIDE: &str = "https://docs.example/guide";

    #[test]
    fn clean_arg_verbatim_from_user_request_is_clean() {
        let clean = vec![format!("please fetch {GUIDE} for me")];
        let map = arg_taint(&json!({ "url": GUIDE }), &[], &clean);
        assert_eq!(map.get("url"), Some(&Taint::Clean));
    }

    #[test]
    fn arg_derived_from_tainted_output_is_tainted() {
        let tainted = vec![format!("runbook says fetch {GUIDE} then deploy")];
        let map = arg_taint(&json!({ "url": GUIDE }), &tainted, &[]);
        assert_eq!(map.get("url"), Some(&Taint::Tainted));
    }

    #[test]
    fn tainted_wins_when_value_is_in_both_corpora() {
        let both = vec![format!("... {GUIDE} ...")];
        let map = arg_taint(&json!({ "url": GUIDE }), &both, &both);
        assert_eq!(map.get("url"), Some(&Taint::Tainted));
    }

    #[test]
    fn unprovable_arg_is_omitted() {
        // Model-synthesized URL matching neither corpus → omitted → ambient floor.
        let map = arg_taint(
            &json!({ "url": "https://model.invented/path" }),
            &["some tainted doc".to_string()],
            &["some clean prompt".to_string()],
        );
        assert!(!map.contains_key("url"));
    }

    #[test]
    fn multi_leaf_arg_is_clean_only_if_all_leaves_clean() {
        let clean = vec![
            "alpha-token-value".to_string(),
            "beta-token-value".to_string(),
        ];
        // both leaves clean → Clean
        let both_clean = arg_taint(
            &json!({ "pair": ["alpha-token-value", "beta-token-value"] }),
            &[],
            &clean,
        );
        assert_eq!(both_clean.get("pair"), Some(&Taint::Clean));
        // one leaf unprovable → omitted
        let one_unknown = arg_taint(
            &json!({ "pair": ["alpha-token-value", "gamma-unknown-value"] }),
            &[],
            &clean,
        );
        assert!(!one_unknown.contains_key("pair"));
    }

    #[test]
    fn short_values_are_not_evidence() {
        // A 1-char value that happens to appear in the corpus is not provenance.
        let map = arg_taint(&json!({ "x": "a" }), &[], &["banana".to_string()]);
        assert!(!map.contains_key("x"));
    }
}
