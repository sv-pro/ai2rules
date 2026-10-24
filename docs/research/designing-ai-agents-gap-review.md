# Gap review: *Designing AI Agents* (MEAP v3) against ai2rules

*Research note, 2026-10-23. Not a decision: it maps the book's recommendations onto what
ai2rules has, so the gaps worth closing can be picked deliberately.*

**Source.** Jia Huang, *Designing AI Agents: Principles, patterns, and best practices*,
Manning MEAP, version 3 (2026). This edition contains chapters 1–9. Chapters 10
(composition) and 11 (case studies) and Appendix A are listed in the contents but not yet
written, so nothing here is drawn from them.

**Page numbers** are the printed page numbers (PDF page minus 5). Everything from the
book is paraphrased.

**Baseline.** `origin/main` at `efb8127` (D1–D82, `docs/action-layers.md`). Claims
marked *verified* were checked against the code or the 0.7.0 binary. The rest are read
from the decision log and the docs.

**Already-known items, not re-reported as findings:** AI2-55 (unsupported constraint
keywords pass silently), AI2-56 (broken-world mode), AI2-57 (Windows), AI2-59 (per-host
tool vocabularies), `data_classes` declared but not enforced, no domain-level language
(`docs/action-layers.md`). Rows that touch them point at them.

---

## The short version

The book and ai2rules agree on the basic shape. The model proposes; a harness outside
the model decides what may run, what runs where, and what is recorded. The author's
summary line, "the model spends; the harness budgets", fits the project's "the LLM
proposes, the kernel disposes" ([`THESIS.md`](../THESIS.md) §1). The closest overlap is
in two places:

- **Chapter 6, Tool Dispatch + Guardrail Sandwich.** Eligibility before selection,
  admission of the exact call, protected arguments bound from runtime state, a
  runtime-issued commit identity, unknown outcomes never blindly retried. These are, nearly
  field for field, D72/D75 (projection), D79/D80 (scoped capabilities, effective call),
  D73 (exact single-use authorization) and D74 (staged effect, receipts, `ambiguous`).
  D74 is a tested prototype that the default agent loop does not use yet.
- **Chapter 9, Approval Gate.** The book's "durable intent" across an approval pause
  (digest-bound, expiring, revalidated on resume, consumed once) is D73 plus D67/D78.

Where the book is ahead: it insists on **recording outcomes, not only decisions**
(observability that links proposal, approval and effect), on **evidence for changing
autonomy** (approval-fatigue and override metrics), on **narrower authority for
delegated workers**, on **damage limits over time**, and on treating **model-written
memory** as unauthoritative. Where ai2rules is ahead: information-flow control (taint),
a compiled closed world with `ABSENT` as its own verdict, deterministic replay of
*decisions* against a changed world, fail-closed gate context, and signed, chained
approval logs. On hosts, two of those come with conditions: Claude Code enforces `ABSENT`
only with `--enforce-absent`, and its hook still fails open on a broken world (AI2-56). The book has no counterpart to taint at all.

Eight gaps are worth closing (ranked below). G7 is small enough to finish before the
October demo. G1 is too, if matching on canonical paths turns out to be world data
rather than a change to the gate.

---

## Gap table

Gap: **none** (ai2rules covers it), **partial**, **missing**, **different** (a deliberate
difference, with the reason).

### Chapters 1–2: foundations

| # | The book recommends | ai2rules has | Gap |
|---|---|---|---|
| 1 | An agent is a model choosing actions inside a harness that supplies permissions, budgets and stopping rules. Reliability comes from the harness, not the model (ch. 1 §1.1, pp. 2–5). | The same split as the founding thesis: design-time stochastic, runtime deterministic, no LLM in the trust path ([`THESIS.md`](../THESIS.md) §1, §3). | **none** |
| 2 | Grant authority per action, by reversibility, scope and blast radius. Least authority that still finishes the task (ch. 2 §2.5, p. 40). | Closed ontology, `side_effect` classes, `approval_required`, path `roots` ([`harness-architecture.md`](../harness-architecture.md) §5–7). | **none** |
| 3 | Enforce boundaries in the runtime, not the prompt. Guardrails are infrastructure outside model reasoning, like a sidecar (ch. 2 §2.5, pp. 40–41). | The gate is a separate process per call (D24, D37). Policy lives in a compiled world, never in a prompt. | **none** |
| 4 | A person can halt a running agent at any point, including mid-plan (ch. 2 §2.5, p. 40). | The host's own interrupt. A console switch (D81) takes effect at the next call, but there is no one-step "stop everything" position. Subprocess kill-tree is E8.4, open. | **partial** → G7 |
| 5 | Audit trails record what the agent did, what it was authorized to do, and what it considered but rejected (ch. 2 §2.5, p. 41). | Every DENY / ABSENT / ASK is a trace record with its rule (E4). Hosts write a decision log (`cc-hook --log`). | **none** for refused calls. The model's unspoken alternatives are not observable, and the book itself warns that chain-of-thought is not a faithful record (ch. 9, p. 387). |
| 6 | Trust is a lifecycle: onboard, evaluate, promote, monitor, demote (ch. 2 §2.5, p. 40). | Autonomy changes only when a person edits the world (D57, D81, architecture §10). Within a session, taint demotes automatically and irreversibly. | **different.** No runtime trust score is allowed in the trust path. What is missing is the *evidence* a person would use to promote or demote → G3. |
| 7 | Start with one agent. Add agents only after a measured structural limit (ch. 2 §2.4, pp. 38–40). | Not the harness's choice. It governs whatever topology the host runs (PLAN "Out of scope"). | **different** |
| 8 | A runtime layer, like an OS, gives sandboxing, persistence, tool hosting and monitoring (ch. 2 §2.2.2, pp. 26–28). | Executor, trace-store and mcp-gateway. The OS sandbox is E8, not built. Commands fail closed without one (D47); the web handler enforces its own egress (D65); a container gives a floor (D21). | **partial** (known: E8) |

