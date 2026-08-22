//! The intentionally weak reference gateway.
//!
//! This is **not** a strawman. It holds the same policy the ai2rules world holds
//! (`pack/weak-gateway.yaml` beside `pack/world.yaml`), and it implements that
//! policy correctly in [`WeakGateway::visible_for`]: privileged tools are for
//! privileged channels, some tools need a human first, one tool is never exposed.
//! Its intent is right. What is wrong is where the enforcement sits:
//!
//! 1. **Discovery is cached before it is filtered.** `tools/list` is expensive, so
//!    the answer is memoised — keyed by the upstream, because that is what the
//!    call depends on. The per-principal filter runs *inside* the memoised
//!    function, so the first caller's privileges are baked into the cache.
//! 2. **An approval is a bearer token naming a tool.** It is not bound to the
//!    arguments the human saw, so any call to the same tool satisfies it.
//! 3. **It is not bound to a principal, and it is never consumed.** Anyone
//!    holding the string can spend it, any number of times.
//!
//! Each defect is a plausible engineering decision on its own. That is the point:
//! a governance surface can be complete on paper and still hold none of these
//! three lines.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::target::{Authorization, Call, Discovery, Invocation, Principal, Target, Verdict};
use crate::upstream::Upstream;

/// The gateway's declared policy — the same intent the ai2rules world declares.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct WeakPolicy {
    /// Tools that must never appear on any surface.
    #[serde(default)]
    pub never_expose: Vec<String>,
    /// Channels allowed to see and use privileged tools.
    #[serde(default)]
    pub privileged_channels: Vec<String>,
    /// Tools only privileged channels may see or use.
    #[serde(default)]
    pub privileged_tools: Vec<String>,
    /// Tools that need a human "yes" before they run.
    #[serde(default)]
    pub approval_required: Vec<String>,
}

/// A bearer grant. Everything this struct does *not* carry is the vulnerability:
/// no principal, no arguments, no remaining-use count, no expiry.
#[derive(Debug, Clone)]
struct Grant {
    tool: String,
}

pub struct WeakGateway {
    upstream: Rc<RefCell<Upstream>>,
    policy: WeakPolicy,
    /// Memoised discovery. The key is the upstream's identity — nothing about the
    /// principal, because the author reasoned that `tools/list` depends only on
    /// the upstream. Defect 1.
    discovery_cache: BTreeMap<String, (Vec<String>, String)>,
    grants: BTreeMap<String, Grant>,
    issued: u64,
}

impl WeakGateway {
    pub const ID: &'static str = "weak-reference-gateway";

    pub fn new(upstream: Rc<RefCell<Upstream>>, policy: WeakPolicy) -> Self {
        Self {
            upstream,
            policy,
            discovery_cache: BTreeMap::new(),
            grants: BTreeMap::new(),
            issued: 0,
        }
    }

    fn privileged(&self, principal: &Principal) -> bool {
        self.policy
            .privileged_channels
            .iter()
            .any(|channel| channel == &principal.channel)
    }

    /// The policy, applied correctly. Nothing below this line is wrong.
    fn visible_for(&self, principal: &Principal) -> Vec<String> {
        let upstream = self.upstream.borrow();
        upstream
            .tools_list()
            .iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str).map(String::from))
            .filter(|name| !self.policy.never_expose.contains(name))
            .filter(|name| {
                !self.policy.privileged_tools.contains(name) || self.privileged(principal)
            })
            .collect()
    }

    fn needs_approval(&self, tool: &str) -> bool {
        self.policy.approval_required.iter().any(|t| t == tool)
    }
}

impl Target for WeakGateway {
    fn id(&self) -> &str {
        Self::ID
    }

    fn description(&self) -> &str {
        "Reference MCP gateway with correct policy and three deliberate enforcement defects"
    }

    fn metadata(&self) -> Value {
        json!({
            "kind": "reference",
            "policy": {
                "never_expose": self.policy.never_expose,
                "privileged_channels": self.policy.privileged_channels,
                "privileged_tools": self.policy.privileged_tools,
                "approval_required": self.policy.approval_required,
            },
            "known_defects": [
                "discovery memoised on an upstream-only cache key",
                "approval bound to a tool name, not to the exact effect",
                "approval bound to no principal and never consumed",
            ],
        })
    }

    fn discover(&mut self, principal: &Principal) -> Discovery {
        // Defect 1: the cache key describes the upstream, not the asker.
        let key = "upstream:default".to_string();
        let (visible, surface_id, cache) = match self.discovery_cache.get(&key) {
            Some((visible, surface_id)) => (visible.clone(), surface_id.clone(), "hit"),
            None => {
                let visible = self.visible_for(principal);
                let surface_id = format!("surface-{}", self.discovery_cache.len() + 1);
                self.discovery_cache
                    .insert(key.clone(), (visible.clone(), surface_id.clone()));
                (visible, surface_id, "miss")
            }
        };
        Discovery {
            verdict: Verdict::Allow,
            visible,
            surface_id: Some(surface_id),
            evidence: json!({"cache_key": key, "cache": cache}),
        }
    }

    fn authorize(&mut self, principal: &Principal, call: &Call) -> Authorization {
        let visible = self.visible_for(principal);
        if !visible.contains(&call.tool) {
            return Authorization {
                verdict: Verdict::Absent,
                rule: Some("not_exposed".to_string()),
                handle: None,
                evidence: json!({"visible": visible}),
            };
        }
        if !self.needs_approval(&call.tool) {
            return Authorization {
                verdict: Verdict::Allow,
                rule: None,
                handle: None,
                evidence: json!({"approval_required": false}),
            };
        }
        self.issued += 1;
        let handle = format!("grant-{}", self.issued);
        // Defect 2 + 3: the grant records the tool and nothing else.
        self.grants.insert(
            handle.clone(),
            Grant {
                tool: call.tool.clone(),
            },
        );
        Authorization {
            verdict: Verdict::Ask,
            rule: Some("approval_required".to_string()),
            handle: Some(handle.clone()),
            evidence: json!({"grant": {"handle": handle, "bound_to": ["tool"]}}),
        }
    }

    fn invoke(&mut self, principal: &Principal, call: &Call, handle: Option<&str>) -> Invocation {
        let visible = self.visible_for(principal);
        if !visible.contains(&call.tool) {
            return Invocation {
                verdict: Verdict::Absent,
                rule: Some("not_exposed".to_string()),
                evidence: json!({"visible": visible}),
            };
        }
        if self.needs_approval(&call.tool) {
            let grant = handle.and_then(|handle| self.grants.get(handle).cloned());
            let Some(grant) = grant else {
                return Invocation {
                    verdict: Verdict::Deny,
                    rule: Some("approval_required".to_string()),
                    evidence: json!({"presented_handle": handle}),
                };
            };
            // The whole check. The grant is never consumed, so it stays valid,
            // and it says nothing about who may spend it or on what arguments.
            if grant.tool != call.tool {
                return Invocation {
                    verdict: Verdict::Deny,
                    rule: Some("grant_tool_mismatch".to_string()),
                    evidence: json!({"grant_tool": grant.tool}),
                };
            }
        }
        let result = self
            .upstream
            .borrow_mut()
            .call(&call.tool, &principal.id, &call.arguments);
        Invocation {
            verdict: Verdict::Allow,
            rule: None,
            evidence: json!({"presented_handle": handle, "upstream_result": result}),
        }
    }
}
