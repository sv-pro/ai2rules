# Decision Log

Architectural decisions for the CLI Agent Harness, with the alternatives we
weighed and why we chose what we did. ADR-lite: one entry per decision.

**Why this exists:** so a choice can be revisited later without re-deriving the
context — especially the alternatives we *didn't* take.

**How to use it:** append a new `D<n>` entry when you make a decision that closes
off a real alternative. Keep entries short; link to the epic in `PLAN.md`, the
commit, or the code. Status is `Accepted` unless later `Superseded by D<n>`.

> Entries D1–D11 were reconstructed from project artifacts (commits, `PLAN.md`,
> the plan files, code comments, `CLAUDE.md`) rather than a contemporaneous log,
> so dates are approximate. D12 onward are recorded as decisions are made.

| Field | Meaning |
|---|---|
| **Decision** | what we chose |
| **Alternatives** | the options we rejected |
| **Why** | the deciding rationale |

---

## D1 — `harness-types` as the foundation crate
- **Epic:** E0 · **Status:** Accepted
- **Decision:** Put the language-neutral contracts in a dedicated `harness-types`
  crate that every other crate depends on inward; keep `IntentIR` in
  `world-kernel`.
- **Alternatives:** Define the contracts inside `world-kernel`.
- **Why:** Lets `executor`, `trace-store`, and the adapters depend on the
  contracts **without** depending on the kernel, while Rust's privacy still
  *seals* `IntentIR` (only `IRBuilder::build` can mint one).

## D2 — Hard taint invariant is a code floor, not manifest-driven
- **Epic:** E2 · **Status:** Accepted
- **Decision:** Enforce the taint × side-effect floor in code (`invariants.rs`),
  run before manifest policy; the manifest's `transition_policies` layer
  *additional* taint policy on top in disposition.
- **Alternatives:** Drive the floor purely from manifest `taint_rules`.
- **Why:** A manifest must never be able to *weaken* the floor. The default
  world's rules coincide with it — harmless overlap; the floor holds even if a
  manifest omits them.

## D3 — Minimal, no-dependency schema validation
- **Epic:** E2 · **Status:** Accepted
- **Decision:** Hand-rolled argument validation (required keys, declared-property
  types, `enum`/`const`) in `world-kernel/schema.rs`.
- **Alternatives:** Pull in a JSON Schema crate.
- **Why:** Keeps the lean offline dependency set; the default world carries no
  schemas yet. Full Draft validation deferred as later hardening.

## D4 — Kernel-side `ExecutionSpec` assembly
- **Epic:** E3 · **Status:** Accepted
- **Decision:** `world-kernel::build_execution_spec` mints the spec from a sealed
  `IntentIR`; `KernelOutcome::Evaluated` carries the intent so an `ALLOW` can be
  lowered. Runtime config arrives via `ExecEnv` (kernel stays pure).
- **Alternatives:** Build the spec in a separate orchestrator step.
- **Why:** The kernel is the sole producer of the only object that crosses the
  boundary (architecture §6); the `executor` keeps **no** dependency on the
  kernel and evaluates no policy.

## D5 — Pragmatic-real execution handlers
- **Epic:** E3 · **Status:** Accepted
- **Decision:** `read` real (readable-root checked); `apply_patch` as a structured
  full-file write (writable-root enforced); `run_command` real via std subprocess
  with a thread-drained deadline + direct-child kill; `SIMULATE` for all.
- **Alternatives:** Simulation-first (EXECUTE stubbed); full-real (unified-diff
  apply + process-group kill-tree now).
- **Why:** Offline-buildable (no diff crate available); process-group kill-tree
  and OS isolation are E8's job, not E3's.

## D6 — Full E4 scope; defer the Rego parity mirror
- **Epic:** E4 · **Status:** Accepted
- **Decision:** Ship record + redaction + replay + drift report + bundle
  (E4.1–E4.5); defer the cross-implementation Rego mirror (E4.6).
- **Alternatives:** Core only (E4.1–E4.3).
- **Why:** Replay + drift + bundle are what make M1's "deterministic core"
  demonstrable; a second-language parity harness adds little before there's a
  benchmark suite.

## D7 — Minimal `*`-glob redaction, no dependency
- **Epic:** E4 · **Status:** Accepted
- **Decision:** Redact JSON values whose key/dotted-path matches a manifest
  pattern via a small `*`-wildcard matcher.
- **Alternatives:** Add a glob crate for full `**`/path semantics.
- **Why:** Lean deps; masking keeps keys present and values string-typed so it
  doesn't change representability. Full glob deferred.

## D8 — Consumer crates depend inward; dev-deps break would-be cycles
- **Epic:** cross-cutting · **Status:** Accepted
- **Decision:** Replay/spec/approvals live where their inputs are: `trace-store`
  depends on `world-kernel` + `compiler`; `world-kernel` uses `compiler`/
  `executor`/`tempfile` as **dev-deps** for tests/demos only.
- **Alternatives:** Keep `trace-store` storage-only with replay elsewhere; avoid
  any cross-crate test deps.
- **Why:** The kernel depends on neither `trace-store` nor `executor`, so there's
  no cycle; the dependency graph still flows inward to `harness-types`.

## D9 — Offline `ModelClient` trait; defer a live HTTP client
- **Epic:** E5 · **Status:** Accepted
- **Decision:** `agent-core` defines a `ModelClient` trait + a deterministic
  `ScriptedModel`; the Anthropic piece is pure format translation. No network,
  no async, no API key.