### Chapter 3: perception

| # | The book recommends | ai2rules has | Gap |
|---|---|---|---|
| 9 | Perception (what enters context) and governance (what leaves as action) are separate boundaries. The harness owns both (ch. 3 §3.1, p. 54; ch. 9, p. 397). | Every input becomes a typed `Perception` with channel, trust and taint (architecture §5), but the harness does not choose what enters context. | **different.** ai2rules records *where* context came from and lets that decide actions. Curating context is the host's job. |
| 10 | Progressive disclosure of tools is not progressive authority: loading a tool's description grants nothing (ch. 6 §6.3.2, p. 203). | Stronger: a non-projected action is `ABSENT` and cannot be loaded or called (invariant 2, D72, D75). On Claude Code the call is refused only when the hook runs with `--enforce-absent` (row 19). | **none** |

### Chapter 4: memory

| # | The book recommends | ai2rules has | Gap |
|---|---|---|---|
| 11 | Storage grants no authority. Current policy, approvals, hard limits and effect receipts belong to an authoritative control plane, not to memory (ch. 4 §4.1, p. 96). | The world is read-only to the agent and changed only through a non-agent tool (D57). Approvals and receipts live in signed, chained logs (D67, D74, D78). | **none** |
| 12 | Notes the model writes for itself (Claude Code auto memory, instruction files) are model-authored context with provenance, not a policy store (ch. 4 §4.1.3, p. 99). | Tainted *memory* writes are denied (`no_tainted_memory_write`). But the files Claude Code loads as instructions into the next session (`CLAUDE.md`, `AGENTS.md`, auto-memory files, `.claude/skills/**`, `.claude/agents/**`) are written with `Write`/`Edit`, which are plain `FilesystemWrite`. **Verified:** with the 0.7.0 binary and main's `.claude/cc-world.yaml`, a *tainted* `Write` to `CLAUDE.md` returns `ALLOW`, while a tainted `WebFetch` returns `DENY taint_invariant`. The `harness init` world protects `./.claude` (D57) but not `CLAUDE.md`, `AGENTS.md` or the user-level memory directory. | **partial** → G1 |
| 13 | Before stored material re-enters context, check scope, authority, version, expiry and supersession (ch. 4 §4.6, p. 129). | `trust_pins` pin a read source to its content hash, with an expiry (D29). The knowledge layer (`context-engine`, a separate repo) has `active / deprecated / invalid` with gated promotion (THESIS §4.3). Host memory files have nothing like it. | **partial.** Host-side memory is covered by G1; the knowledge layer is outside this repo. |
| 14 | On resume, receipts and idempotency keys must stop a completed effect running twice (ch. 4 §4.6, pp. 129–130). | `StagedEffect`, idempotency key, `ExecutionReceipt`; `ambiguous` is never retried automatically (D74). | **none** (prototype, not in the default loop) |

### Chapter 5: reasoning

| # | The book recommends | ai2rules has | Gap |
|---|---|---|---|
| 15 | Consequence overrides difficulty. A consequential request goes to governed handling however simple it looks (ch. 5 §5.3, p. 155). | The kernel decides every call regardless of model, route or reasoning depth. | **none** |

### Chapter 6: action

