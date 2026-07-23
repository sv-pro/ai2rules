# CLI Agent Harness Architecture (Claude synthesis)

This note studies the two sibling projects in this repo — `agent-hypervisor`
and `safe-mcp-proxy` — and distills their best principles into one coherent
harness architecture for a **local CLI developer agent**: the class of tool that
runs on a developer's own machine and edits files, runs shell commands, applies
patches, calls MCP servers, and fetches the web. Concretely, this is the
architecture you would build *underneath* a tool like **Claude Code**,
**Codex CLI**, or **Google Antigravity** — the governance kernel that sits
between the model loop and the developer's real filesystem, shell, and network.

Why these agents need it specifically: a local CLI agent has, by default,
the developer's full ambient authority — their credentials, SSH keys, git
remotes, package managers, and write access to the working tree. A single
prompt injection arriving through tainted context (a `README`, an issue body, a
web page, a poisoned MCP tool result) can turn that authority against the
developer. The harness's job is to make the dangerous actions *not exist* in the
agent's world rather than to detect them after the fact.

It is a peer to [`harness-architecture-codex.md`](harness-architecture-codex.md)
(a Rust ground-up rebuild) and
[`harness-architecture-antigravity.md`](harness-architecture-antigravity.md) (a
security-paradigm framing). This document's angle is **harness integration**: how
the governance kernel threads through a real model loop, and which load-bearing
invariants must survive that integration.

---

## 1. What the two projects actually are

The synthesis only makes sense if the source material is read accurately, so
this section is grounded in the real code, not the marketing.

### `agent-hypervisor` — the research kernel (Python)

A deterministic execution-governance layer organized into four layers:

| Layer | Name | Code home | Role |
|---|---|---|---|
| 0 | Execution Physics | (planned) | container / network isolation |
| 1 | Base Ontology | `compiler/` | World Manifest schema + compiler |
| 2 | Dynamic Ontology | `authoring/` | capability DSL + policy presets |
| 3 | Execution Governance | `runtime/` | IR, taint, provenance, approvals |

The parts worth stealing live in `runtime/`:

- **`IntentIR` is sealed** (`runtime/ir.py`). It cannot be constructed outside
  `IRBuilder.build()` — `__new__` checks a module-private `_IR_SEAL`. *The
  existence of an `IntentIR` object is itself proof that every constraint
  passed.* All six constraints (ontology, capability, approval, taint, taint
  rule, budget) are checked at **construction time**, before any execution path
  is entered. The executor never re-checks.
- **Taint is monotonic** (`runtime/taint.py`). `TaintedValue[T]` is the
  mandatory return type of every execution; `TaintContext` is a mandatory,
  non-variadic argument to `build()` so callers *cannot silently drop taint*.
  Join is `CLEAN ∨ TAINTED = TAINTED`, never decreasing.
- **Process boundary** (`runtime/worker.py`). Handlers run in a subprocess;
  policy evaluation runs in the main process. Neither sees the other's state.
- **Invariants are physics laws** (GLOSSARY). `TaintContainmentLaw`,
  `ProvenanceLaw`, `CapabilityBoundaryLaw` are checked *before* manifest lookup
  and cannot be overridden by a manifest rule or a user approval.
- **No LLM on the execution path.** The stochastic model is used at design time
  (authoring manifests); runtime is deterministic table lookups (~0.5 ms/call on
  the AgentDojo run, 0% ASR / 80% utility).

### `safe-mcp-proxy` — the productized distillation (Python)

A narrow MCP control plane that distills hypervisor Layers 1–3 into a shippable
proxy. Its contribution is sharper *outcome semantics* and *integration shape*:

- **Three blocking outcomes, not one.** `ABSENT` (the tool is hidden from this
  world — never offered), `DENY` (a visible tool blocked by policy), `ASK` (a
  visible tool needs human approval). README's thesis: *"Some actions are
  denied. Others do not exist."*
- **Six ordered, deterministic policy paths** (`policy_engine.py`), first match
  wins: `tool_not_allowlisted` → `capability_not_allowed` → `descriptor_drift`
  → `tainted_external_side_effect` → `approval_required` → `default_allow`.
