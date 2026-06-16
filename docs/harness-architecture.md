# CLI Agent Harness Architecture

This is the canonical architecture note for a local CLI developer-agent harness.
It consolidates the Codex, Antigravity, and Claude architecture drafts, and
distills the useful principles from `agent-hypervisor` and `safe-mcp-proxy`.

The target system is a harness underneath agents such as Codex CLI, Claude Code,
Gemini CLI, Aider, or future local coding agents. The agent may read and edit
files, run commands, apply patches, call MCP tools, fetch web content, and store
memory. The harness defines the world in which those actions can exist.

Core stance:

> The harness is a kernel, not a wrapper. It does not merely inspect dangerous
> calls after the model proposes them; it controls what the model can perceive,
> what actions it can represent, and what validated specs may cross into real
> execution.

## 1. Problem

Local CLI agents inherit ambient developer authority:

- workspace write access
- git remotes and package managers
- shell execution
- SSH keys, cloud credentials, tokens, and local config
- MCP servers and plugin tools
- persistent agent memory

At the same time, they ingest untrusted text from repository files, issues,
README files, dependency metadata, web pages, command output, and MCP results.
Prompt injection is therefore not just a prompt problem; it is an authority
boundary problem.

The harness reduces that authority by construction:

- raw reality becomes typed `Perception`
- model output becomes typed `ToolCall`
- `ToolCall` becomes sealed `IntentIR` only through the kernel
- `IntentIR` becomes `ExecutionSpec` only after deterministic evaluation
- only `ExecutionSpec` can cross the execution boundary

No LLM participates in runtime policy enforcement.

## 2. Source Ideas

### From `agent-hypervisor`

- **Closed action ontology**: absent actions are not forbidden; they do not
  exist in the agent's world.
- **Design-time stochastic, runtime deterministic**: LLMs may help draft world
  manifests, but runtime enforcement is table lookup, schema validation, hash
  verification, and typed state machines.
- **Sealed intent**: an execution intent should be unforgeable outside the
  kernel. Its existence is a validity witness.
- **Monotonic taint**: taint can join and increase; it cannot silently decrease.
- **Provenance on values**: every perception and tool result carries origin,
  trust, lineage, and content identity.
- **Process boundary**: handlers live behind execution isolation. The policy
  layer owns no direct handler callables.
- **Trace replay**: every decision must be reproducible from recorded inputs and
  the compiled world version.

### From `safe-mcp-proxy`

- **ABSENT is not DENY**: ontology outcomes and policy outcomes have different
  semantics, model behavior, and audit meaning.
- **Three decision stages**: ontology, policy, and effect virtualization.
- **Descriptor drift detection**: hash descriptors at projection/registration
  and block calls if runtime descriptors change.
- **Scoped capabilities**: expose constrained forms over broader tools with
  literal argument injection and actor-input stripping.
- **Execution mode**: `INTERACTIVE` can ask a human; `BACKGROUND` must fail
  closed when approval is needed.
- **Provider adapters**: normalize provider-specific tool-call formats into one
  neutral IR, then run one shared gate.
- **Policy-as-data option**: pure code and OPA/Rego can both evaluate the same
  compiled input, as long as parity is tested.

## 3. Two Compatible Views

The architecture has two useful views. They should not be collapsed into one.

### Virtualization Stack

```text
Layer 0: Execution Physics
  OS/container/sandbox boundaries, filesystem mounts, network egress, PTY limits

Layer 1: Base Ontology
  actions, schemas, descriptors, side-effect classes, scoped capabilities

Layer 2: Dynamic Projection
  current visible tool surface by actor, task, mode, trust, approvals, world

Layer 3: Execution Governance
  invariants, policy decisions, taint/provenance, approvals, budgets, trace
```

### Decision Pipeline

```text
Ontology:
  Does this action exist?
  -> UNKNOWN_TO_ONTOLOGY | ABSENT | PRESENT

Policy:
  Is this present action permitted in this context?
  -> ALLOW | DENY | ASK | REPLAN

Effect:
  In what reality does an allowed action run?
  -> EXECUTE | SIMULATE | PROXY | SANITIZE | TRUNCATE | DEFER
```

`SIMULATE` is not a policy decision. It is an effect mode paired with
`ALLOW`.

## 4. Runtime Shape