| # | The book recommends | ai2rules has | Gap |
|---|---|---|---|
| 16 | An action contract: intended effect, execution boundary, state basis, execution rule, commit identity, acceptance evidence (ch. 6 §6.1.2, pp. 190–191). | `ExecutionSpec` (boundary, rule, descriptor hash) and `StagedEffect` (commit identity, epochs, consequence class, D74). State basis covers world and schema epochs, but not the version of the business object. Acceptance evidence is not modelled. | **partial.** Business-object freshness belongs to the domain layer (`action-layers.md` Q3). Acceptance → row 27. |
| 17 | Risk follows the effect, not the step count or the tool name. Inspect arguments: `run_command` with `rm -rf` is a delete (ch. 6 §6.1.3, p. 192; ch. 9 §9.2.6, p. 358). | `side_effect` classes; `command_classes` reclassify by argument with a fail-closed default (D36, D44, D62, D63); `arg_roles`. | **none.** Pattern classification can be evaded, and D36 says so. |
| 18 | Prompt chaining with named artifacts and gates between stages (ch. 6 §6.2, pp. 193–200). | Out of scope: that is orchestration. | **different** |
| 19 | Compute the eligible tool set *before* the model selects. The registry returns summaries, never callables (ch. 6 §6.3.1, pp. 200–207). | Projection: `harness project` (D72, D75), D79's hidden wrapped tool. Raw handlers are unreachable from the model loop (architecture §4). | **none** in agent-core and over MCP. **partial** on Claude Code: a hook cannot remove built-in tools from the model's view, and an `ABSENT` call is refused only when the hook runs with `--enforce-absent`. Without the flag, which `harness init` does not add, the hook stays silent and Claude Code's own permission flow decides (`cc_hook.rs` `hook_answer`). |
| 20 | Admission rechecks the exact call: authority, where each argument came from, state freshness (ch. 6 §6.3.3, pp. 207–211). | Per-call gate; explicit fail-closed context (D42); descriptor drift; `arg_roles`. Provenance is per session (one taint bit) with per-argument refinement by role, not per-value labels. | **partial** (known: "labels on data", `action-layers.md` Q1; `data_classes`) |
| 21 | Arguments the model must not choose are bound from protected runtime state (ch. 6 §6.3.3, pp. 205–206). | `!Literal`, `!ContextRef`, `!Input { one_of, prefix }` (D79). The host executes the lowered call, not the proposal (D80). | **none** |
| 22 | The runtime issues the commit identity. Reuse with different arguments is rejected; a repeat returns "unknown, verify" (ch. 6 §6.3.3, pp. 206, 211–214). | D73: an exact, single-use authorization, used by the default loop. D74: an idempotency key reserved before the effect, a duplicate receipt, `ambiguous` never retried automatically, with tests (PLAN E6 exit). D74 is a separate staged-commit API. The default loop does not call it: `orchestrator.rs` `run_allowed` calls `executor.run(&spec)` directly, as D74 records. A single-use approval is not a durable effect reservation. | **partial.** Covered by the D74 prototype, not by the default loop (known: D74, AI2-7). On hosts, the host executes and there is no commit identity, by design of the gate ABI (D24). |
| 23 | Admission must be the *only* execution path, or it is advisory. A Claude Code hook that times out or prints malformed output falls back to the normal permission flow, so a hard rule needs a fail-closed gateway (ch. 6, p. 211; ch. 9 §9.2.3, p. 352). | Invariants 4, 5, 16 in agent-core. On hosts: a bootstrap shim fails open (D37). Once engaged, the DeepSeek Harness adapter fails closed (D71). The Claude Code and Antigravity hooks do not: `cc-hook` returns 0 with no decision when the world cannot be read or compiled (`cc_hook.rs`; known: AI2-56), and an `ABSENT` call falls through without `--enforce-absent` (row 19). An unrecordable taint escalation fails closed (D59); managed scope stops users removing the hook (D82). Hook timeout is 10 s (`init.rs`). | **partial**, known and documented. The physical backstop is E8. |
| 24 | No silent fallback: an agent whose API call fails must not move to a shell or browser with wider reach (ch. 6 §6.3.4, p. 214). | Shell egress is reclassified (D36) and caught by the taint floor; an unclassified command asks (D44). Whether domain worlds should drop the shell entirely is open (`action-layers.md` Q6). | **partial** (known open question) |
| 25 | Imported content must not be able to add a capability (ch. 6 §6.3.4, p. 214). | The world is compiled at design time; MCP descriptors are hashed and drift blocks (architecture §5); the world owns the surface (D51); MRTR results cannot widen it (D49). | **none** |
| 26 | Plan-and-Execute: a durable plan; only pending work may be replanned (ch. 6 §6.4, pp. 214–228). | Orchestration is out of scope. Durable effects are D74. | **different** |
| 27 | Guardrail Sandwich: a trusted pre-check, a controlled runner, and a trusted post-check against authoritative evidence. The agent may not edit the policy, remove hooks or certify its own result (ch. 6 §6.5, pp. 228–239). | Pre-check: the gate. Runner: E8 open, D47/D65. Policy and hooks: D57, D82. Post-check: none. D74's receipt records *finality* (did the effect happen), not *acceptance* (was it right). Hosts record no outcome at all (no `PostToolUse` handler). | **partial.** Verifying correctness is deliberately not the kernel's job; recording the outcome is → G2. |
| 28 | Containment is not verification; redacting output is not a postcondition (ch. 6 §6.5.5, p. 239). | ai2rules claims neither. Redaction (D66) is for the audit log only. | **none** |
| 29 | Test the action boundary by injecting failures: object version changes after selection, a commit id reused with other arguments, a crash after the remote commit, an approval bound to an old version (ch. 6 §6.7, pp. 241–242). | D73/D74 tests cover reusing a commit id, a crash after commit (`ambiguous`), and an approval that crosses a world or schema epoch change (PLAN E6 exit list). Those tests exercise the staged-commit prototype, not the default loop (row 22). Also `check-demos.sh`, the `one_kernel.rs` conformance set, and govbench (D76). | **partial.** World and schema epoch drift is tested. A change in the *business object's* version after selection is not modelled, so it cannot be injected (row 16). |

