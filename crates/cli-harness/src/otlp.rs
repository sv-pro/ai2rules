//! `harness export-otlp` — the host decision log as OpenTelemetry traces (AI2-66).
//!
//! D84 kept telemetry out of the hook: the OpenTelemetry crates are not in the
//! offline crate set, and a hook that opens a connection on every tool call adds
//! latency and egress from inside the governed session. This converts a
//! **finished** log instead, offline, into OTLP/JSON: one
//! `ExportTraceServiceRequest` per line, the shape a collector's `otlpjsonfile`
//! receiver reads. Nothing here touches the network or the hook.
//!
//! The mapping follows the GenAI semantic conventions' *execute tool* span
//! (`open-telemetry/semantic-conventions-genai`, status Development):
//!
//! - **one trace per session**, so proposal, approval and execution of every
//!   call in a session share a trace id;
//! - **one span per decision line**, named `execute_tool {tool}`, kind
//!   `INTERNAL`, from the gate's decision to the call's outcome;
//! - `gen_ai.operation.name`, `gen_ai.tool.name`, `gen_ai.conversation.id`,
//!   `gen_ai.tool.call.id` (only when the host supplied one, as the convention
//!   requires), `gen_ai.agent.name` (the subagent type), and `error.type` when the
//!   call failed or the gate refused it;
//! - `ai2rules.*` attributes for the verdict, plus two span events, the decision
//!   and the outcome, so the timeline survives inside the span.
//!
//! The conventions make tool arguments and results opt-in because they may be
//! sensitive. The log only ever holds redacted summaries (D66), but they are
//! still exported only with `--with-summaries`, and under `ai2rules.*` names:
//! `gen_ai.tool.call.arguments` must be the argument *object*, which the log
//! deliberately does not keep.
//!
//! An ASK's answer stays labelled as what it is: `ai2rules.human_answer` is
//! inferred from whether the call ran (D84), never attested.

use crate::outcome::{join, read_log};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;

/// `SPAN_KIND_INTERNAL`, `STATUS_CODE_UNSET` / `_OK` / `_ERROR` in OTLP.
const SPAN_KIND_INTERNAL: u8 = 1;
const STATUS_UNSET: u8 = 0;
const STATUS_OK: u8 = 1;
const STATUS_ERROR: u8 = 2;

pub struct Options<'a> {
    pub log: &'a Path,
    pub out: Option<&'a Path>,
    pub with_summaries: bool,
}

pub fn run(opts: &Options<'_>) -> i32 {
    let lines = match read_log(opts.log) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("export-otlp: cannot read {}: {e}", opts.log.display());
            return 2;
        }
    };
    let request = export(&lines, opts.with_summaries);
    let spans = request["resourceSpans"][0]["scopeSpans"][0]["spans"]
        .as_array()
        .map_or(0, Vec::len);
    let text = format!("{request}\n");
    match opts.out {
        Some(path) => {
            if let Err(e) = std::fs::write(path, text) {
                eprintln!("export-otlp: cannot write {}: {e}", path.display());
                return 2;
            }
            eprintln!("export-otlp: {spans} spans -> {}", path.display());
        }
        None => print!("{text}"),
    }
    0
}