- **`ExecutionMode`** (`execution_mode.py`): `INTERACTIVE` can mint approval
  tokens; `BACKGROUND` collapses `ASK` → `DENY` (autonomous agents must not
  block on a human).
- **Descriptor drift** (`descriptor.py`): SHA256 of the JSON-normalized schema,
  hashed at registration, re-checked before execution.
- **Scoped capabilities** (`capability_dsl.py`): `LiteralSource` /
  `ActorInputSource` / `ContextRefSource`. A literal arg is invisible to the
  actor and injected at execution — e.g. `send_me_email` wraps `send_email` with
  `to` locked to the owner, so a tainted payload can't redirect the recipient.
- **Provider adapter → neutral IR → one gate** (Gemini integration): a
  stateless `GeminiAdapter.parse()` produces a `ToolCall`, `IntentMapper.map()`
  produces a frozen `IntentIR`, `GeminiPolicyGate.evaluate()` returns a typed
  `ExecutionSpec`. Crucially it draws a **two-level absence** distinction:
  `IntentIRError` (action unknown to the *entire system* / ontology) is
  different from `ABSENT` (action known but hidden by *this world*).
- **Append-only audit + deterministic `replay()`**, plus a pluggable policy
  engine (Python *or* OPA/Rego, with parity tests) — policy as data.

---

## 2. Synthesis thesis

Three ideas survive the merge and become the spine of the harness.

1. **Kernel, not wrapper.** A wrapper inspects calls *after* the agent already
   believes the raw world exists. A kernel defines what the agent can *perceive
   and propose* in the first place. This is the irreducible lesson of
   `agent-hypervisor`.

2. **Validity is a construction-time property, proven by a type.** Borrow the
   sealed-`IntentIR` discipline: the only object that can cross into execution is
   one that could not have been built unless every constraint held. The executor
   carries no policy logic — it physically *cannot* receive an invalid intent.
   This collapses an entire class of "check was skipped / check raced the call"
   bugs.

3. **Absence has two levels, and both beat denial.** From `safe-mcp-proxy`:
   - *Ontological absence* — the action does not exist in the compiled world at
     all (the codex/Gemini `IntentIRError`).
   - *Projected absence* — the action exists in the full ontology but is not
     projected into the current world/context (`ABSENT`).
   - *Denial* — the action is visible and was blocked (`DENY`).
   An agent cannot be prompt-injected into calling a tool it cannot perceive.
   Surface absence to the model as *"unavailable in this world,"* not as a
   permission error, so the planner routes around it instead of retrying.

---

## 3. Best principles, extracted and attributed

| Principle | Primary source | How it lands in the harness |
|---|---|---|
| Closed action ontology | hypervisor | Unknown action names raise before any execution path; absent ≠ denied |
| Sealed typed intent | hypervisor (`IntentIR`) | One unforgeable object crosses into execution; existence = proof of validity |
| Construction-time enforcement | hypervisor (`IRBuilder.build`) | All constraints checked once, at build; executor never re-checks |
| Monotonic taint + provenance chain | hypervisor (`taint.py`) | Every perception/output carries origin + taint; join only increases |
| Invariants before policy | hypervisor (physics laws) | Taint-containment / provenance / capability laws can't be overridden by manifest or approval |
| Design-time LLM, runtime determinism (AI Aikido) | hypervisor | Manifests authored with a model; runtime is table lookups, no LLM on path |
| ABSENT vs DENY vs ASK | safe-mcp-proxy | Three distinct outcomes with distinct model behavior + audit meaning |
| Two-level absence | safe-mcp-proxy (Gemini) | `unknown-to-ontology` ≠ `hidden-by-world` |
| Ordered deterministic decision paths | safe-mcp-proxy | Fixed rule order, first match wins, fully replayable |
| Effect mode ⟂ decision | both | `SIMULATE`/`PROXY`/`TRUNCATE` are *how* an allowed action runs, not a verdict |
| Execution mode (interactive/background) | safe-mcp-proxy | `ASK` → `DENY` when no human is present |
| Descriptor drift detection | safe-mcp-proxy | Hash tool schemas at registration; block on mutation |
| Scoped capabilities (literal injection) | safe-mcp-proxy | Expose `run_tests`, not raw `run_command`; lock args after stripping actor input |
| Provider adapter → neutral IR → one gate | safe-mcp-proxy | Each model's tool-call dialect normalizes into the same `IntentIR` |
| Approval as durable token | safe-mcp-proxy (`approval_store`) | Approval is a record consumed by execution, not a prompt convention |
| Append-only audit + replay | both | Every decision reproducible offline for forensics and drift tests |
| Pluggable policy engine (policy-as-data) | safe-mcp-proxy (OPA parity) | Same decisions whether evaluated in code or Rego |

