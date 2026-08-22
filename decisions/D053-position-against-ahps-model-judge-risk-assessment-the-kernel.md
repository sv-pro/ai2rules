# D53 — Position against AHP's model-judge risk assessment; the kernel may *source* a risk assessment, never *consume* one


**Date:** 2026-08-06. **Extends** D27 (position against Microsoft's AGT) to a second
Microsoft artifact, and **applies D52's one-way rule** to a new surface. Constrained by
THESIS §2 (the stochastic-classifier objection) and D34/D24 (where a verdict is allowed to
live).

- **Context.** The **Agent Host Protocol** (`microsoft/agent-host-protocol`, MIT, v0.7.0,
  created 2026-03-12, 23 contributors, five SDKs, VS Code as reference server) is a host↔client
  coordination protocol: *N* clients sharing one agent session over an immutable state tree
  with pure reducers and write-ahead reconciliation. Its own doctrine places it *above* ACP —
  "AHP is a coordination layer. ACP is a communication layer" — and calls itself a
  **client-facing presentation model** that refuses to define agent loops, model providers, or
  tool registries. On that reading it is orthogonal to the gate and the correct disposition is
  *Ignore*. **That reading is wrong**, and the reason is one enum.
- **The finding.** AHP's chat channel carries a full tool-call confirmation state machine
  (`pending-confirmation` → approve/deny, `ConfirmationOption[]`, `pendingPermissions`). The
  only source of a confirmation requirement its type system can name is a model judge:

  ```ts
  /** Identifies a model judge as the source of a confirmation requirement. */
  export const enum ToolCallRiskAssessmentKind { Judge = 'judge' }

  export interface ToolCallRiskAssessmentCompleteState {
    status: ToolCallRiskAssessmentStatus.Complete;
    reason: StringOrMarkdown;
    /** The judge's normalized safety score, where `0` is unsafe and `1` is safe. */
    safety: number;
  }
  ```
  <sub>`types/channels-chat/state.ts`</sub>

  A single-variant enum, whose payload is a float. THESIS §2 states the objection directly:
  *"if your security depends on a stochastic classifier being right 100% of the time, you are
  already compromised."* This is the D52 prohibition — no probabilistic classifier in the trust
  path — encoded as a protocol type, shipping, versioned, and pre-1.0. It is prior art that
  disagrees with us **in its schema**, which is a sharper contrast than D27's AGT (where the
  disagreement lives in an architecture note) and a strictly better foil for it.
- **Decision, part 1 — disposition.** **Position-against (primary) + Incorporate (narrow,
  gated).** Not Adopt-as-tool. AHP joins D27's shelf in `THIRD-PARTY-ADOPTION.md` and gets a
  THESIS §8 paragraph. The differentiation is again **mechanism, not goal**: AHP escalates by
  *score*, the border decides by *structure* — the dangerous capability is `ABSENT`, taint is
  monotonic and provenanced, and no float appears anywhere in a verdict.
- **Decision, part 2 — the one-way rule, which is the load-bearing half.** **AHP client state
  may never become a kernel input.** The kernel may be a **source** of an AHP risk assessment;
  it may never be a **consumer** of one. This is D52's rule on a new surface, and here it has a
  second, independent justification: AHP clients are *multi-party*, and write-ahead
  reconciliation means their local state is **optimistically divergent by design** — clients
  apply their own actions before the host echoes them back. Optimism is correct for a UI and
  disqualifying for a trust input.
- **Decision, part 3 — AHP is not a governance seam.** Enforcement stays where D24/D34 put it:
  the host PreToolUse seam, ACP, and the executor boundary. A verdict rendered into a
  synchronized presentation state tree is *advisory*, and shipping one there would be governance
  theatre of exactly the kind this project exists to name. Concretely: **no AHP dependency from
  `world-kernel` or `harness-preview`**; if a fourth-host adapter ever lands it lives in
  `cli-harness` behind a feature flag, like every other host.
- **The fairness caveat, recorded here so the blog post cannot quietly drop it.** The judge
  gates whether a **confirmation is requested**, not whether the call **executes**. It is an
  escalation heuristic, not an enforcement decision, and "Microsoft ships classifier-based
  enforcement" would be a false claim. The critique survives the correction — a mis-scored
  escalation is a prompt that silently never appears, and a confirmation nobody was asked for
  is indistinguishable from one nobody needed — but it must be *stated as the weaker claim it
  is*. `detbench` learned this the expensive way (a headline that inverted once the defended
  variant ran); the same discipline binds here **before** publication rather than after.
