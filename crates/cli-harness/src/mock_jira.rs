//! `harness mock-jira` — a self-contained MCP **upstream** for the governability
//! demo (DECISIONS D33 / E16.A).
//!
//! It exposes a small JIRA-shaped tool surface — including deliberately
//! **destructive** tools (`jira_delete_issue`, `jira_bulk_create_issues`) — so the
//! `harness mcp-gateway` has something real to govern with no Atlassian creds, no
//! Node, and no Python. It speaks MCP JSON-RPC 2.0 over stdio by hand (the protocol
//! over stdio is just newline-delimited JSON-RPC), and every tool returns a canned,
//! side-effect-free result.
//!
//! **`--rovo` mode (E16.E)** swaps the invented `jira_*` names for the *real*
//! Atlassian Rovo MCP tool names (`getJiraIssue`, `transitionJiraIssue`, …) and adds
//! a Confluence tool as cross-product noise. This lets the real-Atlassian manifest
//! (`jira-atlassian.world.yaml`) be exercised end-to-end **offline** — proving the
//! gateway shapes the genuine Rovo surface (and that the manifest's tool names match
//! it) with no creds and no live OAuth hop. It's the same skin the live demo wears.

use serde_json::{json, Value};
use std::io::{BufRead, Write};

const PROTOCOL_VERSION: &str = "2024-11-05";

/// The advertised tool surface — read, comment, and (deliberately) destructive.
/// `rovo` selects the real Atlassian Rovo tool names + a Confluence tool instead of
/// the invented `jira_*` mock names (the demo's live-upstream skin; E16.E).
fn tools(rovo: bool) -> Value {
    if rovo {
        return json!([
            // --- reads (allowed by jira-atlassian.world.yaml) ---
            {"name": "getJiraIssue", "description": "Read a Jira issue.",
             "inputSchema": {"type": "object", "properties": {"cloudId": {"type": "string"}, "issueIdOrKey": {"type": "string"}}, "required": ["cloudId", "issueIdOrKey"]}},
            {"name": "searchJiraIssuesUsingJql", "description": "Search issues with JQL.",
             "inputSchema": {"type": "object", "properties": {"cloudId": {"type": "string"}, "jql": {"type": "string"}}, "required": ["cloudId", "jql"]}},
            {"name": "getVisibleJiraProjects", "description": "List visible projects.",
             "inputSchema": {"type": "object", "properties": {"cloudId": {"type": "string"}}, "required": ["cloudId"]}},
            {"name": "getAccessibleAtlassianResources", "description": "Resolve accessible sites / cloudIds.",
             "inputSchema": {"type": "object", "properties": {}}},
            // --- the one allowed write (taint-floored by the manifest) ---
            {"name": "addCommentToJiraIssue", "description": "Comment on a Jira issue.",
             "inputSchema": {"type": "object", "properties": {"cloudId": {"type": "string"}, "issueIdOrKey": {"type": "string"}, "commentBody": {"type": "string"}}, "required": ["cloudId", "issueIdOrKey", "commentBody"]}},
            // --- deliberately NOT in the manifest → must be ABSENT ---
            {"name": "transitionJiraIssue", "description": "Move an issue to a new status.",
             "inputSchema": {"type": "object", "properties": {"cloudId": {"type": "string"}, "issueIdOrKey": {"type": "string"}, "transition": {"type": "string"}}, "required": ["cloudId", "issueIdOrKey", "transition"]}},
            {"name": "editJiraIssue", "description": "Edit issue fields.",
             "inputSchema": {"type": "object", "properties": {"cloudId": {"type": "string"}, "issueIdOrKey": {"type": "string"}, "fields": {"type": "object"}}, "required": ["cloudId", "issueIdOrKey", "fields"]}},
            {"name": "createJiraIssue", "description": "Create an issue.",
             "inputSchema": {"type": "object", "properties": {"cloudId": {"type": "string"}, "projectKey": {"type": "string"}, "issueTypeName": {"type": "string"}, "summary": {"type": "string"}}, "required": ["cloudId", "projectKey", "issueTypeName", "summary"]}},
            {"name": "addWorklogToJiraIssue", "description": "Log work on an issue.",
             "inputSchema": {"type": "object", "properties": {"cloudId": {"type": "string"}, "issueIdOrKey": {"type": "string"}, "timeSpent": {"type": "string"}}, "required": ["cloudId", "issueIdOrKey", "timeSpent"]}},
            // Confluence tool: cross-product noise the Rovo server really emits — proves
            // the manifest shapes beyond Jira (no Confluence tool is declared → ABSENT).
            {"name": "getConfluencePage", "description": "Read a Confluence page.",
             "inputSchema": {"type": "object", "properties": {"cloudId": {"type": "string"}, "pageId": {"type": "string"}}, "required": ["cloudId", "pageId"]}}
        ]);
    }
    json!([
        {"name": "jira_get_issue", "description": "Read a JIRA issue by key.",
         "inputSchema": {"type": "object", "properties": {"issue_key": {"type": "string"}}, "required": ["issue_key"]}},
        {"name": "jira_search_issues_using_jql", "description": "Search issues with JQL.",
         "inputSchema": {"type": "object", "properties": {"jql": {"type": "string"}}, "required": ["jql"]}},
        {"name": "jira_get_project", "description": "Read project metadata.",
         "inputSchema": {"type": "object", "properties": {"project_key": {"type": "string"}}, "required": ["project_key"]}},
        {"name": "jira_add_comment", "description": "Add a comment to an issue.",
         "inputSchema": {"type": "object", "properties": {"issue_key": {"type": "string"}, "body": {"type": "string"}}, "required": ["issue_key", "body"]}},
        {"name": "jira_transition_issue", "description": "Move an issue to a new status.",
         "inputSchema": {"type": "object", "properties": {"issue_key": {"type": "string"}, "transition": {"type": "string"}}, "required": ["issue_key", "transition"]}},
        {"name": "jira_delete_issue", "description": "Permanently delete an issue.",
         "inputSchema": {"type": "object", "properties": {"issue_key": {"type": "string"}}, "required": ["issue_key"]}},
        {"name": "jira_bulk_create_issues", "description": "Create many issues at once.",
         "inputSchema": {"type": "object", "properties": {"issues": {"type": "array"}}, "required": ["issues"]}}
    ])
}

