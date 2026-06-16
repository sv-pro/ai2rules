# CLI Agent Harness Architecture

This note distills the useful principles from `agent-hypervisor` and
`safe-mcp-proxy` into a clean ground-up design for a Codex-like CLI agent
harness. The goal is to borrow the architecture, not the prototype code.

## Direction

Build the harness as two cooperating systems:

- Agent harness: terminal UX, model loop, context management, tool planning,
  command streaming, patch application, session state.
- World kernel: deterministic capability projection, provenance, taint,
  approval, budget, and trace enforcement.

The world kernel is built in, but architecturally separate. The model never
executes tools directly. It receives a projected tool surface and emits typed
intent proposals. The kernel decides whether an intent can become an execution
request.

## Principles to Borrow

The strongest ideas in `agent-hypervisor` are:

- Closed action ontology: absent actions do not exist. Unknown actions fail
  before any execution path is entered.
- Design-time manifest, runtime tables: YAML or TOML is compiled into immutable
  policy artifacts. Runtime enforcement should not interpret authoring files.
- Capability projection: the agent sees only the tools/actions available for the
  current actor, task, trust level, workspace, and approval state.
- Typed intent boundary: natural language and raw JSON do not cross into
  execution. The model proposes `Intent` objects with schemas.
- Provenance on values: tool outputs, file contents, MCP responses, web content,
  user input, and model-derived values carry origin metadata.
- Monotonic taint: untrusted data can become more constrained, not silently
  clean. Unknown provenance defaults to tainted.
- Deterministic gate: allow, deny, ask, or replan are produced without an LLM on
  the critical path.
- Approval as state: human approval creates a durable token/record and is later
  consumed by execution. It is not a prompt convention.
- Trace everything: every perception, proposed intent, decision, approval, and
  execution result gets an auditable record.
- Worker boundary: actual handlers live behind an execution boundary. The policy
  layer owns no direct handler callables.

The strongest additions from `safe-mcp-proxy` are:

- ABSENT versus DENY: `ABSENT` is an ontology outcome. `DENY` is a policy
  outcome. Keep them separate because they create different model behavior and
  different audit meaning.
- Three-layer split: ontology decides whether an action exists, policy decides
  whether the present action is allowed, and effect virtualization decides how
  the action runs.
- Descriptor drift detection: hash tool schemas and descriptors at registration
  time, then block calls if the runtime descriptor changes.
- Scoped capabilities: expose constrained forms over broader tools, with
  literal arguments injected after actor input is stripped. Example:
  `send_status_to_owner` can exist without exposing raw `send_email`.
- Execution mode: `INTERACTIVE` can create approval requests; `BACKGROUND`
  collapses `ASK` to `DENY`.
- Provider adapters: parse provider-specific tool calls into a neutral
  `IntentIR`, then run one shared policy gate.

## Things Not to Copy

Do not copy these prototype traits into the new harness:

- Mixing ontology, policy, gateway, demo UI, and program execution in one layer.
- Runtime fallback from unknown plan types to direct execution.
- Stringly-typed trust and provenance across boundaries.
- Demo adapters that perform real network/filesystem actions without a proper
  sandbox contract.
- Multiple overlapping engines where a "core" engine shadows legacy engines.
- Python module import hacks across package roots.
- Treating pattern blocklists as a primary defense.
- Conflating policy decisions with execution effects. `SIMULATE` is an effect
  mode, not a policy decision.
- In-memory-only approval state for anything beyond demos.
- Boolean-only taint as the whole provenance model. It is useful for an MCP MVP,
  but not enough for a local coding harness.

## Proposed Layering