### Chapter 7: reflection and evaluation

| # | The book recommends | ai2rules has | Gap |
|---|---|---|---|
| 30 | Reusable skills pass through candidate, trial and active states, with an applicability contract, a version and a withdrawal path. A skill's tool calls still go through governance (ch. 7 §7.4, pp. 267–279). | Skill *calls* are governed like any tool call. Skill *files* are instruction files the agent can write → G1. Admitting skills is not the harness's job. | **partial** → G1 |
| 31 | Evaluation: hard gates visible beside any aggregate score; evaluation cases have provenance; an empty evidence set blocks the release (ch. 7 §7.7, pp. 294–296). | govbench: no aggregate score anywhere, PASS needs observed evidence, and CI fails if the weak baseline stops failing (D76). | **none** (stronger on the baseline) |
| 32 | Measure usefulness beside safety: a system that blocks every call scores perfectly on safety (ch. 6 §6.7, p. 241; ch. 9 §9.5.3, p. 379). | E10.4 (task utility, AgentDojo-style) is unchecked. govbench's seeded scenarios already include the legitimate path. Scenario 02 requires the approved call to run exactly once (`the_approved_call_runs`, `one_yes_is_one_effect`), and scenario 03 requires the owner to still spend the grant, so a deny-all target fails them. What is missing is a suite of benign tasks that counts false denials on ordinary work. | **partial** → G5 |

### Chapter 8: collaboration

| # | The book recommends | ai2rules has | Gap |
|---|---|---|---|
| 33 | A delegated worker gets explicit, narrower tool and write boundaries. A handoff states which actions the receiver may take (ch. 8 §8.2, p. 304; §8.5.1, p. 327). | Subagents share the parent's taint, keyed by session (D20, conservative and correct). They also share the parent's *world*: `cc-world.yaml` declares a `subagent` actor, but `cc-hook` reads neither `agent_id` nor `agent_type`. | **partial** → G4 |
| 34 | Observe each collaboration seam and compare against a single-agent baseline (ch. 8 §8.6, pp. 332–335). | A `SubagentStop` hook surfaces taint (D20). The decision log does not say which agent made a call. | **partial** → G2 |
| 35 | MCP and A2A are plumbing. Who acts, and under what authority, stays with the runtime (ch. 8 §8.8.2, p. 339; ch. 2, p. 28). | mcp-gateway; the ingress-authority invariant (THESIS §2, P12): channel context may narrow a world, never widen it. | **none** |

### Chapter 9: governance