---

## 4. Layered architecture

```
┌──────────────────────────────────────────────────────────────┐
│ CLI Shell / TUI                                              │
│ prompt input, streaming output, approval prompts, status      │
└───────────────────────────────┬──────────────────────────────┘
                                │ user text (clean channel)
┌───────────────────────────────▼──────────────────────────────┐
│ Agent Orchestrator                                          │
│ model loop, context packing, tool-call parsing (per provider) │
└───────────────────────────────┬──────────────────────────────┘
        projected tool surface  │  proposed tool calls
┌───────────────────────────────▼──────────────────────────────┐
│ Provider Adapters                                           │
│ Anthropic / OpenAI / Gemini / MCP tool-call → neutral form    │
└───────────────────────────────┬──────────────────────────────┘
                                │ ToolCall (neutral)
┌───────────────────────────────▼──────────────────────────────┐
│ World Kernel                                                │
│ ontology → IRBuilder (sealed IntentIR) → invariants →        │
│ ordered policy → Decision + EffectMode; taint; budgets; trace │
└───────────────────────────────┬──────────────────────────────┘
                                │ ExecutionSpec (only valid objects)
┌───────────────────────────────▼──────────────────────────────┐
│ Execution Boundary (subprocess)                             │
│ PTY, subprocess, fs ops, patch apply, MCP dispatch, web fetch │
└───────────────────────────────┬──────────────────────────────┘
                                │ OS / container / sandbox primitives
┌───────────────────────────────▼──────────────────────────────┐
│ Host Reality                                               │
│ files, shell, network, credentials, external services         │
└──────────────────────────────────────────────────────────────┘
```

Two design rules carried from both repos:

- The **kernel is usable without the CLI**, and the **CLI can run against a
  simulation executor** for tests (hypervisor's Core/Demo split; safe-mcp's
  `simulate_external`).