```text
┌──────────────────────────────────────────────────────────┐
│ CLI Shell / TUI                                           │
│ prompt input, streaming output, approvals, status         │
└───────────────────────────┬──────────────────────────────┘
                            │
┌───────────────────────────▼──────────────────────────────┐
│ Agent Orchestrator                                        │
│ model calls, context packing, planning, tool-call parsing  │
└───────────────────────────┬──────────────────────────────┘
                            │ projected tools + typed intents
┌───────────────────────────▼──────────────────────────────┐
│ World Kernel                                              │
│ compiled ontology, capability projection, taint, policy,   │
│ approval, budgets, trace                                  │
└───────────────────────────┬──────────────────────────────┘
                            │ validated execution specs
┌───────────────────────────▼──────────────────────────────┐
│ Execution Boundary                                        │
│ PTY, subprocesses, filesystem ops, patch apply, MCP, web   │
└───────────────────────────┬──────────────────────────────┘
                            │ OS/container/sandbox primitives
┌───────────────────────────▼──────────────────────────────┐
│ Host Reality                                              │
│ files, shell, network, credentials, external services      │
└──────────────────────────────────────────────────────────┘
```

The world kernel should be usable independently of the CLI. The CLI should be
able to run with a mock/simulation executor for tests.

The kernel should expose three explicit stages:

```text
Ontology:
  action exists in compiled world?
  → ABSENT | PRESENT

Policy:
  present action permitted in current context?
  → ALLOW | DENY | ASK | REPLAN

Effect:
  how does an allowed action interact with reality?
  → EXECUTE | SIMULATE | PROXY | SANITIZE | TRUNCATE | DEFER
```

Do not encode `SIMULATE` as a `Decision`. Use a separate `effect_mode` field.

## Core Contracts

### `WorldManifest`

Authoring artifact. Defines:

- actors: user, model, subagent, MCP server, shell worker
- trust channels: user input, model output, file read, shell output, MCP output,
  network/web, generated code
- data classes: public, workspace, secret, credential, external, generated
- actions: read file, write file, apply patch, run command, start PTY, send
  network request, call MCP tool, update memory
- side-effect surfaces: filesystem, process, network, credential store, memory
- transition policies: e.g. external data cannot flow to network or memory
  without approval
- budgets: command count, wall time, token, cost, network calls, file write
  count
- observability: fields to log and fields to redact

### `CompiledWorld`

Runtime artifact. Immutable and preferably hash-addressed. Contains:

- closed action set
- schema validators
- capability matrix
- taint transition table
- approval rules
- budget rules
- projected tool descriptors
- descriptor hashes
- trace redaction rules

The harness should load this once at session start. Hot reload is a new compiled
world version, not mutation of the current one.

### `Perception`

Everything entering the model is converted to a typed perception:

- user message
- file content
- command stdout/stderr
- MCP response
- web response
- prior memory

Each perception carries provenance, trust, taint, content hash, and redaction
metadata. Raw reality should not be appended directly to model context.

### `Intent`

The model emits proposed actions. Examples:

- `RunCommand { argv, cwd, env_policy, pty, timeout }`
- `ReadFile { path }`
- `ApplyPatch { patch }`
- `WriteFile { path, content, mode }`
- `CallMcpTool { server, tool, arguments }`
- `UpdateMemory { key, value }`
- `RequestApproval { action_ref, summary }`

Intent construction validates schema, but authorization happens in the kernel.

### `Decision`

The kernel returns:

- `absent`: action does not exist in this world
- `allow`: produce an `ExecutionSpec`
- `deny`: no execution path
- `ask`: durable approval request
- `replan`: budget or policy requires a cheaper/safer path

`absent`, `deny`, `ask`, and `replan` should be model-visible as structured
feedback, not as free-form prose only. `absent` should usually be phrased as
"that action is unavailable in this world", not as a permission denial.

### `EffectMode`

Allowed actions still need an execution reality:

- `execute`: real execution
- `simulate`: synthetic result, no real side effect
- `proxy`: route through another controlled service
- `sanitize`: execute but filter/redact result
- `truncate`: execute but bound result size
- `defer`: delay execution until a condition is satisfied

Effect mode is orthogonal to policy. The pair is `Decision::Allow` plus
`EffectMode::Simulate`, not `Decision::Simulate`.

### `ExecutionSpec`

The only object crossing into real execution. It contains only what the executor
needs, not policy objects or model context.

### `DescriptorHash`

Every exposed tool/capability descriptor should have a deterministic hash:

- model-facing schema
- argument constraints
- side-effect class
- handler identity or upstream server identity
- policy-relevant metadata