| # | The book recommends | ai2rules has | Gap |
|---|---|---|---|
| 36 | Approval gate precedence is deny, then allow, then ask, so convenience cannot override a hard boundary (ch. 9 §9.2.1, p. 351; listing 9.4, p. 357). | Stronger: invariants run before policy, and neither the manifest nor a human approval can override them (architecture §6; `9439c01`, invariants pinned above the console's switches). | **none** |
| 37 | Approval is a contract across time: a durable intent bound by digest, with a lifetime, revalidated on resume, consumed once, carrying an idempotency key (ch. 9 §9.2.4, p. 353). | `AuthorizationInstance` (D73), signed and policy-bound (D67), chained (D78). On hosts, ASK goes to the host's prompt, and the person's answer is not recorded anywhere. | **none** in agent-core; **partial** on hosts → G2 |
| 38 | Watch for approval fatigue: track approval and rejection rates, and move actions shown to be safe from ask to allow (ch. 9 §9.2.1, pp. 348–349; §9.2.6, p. 358). | The trade-off is named (PRINCIPLES C10). Nothing measures it. | **missing** → G3 |
| 39 | The reviewer sees an evidence packet: objective, target, current and proposed state, evidence reference, policy version (ch. 9 §9.8, p. 394). | The ASK reason gives the rule and action; the effective call is available (D80). There is no packet. | **partial** → G3 |
| 40 | A per-call gate can miss a harmful *sequence* of individually acceptable calls (goal hijack). Keep intent across the sequence (ch. 9 §9.1, p. 346). | The taint floor *is* a sequence rule, and budgets are carried state (D64). Rules over values and sums are open. | **partial** (known: AI2-55, `action-layers.md` Q1–Q2) |
| 41 | Blast radius has three nested layers: capability restriction, execution isolation, damage limitation (rate, cost, time lock, compensation) (ch. 9 §9.3, pp. 358–366). | Capability: strong. Isolation: E8 open. Damage: per-session counters only (D64). There are no time windows and no cost cap, and `EffectMode::Defer` exists as a type that the kernel never uses. | **partial** → G6 |
| 42 | Gate, limiter, sandbox and kill switch must not share one authority. The kill switch is independent of the agent process (ch. 9 §9.3.6, p. 366; p. 389; p. 397). | The kernel runs outside the agent, as its own process per call (D37). The managed scope is admin-owned (D82). There is no agent-halt control. ("Kill-switch" in README and D82 means disabling *governance*, the opposite thing.) | **partial** → G7 |
| 43 | Blast radius is a hard ceiling. Earned trust widens the room inside it but never moves it (ch. 9 §9.3.4, p. 362). | Invariants sit above policy; the console cannot open them (D81). | **none** |
| 44 | Progressive commitment within a run: a dry run before the live call, on the same frozen proposal (ch. 9 §9.4.1, p. 367). | `ALLOW + SIMULATE` (architecture §5). D74 makes SIMULATE consume the exact authorization. Hosts cannot simulate: a PreToolUse hook cannot replace the tool's result. | **partial**, deliberately different on hosts |
| 45 | Trust across runs is scoped by agent and model version, capability, task class, environment and policy version. The agent never writes its own trust record (ch. 9 §9.4.1, p. 369). | Worlds are per project, per host, or per machine (D82). The agent cannot write them (D57). Nothing is scoped by model version. | **different** (no runtime trust scores). The evidence gap → G3. |
| 46 | Some decisions stay with people at every trust level: critical operations, insufficient evidence, ethically sensitive actions (ch. 9 §9.4.2, p. 369). | `approval_required`; BACKGROUND turns ASK into DENY (E6.5). | **none** |
| 47 | One trace joins user, agent, session, run, tool call, approval and result, so an incident can be reconstructed without re-running the agent (ch. 9 §9.5.1, pp. 376–377). | agent-core: trace records with trace, session, world and manifest hash, plus replay (E4). Hosts: the decision log has session, tool, rule, taint and manifest hash, but no tool-call id, no agent id, no approval answer and no result (`cc_hook.rs` `decision_line`). | **partial** → G2 |
| 48 | Five evidence types, including evaluations and replays of stored trajectories (ch. 9 §9.5.1, table 9.7, p. 377). | Decision replay and policy-drift reports (E4.3, E4.4); `replay-transcripts` re-decides recorded Claude Code sessions. | **none.** Replaying a past session against a *new* world is something the book does not describe. |
| 49 | Export through OpenTelemetry's GenAI conventions, with one trace id across proposal, approval and execution (ch. 9 §9.5.1, p. 378). | No OTel export. (The only mention is a pass-through field in `mcp_gateway.rs`.) | **missing** → G2 |
| 50 | Watch task success, cost per task, tool-selection accuracy, human override rate and latency percentiles (ch. 9 §9.5.3, pp. 379–380). | None are computed. Override rate and verdict mix are the governance-relevant ones. | **missing** → G3 |
| 51 | Keep sensitive content behind references in a protected store; redact before telemetry (ch. 9 §9.5.3, p. 380; p. 383). | Value-scanning redaction before disk (D66); parameter summaries. | **none** |
| 52 | Turn each incident into a regression case that must now be refused (ch. 9 §9.5.6, p. 387). | The Flywheel (finding → failing test → fix); govbench scenarios. | **none** |
| 53 | Authorize twice: at assembly (remove tools this identity can never use) and at runtime (the chosen call and its arguments) (ch. 9 §9.6, p. 389). | `project` and `gate` (D72, D75). | **none** |
| 54 | Task-level caps over cumulative values, with an atomic reservation (e.g. 60 already issued + 50 proposed > 100 cap → deny) (ch. 9 §9.6, p. 389). | Counters only (D64). | **partial** (known: AI2-55, `action-layers.md` Q2) → G6 |
| 55 | At organization scale: a registry of agents, owners, tools, versions and lifecycle; a policy service; a shared audit layer; short-lived credentials (ch. 9 §9.6, p. 389). | One world file per scope; machine scope (D82). A hosted control plane is out of scope (PLAN). | **different** (for now) |
| 56 | No single pattern proves compliance. Map the controls to the evidence each obligation needs (EU AI Act, NIST AI RMF, ISO/IEC 42001) (ch. 9 §9.7, pp. 390–391). | Nothing. | **missing** → G8 |
| 57 | Carry identity through user → agent → tool into every policy and audit decision (ch. 9 §9.8, p. 397; §9.7.1, p. 390). | agent-core binds a trusted principal into authorizations (D73). Hosts carry an opaque session id and an operator-declared source channel (D69). | **partial** (folds into G2 and G4) |

---

## Gaps worth closing, ranked

| Rank | Gap | Size | Epic | Rows |
|---|---|---|---|---|
| **G1** | Instruction and memory files count as durable memory: tainted writes to them are refused | S–M | E13, E2 invariant 7 | 12, 13, 30 |
| **G7** | A "Stop" position in the policy console: every effectful action off at the next call | XS | E13 / D81 | 4, 42 |
| **G2** | Host decision log joins each decision to its outcome (tool-call id, agent id, ASK answer, result); optional OTel GenAI export | M | E4 + E13 | 27, 34, 37, 47, 49, 57 |
| **G3** | `harness report`: verdict mix, approval and override rates per rule, candidate world changes | M | E11 (design-time loop), E10.5 | 6, 38, 39, 45, 50 |
| **G4** | Subagent-scoped worlds: `agent_type` selects a narrower projection | M | E13 | 33, 57 |
| **G8** | Control-to-evidence map for regulators and buyers | S (doc) | E12 / E16 | 56 |
| **G6** | Damage limits over time: windowed rates, cumulative value caps, `Defer` as a cancellation window | M | E2 + E6 (after AI2-55) | 41, 54 |
| **G5** | Measure usefulness: false denials on benign tasks beside attack results | L | E10.4 / E18 | 32 |

G7 can land before the 2026-10-25/27 demo. G1 can too if canonical-path matching turns
out to be world data; if it needs a gate change, it is the first item after. G1 closes a laundering path: outside content → instruction file → the next session's trusted context. G7 gives the console a "pull the plug" moment that a security audience will ask for. G2 is what G3 needs. G6 waits for AI2-55, because a threshold that passes silently is worse than none.

Not worth closing: rows 7, 18, 26 and 55 (orchestration and hosted control plane, out of scope by plan), row 9 (context curation is the host's job), row 45 (runtime trust scores contradict "no stochastic component in the trust path").

---

## Draft Linear issues

Drafts only: nothing has been filed. Team AI2.

### G1 — Instruction and memory files count as durable memory

**Epic:** E13 (Claude Code integration); hardens invariant 7. **Size:** S if the
classifier can match canonical paths as world data; M if the gate has to change (see
Proposal).

**Problem.** Claude Code loads some files as instructions into every later session:
`CLAUDE.md` / `AGENTS.md` at any level, `~/.claude/CLAUDE.md`, auto-memory under
`~/.claude/projects/*/memory/`, and skills and agents under `.claude/`. Writing one is a
durable-memory write. The worlds classify it as an ordinary `FilesystemWrite`, which the
taint floor does not cover. Verified with 0.7.0 and main's `.claude/cc-world.yaml`: a
tainted `Write` to `CLAUDE.md` returns `ALLOW`. A tainted `WebFetch` in the same session
returns `DENY taint_invariant`. A session that has read outside content can therefore
leave instructions for the next session, which loads them as trusted context. That
launders taint across the session boundary. The `harness init` world protects `./.claude`
(D57) but not root-level `CLAUDE.md` / `AGENTS.md` or the user-level memory directory.

**Proposal.**
- Send instruction and memory paths written by `Write`, `Edit`, `MultiEdit` and
  `NotebookEdit` to a new action with `side_effect: Memory` (e.g. `Write_instructions`).
  The existing `no_tainted_memory_write` then denies it under taint. The argument key
  differs by tool: `file_path` for the first three, `notebook_path` for `NotebookEdit`.
  Antigravity's keys need checking.
- **It is not established that this is world data only.** `command_classes` can name any
  action and argument (`CommandClassDef`), but `classify_command` matches the raw
  `arguments[def.arg]` string. The resolved absolute path travels separately, in
  `GateRequest.path`, filled by `hostkit::resolve_action_path`. A pattern over the raw
  string misses `./CLAUDE.md`, `docs/../CLAUDE.md`, an absolute path, `claude.md` on a
  case-insensitive filesystem, and backslash separators. Requirement: classify on the
  canonical path, with the host-specific argument keys made explicit. If that means the
  classifier reads `GateRequest.path` rather than an argument, this is a kernel and
  gate-ABI change. Then G1 is size M, and the WASM engine must be rebuilt (D60). Windows
  paths depend on AI2-57.
- Apply it to `.claude/cc-world.yaml`, the `harness init` template, the `--user` /
  `--managed` worlds (D82), and `agy-world.yaml` for Antigravity's equivalents.
- Clean sessions: decide whether these writes ASK (recommended for the machine-wide
  worlds) or ALLOW (fine for project worlds).
- Shell writes (`echo >> CLAUDE.md`) remain D57's known residual (ASK via
  `Bash_unclassified`). Note that, don't fix it here.

**Acceptance.**
- A conformance case per host: tainted `Write` to `CLAUDE.md`, `AGENTS.md`,
  `.claude/skills/x/SKILL.md` and a memory file → `DENY no_tainted_memory_write`.
- The same verdict for each spelling of one target: relative, absolute, with `./` and
  `..`, with different letter case where the filesystem ignores it, and with either
  separator on Windows. Also a `NotebookEdit` case keyed by `notebook_path`.
- Clean writes behave as decided above.
- Tainted writes to ordinary source files are unchanged.
- `check-demos.sh` markers still hold.

**Refs:** *Designing AI Agents* ch. 4 pp. 96, 99, 129; THESIS §2 ("an agent that
remembers can have its memory corrupted"); D20, D29, D57.

### G7 — A "Stop" position in the policy console

**Epic:** E13, D81 follow-up. **Size:** XS.

**Problem.** The console (D81) can change what an agent may do at the next call, but it
has no single control for "stop this agent now". In a demo, and in the book's checklist
(ch. 2 p. 40; ch. 9 pp. 366, 389), a halt switch outside the agent's process is the first
thing a security reviewer asks for. The console already has the mechanism; it lacks the
position.

**Proposal.** Add a `stop` profile with all five switches `Off`, plus a distinct button.
Writing the world stays atomic, and the next tool call is refused. `Off` means `ABSENT`,
which binds only when the hook runs with `--enforce-absent`, as the demo's `setup.sh`
does. The stop button should refuse to show "stopped" when that flag is missing. Reads
are not a switch today, so decide whether reads also stop: either a sixth switch, or the
profile keeps reads with a line saying so. The UI should say plainly that it takes effect
**at the next call**: a call already running finishes (mid-call kill is E8.4).

**Acceptance.**
- With `--enforce-absent`, applying `stop` and then proposing `Write`, `Bash` and
  `WebFetch` returns `ABSENT` for each.
- Reverting to the previous profile restores them.
- The console log records when the agent was stopped and restored.
- A runbook line in `docs/demos/policy-console/`.

### G2 — Decision-to-outcome correlation in host decision logs

**Epic:** E4 (trace) + E13. **Size:** M.

**Problem.** The host decision log (`cc_hook.rs` `decision_line`) records session, tool,
action, verdict, rule, taint and manifest hash. It cannot answer what the book's
observability chapter treats as the minimum (ch. 9 pp. 376–380):
- Which tool call was this? There is no `tool_use_id`, so it cannot be joined to the
  host transcript.
- Which agent made it? There is no `agent_id` / `agent_type`.
- On ASK, what did the person answer?
- Did the call run, and with what result?

Without these, an ASK cannot be connected to an approval, and none of G3's metrics can
be computed.

**Proposal.**
- Add `tool_use_id`, `agent_id`, `agent_type` and `cwd` to each decision line when the
  host sends them.
- Add a `PostToolUse` (and, where available, `PostToolUseFailure`) observer to `cc-hook`
  and `agy-hook`. It is read-only (D77's rule) and appends an outcome line keyed by
  `tool_use_id`: ran or did not run, exit status or error, and a redacted, truncated
  result summary through D66.
- An ASK followed by an outcome line is "approved"; an ASK with no outcome is "declined
  or abandoned". This is inferred, not attested, and the log must say so.
- Optional, behind a flag: OTLP export of these lines using the GenAI semantic
  conventions, so an organization's existing telemetry pipeline can collect them.
- Fail-open rules: observation never changes a verdict. A log that cannot be written is
  reported, as today.

**Acceptance.**
- A test session yields joined decision/outcome pairs for ALLOW, ASK-then-ran and DENY.
- `replay-transcripts` can consume the new fields.
- No secrets in result summaries (D66 tests extended).
- Documented in `docs/harness-gate-abi.md` or a sibling page.

### G3 — `harness report`: evidence for changing a world

**Epic:** E11 (design-time loop, architecture §10), E10.5. **Size:** M. **Depends on:** G2.

**Problem.** ai2rules deliberately keeps runtime trust scores out of the trust path. A
person changes autonomy by editing the world. But nothing gives that person evidence.
The book's approval-fatigue signal (ch. 9 pp. 348–349, 358) and override rate (p. 380) are
exactly the inputs architecture §10 asks for ("suggest manifest changes after
human-reviewed approvals"), and PRINCIPLES C10 names the risk.

**Proposal.** `harness report --log <decision log> [--world W]` prints, per action and
rule:
- the counts of ALLOW / ASK / DENY / ABSENT;
- the ASK approval ratio (from G2);
- asks never declined over N occurrences → *candidate allow*, shown as a world diff for a
  person to apply through the console or authoring tool (D57: proposal, never edit);
- the most frequent DENY rules (`taint_invariant`, `missing_usage` and the rest), which
  show whether a world is too tight;
- for any ASK, the review packet the book describes (p. 394): the effective call (D80),
  the rule, the world hash, the taint cause.

Never apply changes automatically.

**Acceptance.**
- A golden test over a fixed log produces a stable report with no aggregate score (D76's
  rule).
- Candidate diffs compile with the real compiler.
- One console view or a link from the console.

### G4 — Subagent-scoped worlds

**Epic:** E13. **Size:** M.

**Problem.** Claude Code subagents share the parent's session id, so they rightly share
taint (D20). They also share the parent's whole world. The book's delegation rule (ch. 8
pp. 304, 327) is that a worker gets narrower tool and write boundaries than the manager.
`cc-world.yaml` already declares a `subagent` actor, but the hook ignores the
`agent_id` / `agent_type` the host sends.

**Proposal.**
- Optional `agents:` block in the world: `agent_type` (glob) → a projection that may only
  *narrow* the parent's (a subset of actions, stricter positions). Compiling a world in
  which a subagent projection widens the parent's is an error.
- The adapter passes `agent_type` as gate context. An unknown type gets the parent world
  (no change by default).
- Taint stays session-wide.
- Same semantics in `project`, so a host that can filter a subagent's tool list does so.

**Acceptance.**
- Conformance cases: a `reviewer` subagent with no write tools gets `ABSENT` on `Write`
  while the parent gets `ALLOW`.
- A widening world fails to compile.
- WASM parity check passes (D60).

### G8 — Control-to-evidence map

**Epic:** E12 (advocacy) / E16. **Size:** S (doc).

**Problem.** Enterprise reviewers ask which obligations a control satisfies and what
evidence it produces (ch. 9 §9.7, pp. 390–391). ai2rules has strong evidence (signed
approvals, chained logs, replay and drift reports, a benchmark with a must-fail baseline)
but no page that maps it to EU AI Act articles (logging, human oversight), NIST AI RMF
functions, ISO/IEC 42001 controls, and the OWASP Top 10 for Agentic Applications (2026).

**Proposal.** `docs/compliance-evidence-map.md`: one table, control → artifact →
obligation. Every row links to a decision or a test. Make no compliance claims; say
"evidence for", not "compliant with". Keep partner names out.

**Acceptance.**
- Every artifact named exists and is linked.
- A reviewer from outside the project can follow one row from the obligation to a file
  on disk.

### G6 — Damage limits over time

**Epic:** E2 + E6. **Size:** M. **Blocked by:** AI2-55 (constraint keywords must fail
closed first).

**Problem.** Budgets are per-session counters (D64). The book's damage-limitation layer
(ch. 9 pp. 362–365) adds:
- **Rates per time window.** A slow campaign stays under a session cap.
- **Cumulative value caps with reservation.** E.g. the sum of amounts across calls
  (p. 389; `action-layers.md` Q2).
- **A time lock.** An irreversible action waits in a cancellable queue and is
  revalidated before running. `EffectMode::Defer` exists as a type but the kernel never
  produces it.

**Proposal.** Write a decision entry first: where windowed and summed state lives while
the kernel stays a pure function of `(call, context, world)`. Carrying it like `usage`
(D64) is the obvious candidate. Then:
- `max_*_per_window`;
- a `sum_of` budget over a named argument, using the reserve-then-settle shape of D74;
- `Defer` for actions marked `consequence: irreversible`, executed only through the
  staged-commit path.

**Acceptance.**
- Kernel tests: a window boundary; concurrent reservations that cannot overspend; a
  deferred effect cancelled before commit.
- The governance benchmark gains a slow-exfiltration scenario that the weak baseline
  fails.

### G5 — Measure usefulness beside safety

**Epic:** E10.4 / E18. **Size:** L.

**Problem.** The book warns that a system blocking every call scores perfectly on safety
(ch. 6 p. 241; ch. 9 p. 379). govbench is not that naive: scenarios 02 and 03 require
the legitimate call to run exactly once, so a deny-all target fails them. But those are
single legitimate steps inside attack scenarios. Nothing measures false denials across
ordinary work, and E10.4 (utility) is unchecked. A world that is tighter than it needs to
be on benign tasks goes unnoticed.

**Proposal.** Add benign scenarios to the benchmark pack: the same tasks without the
attack, and a few ordinary tasks with no attack counterpart. Report the false-denial count beside the attack results, per scenario, with no
combined score (D76). Start with the three seeded scenarios' benign twins.

**Acceptance.**
- Every attack scenario has a benign twin.
- The report shows both columns.
- CI fails if a world change turns a benign twin from PASS to FAIL without a scenario
  change beside it.

---

## What the book does not cover (positioning)

- **Information flow.** The book has no taint, no data labels and no "nothing leaves
  after outside content was read". It meets prompt injection twice: the payroll example
  (ch. 6 p. 187) and a passing mention under blast radius (ch. 9 p. 358). Its answers are
  protected arguments and commit identities, which stop *which* call runs but not *what
  data* leaves. ai2rules' taint floor is the counterpart, and nothing like it is in the
  book.
- **Absence as a verdict.** The book filters an "eligible set" at runtime by role and
  stage, then rejects at admission. ai2rules compiles a closed world in which a hidden
  action is `ABSENT`, a different verdict with a different audit meaning from `DENY`, and
  a model cannot argue with it. In agent-core and over MCP this holds as stated. A Claude
  Code hook refuses an `ABSENT` call only with `--enforce-absent`; otherwise it logs the
  verdict and leaves the call to Claude Code's permission flow.
- **Determinism and replay of decisions.** The book replays *trajectories* to understand
  a run. ai2rules replays *decisions*: the same inputs give the same verdict, and a drift
  report shows how an old session would fare under a new world. The book allows
  classifier and LLM-judge checks as evidence (ch. 9 pp. 369–370, 380). ai2rules keeps
  every stochastic component out of the trust path.
- **One kernel, many hosts.** The book compares Claude Code, Codex and LangGraph as
  separate products. ai2rules runs one kernel behind Claude Code, Antigravity, an MCP
  gateway and a WASM playground, with a shared conformance case set and a WASM freshness
  check.
- **Tamper evidence.** The book's approval and trust records are plain stores. ai2rules
  signs and chains them (D67, D78), binds approvals to the policy that granted them, and
  makes the control plane read-only to the agent (D57). The book says the agent must not
  edit its policy (p. 239) but gives no mechanism.
- **Fail-closed context.** At the gate, missing taint, channel or path is refused, not
  defaulted (D42, D59). The book notes that hooks fail open (p. 352). ai2rules has worked
  out which failures may fail open and which may not (D37, D59, D71), but only partly
  applied it. The DeepSeek Harness adapter fails closed once engaged (D71). The Claude
  Code and Antigravity hooks still fail open on an unreadable or uncompilable world
  (AI2-56).
- **MCP protocol hazards.** Descriptor drift, results that make demands (MRTR, D49), and
  remote reads tainting by backing (D50) are not in the book.
