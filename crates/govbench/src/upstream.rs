//! The mock MCP upstream and the external-effect counter.
//!
//! Deliberately in-process. The question these scenarios ask is where authority
//! lives, and a stdio subprocess would add scheduling and framing noise without
//! changing a single verdict. It speaks the two MCP shapes the pack needs —
//! `tools/list` (an array of `{name, description, inputSchema}`) and `tools/call`
//! — so a target that shells out to a real server is a transport change, not a
//! redesign. (`harness mock-jira` remains the repo's stdio-level mock.)
//!
//! **The counter is the point.** It lives here, in the runner's own object, not
//! in either target. A target reports what it decided; only this object knows
//! what actually happened downstream.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// One tool the upstream advertises, exactly as its registry would.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UpstreamTool {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(rename = "inputSchema", default)]
    pub input_schema: Value,
    /// True when a successful `tools/call` changes the world outside the agent.
    #[serde(default)]
    pub effectful: bool,
}

/// One downstream effect that actually happened.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AppliedEffect {
    pub tool: String,
    pub principal: String,
    pub arguments: Value,
}

/// The mock upstream plus the effect ledger the runner reads.
#[derive(Debug, Clone)]
pub struct Upstream {
    tools: Vec<UpstreamTool>,
    applied: Vec<AppliedEffect>,
}

impl Upstream {
    pub fn new(tools: Vec<UpstreamTool>) -> Self {
        Self {
            tools,
            applied: Vec::new(),
        }
    }

    /// `tools/list` — every name the upstream is willing to advertise, ungoverned.
    pub fn tools_list(&self) -> Vec<Value> {
        self.tools
            .iter()
            .map(|tool| {
                json!({
                    "name": tool.name,
                    "description": tool.description,
                    "inputSchema": tool.input_schema,
                })
            })
            .collect()
    }

    pub fn tool(&self, name: &str) -> Option<&UpstreamTool> {
        self.tools.iter().find(|tool| tool.name == name)
    }

    /// `tools/call` — the only path to a downstream effect. Returns the result and
    /// records the effect when the tool is one that changes the world.
    pub fn call(&mut self, tool: &str, principal: &str, arguments: &Value) -> Value {
        let Some(declared) = self.tool(tool).cloned() else {
            return json!({"isError": true, "error": format!("unknown tool {tool}")});
        };
        if declared.effectful {
            self.applied.push(AppliedEffect {
                tool: tool.to_string(),
                principal: principal.to_string(),
                arguments: arguments.clone(),
            });
        }
        json!({"ok": true, "tool": tool, "arguments": arguments})
    }

    /// How many downstream effects actually happened.
    pub fn effect_count(&self) -> usize {
        self.applied.len()
    }

    pub fn effects(&self) -> &[AppliedEffect] {
        &self.applied
    }
}
