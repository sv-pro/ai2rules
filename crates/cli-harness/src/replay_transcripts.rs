//! `harness replay-transcripts` — what would the hook have decided in sessions you
//! have already run?
//!
//! Reads Claude Code session transcripts (JSONL, by default everything under
//! `~/.claude/projects`), takes every tool call the agent actually made, and asks
//! the kernel about each one exactly as `harness cc-hook` would have: the same
//! request mapping and answer mapping (shared with the hook), the world compiled
//! against the session's own working directory, and taint and budget carried
//! per session in call order. Subagent transcripts share their parent's session
//! id, so all files are merged by timestamp before replaying.
//!
//! Every replayed call carries the transcript's `tool_use` id and, for a
//! subagent, its `agentId` — the same keys the hook writes into its decision log
//! (D84). Pass that log with `--decisions` and each call is joined to what the
//! live hook decided and how the call ended, so a replay against a changed world
//! shows which real verdicts would move, and the ASKs show whether the person
//! approved them (inferred from whether the call ran).
//!
//! Nothing is executed and no transcript is modified. The point is evidence before
//! enforcement: how often the hook would have asked or denied in your real work,
//! and why, without connecting it to a live agent.
//!
//! Two limits, both from path-scope (roots) reading today's filesystem, as the hook
//! reads it at call time:
//! - a path that has since disappeared (a deleted temp folder) can no longer be
//!   resolved, and replays as `missing_path`;
//! - replay on the OS the sessions ran on. A Windows path (`C:\...`) is not a path
//!   to a Linux build, so Windows sessions replayed in WSL report their file
//!   actions as `missing_path`.

use crate::cc_hook::{compile_hook_world, hook_answer, hook_request, HookEvent, HookSession};
use crate::init::STARTER_WORLD;
use crate::outcome::{join, read_log, Joined};
use harness_preview::{gate, GateUsage};
use harness_types::CompiledWorld;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

/// One tool call found in a transcript.
struct Call {
    timestamp: String,
    session: String,
    tool_use_id: String,
    agent_id: Option<String>,
    cwd: String,
    tool: String,
    input: Value,
}

/// What the kernel and the hook would have said about one call.
#[derive(Serialize)]
struct Replayed {
    timestamp: String,
    session: String,
    /// The host's id for this call: the join key into the decision log.
    tool_use_id: String,
    /// The subagent that made the call, when it was not the main agent.
    agent_id: Option<String>,
    tool: String,
    /// A one-line summary of the call's arguments (command, path, URL, …).
    summary: String,
    /// The kernel's verdict and the rule that fired.
    decision: String,
    rule: Option<String>,
    /// The kernel's effective action (after `command_classes`, D36).
    action: String,
    /// What the hook would have answered: `deny`, `ask`, `allow` (only with
    /// `--grant`), or `silent` (defer to the host).
    hook: String,
    reason: String,
    taint_in: String,
    taint_out: String,
    /// What the live hook decided and how the call ended, from `--decisions`.
    #[serde(skip_serializing_if = "Option::is_none")]
    live: Option<Joined>,
}

pub struct Options<'a> {
    pub paths: &'a [PathBuf],
    pub world: Option<&'a Path>,
    pub mode: &'a str,
    pub source_channel: &'a str,
    pub enforce_absent: bool,
    pub grant: bool,
    pub examples: usize,
    pub json: bool,
    /// The hook's decision log (`cc-hook --log`) to join replayed calls to.
    pub decisions: Option<&'a Path>,
}

