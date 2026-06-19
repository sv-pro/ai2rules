# CLI Agent Harness — Execution Plan

High-level execution plan for building the harness specified in
[`docs/harness-architecture.md`](docs/harness-architecture.md): a deterministic
governance kernel that sits underneath a local CLI developer agent (Claude Code,
Codex CLI, Gemini CLI, Aider, …) and controls what the agent can perceive, what
actions it can represent, and what validated specs may cross into real execution.

This document is the **epic-level roadmap only**. Each epic lists its goal,
dependencies, constituent tasks, and exit criteria. Detailed task breakdown
(issues, estimates, owners, acceptance tests per task) is deliberately deferred —
see [Next step](#next-step).

---

## North star

A developer can run the harness against the default world and have it drive a
real model through file edits, commands, patches, MCP tools, and web fetches —
where every dangerous action is *absent or denied by construction*, every
decision is *deterministic and replayable*, and no LLM sits on the enforcement
path. Success is measured by the [16 acceptance invariants](#acceptance-invariant-coverage)
passing deterministically in CI, with bounded attack-success-rate and high task
utility on a benchmark suite.

## Guiding principles (carried from the architecture)

1. **Kernel, not wrapper** — define the world the agent perceives; don't inspect
   calls after the fact.
2. **Validity by construction** — only a sealed `IntentIR` reaches the kernel;
   only an `ExecutionSpec` reaches the executor.
3. **Deterministic runtime** — no LLM, no I/O, no mutable shared state in the
   kernel; same inputs ⇒ same decision.
4. **Absence over denial** — absent actions don't exist; `ABSENT` ≠ `DENY` ≠
   `UNKNOWN_TO_ONTOLOGY`.
5. **Monotonic taint + provenance** — taint only increases, survives sessions.
6. **Effect ⟂ decision** — `SIMULATE`/`PROXY`/… are effect modes, never verdicts.
7. **Design-time stochastic, runtime deterministic** — LLMs draft manifests;
   humans review; the compiler freezes them.
8. **Fail closed** — `ASK` collapses to `DENY` in `BACKGROUND`; the executor
   refuses anything not in its local closed registry.
9. **Everything is traced and replayable** — append-only, redacted, reproducible.

## How this plan is organized

- Work is grouped into **epics** (`E0`–`E10`); each epic holds **tasks**
  (`E2.3`, …).
- Epics are ordered by dependency / build order and grouped into four
  **milestones** (`M1`–`M4`).
- Status legend: `[ ]` not started · `[~]` in progress · `[x]` done.

---

## Milestones

| Milestone | Theme | Epics | Outcome |
|---|---|---|---|
| **M1 — Deterministic Core** | Kernel works in simulation | E0, E1, E2, E3, E4 | Vertical slice: `read_file` → kernel → sim executor → trace → replay, all deterministic |
| **M2 — Live Agent** | A real model drives the loop | E5, E6 | One provider proposes through the projected surface; interactive approvals work; background fails closed |
| **M3 — Full Tool Surface** | Real-world capabilities | E7, E9 | MCP + web + scoped capabilities behind one gate; usable interactive CLI/TUI |
| **M4 — Isolation & Hardening** | Production posture | E8, E10, E11, E12 | OS-level sandbox backstop; all acceptance invariants + security scenarios + benchmarks green; visual World Authoring UI for manifest design; establish industry authority via tech blog |

### Dependency sketch

```
E0 ─┬─> E1 ─┬─> E2 ─┬─> E3 ─┬─> E4 ──(M1)
    │       │       │       │
    │       │       │       └─> E6 ──┐
    │       │       └─────> E5 ──────┴─(M2)
    │       └─────────────> E7 ──┐
    └─────────────────────> E9 ──┴─(M3)
                            E8 ──┐
                            E10 ─┼─(M4, depends on all)
                            E11 ─┤
                            E12 ─┘
```


---

## Epics

### E0 — Foundations & Core Contracts
**Goal:** a buildable workspace and the typed contracts every other epic depends
on. **Depends on:** nothing. **Status:** core done; serialization + contributor
docs in progress.

- [x] **E0.1** Rust workspace (`Cargo.toml`, `resolver = "2"`) scaffolded with the
  crate skeleton: `cli-harness`, `agent-core`, `provider-adapters`,
  `world-kernel`, `executor`, `trace-store`, `compiler`, plus a foundational
  **`harness-types`** crate (see note below). Builds clean offline.
- [x] **E0.2** Core contract types defined in `harness-types`: `Perception`,
  `Provenance`, `Taint`/`TaintedValue`/`TaintContext`, `ToolCall`, `Descriptor`,
  `Decision`, `EffectMode`, `ExecutionMode`, `Disposition`, `ExecutionSpec`,
  `ApprovalToken`, `CompiledWorld`, `WorldManifest`. `IntentIR` lives in
  `world-kernel` (it is sealed — see E0.4).
- [x] **E0.3** Failure/outcome taxonomy defined as `BuildError`
  (`UnknownToOntology`, `Absent`, `SchemaViolation`, `CapabilityViolation`,
  `InvariantViolation`, `DescriptorDrift`, `TaintViolation`, `ApprovalRequired`,
  `BudgetExceeded`).
- [x] **E0.4** Structural invariants enforced by the type system: `IntentIR` has
  private fields and no public constructor (only `IRBuilder::build` in
  `world-kernel` can mint one); `CompiledWorld` exposes getters only and is built
  once from `CompiledWorldParts`. Covered by unit tests (absence levels, taint
  carry-through, approval transitions).
- [~] **E0.5** Serialization wired via `serde`/`serde_json` derives on all
  contracts; formats chosen (manifest YAML, trace JSONL). Remaining: the SHA-256
  descriptor/manifest **hashing + versioning scheme** (lands with E1.4).
- [~] **E0.6** CI added (`.github/workflows/ci.yml`: fmt-check, `clippy -D
  warnings`, build, test). Module-boundary rules captured in crate doc comments;
  a dedicated `CONTRIBUTING.md` is still to write.

> **Note — added crate `harness-types`.** A deliberate refinement to E0.1's
> seven-crate list. The shared contracts live in `harness-types` so `executor`,
> `trace-store`, and the adapters can depend on the types **without** depending
> on `world-kernel`. `IntentIR` is the one exception: it stays in `world-kernel`
> so Rust's privacy can *seal* it (only `IRBuilder` constructs it). This keeps
> the dependency graph flowing inward to `harness-types` and satisfies the
> architecture's "keep core contracts language-neutral" rule.

**Exit:** types compile; sealing/immutability enforced and tested; kernel crate
has no I/O or LLM dependencies. **Met** (build + `clippy -D warnings` + 9 unit
tests green offline); E0.5/E0.6 tails tracked above.

---

### E1 — Manifest & Compiler
**Goal:** turn a reviewed manifest into an immutable `CompiledWorld`.
**Depends on:** E0. **Status:** done (M1 progressing).

- [x] **E1.1** `WorldManifest` schema complete in `harness-types`: actors, trust
  channels, data classes, **capability matrix** (`CapabilityGrant`), base actions
  (now with `arg_constraints` + optional `backing`), scoped capabilities,
  taint/transition policies, budgets, observability/redaction.
- [x] **E1.2** Loader + validator in `compiler` (`loader.rs`): `load_yaml` /
  `load_json`; `validate` checks empty `world_id`, duplicate actions, scoped-cap
  name collisions, and unknown base-action references, surfaced via a
  human-readable `CompileError`.
- [x] **E1.3** `compile()` (`compile.rs`): manifest → immutable `CompiledWorld`
  with `world_id` + `manifest_hash`; pure and deterministic.
- [x] **E1.4** Descriptor hashing (`hashing.rs`): real SHA-256 (via `sha2`) over
  JSON-normalized descriptors/manifest, verified against FIPS test vectors;
  closed-ontology + projected-world tables built (projection = full ontology by
  default; dynamic narrowing deferred to E2).
- [x] **E1.5** Default CLI world authored as `crates/compiler/assets/default_world.yaml`
  (all eight base actions + scoped caps `read_repo_file`, `apply_workspace_patch`,
  `run_tests`, `git_commit`), embedded via `include_str!` and exposed through
  `default_cli_world()` / `compile_default()`.
- [x] **E1.6** Hot reload = recompile → a new value; `CompiledWorld` is never
  mutated. Determinism (same manifest ⇒ equal world + hash) and version change
  (changed manifest ⇒ different `manifest_hash`) covered by tests.

**Exit:** the default world compiles to a stable frozen artifact; descriptor
hashes are reproducible; invalid manifests are rejected with clear errors.
**Met** — 13 compiler tests green; full workspace at 22 tests, `clippy -D
warnings` + fmt clean offline.

---

### E2 — World Kernel
**Goal:** the deterministic heart — representability + disposition.
**Depends on:** E0, E1. **The most security-critical epic. Status:** done.

- [x] **E2.1** `IRBuilder`: representability checks (ontology, projection,
  capability, schema, descriptor, hard taint invariant) → sealed `IntentIR` or
  typed failure. (`intent.rs`; `schema.rs` for minimal arg validation.)
- [x] **E2.2** Provenance + monotonic taint engine: primitives stay in
  `harness-types`; the kernel adds `taint::externally_effectful` and reads taint
  structurally at the build seam so it can only increase (incl. cross-session).
- [x] **E2.3** Invariants engine (`invariants.rs`): code-level hard floor (taint
  × side-effect; descriptor-drift gate), run before manifest policy and
  non-overridable by manifest or human approval.
- [x] **E2.4** Disposition evaluation (`disposition.rs`): ordered deterministic
  rules (manifest taint policy → approval → budget → default allow + effect
  mode) → `Decision` + `EffectMode`, first match wins.
- [x] **E2.5** Budget accounting: caller-supplied `BudgetUsage` in `EvalContext`
  (commands, tokens, network, writes) compared against `world.budget()` →
  `REPLAN`. The kernel reads usage; it never accumulates state.
- [x] **E2.6** Determinism guarantee: `build`/`evaluate`/`decide` are pure fns of
  `(intent, context, world)`; a matrix test asserts repeated `decide` is stable.

**Exit:** kernel returns a deterministic outcome for any
`CompiledWorld` + `ToolCall` + context via `decide()`; representability/
disposition split is enforced. Satisfies invariants **1, 2, 3, 6** (and the
invariant-7 taint floor). **Met** — 21 new kernel tests; full workspace at 43,
`clippy -D warnings` + fmt clean offline.

---

### E3 — Execution Boundary
**Goal:** run validated specs behind a hard process boundary. **Depends on:**
E0 (integrates with E2). **Status:** done.

- [x] **E3.1** `Executor` (closed registry) with handlers isolated from policy
  state; the `Handler` trait holds no policy and never decides.
- [x] **E3.2** `run()` accepts only `ExecutionSpec`; an unregistered action is
  refused (`ExecError::Unregistered`), and descriptor drift is caught before any
  handler (`ExecError::DescriptorDrift`).
- [x] **E3.3** Core handlers: `ReadHandler`, `PatchHandler` (structured full-file
  write — real unified-diff parsing deferred, no offline diff crate),
  `CommandHandler` (real subprocess, thread-drained output, deadline + direct
  child kill; process-group kill-tree deferred to E8).
- [x] **E3.4** Effect-mode application: `Execute` / `Simulate` / `Truncate` real;
  `Proxy` / `Sanitize` / `Defer` return `UnsupportedEffectMode` (later epics;
  Sanitize needs E4 redaction).
- [x] **E3.5** Simulation = an `Executor` driven with `Simulate` specs (no
  separate type); no real side effects.
- [x] **E3.6** `run()` returns `TaintedValue<ExecOutput>` (execution results are
  tainted by default).
- [x] **E3.A** Kernel-side spec assembly: `world-kernel::build_execution_spec` +
  `ExecEnv`; `KernelOutcome::Evaluated` now carries the sealed `IntentIR` so an
  `ALLOW` lowers to an `ExecutionSpec`.

**Exit:** core handlers run in sim and real modes; executor rejects non-spec and
unregistered actions; writes constrained to writable roots. Satisfies invariants
**4, 5, 8, 13, 16** (and **11** at the boundary). **Met** — 9 executor tests + 2
kernel round-trip tests; full workspace at 54, `clippy -D warnings` + fmt clean
offline; `kernel_demo` shows the end-to-end round-trip.

---

### E4 — Trace, Audit & Replay
**Goal:** every decision reproducible; secrets never leaked. **Depends on:** E0,
E2, E3. **Completes M1. Status:** done.

- [x] **E4.1** Append-only JSONL trace store (`TraceStore`) with `TraceRecord`s
  (`record.rs`/`store.rs`); Decision + Execution payloads, with enum room for
  perception/projection/proposal/approval stages as E5/E6 wire them.
- [x] **E4.2** Redaction before disk write (`redact.rs`): masks values whose
  key/dotted-path matches a manifest pattern (`*`-glob); reuses
  `world.redaction_patterns()`. Full glob/value semantics deferred.
- [x] **E4.3** Deterministic replay (`replay.rs`): reconstruct inputs, re-run
  `decide`, compare — same world ⇒ `matched == total`.
- [x] **E4.4** Policy-drift report: `drift_report` = replay vs a different world;
  mismatches are the explicit diff.
- [x] **E4.5** Bundle export/import (`bundle.rs`): `Bundle { manifest, records }`
  → JSON; `replay_bundle` recompiles the world and replays offline.
- [ ] **E4.6** (Optional) cross-implementation parity harness (e.g. Rego mirror)
  — deferred.

**Exit:** replay reproduces decisions; redaction holds; the M1 vertical slice
(`read_file` → kernel → sim executor → trace → replay) passes end to end.
Satisfies invariants **14, 15**. **Met** — 9 trace-store tests; full workspace at
63, `clippy -D warnings` + fmt clean offline; `trace_demo` shows
record → redact → replay → drift. **Milestone M1 complete (E0–E4).**

---

### E5 — Provider Adapters & Orchestrator
**Goal:** a real model drives the loop, seeing only the projected surface.
**Depends on:** E0, E2. **Status:** done (offline; live HTTP deferred).

- [x] **E5.1** Neutral result contract: `ToolOutcome` (`provider-adapters`) the
  orchestrator fills and an adapter formats; `ToolCall` (the inbound contract)
  already lived in `harness-types`.
- [x] **E5.2** Anthropic adapter (`anthropic.rs`): `tool_use` → `ToolCall`,
  `tool_definitions` from the projected surface, `format_tool_result`. Pure
  format; no policy.
- [x] **E5.3** Intent mapper (`agent-core/intent.rs`): ontology pre-check
  (`UNKNOWN_TO_ONTOLOGY` vs known); the kernel's `decide` stays authoritative.
- [x] **E5.4** `agent-core` context packing + projected tool surface
  (`context.rs`, via the new `CompiledWorld::projected_actions()`); only projected
  actions are exposed.
- [x] **E5.5** Model loop (`orchestrator.rs`): propose → adapt → `decide` →
  record (E4) → on ALLOW build spec + execute (SIMULATE) → perceive tainted
  result → feed back; a `ModelClient` trait + deterministic `ScriptedModel`
  stand in for an LLM.
- [ ] **E5.6** Additional adapters (OpenAI, Gemini) — deferred. A real Anthropic
  HTTP client is also deferred (feature-gated, out of the offline core).

**Exit:** a model completes a task through projected tools only; one policy gate
regardless of provider; adapters enforce no policy. Reinforces invariants **3**
and **4** (the model only proposes; the kernel alone mints an `ExecutionSpec`).
**Met** — 5 adapter + 6 agent-core tests; full workspace at 74, `clippy -D
warnings` + fmt clean offline; `agent_loop` demo runs a scripted session end to
end.

---

### E6 — Approvals & Execution Modes
**Goal:** human-in-the-loop as durable state; autonomous runs fail closed.
**Depends on:** E2, E3, E4. **Completes M2. Status:** done.

- [x] **E6.1** `ExecutionMode` threaded into `evaluate` (`EvalContext.mode`); an
  approval-required action branches on it.
- [x] **E6.2** Durable `ApprovalStore` (`trace-store/approval.rs`): append-only
  JSONL of lifecycle transitions folded on load; `pending → approved/rejected →
  executed`. Persisted, not in-memory.
- [x] **E6.3** `ASK` lifecycle in the orchestrator: mint a pending token →
  resolve via `ApprovalPolicy` → on approve, re-decide with the grant → `ALLOW`
  → execute → `mark_executed`; reject path surfaces a refusal.
- [x] **E6.4** Binding: `is_granted` matches an Approved token on action +
  params hash + world id + descriptor hash + provenance + effect mode; any drift
  voids reuse.
- [x] **E6.5** `BACKGROUND` fails closed — an approval-required action collapses
  `ASK → DENY` ("background_denies_ask"); no token minted.

**Exit:** interactive approvals resume execution; background denies; tokens
invalidated by drift. Satisfies invariants **9, 10**. **Met** — kernel mode tests
+ 4 store tests (mint/approve/drift/reopen) + orchestrator resume/deny tests;
full workspace at 82, `clippy -D warnings` + fmt clean offline; `approvals_demo`
shows both paths. **Milestone M2 complete (E5–E6).** Decisions logged in
`DECISIONS.md`.

---

### E7 — MCP, Web & Scoped Capabilities
**Goal:** external tools and the web flow through the same gate; broad tools are
narrowed. **Depends on:** E1, E2, E3. **Status:** done (offline mock transports).

- [x] **E7.1** MCP dispatch via `BackingIdentity::McpServer` → `build_execution_spec`
  lowers to a structured `{server,tool,input}` op; the executor's `McpHandler`
  dispatches through a pluggable `McpTransport` (mock; real stdio/HTTP deferred).
- [x] **E7.2** MCP descriptor drift: handlers register under the action's
  descriptor hash; the executor's existing pre-dispatch hash check blocks a stale
  upstream tool (invariant 11).
- [x] **E7.3** Web fetch (`WebHandler` + `WebFetcher`); results are tainted by the
  executor's default, so tainted web can never drive egress (floored at build).
- [x] **E7.4** Scoped-capability machinery in `build_execution_spec`: keep only
  declared `ActorInput` args (strip locked/unknown), inject `Literal`s, resolve
  `ContextRef` from `ExecEnv.context`. `CompiledWorld::scoped_capability` exposes
  the spec.
- [x] **E7.5** Default world ships `read_repo_file`, `apply_workspace_patch`,
  `run_tests`, `git_status`/`git_diff`/`git_commit`, and `call_known_mcp_tool`.

**Exit:** MCP/web run through one gate; tainted MCP/web cannot drive external
effects; scoped-cap stripping verified. Satisfies invariants **7, 11, 12**.
**Met** — spec stripping tests + executor MCP/web/drift tests + loop test; full
workspace at 89, `clippy -D warnings` + fmt clean offline; `tools_demo` shows
all three. E9 adds the interactive CLI and structured decision feedback,
completing **Milestone 3**. Decisions D14–D16 logged.

---

### E8 — Execution Physics (Layer 0 Sandbox)
**Goal:** an OS-level backstop independent of policy. **Depends on:** E3.

- [ ] **E8.1** Isolated working dir + explicit writable roots (writes can't
  escape, enforced at the OS level too).
- [ ] **E8.2** Isolated `HOME`; no ambient host SSH/cloud credentials; env
  allowlist.
- [ ] **E8.3** Network disabled by default + manifest-defined egress exceptions.
- [ ] **E8.4** Subprocess timeout/kill-tree; PTY session ownership +
  cancellation.
- [ ] **E8.5** Pluggable container / gVisor / namespace backend for higher
  isolation.

**Exit:** the sandbox enforces physics even if policy is imperfect; the two are
independent backstops. Hardens invariant **8**.

---

### E9 — CLI / TUI
**Goal:** the user-facing harness. **Depends on:** E5, E6. **Completes M3.**

- [x] **E9.1** Terminal UX: prompt input, streaming output, status. (Basic REPL loop using `inquire`).
- [x] **E9.2** Approval UX surfacing action / reasoning / provenance, with
  one-shot vs manifest-extension paths kept separate. (Using `ApprovalPolicy::Interactive` callback).
- [x] **E9.3** Structured rendering of `ABSENT` / `DENY` / `ASK` / `REPLAN`
  feedback with explicit decision, rule, effect, and operator-facing guidance.
- [x] **E9.4** Session management, world selection, mode toggle. (Via `--world`, `--background` CLI args).
- [x] **E9.5** `--simulate` flag to run against the simulation executor for safe
  demos/tests.

**Exit:** a developer can run the harness interactively against the default world
end to end.
**Met** — `TranscriptEntry` carries structured decision/rule/effect metadata and
`harness` renders governed steps as explicit `Decision`, `Rule`, `Effect`, and
`Feedback` fields. **Milestone 3 complete.**

---

### E10 — Acceptance, Security Scenarios & Benchmarks
**Goal:** prove the invariants and measure security/utility. **Depends on:** all.
**Completes M4.**

- [ ] **E10.1** Encode all 16 acceptance invariants (architecture §14) as a
  deterministic CI test suite.
- [ ] **E10.2** Security scenarios: prompt injection, cross-session zombie taint,
  descriptor drift / rug-pull, exfiltration attempt, dependency poisoning.
- [ ] **E10.3** Determinism + replay regression suite gated in CI.
- [ ] **E10.4** Utility/benchmark harness (AgentDojo-style task × attack pairs)
  reporting attack-success-rate and task utility.
- [ ] **E10.5** Design-time loop tooling: manifest-drafting assistant +
  trace-failure explainer (LLM at design time only).

**Exit:** all invariants green in CI; ASR/utility tracked over time; drift and
regression gates enforced.

---

### E11 — World Authoring Tool (Design-Time UI)
**Goal:** A browser-based visual editor for drafting, testing, and visualizing world manifests with real-time feedback on the resulting tool surface, capability mapping, and security decisions. **Depends on:** E1, E2, E4, E5. **Status:** [ ] not started.

- [ ] **E11.1** Scaffolding the React/Vite SPA frontend: 3-column layout (registered/base tools + scoped capabilities on the left, YAML manifest editor with live syntax highlighting/linting in the center, and a live "Effective Tool Surface" + preview panel on the right).
- [ ] **E11.2** Local Rust API Endpoint: Implement a thin API server subcommand in the CLI harness (e.g. `cli-harness serve --port 8080`) that hosts the static SPA assets and exposes endpoints:
  - `GET /api/base-tools`: List available base tools/actions and their descriptor schemas.
  - `POST /api/manifest/compile`: Compiles draft YAML manifest and returns errors or structural success.
  - `POST /api/manifest/preview`: Accepts a draft YAML manifest and returns the projected tool surface (names, renames, stripped/literal args) and decision matrix (ALLOW/ASK/DENY/ABSENT with reasons).
- [ ] **E11.3** Live Preview UI Panel: Render the compiled tool surface with color-coded collision flags, mapped scoped capability parameters, and simulated execution previews.
- [ ] **E11.4** Manifest Exporter: Support local download/save of validated YAML manifest files (updating the workspace `.agents/default_world.yaml` if configured).
- [ ] **E11.5** Integration with E10.5: Embed manifest-drafting LLM assistant and trace-failure explainer in the UI (e.g., loading an audit trace file to visually trace a denied action and suggest manifest edits to resolve it).

**Exit:** Developer can start `cli-harness serve`, edit the manifest YAML side-by-side with live compilation and tool-surface preview, and download/commit the result.

---

### E12 — Developer Advocacy & Blog Platform
**Goal:** Establish industry authority in deterministic AI execution via a Google Discover-optimized blog. **Depends on:** M1, M2. **Status:** [ ] not started.

- [ ] **E12.1 [Tech]** Scaffold the blog platform: Initialize an Astro project optimized for Core Web Vitals, MDX for content, automated WebP/AVIF image generation, and `<meta name="robots" content="max-image-preview:large">`.
- [ ] **E12.2 [Tech]** Implement SEO & Discovery layer: Generate `Article` / `TechArticle` JSON-LD schema, configure automated OpenGraph/Twitter Card generation, and setup WebSub/RSS feeds for instant indexing.
- [ ] **E12.3 [Content]** Draft "Why 'Deny' is Dangerous: The Case for Absent Tools in AI" (Thought Leadership): Focus on the architectural failure of wrappers vs. kernels, and the case for "Absent over Deny." Include a stark architectural diagram.
- [ ] **E12.4 [Content]** Draft "AI Aikido: Using Deterministic Rules to Neutralize Prompt Injection" (Deep Dive): Translate ADRs to prose, focusing on the `WorldManifest`, the design-time stochastic vs runtime deterministic philosophy.
- [ ] **E12.5 [Content]** Draft "Running Claude Code Safely: A Sandbox Setup Guide" (Tutorial): Provide a practical guide to using the `cli-harness`, demonstrating the interactive approval UI and preventing destructive commands.
- [ ] **E12.6 [Promotion]** Kickstart Discover algorithm: Seed initial deep dives and architecture arguments on Hacker News, relevant subreddits (`r/LocalLLaMA`, `r/rust`, `r/MachineLearning`), and X (Twitter threads).

---


## Acceptance invariant coverage

Traceability from the architecture's 16 acceptance invariants to the epic that
delivers each (and the epic that hardens it).

| # | Invariant (abbreviated) | Primary | Hardened by |
|---|---|---|---|
| 1 | Unknown actions cannot form `IntentIR` | E2 | E10 |
| 2 | Known-but-non-projected → `ABSENT`, not `DENY` | E2 | E10 |
| 3 | `UNKNOWN_TO_ONTOLOGY` distinct from `ABSENT` | E2 | E5 |
| 4 | Model cannot invoke an executor directly | E3 | E10 |
| 5 | Only `ExecutionSpec` crosses into execution | E3 | E10 |
| 6 | Taint never decreases (incl. across sessions) | E2 | E10 |
| 7 | Tainted file/web/MCP/shell can't drive egress/cred/memory/external | E2 | E7 |
| 8 | Writes can't escape writable roots | E3 | E8 |
| 9 | Destructive commands require approval | E6 | E10 |
| 10 | Approval-required in background denies | E6 | E10 |
| 11 | Descriptor drift blocks before handler | E1 | E7 |
| 12 | Scoped caps strip locked args before injecting literals | E7 | E10 |
| 13 | `ALLOW + SIMULATE` has no real side effect | E3 | E10 |
| 14 | Replay with same world reproduces the decision | E4 | E10 |
| 15 | Redaction keeps secrets out of audit logs | E4 | E10 |
| 16 | Executor refuses actions absent from its local registry | E3 | E10 |

---

## Out of scope (for now)

- Multi-agent / sub-agent orchestration beyond a single agent loop.
- Hosted/remote control plane, multi-tenant deployment, web dashboards.
- Resolving semantic ambiguity ("forward this to Alex") — an acknowledged open
  problem, not a deliverable.
- Non-CLI surfaces (IDE/browser extensions).

## Next step

This plan stops at the epic level by design. The immediate follow-up is to
**decompose E0 into concrete tasks** (issues with acceptance tests, estimates,
and sequencing) and stand up the workspace + CI, since every other epic depends
on it. Subsequent epics are decomposed just-in-time as their milestone
approaches.