/// Canned, side-effect-free responses. (A governed gateway should never let the
/// destructive ones through — but if asked directly, the mock still "works".)
fn call_tool(name: &str, args: &Value) -> Value {
    match name {
        // mock (jira_*) surface
        "jira_get_issue" => {
            json!({"key": args.get("issue_key"), "summary": "Mock issue", "status": "Open"})
        }
        "jira_search_issues_using_jql" => json!({"issues": [{"key": "DEMO-1"}, {"key": "DEMO-2"}]}),
        "jira_get_project" => json!({"key": args.get("project_key"), "name": "Demo Project"}),
        "jira_add_comment" => json!({"ok": true, "issue_key": args.get("issue_key")}),
        "jira_transition_issue" => {
            json!({"ok": true, "issue_key": args.get("issue_key"), "status": args.get("transition")})
        }
        "jira_delete_issue" => json!({"deleted": true, "issue_key": args.get("issue_key")}),
        "jira_bulk_create_issues" => json!({"created": 0}),
        // rovo surface (real Atlassian tool names)
        "getJiraIssue" => {
            json!({"key": args.get("issueIdOrKey"), "fields": {"summary": "Mock Rovo issue", "status": {"name": "Open"}}})
        }
        "searchJiraIssuesUsingJql" => json!({"issues": [{"key": "DEMO-1"}, {"key": "DEMO-2"}]}),
        "getVisibleJiraProjects" => json!({"values": [{"key": "DEMO", "name": "Demo Project"}]}),
        "getAccessibleAtlassianResources" => {
            json!([{"id": "mock-cloud-id", "name": "mock-site", "url": "https://mock.atlassian.net"}])
        }
        "addCommentToJiraIssue" => json!({"id": "10001", "issueIdOrKey": args.get("issueIdOrKey")}),
        "transitionJiraIssue" => json!({"ok": true, "issueIdOrKey": args.get("issueIdOrKey")}),
        "editJiraIssue" => json!({"ok": true, "issueIdOrKey": args.get("issueIdOrKey")}),
        "createJiraIssue" => json!({"key": "DEMO-99"}),
        "addWorklogToJiraIssue" => json!({"ok": true}),
        "getConfluencePage" => json!({"id": args.get("pageId"), "title": "Mock Page"}),
        other => json!({"error": format!("unknown tool: {other}")}),
    }
}

pub fn run(rovo: bool) -> i32 {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[mock-jira] malformed JSON: {e}");
                continue;
            }
        };
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let id = req.get("id").cloned().unwrap_or(Value::Null);

        if method == "notifications/initialized" {
            continue; // notification — no response
        }

        let result = match method {
            "initialize" => json!({
                "protocolVersion": PROTOCOL_VERSION,
                "serverInfo": {"name": if rovo { "mock-jira-rovo" } else { "mock-jira" }, "version": "0.1.0"},
                "capabilities": {"tools": {}}
            }),
            "tools/list" => json!({"tools": tools(rovo)}),
            "tools/call" => {
                let params = req.get("params").cloned().unwrap_or_else(|| json!({}));
                let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                json!({"content": [{"type": "text", "text": call_tool(name, &args).to_string()}]})
            }
            other => {
                let err = json!({"jsonrpc": "2.0", "id": id,
                    "error": {"code": -32601, "message": format!("Method not found: {other}")}});
                let _ = writeln!(out, "{err}");
                let _ = out.flush();
                continue;
            }
        };

        let resp = json!({"jsonrpc": "2.0", "id": id, "result": result});
        let _ = writeln!(out, "{resp}");
        let _ = out.flush();
    }
    0
}
