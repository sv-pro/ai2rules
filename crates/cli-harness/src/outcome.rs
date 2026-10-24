//! The host decision log, joined to what happened next (D84, gap G2).
//!
//! `cc-hook --log` writes one line per verdict. On its own that line cannot say
//! which tool call it was, which agent made it, what the person answered to an
//! ASK, or whether the call ran. This module adds those:
//!
//! - **Identity on every decision line**: `tool_use_id`, `agent_id`,
//!   `agent_type` and `cwd`, copied from the host event when the host sends them
//!   (`null` when it does not).
//! - **An outcome line per executed call**, written by the same hook binary when
//!   the host fires `PostToolUse` or `PostToolUseFailure`. It is keyed by
//!   `tool_use_id` and says how the call ended: status, exit code, error, and a
//!   short result summary that has been through D66 redaction.
//! - **[`join`]**, which pairs the two. An ASK followed by an outcome means the
//!   person approved it; an ASK with no outcome means they declined, or the
//!   session ended first. The hook never sees the answer itself, so this is
//!   **inferred, not attested**, and the decision line says so
//!   (`"human_answer": "inferred"`).
//!
//! The observer is read-only (D77's rule): it prints nothing, always exits 0,
//! and never touches taint, budget or the verdict. A log that cannot be written
//! is reported on stderr, as the decision log always has been.

use crate::cc_hook::call_summary;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use trace_store::{redact, secrets::mask_secrets};

/// What a decision line says about the person's answer to an ASK: the hook
/// cannot see it, so it is read from whether an outcome line follows.
pub(crate) const HUMAN_ANSWER_INFERRED: &str = "inferred";

/// How much of a result the outcome line keeps.
const RESULT_SUMMARY_MAX: usize = 240;
/// A string field inside a structured result longer than this is logged as its
/// size, not its text: file bodies and patches are data, not an outcome.
const RESULT_FIELD_MAX: usize = 80;
/// How much of a command's output is kept: its tail, which is where a command
/// says how it ended.
const OUTPUT_TAIL: usize = 120;

/// Who made a call and where, as the host reported it.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct HostIds {
    pub host: &'static str,
    pub tool_use_id: Option<String>,
    pub agent_id: Option<String>,
    pub agent_type: Option<String>,
    pub cwd: Option<String>,
    /// Antigravity only: it sends no tool-call id, and `stepIdx` is the nearest
    /// key into its transcript.
    pub step_idx: Option<u64>,
}

fn str_field(ev: &Value, key: &str) -> Option<String> {
    ev.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

impl HostIds {
    /// Claude Code's common hook input fields (`tool_use_id` on tool events;
    /// `agent_id` / `agent_type` inside a subagent or under `--agent`).
    pub fn claude_code(ev: &Value) -> Self {
        Self {
            host: "claude-code",
            tool_use_id: str_field(ev, "tool_use_id"),
            agent_id: str_field(ev, "agent_id"),
            agent_type: str_field(ev, "agent_type"),
            cwd: str_field(ev, "cwd"),
            step_idx: None,
        }
    }

    /// Antigravity's payload: no tool-call id and no agent identity, so those stay
    /// empty rather than invented. `cwd` is the call's `Cwd`, else the workspace.
    pub fn antigravity(ev: &Value, raw_args: &Value) -> Self {
        let workspace = ev
            .get("workspacePaths")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(Value::as_str)
            .map(str::to_string);
        Self {
            host: "antigravity",
            tool_use_id: None,
            agent_id: None,
            agent_type: None,
            cwd: str_field(raw_args, "Cwd").or(workspace),
            step_idx: ev.get("stepIdx").and_then(Value::as_u64),
        }
    }

    /// Write these fields into a log line.
    pub fn stamp(&self, line: &mut Value) {
        let Some(obj) = line.as_object_mut() else {
            return;
        };
        obj.insert("host".into(), json!(self.host));
        obj.insert("tool_use_id".into(), json!(self.tool_use_id));
        obj.insert("agent_id".into(), json!(self.agent_id));
        obj.insert("agent_type".into(), json!(self.agent_type));
        obj.insert("cwd".into(), json!(self.cwd));
        if let Some(step) = self.step_idx {
            obj.insert("step_idx".into(), json!(step));
        }
    }
}

pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Append one line to the decision log. Observability, not enforcement: a log
/// that cannot be written is reported on stderr and nothing else changes.
///
/// The line goes out in a single `write` on an append-mode file, so lines from
/// hooks the host runs in parallel do not interleave.
pub(crate) fn append_line(path: &Path, line: &Value, who: &str) {
    use std::io::Write;
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        let _ = std::fs::create_dir_all(dir);
    }
    let text = format!("{line}\n");
    let written = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut f| f.write_all(text.as_bytes()));
    if let Err(e) = written {
        eprintln!(
            "harness {who}: decision log {} not written: {e}",
            path.display()
        );
    }
}