pub fn run(opts: &Options<'_>) -> i32 {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok();
    let inputs: Vec<PathBuf> = if opts.paths.is_empty() {
        match &home {
            Some(h) => vec![Path::new(h).join(".claude").join("projects")],
            None => {
                eprintln!("replay-transcripts: no HOME; pass transcript files or folders");
                return 2;
            }
        }
    } else {
        opts.paths.to_vec()
    };
    let mut files = Vec::new();
    for input in &inputs {
        collect_jsonl(input, &mut files);
    }
    files.sort();
    if files.is_empty() {
        eprintln!(
            "replay-transcripts: no .jsonl transcripts under {}",
            inputs
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        return 2;
    }

    let (world_label, world_text) = match opts.world {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(text) => (path.display().to_string(), text),
            Err(e) => {
                eprintln!("replay-transcripts: cannot read {}: {e}", path.display());
                return 2;
            }
        },
        None => (
            "the starter manifest `harness init` installs".to_string(),
            STARTER_WORLD.to_string(),
        ),
    };

    // The live log, keyed by tool-call id. Its own lines name their session, so
    // the key is the id alone: Claude Code ids are unique across sessions.
    let mut live: HashMap<String, Joined> = HashMap::new();
    if let Some(path) = opts.decisions {
        match read_log(path) {
            Ok(lines) => {
                for j in join(&lines) {
                    live.insert(j.tool_use_id.clone(), j);
                }
            }
            Err(e) => {
                eprintln!("replay-transcripts: cannot read {}: {e}", path.display());
                return 2;
            }
        }
    }

    let (mut calls, skipped_lines) = read_calls(&files);
    calls.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    // The world is compiled per working directory: roots resolve against the
    // project, as they would for a hook running in that session.
    let mut worlds: HashMap<String, CompiledWorld> = HashMap::new();
    let mut sessions: HashMap<String, HookSession> = HashMap::new();
    let mut replayed = Vec::with_capacity(calls.len());
    for call in calls {
        let base = (!call.cwd.is_empty()).then_some(call.cwd.as_str());
        if !worlds.contains_key(&call.cwd) {
            match compile_hook_world(&world_text, base, home.as_deref()) {
                Some(w) => {
                    worlds.insert(call.cwd.clone(), w);
                }
                None => {
                    eprintln!("replay-transcripts: the world in {world_label} does not compile");
                    return 2;
                }
            }
        }
        let world = &worlds[&call.cwd];
        let session = sessions
            .entry(call.session.clone())
            .or_insert_with(|| HookSession {
                tainted: false,
                usage: Some(GateUsage::default()),
            });
        let summary = summarize(&call.input);
        let req = hook_request(
            world,
            HookEvent {
                tool: &call.tool,
                tool_input: call.input,
                session_id: call.session.clone(),
            },
            session,
            base,
            home.as_deref(),
            opts.mode,
            opts.source_channel,
        );
        let res = gate(world, &req);
        let taint_in = if session.tainted { "tainted" } else { "clean" };
        session.tainted |= res.context.taint == "tainted";
        session.usage = Some(res.context.usage);
        let (hook, reason) = match hook_answer(&res, opts.enforce_absent, opts.grant) {
            Some((decision, reason)) => (decision.to_string(), reason),
            None => ("silent".to_string(), res.reason.clone()),
        };
        replayed.push(Replayed {
            timestamp: call.timestamp,
            session: call.session,
            live: live.remove(&call.tool_use_id),
            tool_use_id: call.tool_use_id,
            agent_id: call.agent_id,
            tool: call.tool,
            summary,
            decision: res.decision.clone(),
            rule: res.rule.clone(),
            action: res.action.clone(),
            hook,
            reason,
            taint_in: taint_in.to_string(),
            taint_out: res.context.taint.clone(),
        });
    }

    if opts.json {
        let out = json!({
            "world": world_label,
            "transcripts": files.len(),
            "skipped_lines": skipped_lines,
            "mode": opts.mode,
            "enforce_absent": opts.enforce_absent,
            "grant": opts.grant,
            "decisions": opts.decisions.map(|p| p.display().to_string()),
            "calls": replayed,
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
    } else {
        print_report(&replayed, &files, skipped_lines, &world_label, opts);
    }
    0
}

/// Every `.jsonl` file at `path`, recursively when it is a folder.
fn collect_jsonl(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                collect_jsonl(&entry.path(), out);
            }
        }
    } else if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
        out.push(path.to_path_buf());
    }
}

/// The tool calls in `files`, each counted once (a call is identified by its
/// `tool_use` id), and the number of lines that were not JSON.
fn read_calls(files: &[PathBuf]) -> (Vec<Call>, usize) {
    let mut seen = HashSet::new();
    let mut calls = Vec::new();
    let mut skipped = 0;
    for file in files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let Ok(event) = serde_json::from_str::<Value>(line) else {
                skipped += 1;
                continue;
            };
            if event["type"] != "assistant" {
                continue;
            }
            let Some(blocks) = event["message"]["content"].as_array() else {
                continue;
            };
            for block in blocks.iter().filter(|b| b["type"] == "tool_use") {
                let id = id_of(block);
                if !id.is_empty() && !seen.insert(id) {
                    continue;
                }
                calls.push(Call {
                    timestamp: event["timestamp"].as_str().unwrap_or_default().to_string(),
                    session: event["sessionId"].as_str().unwrap_or("default").to_string(),
                    tool_use_id: id_of(block),
                    agent_id: event["agentId"].as_str().map(str::to_string),
                    cwd: event["cwd"].as_str().unwrap_or_default().to_string(),
                    tool: block["name"].as_str().unwrap_or_default().to_string(),
                    input: block["input"].clone(),
                });
            }
        }
    }
    (calls, skipped)
}

