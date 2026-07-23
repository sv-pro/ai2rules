# AGENTS.md

Canonical guidance for AI coding assistants (Claude Code, Codex, Google
Antigravity, …) and humans working in this repository — **ai2rules**, the umbrella
codebase for the [ai2rules.dev](https://ai2rules.dev) site (its flagship layer is
the CLI governance **harness**). This is the single source of truth for project
conventions; per-assistant files just point here (see [Per-assistant
setup](#per-assistant-setup)).

Orient with `README.md` (overview), `docs/harness-architecture.md` (canonical
design), and `PLAN.md` (epic-level execution plan; the task source of truth).

---

## Repository layout

```
ai2rules/
├── crates/                   # 10 Rust crates (the harness implementation)
├── docs/                     # 17 architecture/design markdown files
├── blog/                     # Astro website (Node sub-project, separate from Rust workspace)
├── docker/                   # Containerisation: Dockerfile, compose, egress proxy
├── .claude/                  # Claude Code integration (hooks, agents, commands, config)
├── _tasks/                   # Task tracking
├── repos/                    # Reference external projects — NOT workspace members, never add
│   ├── agent-hypervisor/
│   ├── safe-mcp-proxy/
│   └── mcp-tool-projection/
├── PLAN.md                   # Epic-level execution plan — the task source of truth
├── DECISIONS.md              # ADR-lite decision log (D1–D32+)
├── README.md                 # Project overview, milestone status, build instructions
└── rustfmt.toml              # max_width 100, edition 2021
```

---

## Crate map

All crates live under `crates/`. Dependency flow is strictly inward toward
`harness-types`; no cycles exist.

```
harness-types (foundation — language-neutral contracts, pure data)
    ↑
    ├─ world-kernel      pure governance kernel, no I/O
    ├─ compiler          manifest → CompiledWorld
    ├─ executor          execution boundary (FS / subprocess / MCP / web)
    ├─ provider-adapters normalize provider wire formats → neutral ToolCall
    ├─ trace-store       append-only audit, redaction, replay, drift
    │       └─ depends on world-kernel, compiler
    ├─ agent-core        model loop orchestration (depends on all above)
    ├─ harness-preview   pure preview: manifest → surface + decision matrix
    │       └─ shared by both harness-wasm and cli-harness serve
    ├─ harness-wasm      cdylib/rlib compiled to WebAssembly (wasm-bindgen)
    └─ cli-harness       binary `harness` — REPL, serve, gate subcommands
```

| Crate | Primary public API |
|---|---|
| **harness-types** | `Taint`, `TaintedValue<T>`, `Perception`, `ToolCall`, `CompiledWorld`, `WorldManifest`, `Decision`, `ExecutionSpec`, `BuildError`, `ApprovalToken` |
| **world-kernel** | `IRBuilder::build`, `disposition::evaluate`, `decide`, `build_execution_spec` |
| **compiler** | `compile`, `compile_default`, `load_yaml`, `load_json`, `validate`, `hash_manifest` |
| **executor** | `Executor::builder()`, `ExecutorBuilder::register`, `Executor::run` |
| **provider-adapters** | `anthropic::parse_tool_use`, `parse_tool_definitions`, `format_tool_result` |
| **trace-store** | `TraceStore`, `record_decision`, `replay`, `drift_report`, `export_bundle`, `ApprovalStore` |
| **agent-core** | `run(SessionConfig)`, `tool_surface`, `ModelClient` trait, `ScriptedModel` |
| **harness-preview** | `gate(request) → GateResponse`, `preview(yaml) → PreviewResponse` |
| **harness-wasm** | `preview(yaml)`, `default_world()`, `version()` (wasm-bindgen exports) |
| **cli-harness** | `harness [--world] [--simulate] [--background]`, `harness serve`, `harness gate` |

**Test counts (all passing, native):**
harness-types 5 · world-kernel 41 · compiler 18 · executor 12 · trace-store 13 ·
provider-adapters 5 · agent-core 16 · harness-preview 42 · cli-harness 29 ·
harness-wasm 0 · **total 181** (plus the harness-wasm Node smoke tests, run via
wasm-pack)

---

## Architecture invariants (don't regress)

- The kernel (`world-kernel`) is pure: no I/O, no LLM, no mutable shared state.
- `IntentIR` is sealed — only `IRBuilder::build` can construct one.
- Taint is monotonic; `CompiledWorld` is immutable after construction.
- Only `ExecutionSpec` crosses the kernel→executor boundary.
- The `executor` never imports `world-kernel` — decisions and execution are separate.
- `harness-preview` and `harness-wasm` expose the same logic: no drift between
  native and WASM gate/preview functions.

The five conceptual layers (bottom to top): **substrate** → **capability** →
**knowledge** → **intent** → **action**. The harness sits at the intent/action
boundary, enforcing that only `ExecutionSpec` (a typed, sealed artifact) reaches
the executor.

---

## Current milestone state (as of 2026-06-28)

| Milestone | Epics | Status |
|---|---|---|
| **M1 — Deterministic Core** | E0–E4 | Complete ✅ |
| **M2 — Live Agent** | E5–E6 | Complete ✅ |
| **M3 — Full Tool Surface** | E7, E9 | Complete ✅ |
| **M4 — Isolation & Hardening** | E8, E10–E13 | In progress 🚧 |
| **M5 — Interactive Advocacy** | E14–E15 | E14 (WASM) done; E15 visualization planned |

**Immediate priority:** E16 — internal JIRA MCP demo on GitHub Copilot + Claude
Code via Safe MCP Proxy.

See `PLAN.md` for epic detail, acceptance invariants, and the dependency DAG.

---

## Conventions

- **Keep `README.md` current on every commit.** If a change affects project
  status, capabilities, crate layout, build/run instructions, or test counts,
  update the README's Status / layout / Build & test sections in the *same*
  commit. Keep `PLAN.md` epic checkboxes in sync too.
- **Don't commit the reference repos.** `repos/` holds `agent-hypervisor/`,
  `safe-mcp-proxy/`, and `mcp-tool-projection/` — separate git repositories kept
  only as references, not Cargo workspace members. Never `git add repos/`; stage
  harness paths explicitly.
- **Record architectural decisions in `DECISIONS.md`.** When a choice closes off
  a real alternative, append a `D<n>` entry (decision + alternatives + why) so it
  can be revisited later. Currently D1–D32.
- **No new workspace members without updating the crate map above** and
  `README.md`.
- **Default world lives in `crates/compiler/assets/default_world.yaml`.** It
  contains 8 base actions + 4 scoped capabilities. Changes to it affect
  `compile_default()` and the embedded WASM artifact.
- **WASM artifact** is committed to `blog/public/vendor/harness-wasm/` as a
  release build (480 KB optimised). Rebuild with `wasm-pack build --target web
  --release` inside `crates/harness-wasm/` after any change to `harness-preview`
  or `compiler`.

---

## Build & test

The local crate cache supports offline builds; prefer `--offline`.

```bash
cargo build --workspace --offline
cargo test  --workspace --offline
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --offline -- -D warnings
```

CI runs fmt-check, `clippy -D warnings`, build, and test on every push/PR
(`.github/workflows/ci.yml`).

**Demo binaries** (run via `cargo run --example <name> --offline`):
- `kernel_demo` — taint + disposition walkthrough
- `execution_demo` — executor boundary
- `trace_demo` — append/replay/drift
- `agent_loop` — full model loop with ScriptedModel
- `approvals_demo`, `tools_demo`, `poisoned_knowledge_demo`

---

## Key reference files

| File | What it contains |
|---|---|
| `README.md` | Project overview, milestone table, build/run instructions |
| `PLAN.md` | Epic definitions, acceptance invariants, dependency DAG, task source of truth |
| `DECISIONS.md` | ADR-lite log D1–D32+; consult before choosing alternatives |
| `docs/harness-architecture.md` | Canonical runtime design (5 sections) |
| `docs/THESIS.md` | Positioning: five layers, stochastic/deterministic border |
| `docs/GLOSSARY.md` | Normalised vocabulary — use these terms, not synonyms |
| `docs/harness-gate-abi.md` | Gate ABI schema (D24) |
| `docs/trust-pins.md` | Trust pins design (D29) |
| `docs/demos/jira-copilot/` | E16 JIRA MCP demo runbook |
| `.claude/cc-world.yaml` | Live `WorldManifest` governing Claude Code (dogfood), incl. D36 `command_classes` |
| `.claude/hooks/world-gate.sh` | PreToolUse bootstrap shim → `harness cc-hook` (the real kernel; D37) |
| `docs/one-kernel-many-hosts.md` | Cross-host parity design note (D36/D37) |
| `docs/demos/one-kernel/` | Canonical demo world + shared case set (conformance source) |
| `scripts/demo-one-kernel-many-hosts.sh` | Offline cross-host parity demo |

---

## Claude Code integration (dogfooding)

The `.claude/` directory dogfoods the harness against Claude Code itself (E13
slice):

- **`cc-world.yaml`** — the `WorldManifest` that governs this session (a real
  manifest compiled by the real compiler; Bash classification lives in its
  `command_classes` block, D36).
- **`hooks/world-gate.sh`** — the PreToolUse **bootstrap shim** (canonical
  wiring): locates the `harness` binary and `exec`s `harness cc-hook --world
  .claude/cc-world.yaml --state .claude/state`. Fail-open if no binary. No
  governance logic lives in the shim (D37).
- **`hooks/world-gate.py`** — the same shim in Python, kept **in place** for
  sessions whose hook config snapshotted the old path. NEVER move or delete it
  mid-session: a missing hook file bricks every subsequent tool call (D37).
- **`hooks/superseded/`** — the archived pre-cutover Python engine
  (world-gate.py original, `_gatelib.py`, `cc-world.json`, tests, demos) + README.
- **`hooks/taint-notify.py`** — SubagentStop hook for cross-agent taint
  observability (D21); degrades gracefully without `_gatelib`.
- **`agents/correcting-reviewer.md`** — Flywheel correcting-reviewer subagent
  (E13.1).
- **`commands/review-blog.md`** — `/review-blog` skill.

When editing the world manifest or the adapters, validate with
`cargo test -p cli-harness` (the `one_kernel.rs` conformance suite + adapter
contract tests) before committing.

---

## Per-assistant setup

These instructions are shared by reference, not by copy — keep the content here
and let each tool point at it:

- **Codex** and **Google Antigravity** read this `AGENTS.md` at the repo root
  natively; no extra file is needed. (Antigravity also supports workspace rules
  under `.agents/rules/` and global rules at `~/.gemini/GEMINI.md` if you want
  machine- or user-scoped additions.)
- **Claude Code** reads `CLAUDE.md`, which imports this file via `@AGENTS.md`.

When updating project conventions, edit **this file**; the per-assistant pointers
should not accumulate their own copies.
