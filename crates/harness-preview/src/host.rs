//! Shared **host-outcome** layer — the one mapping from a kernel
//! [`GateResponse`] to what a host adapter must do with it.
//!
//! Every in-tree Rust adapter (`harness cc-hook`, `harness mcp-gateway`)
//! consumes this instead of matching decision strings itself, so the
//! verdict→host translation cannot drift between hosts. `ABSENT` stays
//! distinguishable from `DENY` ([`BlockKind`] + [`BlockKind::label`]) even on
//! hosts whose only structural channel is an error message.
//!
//! **A PROCESS failure is never an outcome.** This module maps *evaluated*
//! verdicts only. What an adapter does when the gate could not evaluate at all
//! (unreadable manifest, malformed event, missing binary) is that adapter's
//! explicitly documented fail-open/fail-closed strategy: `cc-hook` and the
//! OpenCode plugin fail **open** (a broken hook must not brick a host session);
//! `mcp-gateway` fails **closed** (an unevaluated call is never forwarded).

use crate::gate::GateResponse;

/// Why a blocked call is blocked — the kernel's three block classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    /// The action does not exist in this world (unknown to the ontology, not
    /// projected, or invisible to the caller's capability). Stronger than a
    /// deny: there is nothing to argue with.
    Absent,
    /// A policy or invariant denied a visible action.
    Deny,
    /// Over budget / too broad; the caller should propose a smaller step.
    Replan,
}

impl BlockKind {
    /// The wire label hosts prefix into their message channel when they have no
    /// structural way to express the distinction.
    pub fn label(&self) -> &'static str {
        match self {
            BlockKind::Absent => "ABSENT",
            BlockKind::Deny => "DENY",
            BlockKind::Replan => "REPLAN",
        }
    }
}

/// What the host must do with an evaluated verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostOutcome {
    /// `ALLOW` — the host runs its own tool.
    Proceed,
    /// `ASK` — surface the host's approval flow before executing.
    NeedsApproval { reason: String },
    /// `ABSENT` / `DENY` / `REPLAN` — do not execute; `kind` keeps them distinct.
    Block { kind: BlockKind, reason: String },
}

/// Map a kernel verdict to the host action it requires. Unknown decision
/// strings (a newer kernel talking to an older adapter) map to
/// `Block { Deny }` — fail-closed on the verdict channel.
pub fn host_outcome(res: &GateResponse) -> HostOutcome {
    let reason = match res.rule.as_deref() {
        Some(rule) if !rule.is_empty() => format!("{} ({rule})", res.reason),
        _ => res.reason.clone(),
    };
    match res.decision.as_str() {
        "ALLOW" => HostOutcome::Proceed,
        "ASK" => HostOutcome::NeedsApproval { reason },
        "ABSENT" => HostOutcome::Block {
            kind: BlockKind::Absent,
            reason,
        },
        "REPLAN" => HostOutcome::Block {
            kind: BlockKind::Replan,
            reason,
        },
        _ => HostOutcome::Block {
            kind: BlockKind::Deny,
            reason,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::{GateResponse, GateResponseContext, ABI_VERSION};

    fn res(decision: &str, rule: Option<&str>, reason: &str) -> GateResponse {
        GateResponse {
            v: ABI_VERSION,
            decision: decision.to_string(),
            action: "x".to_string(),
            rule: rule.map(String::from),
            reason: reason.to_string(),
            context: GateResponseContext {
                taint: "clean".to_string(),
            },
            approval: None,
            manifest_hash: "abc".to_string(),
        }
    }

    #[test]
    fn allow_proceeds() {
        assert_eq!(
            host_outcome(&res("ALLOW", None, "ok")),
            HostOutcome::Proceed
        );
    }

    #[test]
    fn ask_needs_approval_with_rule_in_reason() {
        assert_eq!(
            host_outcome(&res("ASK", Some("approval_required"), "needs a human")),
            HostOutcome::NeedsApproval {
                reason: "needs a human (approval_required)".to_string()
            }
        );
    }

    #[test]
    fn absent_deny_replan_stay_distinguishable() {
        for (decision, kind, label) in [
            ("ABSENT", BlockKind::Absent, "ABSENT"),
            ("DENY", BlockKind::Deny, "DENY"),
            ("REPLAN", BlockKind::Replan, "REPLAN"),
        ] {
            match host_outcome(&res(decision, Some("r"), "why")) {
                HostOutcome::Block { kind: k, reason } => {
                    assert_eq!(k, kind);
                    assert_eq!(k.label(), label);
                    assert_eq!(reason, "why (r)");
                }
                other => panic!("{decision} must block, got {other:?}"),
            }
        }
    }

    #[test]
    fn unknown_decisions_fail_closed_to_deny() {
        match host_outcome(&res("SOMETHING_NEW", None, "future verdict")) {
            HostOutcome::Block { kind, .. } => assert_eq!(kind, BlockKind::Deny),
            other => panic!("unknown decision must fail closed, got {other:?}"),
        }
    }
}