/// Collapse whitespace and keep at most `max` characters, from the start.
fn head(text: &str, max: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max {
        return flat;
    }
    let cut: String = flat.chars().take(max - 1).collect();
    format!("{cut}…")
}

/// Collapse whitespace and keep at most `max` characters, from the end.
fn tail(text: &str, max: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let n = flat.chars().count();
    if n <= max {
        return flat;
    }
    let cut: String = flat.chars().skip(n - (max - 1)).collect();
    format!("…{cut}")
}

/// Replace every long string in a structured result with its size, so a file
/// body or a patch is never copied into the log.
fn shrink(value: &Value) -> Value {
    match value {
        Value::String(s) if s.chars().count() > RESULT_FIELD_MAX => {
            json!(format!("<{} chars>", s.chars().count()))
        }
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), shrink(v)))
                .collect::<Map<_, _>>(),
        ),
        Value::Array(items) if items.len() > 3 => json!(format!("<{} items>", items.len())),
        Value::Array(items) => Value::Array(items.iter().map(shrink).collect()),
        other => other.clone(),
    }
}

/// A one-line summary of a tool's result, safe to log.
///
/// Redaction runs **first**, over the full text, with the world's
/// `observability.redact` patterns and the D66 value scanner; only then is
/// anything cut. Cutting first could leave half a token whose prefix no longer
/// meets a detector's length floor, and that half would be logged.
///
/// A command's `stdout` / `stderr` keep their tail. Any other string longer than
/// a path or a status word is replaced by its size.
pub(crate) fn result_summary(response: &Value, redact_patterns: &[String]) -> String {
    let clean = redact(response, redact_patterns);
    let text = match &clean {
        Value::Null => String::new(),
        Value::String(s) => tail(s, OUTPUT_TAIL),
        Value::Object(map) if map.contains_key("stdout") || map.contains_key("stderr") => {
            let part = |k: &str| {
                map.get(k)
                    .and_then(Value::as_str)
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| tail(s, OUTPUT_TAIL))
            };
            match (part("stdout"), part("stderr")) {
                (Some(o), Some(e)) => format!("stdout: {o} | stderr: {e}"),
                (Some(o), None) => format!("stdout: {o}"),
                (None, Some(e)) => format!("stderr: {e}"),
                (None, None) => "no output".to_string(),
            }
        }
        other => shrink(other).to_string(),
    };
    head(&text, RESULT_SUMMARY_MAX)
}

/// The exit code a failed command reports. Claude Code's `PostToolUseFailure`
/// `error` text starts with `Exit code N` for a command that exited non-zero.
pub(crate) fn exit_code_of(error: &str) -> Option<i64> {
    error
        .lines()
        .next()?
        .trim()
        .strip_prefix("Exit code ")?
        .trim()
        .parse()
        .ok()
}

/// The host events the observer answers. Everything else is a gate call.
pub(crate) fn is_outcome_event(ev: &Value) -> bool {
    matches!(
        ev.get("hook_event_name").and_then(Value::as_str),
        Some("PostToolUse" | "PostToolUseFailure")
    )
}