- **Raw reality never reaches the model.** Everything entering context is first
  turned into a typed `Perception` (hypervisor's "Semantic Event") with
  provenance and taint attached.

---

## 5. The request pipeline

Every turn flows through one fixed pipeline. Stages are deterministic; the only
non-deterministic component (the model) sits *above* the kernel and only ever
*proposes*.

```
Perception(s)  ── typed, provenance- and taint-tagged, never raw text
   ↓ packed into context
Model proposes tool call(s)
   ↓
Provider Adapter → ToolCall (neutral)
   ↓
IntentMapper → IntentIR or  IntentIRError (ontological absence)
   ↓  (sealed object: built only if it could pass)
Kernel evaluation:
   1. invariants (physics) ........... violation → DENY (hard, non-overridable)
   2. projection ..................... not in this world → ABSENT
   3. capability ..................... trust lacks capability → ABSENT
   4. descriptor drift ............... schema hash changed → DENY
   5. taint × side-effect ............ tainted → external/persistent → DENY
   6. approval ....................... requires_approval → ASK
   7. budget ......................... over limit → REPLAN
   8. default ........................ → ALLOW + EffectMode
   ↓
ExecutionSpec (the only thing that crosses into execution)
   ↓
Executor (subprocess) applies EffectMode: EXECUTE | SIMULATE | PROXY |
                                          SANITIZE | TRUNCATE | DEFER
   ↓
TaintedValue result  +  append-only audit record
```

Two merges happened here:

- **Invariants run before the ordered policy paths.** hypervisor enforces
  physics laws ahead of manifest lookup; safe-mcp-proxy contributes the precise
  *ordering and naming* of the remaining paths. The harness uses both: hard laws
  first, then the named, replayable rule list.
- **`REPLAN` is added as a first-class outcome** for budgets/altitude. Unlike
  `DENY`, it tells the planner *"find a cheaper/safer path,"* which a coding
  agent can act on (smaller diff, fewer files, narrower command).

---

## 6. Core contracts

These mirror the real types in both repos, renamed for a coding harness.

### `Perception` (a.k.a. Semantic Event)

The typed form of everything entering the model: user message, file content,
command stdout/stderr, MCP response, web body, recalled memory. Carries
`source`, `trust_level`, `taint`, `content_hash`, `provenance_chain`,
`redaction`. **Raw bytes are never appended to context directly.**

### `IntentIR` (sealed)

The only representation of execution intent inside the kernel — no NL, no raw
dicts, no stringly-typed action names past this point. Built solely by
`IRBuilder.build()`; constructing it directly is a type error. Fields:
`action` (descriptor, *metadata only — no handler callable*), `source`,
`params`, `taint`. Two failure modes at build:

- `IntentMapperError` — action is unknown to the *whole ontology* (level-1
  absence).
- `ConstructionError` subclasses — `NonExistentAction` (not in *this world*),
  `ConstraintViolation`, `TaintViolation`, `BudgetExceeded`, etc.

### `Decision`

`ABSENT` | `ALLOW` | `DENY` | `ASK` | `REPLAN`. `ABSENT`/`DENY`/`ASK`/`REPLAN`
are returned to the model as **structured feedback**, not free prose. `ABSENT`
is phrased as unavailability, not refusal.

### `EffectMode` (orthogonal to `Decision`)

`EXECUTE` | `SIMULATE` | `PROXY` | `SANITIZE` | `TRUNCATE` | `DEFER`. The pair is
`Decision::Allow + EffectMode::Simulate` — *never* a `Decision::Simulate`. This
fixes a real anti-pattern both repos warn about: conflating "is it permitted"
with "how does it touch reality."

### `ExecutionSpec`

The only object that crosses into the execution boundary. Contains *only* what
the executor needs (argv, cwd, env policy, timeout, descriptor identity) — no
policy objects, no model context, no provenance internals.

### `CompiledWorld`

Immutable, hash-addressed runtime artifact produced from the authoring manifest:
closed action set, schema validators, capability matrix, taint transition table,
approval rules, budget rules, projected descriptors + their hashes, redaction
rules. Loaded once at session start. **Hot reload mints a new version; it never
mutates the current one** (hypervisor's frozen `CompiledPolicy`).

### `Provenance` / `Taint`

`Provenance { source_channel, trusted, parent_sources, session_id }`; `derive()`
propagates taint through chains and *across sessions* (the ZombieAgent property —
tainted memory written in session N must remain tainted in session N+1). Taint
is a property of the channel and lineage, not of content the agent can relabel.

### `DescriptorHash`

Deterministic hash over each exposed descriptor: model-facing schema, arg
constraints, side-effect class, handler/server identity, policy-relevant
metadata. Mismatch at call time → `DENY (descriptor_drift)` before execution.

### `ApprovalToken`

A durable record (`pending → approved/rejected → executed`). `ASK` mints it in
`INTERACTIVE` mode; execution later *consumes* it. In `BACKGROUND` mode `ASK`
collapses to `DENY`. Approval is state, not a console convention — and for
anything beyond a demo it must be persisted, not in-memory only.

---

## 7. Provider & tool adapters

A CLI harness must serve several model dialects and tool transports. The
safe-mcp-proxy Gemini stack is the template: **normalize early, gate once.**

```
Anthropic tool_use ┐
OpenAI function    ├─→ Provider Adapter → ToolCall ─→ IntentMapper ─→ IntentIR ─→ ONE kernel gate
Gemini functionCall│
MCP tools/call     ┘
```

- Adapters are **stateless parsers** — dialect in, neutral `ToolCall` out. No
  policy lives here.
- `IntentMapper` consults the **full ontology** (so it can distinguish "unknown
  to the system" from "not in this world").
- Exactly **one policy gate** runs regardless of provider — no per-provider
  enforcement to drift apart.
- MCP tools are subject to the same `IntentIR`, taint, descriptor-hash, and
  capability path as local tools. An MCP server is just another (untrusted by
  default) channel and descriptor source.

---

## 8. Design-time vs runtime (AI Aikido)

Keep the stochastic model off the critical path by moving its work earlier:

- **Design time (stochastic OK):** a model assistant reads the repo, infers
  test/build commands, writable roots, and trusted MCP servers, and *drafts* a
  world manifest. A human reviews it. The compiler turns it into a
  `CompiledWorld` and pins descriptor hashes.
- **Runtime (deterministic only):** lookups, schema validation, shell-arg
  parsing, taint joins, hash checks. Sub-millisecond, reproducible, no API call
  on the hot path.

`ASK` is the learning signal that closes the loop: an uncovered-but-approved
action is a candidate **manifest extension** (a permanent world change requiring
review), distinct from a one-shot approval. This is hypervisor's Design-Time HITL
— O(log n) governance instead of O(n) per-call prompts.

---

## 9. CLI-specific world model

A first useful manifest for a coding agent is small. Base actions:

- `read_workspace`, `write_workspace`, `apply_patch`
- `run_command` (no network), `run_network_command` (approval-gated),
  `start_pty` (gated by command class)
- `call_mcp_tool` (trust per server profile)
- `update_memory` (denied for tainted external content — `ProvenanceLaw`)
- `fetch_web` (untrusted, always tainted output)

Scoped capabilities over those base actions (literal-arg injection):

- `run_tests` → `run_command` with fixed argv prefix, network disabled
- `apply_workspace_patch` → `apply_patch` full-file write (`path`, `contents`) limited to writable roots
- `read_repo_file` → `read_file` limited to workspace roots
- `git_commit` → `run_command` with `git commit` verb only
- `call_known_mcp_tool` → `call_mcp_tool` pinned to a server + descriptor hash

Default rules (the deterministic gate, in evaluation order from §5):

| Condition | Outcome |
|---|---|
| Action name unknown to ontology | `IntentMapperError` (level-1 absent) |
| Known action, not projected into world/context | `ABSENT` |
| Trust level lacks capability | `ABSENT` |
| Descriptor hash drift | `DENY` |
| Tainted data → network / credential / durable memory / external write | `DENY` |
| Destructive fs/process op | `ASK` |
| Path outside writable roots (for a write) | `DENY` |
| Path outside any known root | `ASK` (interactive) / `DENY` (background) |
| Over budget (commands, wall-time, tokens, cost) | `REPLAN` |
| `ASK` while in `BACKGROUND` | `DENY` |
| Everything passes | `ALLOW` + `EffectMode` |

Channel trust defaults: user input *trusted* (still traced); workspace files
*workspace-trusted, tainted if outside trusted roots or model-generated*; shell
output *semi-trusted, tainted by default*; MCP output *tainted unless server
profile says otherwise*; web *untrusted, always tainted*.

---

## 10. Audit & deterministic replay

Every perception, proposed intent, decision, approval, and result is appended to
an `audit.jsonl` with enough fields to re-derive the decision offline:

```jsonl
{"trace_id":"tx_8f9c1b","stage":"decision","tool":"apply_workspace_patch","source_channel":"workspace_files","taint":true,"descriptor_hash":"3c98de1a...","decision":"ASK","rule":"confirm_tainted_write","effect":"execute","mode":"interactive","ts":"2026-06-17T00:48:12Z"}
```

- `replay(record)` re-runs the recorded inputs against the same `CompiledWorld`
  and must produce the identical decision (both repos ship this).
- Redaction rules from the manifest run *before* write, so secrets never land in
  the log in plaintext.
- The same record set drives drift tests: re-evaluating an old bundle against a
  new world surfaces policy changes intentionally (safe-mcp `bundle_replay`).

---

## 11. What NOT to copy

Both repos are explicit about their own rough edges; the harness should not
inherit them.

- Don't mix ontology, policy, gateway, demo UI, and program execution in one
  layer (hypervisor's `hypervisor/` PoC is "not production hardened").
- Don't allow a runtime fallback from "unknown plan type" to direct execution.
- Don't use stringly-typed trust/provenance across boundaries — types only.
- Don't encode `SIMULATE` as a `Decision`; it is an `EffectMode`.
- Don't keep approval state in memory only beyond demos (safe-mcp's
  `ApprovalStore` is explicitly in-memory).
- Don't treat boolean taint as the whole model — it's a fine MCP MVP, but a
  coding harness needs the full provenance chain (sessions, parents, roots).
- Don't put an LLM, mutable shared state, or I/O (beyond subprocess dispatch) in
  the runtime/kernel layer (hypervisor `.claude/rules/runtime-security.md`).
- Don't add probabilistic "does this look safe?" checks to the gate — pattern
  blocklists are not a primary defense.
- Don't let handler code touch policy state — keep the process boundary hard.
- Don't carry the Python import-path quirks (`pythonpath=src/agent_hypervisor`,
  `src/main/python`) into a clean build.

---

## 12. Acceptance invariants

The harness is not done until these pass *deterministically*:

1. An unknown action cannot form an `IntentIR` (sealed-construction proof).
2. A known-but-unprojected action returns `ABSENT`, not `DENY`.
3. A wholly-unknown action returns ontological absence, distinct from `ABSENT`.
4. The model cannot invoke an executor directly — only via `ExecutionSpec`.
5. Tainted file/web/MCP content cannot drive network egress, durable memory
   writes, credential access, or external side effects.
6. Taint never decreases across a transformation or a session boundary.
7. Workspace writes outside writable roots are denied.
8. Destructive commands require approval; `ASK` in background mode denies.
9. Descriptor drift blocks before the handler runs.
10. Scoped capabilities strip actor-supplied locked args before injecting
    literals (a tainted `to` cannot redirect `send_me_email`).
11. `Decision::Allow + EffectMode::Simulate` produces no real side effect.
12. Same manifest + same trace replay → identical decisions (Python and, if
    used, OPA/Rego agree).
13. Redaction keeps secrets out of the audit log in plaintext.
14. The executor refuses any action absent from its local closed registry.

---

## 13. Implementation shape & MVP sequence

Language is a secondary choice — both sources are Python; the codex peer doc
argues for Rust. The non-negotiable is the **module boundary**, not the
language: a pure, deterministic kernel; a separate executor across a process
boundary; thin adapters at the edges.

```
kernel/        compiled world, IRBuilder, decisions, taint, budgets   (no I/O, no LLM)
adapters/      provider tool-call normalization → neutral ToolCall
executor/      subprocess: PTY, fs, patch apply, MCP dispatch, web
trace/         append-only audit, redaction, replay, bundle drift
compiler/      manifest → CompiledWorld (+ descriptor hashing)
cli/           TUI, model loop, context packing, approval prompts
```

MVP order:

1. Define the types: `Perception`, `IntentIR` (sealed), `Decision`,
   `EffectMode`, `ExecutionSpec`, `Provenance`, `Taint`, `CompiledWorld`.
2. Minimal manifest compiler + a hardcoded default CLI world.
3. `kernel.evaluate(intent, context)` with invariants-first, then ordered paths.
4. Executor adapters for read file, apply patch, run command (with a simulate
   mode for tests).
5. Append-only trace + `replay` tests.
6. Wire the model loop through the **projected** tool surface only.
7. Approval flow with a durable `ApprovalToken`.
8. MCP tools behind the same intent/taint/descriptor path.
9. Descriptor hashing + drift tests.
10. `EffectMode::Simulate` + scenario tests; then optional OPA/Rego parity.

---

## 14. Conclusion

`agent-hypervisor` proves that safety can be a *structural* property — an action
that cannot be represented cannot be executed, and the existence of a typed
intent is itself a proof of validity. `safe-mcp-proxy` proves that the same
discipline ships in a narrow, fast, deterministic control plane with clean
outcome semantics (`ABSENT` ≠ `DENY` ≠ `ASK`) and pragmatic integration shape
(provider adapters, audit, replay, policy-as-data).

The harness that combines them is a **kernel, not a wrapper**: the model
perceives a virtualized world, proposes typed intents, and receives structured
decisions; only objects that *could not have been built invalid* ever reach the
execution boundary; and every decision is reproducible from an append-only
trace. That is bounded, measurable, auditable security for a CLI coding agent —
not perfect security, but security you can replay.