If a descriptor changes after projection or registration, the next call should
fail before execution with a descriptor-drift decision.

## CLI-Specific World Model

For a coding CLI, the first useful manifest should be small:

- `read_workspace`: read files under workspace roots.
- `write_workspace`: write files under workspace roots.
- `apply_patch`: modify workspace files using a structured patch.
- `run_command`: run non-network commands with timeout.
- `run_network_command`: explicit network-capable execution, approval-gated.
- `start_pty`: long-running command session, approval-gated by command class.
- `call_mcp_tool`: external tool boundary, trust depends on server profile.
- `update_memory`: durable memory write, denied for tainted external content.

Also define scoped capabilities over raw actions:

- `run_tests`: constrained `run_command` with fixed argv prefixes.
- `cargo_check`: constrained `run_command` with network disabled.
- `apply_workspace_patch`: constrained `apply_patch` limited to writable roots.
- `read_repo_file`: constrained `read_file` limited to workspace roots.
- `call_known_mcp_tool`: constrained `call_mcp_tool` bound to a pinned server and
  descriptor hash.

Default rules:

- Unknown action: deny.
- Unknown tool not in the full ontology: absent.
- Known tool not in the projected world: absent.
- Unknown path root: ask or deny depending on mode.
- User input: trusted but still traced.
- File content: workspace-trusted, tainted if outside trusted root or generated.
- Shell output: semi-trusted and tainted by default.
- MCP output: tainted by default unless server profile says otherwise.
- Web/network output: untrusted and tainted.
- Tainted data to network, credential, durable memory, or external write: ask or
  deny.
- Destructive filesystem and process operations: ask.
- Descriptor drift: deny.
- `ASK` in background mode: deny.

## Recommended Implementation Shape

Use Rust for the core harness and world kernel:

```text
crates/
  cli-agent/          CLI/TUI entrypoint
  agent-core/         model loop, session, context, typed intents
  world-kernel/       compiled world, decisions, taint, provenance, budgets
  executor/           PTY, subprocess, filesystem, patch, MCP dispatch
  trace-store/        append-only traces, redaction, replay
  manifest-compiler/  manifest validation and compile artifacts
  provider-adapters/   OpenAI/Anthropic/Gemini/MCP call normalization
```

Use TypeScript or Python only where they are clearly better:

- plugin SDKs
- evals and benchmarks
- manifest authoring experiments
- provider-specific scripts

## MVP Sequence

1. Define the Rust types for `Perception`, `Intent`, `Decision`,
   `EffectMode`, `ExecutionSpec`, `Provenance`, `Taint`, and `CompiledWorld`.
2. Implement a minimal manifest compiler with a hardcoded default CLI world.
3. Build `world-kernel.evaluate(intent, context)`.
4. Implement executor adapters for read file, apply patch, and run command.
5. Add append-only traces and replay tests.
6. Wire the model loop through projected tools only.
7. Add approval flow.
8. Add MCP tools behind the same intent and provenance model.
9. Add descriptor hashing and drift tests.
10. Add simulation mode and scenario tests.

## Acceptance Tests

The first version is not done until these pass deterministically:

- Unknown action cannot form an execution spec.
- Known but non-projected action returns `ABSENT`, not `DENY`.
- Model cannot call an executor directly.
- Untrusted command output cannot be written to durable memory without approval.
- Tainted file/web/MCP content cannot drive network egress.
- Workspace writes outside writable roots are denied.
- Destructive commands require approval.
- Approval-required actions in background mode deny deterministically.
- Descriptor drift blocks execution.
- Scoped capabilities strip actor-supplied locked args before injecting literals.
- Same manifest plus same trace replay produces the same decisions.
- Trace redaction prevents secrets from being logged in plaintext.
- Executor refuses any action not in its local closed registry.

## Key Architectural Decision

The hypervisor should be a kernel, not a wrapper.

A wrapper checks calls after the agent already believes the raw world exists.
A kernel defines what the agent can perceive and propose in the first place.
That distinction is the part of `agent-hypervisor` worth preserving.