/// A stable id of `bytes` hex characters for `seed`. Ids only need to be stable
/// across runs and well spread, not secret, so the `md5` already in the build
/// is enough; a run over the same log always yields the same trace.
fn hex_id(seed: &str, bytes: usize) -> String {
    md5::compute(seed.as_bytes())[..bytes]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn nanos(ms: u64) -> String {
    (u128::from(ms) * 1_000_000).to_string()
}

/// An OTLP `KeyValue`. Integers are strings in OTLP/JSON (protobuf int64).
fn attr(key: &str, value: &Value) -> Option<Value> {
    let v = match value {
        Value::String(s) => json!({ "stringValue": s }),
        Value::Bool(b) => json!({ "boolValue": b }),
        Value::Number(n) if n.is_i64() || n.is_u64() => json!({ "intValue": n.to_string() }),
        Value::Number(n) => json!({ "doubleValue": n.as_f64() }),
        _ => return None,
    };
    Some(json!({ "key": key, "value": v }))
}

fn attrs(pairs: &[(&str, Value)]) -> Vec<Value> {
    pairs.iter().filter_map(|(k, v)| attr(k, v)).collect()
}

/// The whole log as one `ExportTraceServiceRequest`.
pub fn export(lines: &[Value], with_summaries: bool) -> Value {
    let text = |l: &Value, k: &str| l.get(k).and_then(Value::as_str).map(str::to_string);
    let key = |l: &Value| -> Option<(String, String)> {
        Some((
            text(l, "session").unwrap_or_default(),
            text(l, "tool_use_id")?,
        ))
    };

    let mut outcomes: HashMap<(String, String), &Value> = HashMap::new();
    for l in lines.iter().filter(|l| l["kind"] == "outcome") {
        if let Some(k) = key(l) {
            outcomes.insert(k, l);
        }
    }
    // join preserves decision order and skips only decisions without call ids.
    // A map keyed by call would overwrite the earlier decisions' ASK/DENY
    // inferences when duplicate hook registrations observe the same call.
    let mut joined = join(lines).into_iter();
    let mut occurrences: HashMap<(String, Option<String>), u64> = HashMap::new();

    let mut spans = Vec::new();
    let mut host = None;
    for d in lines {
        // A missing `kind` is a pre-D84 decision line.
        if !matches!(
            d.get("kind").and_then(Value::as_str),
            None | Some("decision")
        ) {
            continue;
        }
        let session = text(d, "session").unwrap_or_else(|| "default".into());
        let tool = text(d, "tool").unwrap_or_default();
        let id = text(d, "tool_use_id");
        let k = id.clone().map(|id| (session.clone(), id));
        let outcome = k.as_ref().and_then(|k| outcomes.get(k)).copied();
        let j = id.as_ref().and_then(|_| joined.next());
        let next = occurrences
            .entry((session.clone(), id.clone()))
            .or_default();
        let decision_index = *next;
        *next += 1;
        host = host.or_else(|| text(d, "host"));

        let start = d["ts_ms"].as_u64().unwrap_or(0);
        let end = outcome
            .and_then(|o| o["ts_ms"].as_u64())
            .filter(|e| *e >= start)
            .unwrap_or(start);
        let answer = text(d, "answer");
        let status = outcome.and_then(|o| text(o, "status"));

        // Span status, per the Recording Errors convention: a refusal by the
        // gate and a failed or interrupted tool are errors; a call that did not
        // run for any other reason (a declined prompt) is not.
        let (code, error_type, message) = match (answer.as_deref(), status.as_deref()) {
            (Some("deny"), None) => (STATUS_ERROR, Some("ai2rules.denied"), text(d, "reason")),
            (_, Some("error")) => (
                STATUS_ERROR,
                Some("tool_error"),
                outcome.and_then(|o| text(o, "error")),
            ),
            (_, Some("interrupted")) => (STATUS_ERROR, Some("interrupted"), None),
            (_, Some(_)) => (STATUS_OK, None, None),
            _ => (STATUS_UNSET, None, None),
        };

        let mut a = attrs(&[
            ("gen_ai.operation.name", json!("execute_tool")),
            ("gen_ai.tool.name", json!(tool)),
            ("gen_ai.conversation.id", json!(session)),
            ("ai2rules.decision", d["decision"].clone()),
            ("ai2rules.action", d["action"].clone()),
            ("ai2rules.rule", d["rule"].clone()),
            (
                "ai2rules.answer",
                json!(answer.as_deref().unwrap_or("none")),
            ),
            ("ai2rules.taint", d["taint"].clone()),
            ("ai2rules.manifest_hash", d["manifest_hash"].clone()),
            ("ai2rules.host", d["host"].clone()),
            ("ai2rules.step_idx", d["step_idx"].clone()),
            ("ai2rules.decision_index", json!(decision_index)),
        ]);
        if let Some(id) = &id {
            a.extend(attr("gen_ai.tool.call.id", &json!(id)));
        }
        // `gen_ai.agent.name` is the human-readable agent; Claude Code's
        // `agent_type` ("Explore") is that. Its `agent_id` names one subagent
        // run, not the stable hosted-agent resource `gen_ai.agent.id` means.
        a.extend(attr("gen_ai.agent.name", &d["agent_type"]));
        a.extend(attr("ai2rules.agent_id", &d["agent_id"]));
        a.extend(attr("error.type", &json!(error_type)));
        if let Some(j) = j {
            a.extend(attr("ai2rules.ran", &json!(j.ran)));
            a.extend(attr("ai2rules.human_answer", &json!(j.human_answer)));
            if j.ran_despite_deny {
                a.extend(attr("ai2rules.ran_despite_deny", &json!(true)));
            }
        }
        if let Some(o) = outcome {
            a.extend(attrs(&[
                ("ai2rules.status", o["status"].clone()),
                ("ai2rules.exit_code", o["exit_code"].clone()),
                ("ai2rules.duration_ms", o["duration_ms"].clone()),
            ]));
        }
        if with_summaries {
            a.extend(attr("ai2rules.call", &d["call"]));
            if let Some(o) = outcome {
                a.extend(attr("ai2rules.result", &o["result"]));
            }
        }

        let mut events = vec![json!({
            "timeUnixNano": nanos(start),
            "name": "ai2rules.decision",
            "attributes": attrs(&[
                ("ai2rules.decision", d["decision"].clone()),
                ("ai2rules.answer", json!(answer.as_deref().unwrap_or("none"))),
                ("ai2rules.human_answer", d["human_answer"].clone()),
            ]),
        })];
        if let Some(o) = outcome {
            events.push(json!({
                "timeUnixNano": nanos(end),
                "name": "ai2rules.outcome",
                "attributes": attrs(&[
                    ("ai2rules.status", o["status"].clone()),
                    ("ai2rules.exit_code", o["exit_code"].clone()),
                ]),
            }));
        }

        // Keep every decision, including byte-identical duplicate hook events.
        // The per-call ordinal is stable for a finished log and unaffected by
        // interleaved outcomes, policy records or other calls. The original
        // host call id correlates these spans; it is never invented or changed.
        // JSON framing separates absent ids from strings and avoids delimiter
        // ambiguity in host-supplied session/call ids.
        let span_seed = json!(["ai2rules-decision", session, id, decision_index]).to_string();
        let mut span = json!({
            "traceId": hex_id(&format!("ai2rules-session\u{1f}{session}"), 16),
            "spanId": hex_id(&span_seed, 8),
            "name": format!("execute_tool {tool}"),
            "kind": SPAN_KIND_INTERNAL,
            "startTimeUnixNano": nanos(start),
            "endTimeUnixNano": nanos(end),
            "attributes": a,
            "events": events,
            "status": { "code": code },
        });
        if let Some(m) = message {
            span["status"]["message"] = json!(m);
        }
        spans.push(span);
    }

    json!({ "resourceSpans": [{
        "resource": { "attributes": attrs(&[
            ("service.name", json!("ai2rules-harness")),
            ("ai2rules.host", json!(host)),
        ]) },
        "scopeSpans": [{
            "scope": { "name": "ai2rules.host_decision_log", "version": env!("CARGO_PKG_VERSION") },
            "spans": spans,
        }],
    }]})
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans(v: &Value) -> &Vec<Value> {
        v["resourceSpans"][0]["scopeSpans"][0]["spans"]
            .as_array()
            .unwrap()
    }

    fn attr_of<'a>(span: &'a Value, key: &str) -> Option<&'a Value> {
        span["attributes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["key"] == key)
            .map(|a| &a["value"])
    }

    #[test]
    fn ids_are_stable_hex_of_the_right_width() {
        assert_eq!(hex_id("x", 16).len(), 32);
        assert_eq!(hex_id("x", 8).len(), 16);
        assert_eq!(hex_id("x", 8), hex_id("x", 8));
        assert_ne!(hex_id("x", 8), hex_id("y", 8));
    }

    #[test]
    fn integers_are_strings_and_nulls_are_dropped() {
        assert_eq!(attr("n", &json!(3)).unwrap()["value"]["intValue"], "3");
        assert_eq!(attr("b", &json!(true)).unwrap()["value"]["boolValue"], true);
        assert!(attr("z", &Value::Null).is_none());
        assert_eq!(nanos(2), "2000000");
    }

    #[test]
    fn a_pre_d84_line_still_exports_without_a_call_id() {
        let v = export(
            &[
                json!({"ts_ms": 5, "session": "s", "tool": "Read", "decision": "ALLOW", "answer": null}),
            ],
            false,
        );
        let s = &spans(&v)[0];
        assert_eq!(s["name"], "execute_tool Read");
        assert!(attr_of(s, "gen_ai.tool.call.id").is_none());
        assert_eq!(s["status"]["code"], STATUS_UNSET);
    }

    #[test]
    fn policy_and_outcome_lines_are_not_spans() {
        let v = export(
            &[
                json!({"kind": "policy", "ts_ms": 1}),
                json!({"kind": "outcome", "ts_ms": 2, "session": "s", "tool_use_id": "t"}),
            ],
            false,
        );
        assert!(spans(&v).is_empty());
    }
}
