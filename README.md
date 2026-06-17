# CLI Agent Harness

> Deterministic virtualization for local CLI developer agents.
> We don't filter what the agent does — we compile the physics of the world the
> agent lives in. Dangerous actions aren't blocked by a rule; they don't exist.

A governance **kernel** that sits underneath a local CLI developer agent (Claude
Code, Codex CLI, Gemini CLI, Aider, …) and controls what the agent can perceive,
what actions it can represent, and what validated specs may cross into real
execution. The model never touches raw reality: it sees a virtualized world
defined by a compiled manifest and can only *propose* typed intents into it.

This repository is **early-stage**. The architecture is fully specified; the
foundations and the manifest compiler are in place — see [Status](#status).

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
| **M1** Deterministic Core | kernel works in simulation | in progress |
| M2 Live Agent | a real model drives the loop | planned |
| M3 Full Tool Surface | MCP, web, scoped capabilities, CLI/TUI | planned |
| M4 Isolation & Hardening | sandbox + acceptance + benchmarks | planned |

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

Builds clean offline with `clippy -D warnings`; **43 unit tests** green.

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
  cli-harness/        terminal entrypoint (binary `harness`) (E9)
docs/                 architecture (harness-architecture.md is canonical)
PLAN.md               epic-level execution plan
CLAUDE.md             repo conventions for Claude Code / contributors
agent-hypervisor/     reference project (separate repo, not a workspace member)
safe-mcp-proxy/       reference project (separate repo, not a workspace member)
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
cargo run --bin harness        # prints the E0 skeleton banner
```

CI runs all four checks on every push and PR
([`.github/workflows/ci.yml`](.github/workflows/ci.yml)).

### See the kernel decide

Until the executor and interactive CLI land (E3/E9), the runnable demo is the
deterministic kernel itself. This compiles the default world and feeds it a
handful of proposed tool calls — printing `ALLOW` / `ASK` / `DENY` / `ABSENT` /
`REPLAN` for each, with **no real side effects**:

```bash
cargo run -p world-kernel --example kernel_demo
```

It shows the core idea in action: an undefined action is `UNKNOWN_TO_ONTOLOGY`,
an untrusted writer is `ABSENT` by capability, tainted data into the network is
`DENY` by a hard invariant, a PTY is `ASK`, and an over-budget command is
`REPLAN` — all decided by a pure function, no LLM on the path.

---

## Reference projects

The harness distills the best ideas from two prior projects kept alongside it
(as separate git repositories, not Cargo workspace members):

- **`agent-hypervisor/`** — the research kernel: sealed typed intent, monotonic
  taint, the process boundary, design-time HITL, invariants-as-physics.
- **`safe-mcp-proxy/`** — the productized MCP control plane: `ABSENT`/`DENY`/`ASK`
  semantics, descriptor-drift detection, scoped capabilities, provider adapters,
  append-only audit and replay.

`docs/harness-architecture.md` attributes each borrowed principle to its source.

---

## License

Dual-licensed under MIT or Apache-2.0.
