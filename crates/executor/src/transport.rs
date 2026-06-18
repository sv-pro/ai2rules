//! Pluggable transports for the external channels (E7), with deterministic mock
//! impls so MCP/web flow through the gate fully offline.
//!
//! Real stdio/HTTP MCP transports and a real web client are a later, feature-
//! gated add; the traits keep the executor agnostic to which is in use.

use std::collections::BTreeMap;

use serde_json::Value;

/// Dispatches a call to an MCP server's tool. The executor's `McpHandler` holds
/// one; it never decides policy.
pub trait McpTransport: Send + Sync {
    fn call(&self, server: &str, tool: &str, input: &Value) -> Result<Value, String>;
}

/// Fetches a URL's content for the web channel.
pub trait WebFetcher: Send + Sync {
    fn fetch(&self, url: &str) -> Result<String, String>;
}

/// Deterministic MCP transport returning canned results keyed by `server/tool`.
#[derive(Default)]
pub struct MockMcpTransport {
    responses: BTreeMap<String, Value>,
}

impl MockMcpTransport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, server: &str, tool: &str, response: Value) -> Self {
        self.responses.insert(format!("{server}/{tool}"), response);
        self
    }
}

impl McpTransport for MockMcpTransport {
    fn call(&self, server: &str, tool: &str, _input: &Value) -> Result<Value, String> {
        self.responses
            .get(&format!("{server}/{tool}"))
            .cloned()
            .ok_or_else(|| format!("no mock response for {server}/{tool}"))
    }
}

/// Deterministic web fetcher returning canned page bodies keyed by URL.
#[derive(Default)]
pub struct MockWebFetcher {
    pages: BTreeMap<String, String>,
}

impl MockWebFetcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, url: &str, body: &str) -> Self {
        self.pages.insert(url.to_string(), body.to_string());
        self
    }
}

impl WebFetcher for MockWebFetcher {
    fn fetch(&self, url: &str) -> Result<String, String> {
        self.pages
            .get(url)
            .cloned()
            .ok_or_else(|| format!("no mock page for {url}"))
    }
}