- **Two more type-level findings, both already met elsewhere in this project.**
  - `ToolCallConfirmationReason.Setting` — "Approved by a persistent user setting." The
    **cache-satisfiable ask**, promoted to a protocol enum. Already met as Antigravity's stored
    "Always Allow" and answered by D48's `force_ask` default. A protocol-level `Setting` means
    an approval can be satisfied by a past decision rather than a present human, and nothing in
    the state distinguishes the two after the fact.
  - `editedToolInput` on `ChatToolCallApprovedAction` — a client may **modify the tool's input
    parameters while approving them**. Approval and mutation arrive in one action, so the thing
    approved is not necessarily the thing proposed. TOCTOU-shaped; treat as a research note, not
    a finding, until reproduced.
- **Recorded as unverified. None of these may appear in public writing until checked.**
  (a) Whether *any* connected client may approve a tool call — `ChatToolCallConfirmed` is
  `@clientDispatchable` and no per-client authorization model was found, but absence of evidence
  in a spec read is not evidence of absence. (b) Whether a third party can host AHP or interpose
  on the VS Code reference server at all — this gates the adapter entirely. (c) Whether the judge
  is enabled by default in the reference implementation. There is also **no threat-model document
  in the repository**; the only trust discussion (`docs/guide/mcp.md`) concerns sandboxing the
  MCP UI View, and terminals document no approval gate.
- **Alternatives rejected.**
  - *Ignore it, per the doctrine's own layering.* Rejected on D27(b)'s reasoning — silence cedes
    the comparison that **is** the contribution — and more so here, because the disagreement is
    in a type rather than a design note.
  - *Adopt AHP now as the structured ASK transport.* Genuinely tempting: elicitation and
    `ConfirmationOption[]` are the structured ask channel OpenCode lacks entirely (D35) and
    Antigravity satisfies from cache (D48). Rejected on sequencing and on part 3 — the seam is
    unverified, and moving the approval channel into the presentation plane before a
    deterministic assessment variant exists means speaking a protocol that can only express a
    float.
  - *Build the fourth host adapter first.* Rejected: the upstream proposal is only landable
    **pre-1.0**, and an adapter shipped ahead of it inherits the schema it was meant to change.
  - *Fork or vendor.* No. Same rule as every other entry in `THIRD-PARTY-ADOPTION.md`.
- **Sequenced actions.** Wave 0 (now): this entry; the `THIRD-PARTY-ADOPTION.md` row; THESIS §8;
  the blog post, leading with the fairness caveat; Flywheel discovery cases for `Setting` and
  `editedToolInput` against the three live hosts. Wave 1 (gated): an upstream proposal adding a
  **second `ToolCallRiskAssessmentKind`** — a deterministic/policy variant carrying
  `{rule_id, manifest_hash, decision}` in place of `safety: number` — plus a paper-only
  `GateResponse` → AHP mapping (ASK → `pending-confirmation` + `ConfirmationOption[]`; DENY →
  `ToolCallCancellationReason.Denied`; ABSENT → never advertised, which is where D51's "the world
  owns the surface it publishes" lands, since the MCP channel's capability advertisement is a
  real surface-shaping mechanism). Wave 2 (only if Wave 1 lands and (b) above resolves): a fourth
  host in `tests/one_kernel.rs`.
- **Known residual: the protocol is pre-1.0 and says so.** Breaking changes are expected. Unlike
  D48's `agy` contract — reverse-engineered from a shipped binary — this one is vendor-published,
  versioned, and MIT, which is better footing, not stable footing.
- **Review condition.** If the upstream proposal is declined or ignored by **2026-11-06**, drop
  to Position-against only and do not build the adapter: without a deterministic variant, a
  conformant integration would be required to represent kernel verdicts as a safety float, which
  part 2 forbids.
- **Caveat: the acronym is contested.** At least three other agent-space protocols call
  themselves AHP (two "Agent Handoff", one "Agent Handshake"), and Analytic Hierarchy Process
  dominates the plain search. Expand on first use in any public writing — the same hygiene D27
  requires for the AGT / "Agent Hypervisor" collision.
- **Related:** D24, D27, D34, D40, D48, D51, D52; THESIS §2, §3, §8; `STRATEGY.md` (witness over
  depth); [`docs/THIRD-PARTY-ADOPTION.md`](../docs/THIRD-PARTY-ADOPTION.md);
  <https://github.com/microsoft/agent-host-protocol> · <https://microsoft.github.io/agent-host-protocol/>
