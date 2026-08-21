# `harness gate` — the host-neutral governance ABI

Status: **shipped (gate v1 + discovery projection v1)**, updated 2026-08-21.
Decisions: `DECISIONS.md` **D24** (refines D19), **D34** (in-process vs wire),
**D36** (kernel-side classification), **D37** (live-hook cutover), **D41**
(approval tokens are correlation ids, not bearer grants), **D42** (gate context
is explicit and fail-closed), **D43** (source-channel trust is compiled manifest
policy), **D44** (shell classifiers fail closed), **D46** (trusted harness
resolution). Vocabulary:
`docs/GLOSSARY.md` → *Integration / topology*. Cross-host parity is pinned by
`crates/cli-harness/tests/one_kernel.rs` (see `docs/one-kernel-many-hosts.md`).

This is the single interface through which **any host** asks the kernel for a
verdict on a proposed tool call. A host integrates by writing a **thin adapter**
that speaks this ABI — never by reimplementing governance. The same kernel binary
serves Claude Code, a Hermes agent, Codex CLI, and the MCP proxy; only the adapter
and the world manifest differ (D24 *Consequence*).

```
host intercept point ──▶ host adapter ──▶ `harness gate --world W` ──▶ host adapter ──▶ host
   (PreToolUse, MCP        (GateRequest)      pure decide() over W        (GateResponse)    allow/
    request, …)                                                                              deny/ask
```

The gate is **decision-only**: it returns `ABSENT/ALLOW/DENY/ASK/REPLAN` and the
post-call taint state. It does **not** execute — on `ALLOW` the host runs its own
tool. (Execution is a different surface: `harness run` / `harness proxy`.)

---

## 1. Where the logic lives

One **pure** function, native and WASM identical:

```rust
// in crate `harness-preview`, beside `preview()`
pub fn gate(world: &CompiledWorld, req: &GateRequest) -> GateResponse;
```

It maps `GateRequest` → the kernel's neutral types, calls
`world_kernel::decide(world, &call, provenance, &ctx)`, and maps the
`KernelOutcome` back to `GateResponse`. Purity is preserved (no I/O, no LLM), so:

- `harness gate` (a thin `cli-harness` subcommand) wraps it with stdin/stdout +
  manifest loading + optional trace I/O;
- `harness-wasm` exposes the *same* `gate()` to JS;
- the **reference harness** (`cargo run --bin harness`) is the conformance oracle —
  native and WASM gate verdicts are pinned to it by golden vectors (extends E14.4).

## 2. Invocation

```bash
harness gate --world .claude/cc-world.yaml   # one GateRequest on stdin → one GateResponse on stdout
```

- **Single-shot** (one process per tool call), matching the current PreToolUse hook
  lifecycle. A long-lived **`--stream`** mode (NDJSON in/out, for the MCP proxy and
  in-process adapters) is a v1.x extension, same schema per line.
- `--world <path>` (required): a real `WorldManifest` (YAML/JSON) — compiled once
  per process via the same `loader::load_yaml` + `compiler::compile` path.
- `--trace <path>` (optional): append the decision to a redacted JSONL trace
  (`trace-store`). This is the only I/O; the kernel stays pure.

## 3. `GateRequest` (stdin)

```json
{
  "v": 1,
  "tool": "Bash",
  "arguments": { "command": "rm -rf /tmp/x" },
  "path": null,
  "context": {
    "session_id": "cc-9f3a",
    "mode": "interactive",
    "taint": "clean",
    "source_channel": "user_prompt",
    "approval_token": null,
    "usage": { "commands_run": 2, "network_calls": 0, "file_writes": 1, "tokens_used": 0 }
  }
}
```

| Field | Req | Meaning |
|---|---|---|
| `v` | ✓ | ABI version (integer). v1. |
| `tool` | ✓ | The action name **in the manifest's vocabulary** (the adapter has already mapped the host's tool name; for CC the manifest *uses* `Bash`/`Read`/…). → `ToolCall.action_name`. |
| `arguments` | ✓ | The proposed call's arguments (object). → `ToolCall.arguments`. |
| `path` |  | Adapter-canonicalized absolute path for path-scoped file actions. Required when roots are enabled and the effective action is a filesystem read/write/patch action; Bash and other non-file actions set `null`. |
| `context.session_id` | ✓ | Opaque host session id. → `SessionId`; trace correlation; taint sidecar key. |
| `context.mode` | ✓ | `interactive` \| `background`. → `ExecutionMode` (drives ASK→DENY fail-closed). |
| `context.taint` | ✓ | Monotonic state carried by the adapter: `clean` \| `tainted`. → `TaintContext`. |
| `context.source_channel` | ✓ | Provenance of this call's trigger: `user_prompt` \| `cli` \| `web` \| `workspace_file` \| `workspace_files` \| `mcp_output` \| …, resolved against the compiled manifest `channels:` table. Manifest trust drives capability checks; manifest taint is joined into `context.taint`. |
| `context.approval_token` |  | Optional correlation id from a prior `ASK`. The pure gate ignores request-supplied tokens; it never maps this field to `EvalContext.approval_granted`. |
| `context.usage` |  | Budget counters consumed so far this session. **Required when the world declares a counted budget** (`max_tokens_per_session`, `max_commands_per_task`, `max_network_calls`, `max_file_writes`); omitted then, the gate returns `DENY` / `missing_usage`. Absent counters within the object are zero, so a caller may send only what it tracks. `command_timeout_ms` alone does not oblige a caller to carry usage — it bounds one execution and the executor enforces it. |

