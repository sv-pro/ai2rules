# Glossary

Normalized vocabulary for the unified project (see [`THESIS.md`](THESIS.md)).
These terms must mean the **same thing** across every fragment's docs and code —
the harness, `context-engine`, `cedar-world-playground`, the IntentOS crates, and
the source reference repos. When a fragment uses one of these words, it uses this
definition. Each entry tags its **layer** (Thesis · Action · Capability ·
Knowledge · Intent · Substrate) or its **source** repo.

---

## Thesis

- **Agentic Governance** *(Thesis)* — the discipline of governing what an
  autonomous agent may *perceive, represent, and execute*, by construction rather
  than by asking an LLM to police itself. The headline category of the project.
- **Stochastic–Deterministic Border** *(Thesis)* — the architectural principle:
  *design-time stochastic, runtime deterministic*. The LLM proposes
  (synthesis/creativity); a deterministic kernel disposes (table lookup, schema,
  taint, hash). **No LLM sits in the trust path.** Also called the *border*.
- **AI Aikido** *(Thesis)* — using the attacker's reliance on the LLM against
  them by removing the LLM from the security path. The rhetorical name for the
  border move (`blog/.../ai-aikido.md`; also `intent-memory-engine/MANIFESTO.md`).

## Shared primitives (the kit every layer reuses)

- **Closed action ontology** *(src: agent-hypervisor)* — the agent's world
  contains only the actions defined for it; everything else does not exist. The
  basis of `ABSENT`.
- **ABSENT** *(Capability/Action; src: safe-mcp-proxy)* — outcome for an action
  that is *not in the projected world at all*. Ontological absence, not refusal —
  distinct from `DENY` in behavior and audit meaning. **`ABSENT ≠ DENY`.**
- **DENY** *(Action)* — outcome for an action that *exists* in the world but is
  refused by a policy/invariant in the current context (e.g. the taint floor).
- **ASK** *(Action)* — outcome requiring human approval; fails **closed to `DENY`**
  in non-interactive (background) mode. Approvals are durable tokens bound to the
  exact call, voided by drift.
- **SIMULATE** *(Capability/Action)* — an action projected as present but whose
  effects are virtualized (no real side effect). Part of *effect virtualization*.
- **ALLOW** *(Action)* — the action is present, permitted, and lowered to an
  `ExecutionSpec` for real (or simulated) execution.
- **Capability projection** *(Capability; src: mcp-tool-projection)* — deriving
  the *per-actor* tool surface from policy + world, rather than exposing tools
  wholesale. Asks "what world exists for this actor?" not "can this actor do X?"
- **Taint** *(Action; src: agent-hypervisor)* — a monotonic trust marker on values
  and context. It can only **join and increase, never silently decrease**.
- **Taint floor** *(Action)* — the hard invariant (acceptance **invariant 7**) that
  tainted data may never reach an externally-effectful surface
  (`no_tainted_network`/`_external`/`_credential`/`_memory`/`_persistent_write`).
  The mechanism behind the cross-layer demo.
- **Provenance** *(Action+Knowledge)* — origin, trust, lineage, and content
  identity carried by every value (Action) or the `doc_id`+line-span on every fact
  or rule (Knowledge). The thing that *propagates* trust decisions.
- **Descriptor drift** *(Action; src: safe-mcp-proxy)* — a change between the tool
  descriptor hashed at projection and the one seen at runtime; detected and
  **blocked**. Also voids approval-token reuse.
- **Three decision stages** *(src: safe-mcp-proxy)* — ontology → policy → effect
  virtualization. `ABSENT` is stage 1; `DENY`/`ASK` are stage 2; `SIMULATE` is
  stage 3.
- **Trace / Replay** *(Action; src: agent-hypervisor)* — every decision is recorded
  to an append-only, secret-redacted log and is reproducible from recorded inputs
  + the compiled-world version (determinism); drift diffs verdicts against a
  changed manifest.

## Integration / topology (how the kernel attaches to the world)

These terms name the *deployment* relationships — agent, host, harness, adapter —
so they mean one thing across fragments and constellations. The internals they
wire to (**World Kernel**, **ExecutionSpec**, **Capability projection**) are
defined below under *Action layer*.

- **Model** *(Integration)* — the LLM that *proposes* text and tool calls. The
  stochastic side of the border; not an agent on its own.
- **Agent** *(Integration)* — a **Model + a control loop** (propose → act →
  observe, across turns). The agent is **what the harness governs** — the subject,
  not a part of the harness.
- **Host** *(Integration)* — the product/runtime that runs an agent and **owns the
  real tools and ambient authority** (filesystem, shell, git, credentials):
  Claude Code, Codex CLI, a Hermes-based agent, or the reference `harness` binary.
  Interception happens *at the host*, because that is where authority lives.
- **Harness** *(Integration)* — the **governance membrane** wrapping an agent: it
  virtualizes what the agent may **perceive** and gates what crosses into the
  host's tools. = **Kernel + compiler + execution boundary + trace + adapters**.
  It is **neither the agent nor the host**. It can *be its own host* (the reference
  binary) or *attach to* a third-party host (embedded via a host adapter).
  *(Distinct from MetaHarness's "generated harness" — a packaging artifact that
  sits above this whole stack, not a layer within it.)*