```text
┌──────────────────────────────────────────────────────────────┐
│ CLI / TUI                                                    │
│ user prompt, streaming output, approval UX, status            │
└───────────────────────────────┬──────────────────────────────┘
                                │
┌───────────────────────────────▼──────────────────────────────┐
│ Agent Orchestrator                                            │
│ model loop, context packing, planning, provider tool parsing   │
└───────────────────────────────┬──────────────────────────────┘
       projected tool surface   │ proposed provider-native call
┌───────────────────────────────▼──────────────────────────────┐
│ Provider Adapters                                             │
│ OpenAI / Anthropic / Gemini / MCP dialect -> neutral ToolCall  │
└───────────────────────────────┬──────────────────────────────┘
                                │
┌───────────────────────────────▼──────────────────────────────┐
│ World Kernel                                                  │
│ compiled world, IR builder, invariants, policy, effect choice, │
│ taint, provenance, approvals, budgets, trace                   │
└───────────────────────────────┬──────────────────────────────┘
                                │ ExecutionSpec only
┌───────────────────────────────▼──────────────────────────────┐
│ Execution Boundary                                            │
│ subprocess/PTY, filesystem ops, patch apply, MCP dispatch, web │
└───────────────────────────────┬──────────────────────────────┘
                                │
┌───────────────────────────────▼──────────────────────────────┐
│ Host Reality                                                  │
│ workspace, shell, network, credentials, external services      │
└──────────────────────────────────────────────────────────────┘
```

Rules:

- The world kernel must be usable without the CLI.
- The CLI must be able to run against a simulation executor.
- Provider adapters do not enforce policy.
- Executors do not evaluate policy.
- Raw tool handlers are not reachable from the model loop.

## 5. Core Contracts

### `WorldManifest`

The design-time authoring artifact. It defines:

- actors: user, model, subagent, MCP server, shell worker
- trust channels: user, workspace file, shell output, MCP output, web, memory
- data classes: public, workspace, secret, credential, generated, external
- base actions: read, write, patch, command, PTY, MCP, web, memory
- scoped capabilities over base actions
- side-effect surfaces: filesystem, process, network, credential, memory
- transition policies: e.g. tainted external data cannot reach network or
  durable memory
- approval rules
- budget rules
- observability and redaction rules

Manifests are authored and reviewed at design time. Runtime uses only compiled
artifacts.

### `CompiledWorld`

Immutable runtime artifact produced by compiling a manifest:

- world id and manifest hash
- closed full ontology
- projected-world rules
- schema validators
- descriptor hashes
- capability matrix
- side-effect metadata
- taint rules
- approval rules
- budget rules
- effect-mode rules
- trace redaction rules

Hot reload creates a new compiled world version. It does not mutate the current
one.

### `Perception`

Typed form of anything entering model context:

- user prompt
- file content
- command stdout/stderr
- MCP response
- web response
- memory recall

Fields:

- `id`
- `source_channel`
- `trust_level`
- `taint`
- `content_hash`
- `provenance_chain`
- `payload_ref`
- `redaction_policy`

Raw bytes should not be appended directly to context. The orchestrator may
render perceptions for the model, but the kernel keeps the typed record.

### `Provenance` and `Taint`

Carried by every `Perception` and every tool result:

- `source_channel`: originating channel; trust is a property of the channel, not
  of the content
- `trust_level`: derived from the channel
- `parent_sources`: lineage of the prior values that produced this one
- `session_id`: session in which the value originated
- `content_hash`: payload identity

Taint is the single monotonic property derived from provenance:

- `CLEAN` or `TAINTED`, joined as `CLEAN ∨ TAINTED = TAINTED`
- it can increase through transformation; it can never be cleared by the model
- it is preserved across sessions, so memory written tainted stays tainted

`TaintedValue<T>` is the return type of every executor call: a value paired with
its taint. There is no untainted execution result.

### `ToolCall`

Neutral provider-independent proposal:

- `action_name`
- `arguments`
- `provider`
- `call_id`
- `source_perceptions`
- `session_id`

OpenAI, Anthropic, Gemini, MCP, and CLI-native tool formats normalize into this
shape before reaching the kernel.

### `IntentIR`

Sealed internal intent. It can only be built by `IRBuilder`.

It contains:

- action descriptor metadata
- validated parameters
- source/provenance references
- computed taint
- descriptor hash expected at build time
- current world/version

Construction may fail with:

- `UnknownToOntology`: action is not in the full compiled ontology
- `Absent`: action exists globally but is not projected into this world/context
- `SchemaViolation`
- `CapabilityViolation`
- `InvariantViolation`
- `DescriptorDrift`
- `TaintViolation`
- `ApprovalRequired`
- `BudgetExceeded`

The executor never receives raw `ToolCall`. It receives only `ExecutionSpec`.

### `Descriptor`

The frozen, hashable identity of an exposed action:

- model-facing schema
- argument constraints
- side-effect class
- backing handler or MCP server identity
- policy-relevant metadata

`descriptor_hash` is the deterministic hash of that record, computed at
projection/registration. `IntentIR` records the hash expected at build time and
`ExecutionSpec` carries it forward; a mismatch at execution is `DescriptorDrift`
and blocks before the handler runs.

### `Decision`

Policy outcome:

- `ABSENT`: action is unavailable in this world/context
- `ALLOW`: action may run with an effect mode
- `DENY`: action is visible/present but blocked
- `ASK`: action requires human approval
- `REPLAN`: request is over budget or too broad; planner should choose a
  smaller/cheaper/safer path

`ABSENT`, `DENY`, `ASK`, and `REPLAN` are returned as structured feedback.
Avoid turning them into vague prose.

### `EffectMode`

Execution reality for an allowed action:

- `EXECUTE`: real execution
- `SIMULATE`: synthetic result, no real side effect
- `PROXY`: indirect controlled execution
- `SANITIZE`: execute and filter/redact result
- `TRUNCATE`: execute and bound output size
- `DEFER`: delay execution until a condition is satisfied

Use `Decision::ALLOW + EffectMode::SIMULATE`, never
`Decision::SIMULATE`.

### `ExecutionMode`

A property of the run, set by the orchestrator and carried into evaluation. Not
to be confused with `EffectMode` (how an allowed action touches reality);
`ExecutionMode` is whether a human is available to approve:

- `INTERACTIVE`: `ASK` can pause and mint an `ApprovalToken`
- `BACKGROUND`: `ASK` fails closed to `DENY`; an autonomous run never blocks on a
  human

Every other decision is identical across modes. Only `ASK` resolution differs.

### `ExecutionSpec`

The only object crossing into execution:

- action id
- argv or structured operation
- cwd/root policy
- environment policy
- timeout
- network policy
- filesystem policy
- expected descriptor hash
- effect mode
- trace id

It does not contain model context, raw prompt text, policy internals, or handler
function references.

### `ApprovalToken`

Durable state transition:

```text
pending -> approved -> executed
pending -> rejected
```

Approvals are specific to action, parameters, world version, descriptor hash,
provenance, and effect mode. A token approved for one world or descriptor cannot
be reused after drift.

In `BACKGROUND` mode, `ASK` resolves to `DENY`.

## 6. Request Pipeline

```text
Perceptions enter context
  -> source classification + provenance + taint

Model proposes provider-native tool call
  -> provider adapter emits neutral ToolCall

IntentMapper checks full ontology
  -> unknown action: UNKNOWN_TO_ONTOLOGY

IRBuilder validates against compiled world
  -> known but not projected: ABSENT
  -> schema/capability/invariant failure: DENY/ABSENT
  -> sealed IntentIR on success

Kernel evaluates deterministic rules
  1. hard invariants
  2. projection and capability
  3. schema and path constraints
  4. descriptor drift
  5. taint x side-effect
  6. reversibility/destructiveness
  7. approval state
  8. budgets
  9. default allow

Kernel emits Decision + EffectMode
  -> ABSENT/DENY/ASK/REPLAN: structured response and trace
  -> ALLOW: build ExecutionSpec

Executor runs ExecutionSpec
  -> apply EXECUTE/SIMULATE/PROXY/SANITIZE/TRUNCATE/DEFER
  -> return TaintedValue result
  -> append trace
```