fn id_of(block: &Value) -> String {
    block["id"].as_str().unwrap_or_default().to_string()
}

/// A one-line summary of a call's arguments: the command, path, URL or query.
fn summarize(input: &Value) -> String {
    let text = [
        "command",
        "file_path",
        "path",
        "notebook_path",
        "url",
        "pattern",
        "query",
        "description",
    ]
    .iter()
    .find_map(|k| input.get(*k).and_then(Value::as_str))
    .unwrap_or_default();
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() > 90 {
        format!("{}...", line.chars().take(87).collect::<String>())
    } else {
        line
    }
}

fn print_report(
    calls: &[Replayed],
    files: &[PathBuf],
    skipped_lines: usize,
    world_label: &str,
    opts: &Options<'_>,
) {
    let sessions: HashSet<&str> = calls.iter().map(|c| c.session.as_str()).collect();
    let first = calls.first().map(|c| c.timestamp.get(..10).unwrap_or(""));
    let last = calls.last().map(|c| c.timestamp.get(..10).unwrap_or(""));
    println!(
        "Replayed {} tool calls from {} transcripts ({} sessions){}",
        calls.len(),
        files.len(),
        sessions.len(),
        match (first, last) {
            (Some(a), Some(b)) => format!(", {a} to {b}"),
            _ => String::new(),
        }
    );
    println!("World: {world_label}");
    println!(
        "Hook: mode {}, {}{}. Nothing was executed.",
        opts.mode,
        if opts.grant {
            "replace (--grant)"
        } else {
            "additive"
        },
        if opts.enforce_absent {
            ", ABSENT enforced"
        } else {
            ""
        }
    );
    if skipped_lines > 0 {
        println!("({skipped_lines} transcript lines were not JSON and were skipped.)");
    }
    if calls.is_empty() {
        return;
    }

    let pct = |n: usize| 100.0 * n as f64 / calls.len() as f64;
    let count = |f: &dyn Fn(&Replayed) -> bool| calls.iter().filter(|c| f(c)).count();

    println!("\n== What the hook would have done ==");
    for (label, hook) in [
        ("stayed silent (your normal flow)", "silent"),
        ("granted (--grant)", "allow"),
        ("asked you", "ask"),
        ("denied", "deny"),
    ] {
        let n = count(&|c| c.hook == hook);
        if n > 0 || hook != "allow" {
            println!("  {label:<34} {n:>6}  {:>5.1}%", pct(n));
        }
    }

    let mut verdicts: BTreeMap<&str, usize> = BTreeMap::new();
    for c in calls {
        *verdicts.entry(c.decision.as_str()).or_default() += 1;
    }
    println!(
        "\nKernel verdicts: {}",
        verdicts
            .iter()
            .map(|(d, n)| format!("{d} {n}"))
            .collect::<Vec<_>>()
            .join(" | ")
    );

    let tainted: Vec<&Replayed> = calls
        .iter()
        .filter(|c| c.taint_in == "clean" && c.taint_out == "tainted")
        .collect();
    println!(
        "Sessions that became tainted: {} of {}",
        tainted.len(),
        sessions.len()
    );
    for c in tainted.iter().take(opts.examples) {
        println!("    {}  {}  {}", short_ts(&c.timestamp), c.tool, c.summary);
    }

    for (title, hook) in [("Denied", "deny"), ("Asked", "ask")] {
        let mut by_rule: BTreeMap<&str, Vec<&Replayed>> = BTreeMap::new();
        for c in calls.iter().filter(|c| c.hook == hook) {
            by_rule
                .entry(c.rule.as_deref().unwrap_or("-"))
                .or_default()
                .push(c);
        }
        if by_rule.is_empty() {
            continue;
        }
        let total: usize = by_rule.values().map(Vec::len).sum();
        println!("\n== {title} ({total}), by rule ==");
        let mut rules: Vec<_> = by_rule.into_iter().collect();
        rules.sort_by_key(|r| std::cmp::Reverse(r.1.len()));
        for (rule, items) in rules {
            println!("  {rule} ({})", items.len());
            for c in items.iter().take(opts.examples) {
                println!(
                    "    {}  {:<10} {}",
                    short_ts(&c.timestamp),
                    c.tool,
                    c.summary
                );
            }
            if items.len() > opts.examples {
                println!("    ... {} more", items.len() - opts.examples);
            }
        }
    }

    let mut absent: BTreeMap<&str, usize> = BTreeMap::new();
    for c in calls.iter().filter(|c| c.decision == "ABSENT") {
        *absent.entry(c.tool.as_str()).or_default() += 1;
    }
    if !absent.is_empty() {
        let mut tools: Vec<_> = absent.into_iter().collect();
        tools.sort_by_key(|t| std::cmp::Reverse(t.1));
        println!(
            "\n== Unknown to this world (ABSENT, {}) ==",
            if opts.enforce_absent {
                "denied"
            } else {
                "passed through; --enforce-absent would deny them"
            }
        );
        for (tool, n) in tools.iter().take(10) {
            println!("  {tool:<40} {n:>6}");
        }
        if tools.len() > 10 {
            println!("  ... {} more tools", tools.len() - 10);
        }
    }
    if opts.decisions.is_some() {
        print_live(calls);
    }
    println!("\nFor every call with its verdict and reason: --json");
}