/// The outcome line for a `PostToolUse` / `PostToolUseFailure` event, or `None`
/// for any other event.
pub(crate) fn outcome_line(ev: &Value, redact_patterns: &[String]) -> Option<Value> {
    let event = ev.get("hook_event_name").and_then(Value::as_str)?;
    let failed = match event {
        "PostToolUse" => false,
        "PostToolUseFailure" => true,
        _ => return None,
    };
    let tool_input = ev.get("tool_input").cloned().unwrap_or_else(|| json!({}));
    let response = ev.get("tool_response").cloned().unwrap_or(Value::Null);
    let interrupted = ev.get("is_interrupt").and_then(Value::as_bool) == Some(true)
        || response.get("interrupted").and_then(Value::as_bool) == Some(true);
    let error = ev.get("error").and_then(Value::as_str);
    let status = if interrupted {
        "interrupted"
    } else if failed {
        "error"
    } else {
        "ok"
    };
    let mut line = json!({
        "ts_ms": now_ms(),
        "kind": "outcome",
        "event": event,
        "session": ev.get("session_id").and_then(Value::as_str).unwrap_or("default"),
        "tool": ev.get("tool_name").and_then(Value::as_str).unwrap_or(""),
        "call": call_summary(&tool_input),
        "ran": true,
        "status": status,
        // Only a failure reports one: Claude Code routes a non-zero exit to
        // `PostToolUseFailure`, and a success carries no code to copy.
        "exit_code": error.and_then(exit_code_of),
        "error": error.map(|e| head(&mask_secrets(e), RESULT_SUMMARY_MAX)),
        "result": (!failed).then(|| result_summary(&response, redact_patterns)),
        "duration_ms": ev.get("duration_ms").and_then(Value::as_u64),
    });
    HostIds::claude_code(ev).stamp(&mut line);
    Some(line)
}

/// The world's own `observability.redact` patterns, read without compiling: the
/// observer must stay cheap and must not fail because a world does not compile.
pub(crate) fn redact_patterns(world_path: &Path) -> Vec<String> {
    std::fs::read_to_string(world_path)
        .ok()
        .and_then(|text| compiler::loader::load_yaml(&text).ok())
        .map(|m| m.observability.redact)
        .unwrap_or_default()
}

/// Read a decision log: every line that is JSON, in order.
pub fn read_log(path: &Path) -> std::io::Result<Vec<Value>> {
    Ok(std::fs::read_to_string(path)?
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect())
}

/// One decision with what followed it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Joined {
    pub session: String,
    pub tool_use_id: String,
    pub tool: String,
    pub decision: String,
    /// What the hook told the host (`deny`, `ask`, `allow`), or `None` when it
    /// stayed silent and left the call to the host's own flow.
    pub answer: Option<String>,
    /// Whether the call ran. `None` when this session has no outcome lines at
    /// all: the observer was not installed, so absence proves nothing.
    pub ran: Option<bool>,
    /// The outcome line's status (`ok`, `error`, `interrupted`), if it ran.
    pub status: Option<String>,
    pub exit_code: Option<i64>,
    /// For an ASK: `approved` or `declined_or_abandoned`, **inferred** from
    /// whether the call ran, or `unobserved`. `None` for any other answer.
    pub human_answer: Option<String>,
    /// A blocked call that ran anyway. It should never happen; when it does, the
    /// log is the place to see it.
    pub ran_despite_deny: bool,
}