Two phases, one deterministic pass. `IRBuilder` decides **representability**: an
intent that fails ontology, projection, capability, schema, descriptor, or a
hard taint invariant cannot be built at all, and the failure is surfaced to the
model using the same outcome vocabulary (`UNKNOWN_TO_ONTOLOGY`, `ABSENT`, or a
structural `DENY`). A built `IntentIR` is representable by construction; the
remaining contextual rules — reversibility, approval state, budget — decide its
**disposition** (`ALLOW + EffectMode`, `ASK`, or `REPLAN`). The numbered rules
above are the order in which that pass short-circuits; first match wins.

Hard invariants run before manifest policy. A manifest cannot override them.
Human approval cannot override them either.

Minimum hard invariants:

- tainted values cannot reach external write, network egress, credential access,
  or durable memory without a policy-defined safe path
- values without provenance cannot cross trust boundaries
- execution must map to a compiled action and local executor registry entry
- descriptor drift blocks before handler execution
- writes cannot escape writable roots

## 7. CLI World Model

Start with a small ontology. Avoid raw shell as a model-visible capability.

Base actions:

- `read_workspace`
- `write_workspace`
- `apply_patch`
- `run_command`
- `start_pty`
- `call_mcp_tool`
- `fetch_web`
- `update_memory`

Scoped capabilities:

- `read_repo_file`: `read_workspace` limited to configured roots
- `apply_workspace_patch`: `apply_patch` limited to writable roots
- `run_tests`: `run_command` with fixed argv prefix and network disabled
- `cargo_check` / `npm_test` / `pytest`: command-specific wrappers
- `git_status`, `git_diff`, `git_commit`: constrained git verbs
- `call_known_mcp_tool`: MCP call pinned to server profile and descriptor hash

Scoped capabilities should:

- expose only actor-input arguments
- strip unknown or locked actor-supplied arguments
- inject literals after stripping
- hash the exposed descriptor and the backing base-tool binding

Default channel trust:

| Channel | Default |
|---|---|
| direct user prompt | trusted, traced |
| workspace file | workspace data; tainted as instruction source |
| shell output | semi-trusted, tainted by default |
| MCP output | tainted unless server profile says otherwise |
| web/network | untrusted, tainted |
| durable memory | trust and taint preserved from original write |
| generated model text | derived; inherits source taint |

Default action outcomes:

| Condition | Outcome |
|---|---|
| action unknown to full ontology | `UNKNOWN_TO_ONTOLOGY` |
| action known but hidden by world/context | `ABSENT` |
| trust level lacks capability | `ABSENT` |
| schema or path validation fails | `DENY` |
| descriptor hash drift | `DENY` |
| tainted data to network/external write/credential/durable memory | `DENY` |
| destructive filesystem/process operation | `ASK` |
| path outside writable roots | `DENY` |
| path outside any known root | `ASK` interactive, `DENY` background |
| command/network budget exceeded | `REPLAN` |
| approval required in background mode | `DENY` |
| all checks pass | `ALLOW + EffectMode` |

## 8. Execution Physics

Layer 0 is not optional for a real CLI harness. Policy narrows semantics, but
the OS still enforces physics.

Minimum sandbox expectations:

- isolated working directory
- explicit writable roots
- isolated home directory
- no ambient host SSH/cloud credentials
- environment allowlist
- subprocess timeout and kill-tree handling
- PTY session ownership and cancellation
- network disabled by default, with manifest-defined exceptions
- optional container/gVisor/namespace backend for higher isolation

Policy should not depend on the sandbox being perfect, and the sandbox should
not depend on policy being perfect. They are independent backstops.

## 9. Audit and Replay

Every stage emits append-only trace records:

- perception created
- tool surface projected
- tool call proposed
- intent build succeeded or failed
- decision produced
- approval created/resolved
- execution started/completed
- result perception created

Trace records include:

- trace id and session id
- world id and manifest hash
- action name
- decision and rule
- effect mode
- source channel
- taint summary
- descriptor hash
- approval token id if relevant
- redacted parameter summary
- timing and budget fields

Redaction runs before disk write. Secrets should never be written to the audit
log in plaintext.

Replay requirements:

- same trace + same compiled world => same decision
- old trace + new compiled world => explicit policy drift report
- Python/Rust/Rego implementations, if multiple exist, must agree on decision
  outputs for the same compiled input

## 10. Design-Time Loop

The LLM belongs at design time, not in runtime enforcement.

Useful design-time model tasks:

