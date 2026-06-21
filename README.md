# CLI Agent Harness

> Deterministic virtualization for local CLI developer agents.
> We don't filter what the agent does — we compile the physics of the world the
> agent lives in. Dangerous actions aren't blocked by a rule; they don't exist.

A governance **kernel** that sits underneath a local CLI developer agent (Claude
Code, Codex CLI, Gemini CLI, Aider, …) and controls what the agent can perceive,
what actions it can represent, and what validated specs may cross into real
execution. The model never touches raw reality: it sees a virtualized world
defined by a compiled manifest and can only *propose* typed intents into it.

This repository is **early-stage**, but the **deterministic core (Milestone 1) is
complete**: the manifest compiler, the governance kernel, the execution boundary,
and the audit/replay layer are all in place — a proposed tool call flows all the
way to a real or simulated result, and every decision is logged, redacted, and
replayable. See [Status](#status).

---

## Why

A local CLI agent inherits the developer's full ambient authority — credentials,
SSH keys, git remotes, package managers, write access to the working tree — while
ingesting untrusted text on every turn (file contents, issue bodies, web pages,
MCP tool results). Prompt injection is therefore an *authority-boundary* problem,
not just a prompt problem. The harness closes that gap **structurally**:

```
raw reality            → typed Perception
model output           → typed ToolCall
ToolCall               → sealed IntentIR     (only via the kernel)
IntentIR               → ExecutionSpec        (only after deterministic evaluation)
ExecutionSpec          → the execution boundary (nothing else crosses)
```

No LLM participates in runtime policy enforcement. Decisions are a pure function
of `(intent, context, compiled world)` — deterministic and replayable.

Read the full design in **[`docs/harness-architecture.md`](docs/harness-architecture.md)**.

---

## Status

| Milestone | Theme | State |
|---|---|---|
| **M1** Deterministic Core | kernel works in simulation | ✅ done (E0–E4) |
| **M2** Live Agent | a real model drives the loop | ✅ done (E5–E6) |
| **M3** Full Tool Surface | MCP, web, scoped capabilities, CLI/TUI | ✅ done (E7, E9) |
| M4 Isolation & Hardening | sandbox + acceptance + benchmarks + authoring UI + tech blog + dogfooding | 🚧 E11, E12, E13 started; E8, E10 planned |
| M5 Interactive Advocacy | the real kernel in the reader's browser (WASM) + a TF-Playground-class visualization suite | 📋 planned (E14 engine, E15 suite; first viz = Taint-Flow Simulator) |

**Done so far:**

- **E0 — Foundations & Core Contracts:** the Cargo workspace, the language-neutral
  contracts in `harness-types`, the sealed `IntentIR` / `IRBuilder` in
  `world-kernel`, the `BuildError` taxonomy, and CI.
- **E1 — Manifest & Compiler:** a `WorldManifest` compiles into an immutable,
  hash-addressed `CompiledWorld` (`compiler`) — YAML/JSON loader + validator, real
  SHA-256 descriptor/manifest hashing, and a default CLI world
  (`crates/compiler/assets/default_world.yaml`).
- **E2 — World Kernel:** the deterministic heart. `IRBuilder::build` runs the
  representability checks (ontology → projection → capability → schema →
  descriptor → hard taint invariant) to seal an `IntentIR`; `disposition::evaluate`
  applies the contextual rules (manifest taint policy, approval, budgets) to a
  built intent; and `decide()` is the single pure entry point returning a
  `KernelOutcome`. Honors acceptance invariants 1, 2, 3, 6 (and the invariant-7
  taint floor).
- **E3 — Execution Boundary:** the kernel lowers an `ALLOW` to an `ExecutionSpec`
  (`build_execution_spec` + `ExecEnv`), and the `executor` runs it behind a
  closed registry — refusing unregistered actions and descriptor drift, applying
  `Execute` / `Simulate` / `Truncate`, constraining writes to writable roots, and
  returning a `TaintedValue`. Read / patch / command handlers run in both real
  and simulated modes. Honors invariants 4, 5, 8, 11, 13, 16.
- **E4 — Trace, Audit & Replay:** an append-only JSONL trace (`trace-store`)
  records every decision with secrets redacted *before* disk; `replay` reproduces
  decisions against the same world (determinism), `drift_report` diffs them
  against a changed manifest, and a `Bundle` packages a trace with its manifest
  for offline replay. Honors invariants 14, 15 — **completing Milestone 1.**
- **E5 — Provider Adapters & Orchestrator:** a model now drives the loop. The
  `provider-adapters` Anthropic adapter normalizes `tool_use`/`tool_result`/tool
  defs ↔ the neutral `ToolCall`; `agent-core` exposes only the projected tool
  surface, then runs propose → adapt → `decide` → execute (simulated) → perceive
  (tainted) → repeat, recording every decision to the trace. A `ModelClient`
  trait + deterministic `ScriptedModel` keep it fully offline (a live HTTP client
  is a later, feature-gated add). Reinforces invariants 3 and 4 — starting
  Milestone 2.
- **E6 — Approvals & Execution Modes:** human-in-the-loop as durable state. The
  kernel branches on `ExecutionMode` — an approval-required action `ASK`s
  interactively but **fails closed to `DENY` in background**; a durable
  `ApprovalStore` (`trace-store`) mints/persists tokens (`pending → approved →
  executed`) bound to the exact call, so drift voids reuse; the orchestrator
  resumes an approved `ASK` to `ALLOW`. Honors invariants 9, 10 — **completing
  Milestone 2.**
- **E7 — MCP, Web & Scoped Capabilities:** broader reach through the one gate.
  Scoped capabilities narrow a base action — `build_execution_spec` strips
  locked/unknown args and injects literals (so `run_tests` always runs `pytest`,
  invariant 12); MCP calls dispatch via a pluggable `McpTransport` through the
  same descriptor/drift path (invariant 11); web fetch is an always-tainted
  channel (invariant 7). MCP/web use deterministic **mock** transports (real
  stdio/HTTP deferred). Part of Milestone 3.
- **E9 — CLI / TUI:** `cargo run --bin harness` is now an
  interactive session — `clap` flags (`--world`/`--simulate`/`--background`), a
  human-driven `ModelClient` that proposes from the projected tool surface via
  `inquire`, and approval prompts through an `ApprovalPolicy::Interactive`
  callback; each step streams through the loop's observer with structured
  `Decision`, `Rule`, `Effect`, and `Feedback` fields for
  `ABSENT`/`DENY`/`ASK`/`REPLAN` outcomes — **completing Milestone 3.**
- **E11 — World Authoring Tool (in progress):** `harness serve` launches a local
  browser editor for world manifests — a single embedded page (no build step)
  whose `POST /api/preview` endpoint compiles a draft manifest through the
  **real** compiler + kernel and returns the projected tool surface plus a
  clean-vs-tainted decision matrix, so the live preview is faithful to what the
  harness would actually do — no governance logic reimplemented in JS (E11.1–E11.3;
  manifest export and the LLM-assist/trace explainer are the pending E11.4–E11.5).
  See `DECISIONS.md` D17/D18.
- **E13 — Claude Code integration (dogfooding, in progress):** the kernel's
  physics, ported onto the Claude Code host. A `PreToolUse` hook
  (`.claude/hooks/world-gate.py`) drives a JSON `WorldManifest`
  (`.claude/cc-world.json`) to enforce three behaviours — ABSENT-for-native, the
  monotonic **taint floor**, and **ASK** on destructive commands — additively
  (only ever `deny`/`ask`) and fail-open. Cross-agent taint follows the shared
  session sidecar (E13.6, D20), and a containerized governed SUT with an
  egress-allowlist proxy supplies the E8 enforcement floor (E13.7, D21).
  Self-contained demos: `demo-injection-egress.sh` (prompt-injection → egress,
  neutralized — E13.5) and `demo-cross-agent.sh` (subagent → parent taint).
  See `DECISIONS.md` D19–D21.

Builds clean offline with `clippy -D warnings`; **92 unit tests** green.

The epic-by-epic plan, with task checklists and acceptance-invariant traceability,
is in **[`PLAN.md`](PLAN.md)**.

---

## Repository layout

```
Cargo.toml            Rust workspace (resolver 2)
crates/               the harness implementation
  harness-types/      language-neutral core contracts (pure data; no I/O, no LLM)
  world-kernel/       sealed IntentIR + IRBuilder; policy, taint, budgets (E2)
  compiler/           WorldManifest → immutable CompiledWorld (E1)
  executor/           execution boundary: subprocess/PTY/fs/patch/MCP/web (E3)
  trace-store/        append-only audit, redaction, replay (E4)
  provider-adapters/  provider tool-call → neutral ToolCall (E5)
  agent-core/         context packing, projected tool surface, model loop (E5)
  cli-harness/        terminal entrypoint + `serve` authoring tool (binary `harness`) (E9, E11)
docs/                 architecture (harness-architecture.md is canonical)
blog/                 Astro blog — Discover-optimized advocacy site (E12; Node sub-project)
PLAN.md               epic-level execution plan
AGENTS.md             repo conventions (canonical; shared across AI assistants)
CLAUDE.md             Claude Code pointer → AGENTS.md
repos/                reference projects (separate git repos, not workspace members)
  agent-hypervisor/   research kernel
  safe-mcp-proxy/     productized MCP control plane
  mcp-tool-projection/ declarative MCP projections + authoring-UI pattern (informs E11)
```

`harness-types` is the foundation: every other crate depends inward on it.
`IntentIR` lives in `world-kernel` (not `harness-types`) so Rust's privacy can
**seal** it — only `IRBuilder::build` can construct one, making the existence of
an `IntentIR` a proof that representability checks passed.

---

## Build & test

Requires a stable Rust toolchain (developed against 1.87).

```bash
cargo build --workspace
cargo test  --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run --bin harness        # interactive harness (flags: --world <yaml> --simulate --background)
cargo run --bin harness -- serve   # World Authoring Tool at http://127.0.0.1:8787 (flag: --port)
```

CI runs all four checks on every push and PR
([`.github/workflows/ci.yml`](.github/workflows/ci.yml)).

### See the kernel decide

For a quick noninteractive demo, run the kernel plus the execution boundary. It
compiles the default world, feeds it a handful of
proposed tool calls — printing `ALLOW` / `ASK` / `DENY` / `ABSENT` / `REPLAN`
for each — and, for an `ALLOW`, lowers the intent to an `ExecutionSpec` and runs
it through the executor in **simulation** (no real side effects):

```bash
cargo run -p world-kernel --example kernel_demo
```

It shows the core idea in action: an undefined action is `UNKNOWN_TO_ONTOLOGY`,
an untrusted writer is `ABSENT` by capability, tainted data into the network is
`DENY` by a hard invariant, a PTY is `ASK`, and an over-budget command is
`REPLAN` — all decided by a pure function, no LLM on the path — while the allowed
read completes an end-to-end round-trip through the boundary.

For the boundary doing **real** work (confined to a throwaway sandbox), run:

```bash
cargo run -p world-kernel --example execution_demo
```

It actually reads a file, writes one, and runs a command — then has the executor
*refuse* a write that escapes the sandbox, a stale (drifted) descriptor, and a
command that overruns its timeout. (Writes are pinned to a temp dir; network
disable is declared in the spec but not yet OS-enforced — that backstop is E8.)

For the audit trail — every decision logged, redacted, replayed, and drift
detected — run:

```bash
cargo run -p trace-store --example trace_demo
```

It records a handful of decisions to an append-only trace (secrets redacted
before disk), replays them against the same world to prove they reproduce
exactly, then replays against a changed manifest to show the drifted verdict.

For a **model driving the loop** (a deterministic scripted stand-in for an LLM —
no network), run:

```bash
cargo run -p agent-core --example agent_loop
```

The model proposes Anthropic tool calls; the harness governs each through the one
gate — a read is `ALLOW`ed and its tainted result feeds back, which then makes a
web fetch `DENY` by taint, an undefined action `UNKNOWN_TO_ONTOLOGY`, and a PTY
`ASK` — every step recorded to the trace, with no LLM on the gate.

For **approvals + fail-closed background**, run:

```bash
cargo run -p agent-core --example approvals_demo
```

A `start_pty` (approval-required) action: interactive + auto-approve resumes it
`ASK → APPROVED → ALLOW` (a durable token minted → approved → executed); in
background it fails closed to `DENY` with no token minted.

For **scoped capabilities + MCP + web**, run:

```bash
cargo run -p agent-core --example tools_demo
```

`run_tests` proposed with `command: "rm -rf /"` lowers to argv `["pytest"]`
(locked args stripped, literal injected — invariant 12); an MCP call returns a
tainted result, after which a web fetch is `DENY`ed because the context is now
tainted (invariant 7). MCP/web use deterministic mock transports.

> Architectural decisions and the alternatives weighed are logged in
> [`DECISIONS.md`](DECISIONS.md).

---

## Reference projects

The harness distills the best ideas from prior projects kept alongside it under
`repos/` (separate git repositories, not Cargo workspace members):

- **`repos/agent-hypervisor/`** — the research kernel: sealed typed intent,
  monotonic taint, the process boundary, design-time HITL, invariants-as-physics.
- **`repos/safe-mcp-proxy/`** — the productized MCP control plane:
  `ABSENT`/`DENY`/`ASK` semantics, descriptor-drift detection, scoped
  capabilities, provider adapters, append-only audit and replay.
- **`repos/mcp-tool-projection/`** — declarative MCP tool projections
  (`verbatim`/`partial`/`simulated`/`absent`) and a browser **authoring tool**
  with a live "effective tool surface" preview — the pattern behind **E11**
  (World Authoring Tool); see `DECISIONS.md` D17.

`docs/harness-architecture.md` attributes each borrowed principle to its source.

---

## License

Dual-licensed under MIT or Apache-2.0.