- **Alternatives:** Add a real Anthropic HTTP client (reqwest + tokio) now.
- **Why:** Keeps CI offline and the loop deterministic, matching the kernel's
  posture. A live client is a later, feature-gated add.

## D10 — Anthropic-only adapter for now
- **Epic:** E5 · **Status:** Accepted
- **Decision:** Build only the Anthropic adapter (E5.1–E5.5); defer OpenAI/Gemini
  (E5.6).
- **Alternatives:** Build all three adapters now.
- **Why:** One adapter proves the single gate end-to-end; the others share the
  neutral `ToolCall` contract, so adding them later is mechanical.

## D11 — Model proposals carry Trusted provenance; taint is the containment
- **Epic:** E5 · **Status:** Accepted
- **Decision:** The orchestrator proposes with the developer's (Trusted)
  authority; containment of tainted-data-driven actions comes from the **taint**
  carried in `EvalContext`, not from lowering the proposal's trust.
- **Alternatives:** Give model proposals a low trust level.
- **Why:** Low trust would make every non-read action `ABSENT` by capability,
  defeating the loop; taint × side-effect is the correct containment mechanism.

## D12 — ApprovalStore lives in `trace-store`
- **Epic:** E6 · **Status:** Accepted
- **Decision:** The durable approval store is a module in `trace-store`
  (append-only JSONL transitions, folded on load), reusing its serde/JSONL/io
  patterns and `compiler::sha256_hex` for the params-binding hash.
- **Alternatives:** A new `approval-store` crate; or in `agent-core`.
- **Why:** `trace-store` is already the durable-persistence home and carries the
  needed deps; a new crate would re-establish the same dependencies for one
  module. (Trade-off: approvals are operational state, not audit — colocated for
  pragmatism, separable later if it grows.)

## D13 — E6 wires approvals through the full loop
- **Epic:** E6 · **Status:** Accepted
- **Decision:** Beyond the kernel branch + store, wire approvals into the
  orchestrator: an `ApprovalPolicy` (`Manual`/`AutoApprove`/`AutoReject`) + an
  `ExecutionMode` on the session, with a demo showing `ASK → approve → resume →
  ALLOW` and `BACKGROUND → DENY`.
- **Alternatives:** Kernel + store only, deferring loop wiring/demo.
- **Why:** End-to-end wiring is what actually demonstrates invariants 9 and 10,
  and completes Milestone 2.

## D14 — MCP/web via offline mock transports
- **Epic:** E7 · **Status:** Accepted
- **Decision:** MCP and web go through pluggable `McpTransport` / `WebFetcher`
  traits with deterministic mock impls; MCP dispatch and web fetch flow through
  the same IntentIR/descriptor/provenance gate and the executor's drift check,
  with no network or async.
- **Alternatives:** Real stdio/HTTP MCP transport + real web client (reqwest) now.
- **Why:** Keeps CI offline and deterministic, matching the kernel and the E5
  model client (D9). Real transports are a later, feature-gated add.

## D15 — Full E7 in one pass
- **Epic:** E7 · **Status:** Accepted
- **Decision:** Ship scoped-capability machinery (E7.4/E7.5, invariant 12) + MCP
  dispatch (E7.1) + MCP descriptor drift (E7.2) + web channel (E7.3) together,
  via the mock transports; plus `git_status`/`git_diff` and `call_known_mcp_tool`.
- **Alternatives:** Scoped caps + drift only, deferring live MCP/web handlers.
- **Why:** With mock transports the whole epic is deterministic and offline, so
  there's no reason to split; satisfies invariants 7, 11, 12 in one move.

## D16 — Scoped-cap spec keys on the scoped action name
- **Epic:** E7 · **Status:** Accepted
- **Decision:** `build_execution_spec` keeps the spec's `action` = the scoped
  capability's name (e.g. `run_tests`) and carries the scoped cap's descriptor
  hash; the executor registers each scoped cap under its own name mapped to the
  base action's handler kind.
- **Alternatives:** Rewrite the spec's action to the base action (`run_command`).
- **Why:** The descriptor hash that drift-checks (invariant 11) is the scoped
  cap's; keying on the scoped name keeps the spec, the registered hash, and the
  audit trail consistent — rewriting to the base would mismatch the hash.

## D17 — World Authoring Tool architecture
- **Epic:** E11 · **Status:** Accepted
- **Decision:** Adopting the 3-column UI pattern of `mcp-tool-projection` (visualizing live tools + scoped caps vs. manifest YAML vs. effective tool surface & decisions). The implementation uses a dual stack: a TypeScript React/Vite SPA hosted locally from a thin Rust HTTP API (integrated directly into the harness CLI, e.g. via `cli-harness serve`).
- **Alternatives:**
  1. Build a pure Rust Terminal User Interface (TUI).
  2. Implement the manifest evaluation/projection rules in TypeScript/Node for the UI backend to keep the tool standalone.
- **Why:** A browser-based UI is far more expressive and faster to develop for complex JSON/YAML hierarchies and side-by-side comparative views than a Rust TUI. However, rebuilding the complex governance kernel logic (taint propagation, budget checking, descriptor hashing, ontology resolving, scoped cap argument stripping) in TypeScript would lead to double maintenance and inevitable drift. A thin Rust HTTP endpoint wraps the actual production compiler/kernel, ensuring 100% fidelity.