/// Pair each decision line with the outcome line that carries its
/// `tool_use_id`. Decision lines without an id (an older hook, a host that sends
/// none) cannot be joined and are skipped.
///
/// An outcome is only meaningful when the observer was running, so a session
/// with no outcome lines at all reports `ran: None` rather than "nothing ran".
/// The last call of a live session may still be running; a reader of a log
/// that is still being written sees it as not run.
pub fn join(lines: &[Value]) -> Vec<Joined> {
    fn kind(l: &Value) -> &str {
        l.get("kind").and_then(Value::as_str).unwrap_or("decision")
    }
    let text = |l: &Value, k: &str| l.get(k).and_then(Value::as_str).map(str::to_string);

    let mut outcomes: HashMap<(String, String), &Value> = HashMap::new();
    let mut observed: HashSet<String> = HashSet::new();
    for l in lines.iter().filter(|l| kind(l) == "outcome") {
        let session = text(l, "session").unwrap_or_default();
        observed.insert(session.clone());
        if let Some(id) = text(l, "tool_use_id") {
            outcomes.insert((session, id), l);
        }
    }

    lines
        .iter()
        .filter(|l| kind(l) == "decision")
        .filter_map(|l| {
            let session = text(l, "session").unwrap_or_default();
            let id = text(l, "tool_use_id")?;
            let outcome = outcomes.get(&(session.clone(), id.clone()));
            let ran = observed.contains(&session).then_some(outcome.is_some());
            let answer = text(l, "answer");
            let human_answer = (answer.as_deref() == Some("ask")).then(|| {
                match ran {
                    Some(true) => "approved",
                    Some(false) => "declined_or_abandoned",
                    None => "unobserved",
                }
                .to_string()
            });
            Some(Joined {
                ran_despite_deny: answer.as_deref() == Some("deny") && ran == Some(true),
                session,
                tool_use_id: id,
                tool: text(l, "tool").unwrap_or_default(),
                decision: text(l, "decision").unwrap_or_default(),
                answer,
                ran,
                status: outcome.and_then(|o| text(o, "status")),
                exit_code: outcome.and_then(|o| o.get("exit_code").and_then(Value::as_i64)),
                human_answer,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_are_read_from_the_first_line_only() {
        assert_eq!(exit_code_of("Exit code 1\nError: boom"), Some(1));
        assert_eq!(exit_code_of("Exit code 127"), Some(127));
        assert_eq!(exit_code_of("File not found"), None);
        assert_eq!(exit_code_of("ok\nExit code 2"), None);
    }

    #[test]
    fn a_command_result_keeps_the_tail_of_its_output() {
        let out = format!("{}\ntest result: ok. 9 passed", "line\n".repeat(200));
        let s = result_summary(
            &json!({"stdout": out, "stderr": "", "interrupted": false}),
            &[],
        );
        assert!(s.starts_with("stdout: …"), "{s}");
        assert!(s.ends_with("test result: ok. 9 passed"), "{s}");
    }

    #[test]
    fn a_structured_result_logs_sizes_not_bodies() {
        let body = "SECRET-LOOKING BUT ORDINARY ".repeat(20);
        let s = result_summary(
            &json!({"type": "text", "file": {"filePath": "/p/a.md", "content": body, "numLines": 20}}),
            &[],
        );
        assert!(s.contains("/p/a.md"), "{s}");
        assert!(s.contains("chars>"), "{s}");
        assert!(!s.contains("ORDINARY"), "{s}");
    }

    /// D66, extended to results: a secret in a command's output is masked before
    /// the summary is cut, so neither the whole token nor a fragment of it lands.
    #[test]
    fn secrets_in_results_are_masked_before_truncation() {
        let token = format!("ghp_{}", "A1b2C3d4E5".repeat(4));
        let aws = "AKIAIOSFODNN7EXAMPLE";
        for out in [
            format!("{}\nexport GITHUB_TOKEN={token}", "x\n".repeat(300)),
            format!("{token}\n{}", "x\n".repeat(300)),
            format!("curl -H 'Authorization: Bearer {token}' https://api.example\n{aws}"),
        ] {
            let s = result_summary(&json!({"stdout": out, "stderr": ""}), &[]);
            assert!(!s.contains("A1b2C3d4E5"), "token fragment in {s}");
            assert!(!s.contains("IOSFODNN7"), "AWS key in {s}");
        }
        // A plain-string result and an error text go through the same scanner.
        let s = result_summary(&json!(format!("token={token}")), &[]);
        assert!(!s.contains("A1b2C3d4E5"), "{s}");
        let ev = json!({
            "hook_event_name": "PostToolUseFailure", "session_id": "s", "tool_name": "Bash",
            "tool_input": {"command": format!("git push https://bob:{token}@github.com/x")},
            "tool_use_id": "t1", "error": format!("Exit code 128\nfatal: auth failed for {token}")
        });
        let line = outcome_line(&ev, &[]).unwrap().to_string();
        assert!(!line.contains("A1b2C3d4E5"), "{line}");
        assert!(line.contains("[REDACTED]"), "{line}");
    }

    /// The world's own key patterns apply to results as they do to traces.
    #[test]
    fn the_worlds_redact_patterns_apply_to_results() {
        let s = result_summary(
            &json!({"env": {"DB_PASSWORD": "hunter2"}, "ok": true}),
            &["*PASSWORD*".to_string()],
        );
        assert!(!s.contains("hunter2"), "{s}");
    }

    #[test]
    fn ordinary_results_pass_through() {
        let s = result_summary(
            &json!({"stdout": "On branch main\nnothing to commit", "stderr": ""}),
            &[],
        );
        assert_eq!(s, "stdout: On branch main nothing to commit");
        assert_eq!(
            result_summary(&json!({"filePath": "/p/x", "type": "create"}), &[]),
            r#"{"filePath":"/p/x","type":"create"}"#
        );
    }

    #[test]
    fn only_post_tool_events_make_an_outcome() {
        assert!(outcome_line(&json!({"hook_event_name": "PreToolUse"}), &[]).is_none());
        assert!(outcome_line(&json!({}), &[]).is_none());
        let ok = outcome_line(
            &json!({"hook_event_name": "PostToolUse", "session_id": "s", "tool_name": "Write",
                    "tool_input": {"file_path": "/p/f", "content": "BODY"},
                    "tool_response": {"filePath": "/p/f", "type": "create"},
                    "tool_use_id": "toolu_1", "duration_ms": 12, "agent_id": "a1", "agent_type": "Explore"}),
            &[],
        )
        .unwrap();
        assert_eq!(ok["kind"], "outcome");
        assert_eq!(ok["status"], "ok");
        assert_eq!(ok["ran"], true);
        assert_eq!(ok["tool_use_id"], "toolu_1");
        assert_eq!(ok["agent_type"], "Explore");
        assert_eq!(ok["duration_ms"], 12);
        assert_eq!(ok["call"], "/p/f");
        assert!(!ok.to_string().contains("BODY"));
        let stopped = outcome_line(
            &json!({"hook_event_name": "PostToolUseFailure", "tool_use_id": "t",
                    "error": "aborted", "is_interrupt": true}),
            &[],
        )
        .unwrap();
        assert_eq!(stopped["status"], "interrupted");
        assert!(stopped["result"].is_null());
    }

    fn decision(id: &str, verdict: &str, answer: Option<&str>) -> Value {
        json!({"kind": "decision", "session": "s", "tool_use_id": id, "tool": "Bash",
               "decision": verdict, "answer": answer})
    }
    fn outcome(id: &str) -> Value {
        json!({"kind": "outcome", "session": "s", "tool_use_id": id, "status": "ok", "exit_code": null})
    }

    #[test]
    fn join_infers_the_answer_to_an_ask_from_whether_the_call_ran() {
        let lines = vec![
            decision("a", "ALLOW", None),
            outcome("a"),
            decision("b", "ASK", Some("ask")),
            outcome("b"),
            decision("c", "ASK", Some("ask")),
            decision("d", "DENY", Some("deny")),
            json!({"kind": "policy", "headline": "x"}),
            json!({"session": "s", "decision": "ALLOW"}), // pre-D84 line: no id, skipped
        ];
        let j = join(&lines);
        assert_eq!(j.len(), 4);
        assert_eq!((j[0].ran, j[0].human_answer.as_deref()), (Some(true), None));
        assert_eq!(j[1].human_answer.as_deref(), Some("approved"));
        assert_eq!(j[2].human_answer.as_deref(), Some("declined_or_abandoned"));
        assert_eq!((j[3].ran, j[3].ran_despite_deny), (Some(false), false));
    }

    #[test]
    fn without_an_observer_absence_proves_nothing() {
        let j = join(&[decision("b", "ASK", Some("ask"))]);
        assert_eq!(j[0].ran, None);
        assert_eq!(j[0].human_answer.as_deref(), Some("unobserved"));
    }

    #[test]
    fn a_denied_call_that_ran_is_flagged() {
        let j = join(&[decision("d", "DENY", Some("deny")), outcome("d")]);
        assert!(j[0].ran_despite_deny);
    }

    #[test]
    fn antigravity_ids_are_not_invented() {
        let ids = HostIds::antigravity(
            &json!({"conversationId": "c", "stepIdx": 3, "workspacePaths": ["/ws"]}),
            &json!({"CommandLine": "ls"}),
        );
        assert_eq!(ids.tool_use_id, None);
        assert_eq!(ids.agent_id, None);
        assert_eq!(ids.cwd.as_deref(), Some("/ws"));
        assert_eq!(ids.step_idx, Some(3));
    }
}