Unknown fields are ignored (forward-compatible). `context.usage` is the v1.x
budget addition anticipated here — it is now implemented, so v1 no longer assumes
fresh usage. Before it, every call arrived with zeroed counters and **no manifest
budget was ever reachable** (finding #16).

The gate fails closed on missing or malformed security context: omitted/invalid
taint, omitted/undeclared/invalid source channel, an omitted `path` for a
roots-scoped file action, or omitted `usage` for a world that counts calls returns
`DENY` with a specific rule. The pattern is the same in each case — a control a
thin adapter can disable by leaving a field out is not a control. This is an
evaluated verdict (exit `0`), not a malformed-process error, so every host
handles it through the same verdict channel.

`context.approval_token` is not a bearer credential. A host that supports
approval resumption must use it only to locate a durable `AuthorizationInstance`
at a trusted adapter/orchestrator boundary. That boundary atomically consumes one
explicit instance before execution and validates trusted principal, canonical
effect (action + complete arguments), resource, world/schema epochs, provenance,
effect mode, expiry, and remaining-use budget. It then re-runs the kernel; the
instance can narrow an `ASK`, never expand the compiled world's ceiling. The
public `harness gate` ABI has no verifier or store access, so a non-null token
still returns `ASK` for approval-required actions.

## 4. `GateResponse` (stdout)

```json
{
  "v": 1,
  "decision": "DENY",
  "action": "bash_network",
  "rule": "taint_invariant",
  "reason": "tainted context cannot reach an externally-effectful action",
  "context": {
    "taint": "tainted",
    "usage": { "commands_run": 2, "network_calls": 0, "file_writes": 1, "tokens_used": 0 }
  },
  "approval": null,
  "manifest_hash": "ab12cd34ef56"
}
```

| Field | Meaning |
|---|---|
| `decision` | `ABSENT` \| `ALLOW` \| `DENY` \| `ASK` \| `REPLAN`. (`UnknownToOntology` is surfaced as `ABSENT` with `rule:"unknown_to_ontology"`, per `KernelOutcome::decision()`.) |
| `action` | The **effective action** the kernel decided on: the request's `tool` after the world's `command_classes` classifiers ran (D36/D44) — e.g. `bash` with a `curl …` command resolves to `bash_network`, and unmatched shell can resolve to a manifest-declared unclassified fallback. Equal to `tool` only when no classifier applies. Backward-compatible v1 addition; adapters use it in taint-cause notes and it seeds the approval token. |
| `rule` | The rule/invariant that fired (`absent`, `capability`, `taint_invariant`, `approval_required`, `budget_exceeded`, …), or `null` for a plain `ALLOW`. |
| `reason` | Human-readable, for the host's UI / the trace. |
| `context.taint` | **Post-call** monotonic taint the adapter must persist for the next call. `clean` only if it was clean *and* this call is not a declared taint source; otherwise `tainted`. |
| `context.usage` | **Post-call** budget counters the adapter must persist for the next call. Charged only on an `ALLOW` that will actually run — a blocked call consumes nothing, so a refusal never pushes a session toward its limit. Tokens are carried through unchanged; only a caller that has seen a model response can count them. |
| `approval` | On `ASK`: `{ "token": "<id>", "required": true }`. Else `null`. The token is a correlation id for the adapter's approval UI/store, not a grant credential. |
| `manifest_hash` | First 12 hex of the compiled manifest hash — drift correlation + trace join. |

## 5. Exit codes (process-level, **not** the verdict)

| Code | Meaning |
|---|---|
| `0` | The gate evaluated. Read `decision` from stdout. **`DENY`/`ASK` are exit 0** — they are successful evaluations, not errors. |
| `2` | Malformed `GateRequest` or unreadable/uncompilable manifest. |
| `1` | Internal error. |

The adapter decides fail-open vs fail-closed on `≠0` (the current CC hook is
fail-open by policy). The ABI never overloads the exit code with the verdict
(D24 alt (e)); decision→host-convention mapping is the adapter's job.

## 6. Host-adapter contract (the seven steps)

Every host adapter, regardless of language, does exactly this:

1. Receive the host's pre-tool intercept event.
2. Restore monotonic taint for `session_id` from the sidecar (default `clean`),
   **and the session's budget counters** (default all-zero). Counters that cannot
   be read are counters that cannot be enforced: fail closed rather than restart a
   budget at zero.
3. Build a `GateRequest` (map the host tool/args, set `mode`, attach explicit
   `taint` + `source_channel` + `usage`, and attach a symlink-aware canonical
   absolute `path` for path-scoped file actions).
4. Run `harness gate --world <W>` with the request on stdin; read the response.
5. Persist `response.context.taint` back to the sidecar (monotonic; never lowers).
6. **Persist `response.context.usage`** back to the sidecar. If it cannot be
   written, refuse the call it would have charged — a counter that silently fails
   to persist is a budget that silently stops counting (D59's rule, applied to
   budgets).
7. Map `response.decision` → the host's decision shape; fail-open/closed on `≠0`.

No governance logic lives in the adapter — only event-shape translation and
session-state plumbing (taint and budget counters). **The taint *algebra*, the rules (incl. which inputs taint), and —
since D36 — command *classification* live in the kernel + the compiled
`WorldManifest`.** Adapters send the **raw host tool name**; the world's
`command_classes` resolve the effective action (returned as `response.action`).
For shell-shaped actions, D44 requires a fail-closed `default_to` fallback; shell
whitespace such as tabs/newlines is treated as whitespace for command patterns.

### Claude Code adapter (illustrative)

The live wiring (D37): `settings.json` → `.claude/hooks/world-gate.sh`, a
bootstrap shim that `exec`s the in-tree Rust adapter (D34 — in-tree Rust hosts
link `gate()` in-process; the wire ABI serves out-of-process/non-Rust hosts):

```bash
# .claude/hooks/world-gate.sh (bootstrap only — no governance logic)
BIN=…locate harness (absolute $HARNESS_BIN/$AI2RULES_HARNESS or installed absolute path)…
[ -z "$BIN" ] && exit 0   # fail-open: no kernel binary, fall through
exec "$BIN" cc-hook --world "$PD/.claude/cc-world.yaml" --state "$PD/.claude/state"
```

`harness cc-hook` then does exactly the six steps above: event → GateRequest
(raw tool name), sidecar taint restore/persist, in-process `gate()`, and
verdict→PreToolUse mapping via the shared `host_outcome()` layer
(`deny`/`ask` emitted; ALLOW/REPLAN pass through; ABSENT denies only under
`--enforce-absent`, prefixed `ABSENT: `). The behaviours the Python slice once
hand-ported (ABSENT, taint floor, ASK, bash classification) are all the
kernel's, driven by `cc-world.yaml` — a real `WorldManifest`.

## 7. Migration & open items

- **Manifest migration — done (D25/D36/D37):** the Claude Code world is the real
  `WorldManifest` `.claude/cc-world.yaml` (channels, `transition_policies`,
  `approval_required`, `command_classes`); `cc-world.json` and the Python engine
  are archived under `.claude/hooks/superseded/`. Governance has moved out of
  Python.
- **Path-based taint sources** (e.g. "reading `repos/` taints the session") may need
  a small manifest-schema addition (untrusted read-roots → channel trust). If taken,
  it is a *design-level* change → record a `D<n>` and a schema bump, not an
  adapter hack (per the *decisions-outrank-code* principle).
- **Inherited limits (D19/D20):** native-tool arg-rewrite stays in the MCP shim;
  on Claude Code, taint stays per-tool/per-path heuristic (no in-data provenance) —
  the ABI relocates that heuristic into the compiled world, it does not make it exact.
- **Conformance (E14.4+):** golden GateRequest→GateResponse vectors, asserted equal
  for native and WASM `gate()`, with the reference harness as oracle.

## 8. Versioning

`v` is an integer; v1 additions are backward-compatible (new optional fields,
ignored if unknown). A breaking change bumps `v` and the adapter negotiates.

**The fail-closed hardenings are the exception, and they have not bumped `v`.**
Requiring `taint`, `source_channel`, `path` (under roots) and now `usage` (under a
counted budget) each turned a previously-tolerated omission into a `DENY`. That is
breaking for a caller relying on the omission — but such a caller was silently
running ungoverned, which is the condition being removed, and a version bump would
let it keep doing so by pinning v1. A caller that sends the full context is
unaffected by all of them.

## 9. Discovery projection wire operation (D72)

`gate` decides a proposed invocation; it cannot make a host stop advertising an
ABSENT tool. Non-Rust hosts use the sibling operation at their authoritative
prompt/tool-schema seam:

```bash
harness project --world world.yaml
```

stdin:

```json
{"v":1,"context":{"source_channel":"user_cli"},"tools":[{"name":"read","description":"Read a file","parameters":{"type":"object"}}]}
```

stdout:

```json
{"v":1,"tools":[…],"absent":[],"manifest_hash":"ab12cd34ef56","schema_hash":"<sha256>"}
```

The operation compiles the world through the same loader/root-normalization path
as `gate`, keeps only offered names in `CompiledWorld::projected_actions()` that
the declared source channel's capability matrix can see, and preserves each
surviving host schema as the same JSON value. The compiled world owns
whether a capability exists; the host registry owns its operational schema. The
response binds the exact visible schema array to a deterministic SHA-256 identity
and uses the same 12-hex manifest identity as `GateResponse`, so discovery and
invocation evidence join directly.

Missing/undeclared source context, malformed input, duplicate names,
world/compiler failure, or serialization failure
is a process error (`2` for request/world, `1` for internal output). An engaged
adapter must fail discovery closed—normally an empty surface—rather than restore
the host's unfiltered catalog.
