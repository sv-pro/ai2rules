//! Disposition evaluation (architecture §6, E2.4 + E2.5).
//!
//! A built `IntentIR` is representable *by construction* — ontology, projection,
//! capability, schema, and the hard taint invariant already passed in
//! `IRBuilder::build`. This stage decides the remaining contextual rules over
//! that intent and emits a [`Disposition`] (`ALLOW + EffectMode`, `ASK`,
//! `DENY`, or `REPLAN`). It is a pure function of `(world, intent, ctx)`: no
//! I/O, no LLM, no mutable shared state — budget *usage* is supplied by the
//! caller, never accumulated here.

use harness_types::{
    ActionType, Budget, CompiledWorld, Decision, Disposition, EffectMode, ExecutionMode,
    SideEffectClass, TaintContext,
};

use crate::intent::IntentIR;

/// Caller-supplied budget usage for the current task/session. The kernel reads
/// these counters; it does not own or mutate them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BudgetUsage {
    pub commands_run: u64,
    pub tokens_used: u64,
    pub file_writes: u64,
    pub network_calls: u64,
}

/// The runtime moment the kernel evaluates against: the inbound taint context
/// (used by `IRBuilder::build` via [`crate::decide`]), the execution mode, and
/// current budget usage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvalContext {
    pub taint: TaintContext,
    /// Carried now; the `BACKGROUND` → `DENY` collapse for `ASK` is wired in E6
    /// (E6.5). E2 evaluation is mode-agnostic.
    pub mode: ExecutionMode,
    pub usage: BudgetUsage,
}

impl EvalContext {
    /// An interactive context with a clean taint and no budget used — the common
    /// pipeline entry point.
    pub fn interactive_clean() -> Self {
        Self {
            taint: TaintContext::clean(),
            mode: ExecutionMode::Interactive,
            usage: BudgetUsage::default(),
        }
    }

    /// Replace the taint context (keeping mode and usage).
    pub fn with_taint(mut self, taint: TaintContext) -> Self {
        self.taint = taint;
        self
    }

    /// Replace the budget usage (keeping mode and taint).
    pub fn with_usage(mut self, usage: BudgetUsage) -> Self {
        self.usage = usage;
        self
    }
}

/// Evaluate the disposition of a representable intent. Ordered, first match
/// wins — continuing the §6 numbering after representability:
///
/// 1. manifest taint policy (additional to the hard floor enforced at build);
/// 2. destructiveness / approval;
/// 3. budgets;
/// 4. default allow + effect mode.
pub fn evaluate(world: &CompiledWorld, intent: &IntentIR, ctx: &EvalContext) -> Disposition {
    // 1. Manifest taint policy. The hard floor (build-time `TaintViolation`)
    //    already blocked tainted → externally-effectful intents, so those never
    //    reach here; this catches any *additional* manifest-authored rule (e.g.
    //    a softer `Tainted + FilesystemWrite → Ask`).
    for rule in world.taint_rules() {
        if rule.from_taint == intent.taint() && rule.side_effect == intent.side_effect() {
            return Disposition::of(rule.decision, rule.rule.clone());
        }
    }

    // 2. Destructiveness / approval. Reversibility (§6.6) and approval state
    //    (§6.7) are both signalled by the manifest's `approval_required` flag in
    //    E2; the durable token lifecycle and `BACKGROUND` collapse are E6.
    if world.requires_approval(intent.action()) {
        return Disposition::of(Decision::Ask, "approval_required");
    }

    // 3. Budgets.
    if let Some(rule) = budget_exceeded(
        world.budget(),
        &ctx.usage,
        intent.action_type(),
        intent.side_effect(),
    ) {
        return Disposition::of(Decision::Replan, rule);
    }

    // 4. Default allow, with a compiled effect-mode override if one is pinned.
    let effect_mode = world
        .effect_rules()
        .iter()
        .find(|r| r.action == *intent.action())
        .map(|r| r.effect_mode)
        .unwrap_or(EffectMode::Execute);
    Disposition::allow(effect_mode, "default_allow")
}

/// Returns the name of the first exceeded budget for this intent, if any.
/// "At or over" the limit counts as exceeded.
fn budget_exceeded(
    budget: &Budget,
    usage: &BudgetUsage,
    action_type: ActionType,
    side_effect: SideEffectClass,
) -> Option<&'static str> {
    if let Some(max) = budget.max_commands_per_task {
        if matches!(action_type, ActionType::Command | ActionType::Pty) && usage.commands_run >= max
        {
            return Some("max_commands_per_task");
        }
    }
    if let Some(max) = budget.max_network_calls {
        let networky = matches!(action_type, ActionType::Web)
            || matches!(side_effect, SideEffectClass::Network);
        if networky && usage.network_calls >= max {
            return Some("max_network_calls");
        }
    }
    if let Some(max) = budget.max_file_writes {
        let writes = matches!(
            side_effect,
            SideEffectClass::FilesystemWrite | SideEffectClass::PersistentWrite
        );
        if writes && usage.file_writes >= max {
            return Some("max_file_writes");
        }
    }
    if let Some(max) = budget.max_tokens_per_session {
        if usage.tokens_used >= max {
            return Some("max_tokens_per_session");
        }
    }
    None
}