- **Adapter** *(Integration)* — a boundary translator that keeps the **Kernel**
  host- and provider-agnostic. Two kinds, on two boundaries:
  - **Provider adapter** — a model provider's wire format (e.g. Anthropic
    `tool_use`/`tool_result`) ⇄ the neutral `ToolCall`/`Perception`. Boundary:
    harness ↔ model. Today: `provider-adapters/anthropic.rs`.
  - **Host adapter** — a host's intercept event (Claude Code `PreToolUse`, an MCP
    request) ⇄ a neutral gate request, and the kernel's verdict ⇄ the host's
    decision shape. Boundary: harness ↔ host. Also owns per-session **taint** state
    plumbing (session id → sidecar); the taint *algebra* stays in the kernel.
- **MCP proxy** *(Integration; src: safe-mcp-proxy)* — a **host adapter that taps
  the MCP wire** instead of one product, governing *any* MCP-speaking host with a
  single adapter and no per-host code (covers MCP-routed tools only, not a host's
  native tools).
- **Embedding** *(Integration)* — the *form the Kernel ships in*, orthogonal to
  adapters: **library** (Rust link-in), **process ABI** (`harness gate` stdin/stdout
  JSON; `harness proxy`), or **WASM** (in-browser, E14). Adapter = *who calls it*;
  embedding = *how it is packaged*.
- **Reference harness** *(Integration)* — the `harness` binary: a harness that is
  also its own host, used as the **conformance oracle** — its native verdicts pin
  what every other embedding/adapter must reproduce (see E14.4).
- **Constellation** *(Integration)* — one concrete deployment topology: a chosen
  `(host, host adapter, embedding, world)`. The portability goal is *the same
  Kernel + world across many constellations*.

## Action layer (the harness)

- **World Kernel** *(Action)* — the pure decision core (`world-kernel`): no I/O, no
  LLM, no mutable shared state. `decide()` is a pure function of `(intent, context,
  compiled world)`.
- **WorldManifest → CompiledWorld** *(Action)* — a human-reviewed, LLM-draftable
  YAML/JSON manifest compiles **once** into an immutable, hash-addressed
  `CompiledWorld` (the runtime source of truth).
- **IntentIR (sealed intent)** *(Action)* — a typed intent that is *unforgeable
  outside the kernel*: only `IRBuilder::build` can construct one, so its existence
  is a proof that representability checks passed.
- **ExecutionSpec** *(Action)* — the only artifact that crosses the **execution
  boundary**. Produced from an `ALLOW`ed `IntentIR`; the executor runs nothing else.
- **Perception** *(Action)* — raw reality turned into typed, provenance-tagged
  input before the model sees it. Channels carry a trust level and a taint flag.
- **Scoped capability** *(Action/Capability)* — a base action narrowed by locking
  args to literals (e.g. `run_tests` always runs `pytest`; injected/unknown args
  are stripped — invariant 12).

## Knowledge layer

- **Neurosymbolic distillation** *(Knowledge; context-engine)* — using an LLM at
  ingestion to turn prose into typed primitives: **Facts** (S-P-O triples),
  **Rules** (IF-THEN + severity, *normative/executable*), **Capsules** (summaries,
  *non-normative* navigation only).
- **Gated promotion** *(Knowledge; context-engine)* — the policy that an
  *investigation* (descriptive reasoning trace) becomes a *normative* fact/rule
  only under human-approval / multi-source / repeated-evidence rules. The knowledge
  analog of the taint discipline: nothing untrusted silently becomes authoritative.
- **Status lifecycle** *(Knowledge; context-engine)* — facts/rules carry
  `status: active | deprecated | invalid` + versioning + validity window; agents
  prefer `active`, warn on `deprecated`, ignore `invalid`.
- **Personalized PageRank (PPR)** *(Knowledge; HippoRAG)* — graph search seeded at
  query nodes that does multi-hop retrieval in one step; HippoRAG's deterministic
  retrieval core over an LLM-extracted knowledge graph.
- **Recognition memory** *(Knowledge; HippoRAG 2)* — an online LLM pass that
  *filters* retrieved triples to the relevant subset before they seed PPR; the lean
  cousin of `context-engine`'s ReAct loop. See
  [`papers/hipporag2-vs-context-engine.md`](papers/hipporag2-vs-context-engine.md).

## Intent layer

- **Intent Triad** *(Intent; intentos-core)* — the unit of intent memory:
  `Task | Context | Outcome`, ingested into a traversable graph.
- **IntentGraph** *(Intent; intentos-core)* — the deterministic, embedding-indexed
  graph the runtime navigates *without* an LLM call per query (the "AI Aikido"
  core). `invariant_guard` extends the same stance to Rule/Finding checks over code.

## Substrate

- **Gateway** *(Substrate; llm-service-stack)* — the owned, local, cost-tracked
  LLM serving plane (LiteLLM/Ollama). Keeps the *stochastic* side self-hosted and
  observable so the deterministic side can be trusted end-to-end. Peripheral to the
  thesis.
