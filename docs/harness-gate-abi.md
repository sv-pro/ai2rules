# `harness gate` — the host-neutral governance ABI

Status: **scoped (v1 design)**, 2026-06-26. Decision: `DECISIONS.md` **D24**
(refines D19). Vocabulary: `docs/GLOSSARY.md` → *Integration / topology*.

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
  "context": {
    "session_id": "cc-9f3a",
    "mode": "interactive",
    "taint": "clean",
    "source_channel": "user_prompt",
    "approval_token": null
  }
}
```

| Field | Req | Meaning |
|---|---|---|
| `v` | ✓ | ABI version (integer). v1. |
| `tool` | ✓ | The action name **in the manifest's vocabulary** (the adapter has already mapped the host's tool name; for CC the manifest *uses* `Bash`/`Read`/…). → `ToolCall.action_name`. |
| `arguments` | ✓ | The proposed call's arguments (object). → `ToolCall.arguments`. |
| `context.session_id` | ✓ | Opaque host session id. → `SessionId`; trace correlation; taint sidecar key. |
| `context.mode` | ✓ | `interactive` \| `background`. → `ExecutionMode` (drives ASK→DENY fail-closed). |
| `context.taint` | ✓ | Monotonic state carried by the adapter: `clean` \| `tainted`. → `TaintContext`. |
| `context.source_channel` |  | Provenance of this call's trigger: `user_prompt` (default) \| `web` \| `workspace_file` \| `mcp_output` \| … → `SourceChannel` (trust). |
| `context.approval_token` |  | A token previously minted by an `ASK` and now granted, when re-submitting. → `EvalContext.approval_granted`. |

Unknown fields are ignored (forward-compatible). Budgets/usage are a v1.x addition
to `context` (kernel already supports `BudgetUsage`); v1 assumes fresh usage.

## 4. `GateResponse` (stdout)

```json
{
  "v": 1,
  "decision": "DENY",
  "rule": "taint_invariant",
  "reason": "tainted context cannot reach an externally-effectful action",
  "context": { "taint": "tainted" },
  "approval": null,
  "manifest_hash": "ab12cd34ef56"
}
```

| Field | Meaning |
|---|---|
| `decision` | `ABSENT` \| `ALLOW` \| `DENY` \| `ASK` \| `REPLAN`. (`UnknownToOntology` is surfaced as `ABSENT` with `rule:"unknown_to_ontology"`, per `KernelOutcome::decision()`.) |
| `rule` | The rule/invariant that fired (`absent`, `capability`, `taint_invariant`, `approval_required`, `budget_exceeded`, …), or `null` for a plain `ALLOW`. |
| `reason` | Human-readable, for the host's UI / the trace. |
| `context.taint` | **Post-call** monotonic taint the adapter must persist for the next call. `clean` only if it was clean *and* this call is not a declared taint source; otherwise `tainted`. |
| `approval` | On `ASK`: `{ "token": "<id>", "required": true }`. Else `null`. The adapter surfaces the host's approval UI; on grant it re-submits with `context.approval_token`. |
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

## 6. Host-adapter contract (the six steps)

Every host adapter, regardless of language, does exactly this:

1. Receive the host's pre-tool intercept event.
2. Restore monotonic taint for `session_id` from the sidecar (default `clean`).
3. Build a `GateRequest` (map the host tool/args, set `mode`, attach `taint`).
4. Run `harness gate --world <W>` with the request on stdin; read the response.
5. Persist `response.context.taint` back to the sidecar (monotonic; never lowers).
6. Map `response.decision` → the host's decision shape; fail-open/closed on `≠0`.

No governance logic lives in the adapter — only event-shape translation and taint
state plumbing. **The taint *algebra* and the rules (incl. which inputs taint) live
in the kernel + the compiled `WorldManifest`.**

### Claude Code adapter (illustrative)

The PreToolUse hook collapses from ~150 lines of ported logic to a shim:

```python
ev = json.load(sys.stdin)
taint = "tainted" if sidecar_tainted(ev["session_id"]) else "clean"
req = {"v": 1, "tool": ev["tool_name"], "arguments": ev.get("tool_input", {}),
       "context": {"session_id": ev["session_id"],
                   "mode": "background" if is_background() else "interactive",
                   "taint": taint}}
res, code = run_gate(req)                       # subprocess: harness gate --world cc-world.yaml
if code != 0:  sys.exit(0)                       # fail open
persist_taint(ev["session_id"], res["context"]["taint"])
emit_cc_decision(res["decision"], res["reason"]) # deny/ask → PreToolUse JSON; else pass through
```

The three behaviours the Python slice hand-ported (ABSENT-for-native, taint floor,
ASK) are now the kernel's, driven by `cc-world.yaml` — a real `WorldManifest`
(replacing the bespoke `cc-world.json` schema).

## 7. Migration & open items

- **Manifest migration (required):** express the Claude Code world as a real
  `WorldManifest`. `cc-world.json`'s `taint_sources`/`egress`/`ask`/`projected_tools`
  map onto manifest perception channels, `transition_policies`, `approval_required`,
  and the projected set (per D19's mapping table). This is where governance actually
  moves out of Python.
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