/// What the live decision log adds: how many replayed calls it covers, how the
/// people asked answered (inferred), and where this world disagrees with the one
/// the hook ran under.
fn print_live(calls: &[Replayed]) {
    let joined: Vec<(&Replayed, &Joined)> = calls
        .iter()
        .filter_map(|c| c.live.as_ref().map(|l| (c, l)))
        .collect();
    println!(
        "\n== Live decision log ({} of {} calls joined by tool_use_id) ==",
        joined.len(),
        calls.len()
    );
    if joined.is_empty() {
        println!("  No call matched. The log needs a hook that records tool_use_id (D84).");
        return;
    }
    let asks: Vec<&Joined> = joined
        .iter()
        .map(|(_, l)| *l)
        .filter(|l| l.answer.as_deref() == Some("ask"))
        .collect();
    let count = |a: &str| {
        asks.iter()
            .filter(|l| l.human_answer.as_deref() == Some(a))
            .count()
    };
    println!(
        "  ASKs: {} — approved {}, declined or abandoned {}, unobserved {}",
        asks.len(),
        count("approved"),
        count("declined_or_abandoned"),
        count("unobserved")
    );
    println!("  (inferred from whether the call ran: the hook never sees the answer)");
    let ran = joined.iter().filter(|(_, l)| l.ran == Some(true)).count();
    let failed = joined
        .iter()
        .filter(|(_, l)| matches!(l.status.as_deref(), Some("error" | "interrupted")))
        .count();
    println!("  Ran: {ran}, of which failed or interrupted: {failed}");
    let moved: Vec<&(&Replayed, &Joined)> = joined
        .iter()
        .filter(|(c, l)| c.decision != l.decision)
        .collect();
    println!("  Verdicts this world would change: {}", moved.len());
    for (c, l) in moved.iter().take(10) {
        println!(
            "    {}  {:<10} {} -> {}  {}",
            short_ts(&c.timestamp),
            c.tool,
            l.decision,
            c.decision,
            c.summary
        );
    }
    let breaches = joined.iter().filter(|(_, l)| l.ran_despite_deny).count();
    if breaches > 0 {
        println!("  !! {breaches} denied call(s) ran anyway — check the host's hook wiring");
    }
}

/// `2026-10-21T07:50:49.064Z` -> `2026-10-21 07:50`.
fn short_ts(ts: &str) -> String {
    ts.get(..16).unwrap_or(ts).replace('T', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summaries_are_one_short_line() {
        assert_eq!(
            summarize(&json!({"command": "git   status\n  --short"})),
            "git status --short"
        );
        let long = "x".repeat(200);
        assert_eq!(summarize(&json!({ "url": long })).len(), 90);
        assert_eq!(summarize(&json!({ "other": 1 })), "");
    }

    #[test]
    fn short_timestamps_drop_seconds_and_the_t() {
        assert_eq!(short_ts("2026-10-21T07:50:49.064Z"), "2026-10-21 07:50");
        assert_eq!(short_ts("bad"), "bad");
    }
}