- inspect repository structure
- infer common test/build commands
- identify writable roots
- propose scoped capabilities
- draft world manifest
- explain trace failures
- suggest manifest changes after human-reviewed approvals

The human reviews and commits the manifest. The compiler produces the runtime
artifact. Runtime does not ask an LLM whether an action is safe.

`ASK` events should feed the design loop:

- one-shot approval: allow this exact action once
- manifest extension: after review, change the world so future similar actions
  are projected safely

Keep those two paths separate.

## 11. Things Not to Copy

Do not inherit these traits from the prototypes:

- mixing ontology, policy, UI, demo code, and execution in one module
- fallback from unknown plan type to direct execution
- stringly typed trust and provenance at critical boundaries
- pattern blocklists as the primary defense
- boolean-only taint as the final data model
- in-memory-only approval state beyond demos
- mutable policy state inside executor handlers
- provider-specific policy gates that can drift apart
- `SIMULATE` as a policy decision
- raw shell exposed directly as a model-visible tool

## 12. Implementation Shape

Rust is the best fit for the core harness and kernel because the hard parts are
process control, PTY management, filesystem safety, cancellation, concurrency,
and distribution.

Recommended workspace:

```text
crates/
  cli-harness/        terminal UX, model loop integration, approvals
  agent-core/         context packing, provider-independent turn state
  provider-adapters/  OpenAI/Anthropic/Gemini/MCP tool-call normalization
  world-kernel/       CompiledWorld, IRBuilder, decisions, taint, budgets
  executor/           subprocess, PTY, filesystem, patch, MCP, web
  trace-store/        append-only audit, redaction, replay, drift reports
  compiler/           manifest validation, descriptor hashing, compiled worlds
```

Use TypeScript or Python where they are cheaper:

- provider demos
- evals and benchmark runners
- manifest authoring experiments
- plugin SDKs
- compatibility shims

Keep the core contracts language-neutral so adapters can exist in other
languages without changing kernel semantics.

## 13. MVP Sequence

1. Define core types: `Perception`, `ToolCall`, `IntentIR`, `Decision`,
   `EffectMode`, `ExecutionSpec`, `Provenance`, `Taint`, `CompiledWorld`.
2. Implement manifest compiler for a minimal default CLI world.
3. Implement descriptor hashing and closed ontology lookup.
4. Implement `IRBuilder` and deterministic kernel evaluation.
5. Implement executor adapters for read file, apply patch, and run command.
6. Add append-only trace store and replay tests.
7. Wire one provider/model loop through projected tools only.
8. Add durable approvals and background-mode denial.
9. Add scoped command capabilities.
10. Add MCP behind the same descriptor/provenance path.
11. Add simulation effect mode.
12. Add sandbox backend and network policy.
13. Add bundle replay and policy drift reports.

## 14. Acceptance Invariants

The harness is not minimally correct until these pass deterministically:

1. Unknown actions cannot form `IntentIR`.
2. Known but non-projected actions return `ABSENT`, not `DENY`.
3. Unknown-to-ontology is distinct from projected absence.
4. The model cannot invoke an executor directly.
5. Only `ExecutionSpec` crosses into execution.
6. Taint never decreases across transformations or sessions.
7. Tainted file/web/MCP/shell output cannot drive network egress, credential
   access, durable memory writes, or external side effects.
8. Workspace writes outside writable roots are denied.
9. Destructive commands require approval.
10. Approval-required actions in background mode deny.
11. Descriptor drift blocks before handler execution.
12. Scoped capabilities strip actor-supplied locked args before injecting
    literals.
13. `ALLOW + SIMULATE` produces no real side effect.
14. Replay with the same compiled world reproduces the same decision.
15. Redaction prevents secrets from reaching audit logs.
16. Executor refuses actions absent from its local closed registry.

## 15. Final Design Judgment

The harness should combine the deeper safety model of `agent-hypervisor` with
the sharper product mechanics of `safe-mcp-proxy`:

- from `agent-hypervisor`: sealed construction, monotonic taint, provenance,
  design-time manifests, deterministic runtime, and worker isolation
- from `safe-mcp-proxy`: `ABSENT`/`DENY`/`ASK` semantics, descriptor drift,
  scoped capabilities, provider adapters, execution modes, and replayable audit

The result is not perfect security. It is bounded, measurable, replayable
security for local developer agents. That is the right target.
