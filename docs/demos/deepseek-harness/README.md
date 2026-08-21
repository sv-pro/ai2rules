# DeepSeek Harness × ai2rules — `tools/pre-execute` gate adapter (issue #55)

**The point:** the *same* deterministic kernel and Gate ABI that govern Claude Code
(`cc-hook`), OpenCode, and the MCP gateway also govern **DeepSeek Harness (dsh)
native tool calls** — through a thin cordis plugin that shapes the authoritative
`system-prompt/assemble` result with `harness project`, then subscribes to
`tools/pre-execute` and calls `harness gate`. No policy, taint, or command
classification lives in the plugin; world policy stays in the compiled
`WorldManifest` and the kernel (the one-kernel / thin-adapter rule, D35/D72).

This is the **P1 adapter spike** for [issue #55], the implementation companion to
the [#54 governor-integrity benchmark](../../benchmarks/deepseek-harness/). It
deliberately closes the failure-semantics gap that the #54 review flagged: an
**engaged** gate that becomes unavailable must **fail closed**, not fall through.

[issue #55]: https://github.com/sv-pro/ai2rules/issues/55

```
dsh prompt assembly → system-prompt/assemble → harness project --world W → visible schemas
dsh tool call       → tools/pre-execute      → harness gate --world W    → allow/deny/ask
                                                     same compiled world
```

## Files

| File | What it is |
|---|---|
| [`ai2rules-gate.ts`](ai2rules-gate.ts) | The cordis plugin. Rewrites the final prompt assembly through `harness project`, gates invocation through `harness gate`, maps verdicts to dsh's `PreToolDecision`, and persists monotonic taint + budget usage. Imports **nothing** from dsh — both contracts are structural. |
| [`deepseek-world.yaml`](deepseek-world.yaml) | A **real** `WorldManifest` whose action names are dsh's real registered names. The same file also governs the OpenCode adapter because that host uses the same lowercase names; its classifier patterns are pinned with every host world in `tests/one_kernel.rs`. |
| [`ai2rules-gate.spec.ts`](ai2rules-gate.spec.ts) | Runs discovery and invocation against the **real** dsh `SystemPrompt` + `ToolRegistry` pipeline and the real binary/world. 13 cases include revocation, stale invocation, projection evidence, Code Mode fail-closed, and gate/project outage. |

## Verdict mapping (issue #55)

The adapter maps existing ai2rules outcomes onto dsh's native pre-execute channel
without inventing a second policy vocabulary:

| Kernel decision | dsh `PreToolDecision` | Notes |
|---|---|---|
| `ALLOW` | `next()` → `{kind:'allow'}` | dsh runs the tool |
| `DENY` | `{kind:'deny', reason}` | binding rejection before any effect |
| `ASK` | `{kind:'ask', reason}` | resolved by dsh's own `ctx.approval` one-shot prompt (the narrowest primitive that preserves exact-call binding) |
| `ABSENT` | omitted from `PromptAssembly.tools`; stale/direct invocation → `{kind:'deny'}` | discovery is advisory, invocation remains the authority after revocation |
| `REPLAN` | `{kind:'deny', reason}` | **best-effort.** dsh's pre-execute seam has only allow/deny/ask — no faithful non-effecting "continue without executing" primitive — so REPLAN denies and says so. Documented here as an ABI/host mismatch rather than silently weakened (#55). |
| *(unknown)* | `{kind:'deny', reason}` | unmappable verdict → fail closed, ABI mismatch noted |

Native dsh sandbox / monotonic guards / approval remain a **separate** control
layer underneath: the adapter does not collapse them into ai2rules semantics
(#55 constraint). A kernel `DENY` produces no effect even where dsh-native policy
would have allowed the call.

## Failure semantics — the #55 contract (fail-closed)

Issue #55: *"Gate unavailability must have an explicit tested behavior; governed
effects must not silently fail open."* This adapter is the deliberate **inverse**
of the OpenCode/`cc-hook` **bootstrap shim**, whose fail-open is correct only for
the "not yet engaged" case:

| Situation | Was the action ever governed? | Behaviour | Tested |
|---|---|---|---|
| plugin not mounted / `AI2RULES_DISABLE=1` | no — gate never in the path | run ungoverned (the not-activated case) | ✅ `BYPASS` |
| mounted, but `harness` binary missing / not executable | yes | **DENY** (fail closed) | ✅ `FAIL-CLOSED: binary missing` |
| gate exit 2 (unreadable/uncompilable world) | yes | **DENY** | ✅ `FAIL-CLOSED: exit 2` |
| gate exit 1 (internal error) | yes | **DENY** | (same code path as exit 2) |
| unparseable stdout | yes | **DENY** | (same deny path) |
| unknown / unmappable decision | yes | **DENY** + ABI-mismatch note | (default arm) |
| taint/usage sidecar cannot be persisted | yes | **DENY** the call it would have charged | (D59 applied to budgets) |

Once mounted, **every** path that cannot produce a trustworthy `ALLOW` denies the
governed effect. That is the property #54's review said the benchmark could not yet
claim; here it is implemented and tested.

Discovery fails closed independently: a missing projector, malformed response, or
uncompilable world produces an **empty tool surface**. It never falls back to dsh's
unfiltered registry.

## Discovery projection — `ABSENT` before the model

DeepSeek's implemented architecture makes `system-prompt/assemble` the single
authoritative interception point for prompt sections and wire tool schemas. The
adapter calls the host-neutral projection wire operation with the schemas from
that exact assembly:

```json
{"v":1,"context":{"source_channel":"user_cli"},"tools":[{"name":"read","description":"…","parameters":{}}]}
```

`harness project --world W` compiles `W` through the same loader and root
normalization as `harness gate`, keeps only names in `world.projected_actions()`
that the declared source channel's compiled capability matrix can see, and
returns those host-owned schema values unchanged plus `manifest_hash` and a
SHA-256 `schema_hash` of the visible array. The world owns **existence**; dsh's
registry owns its operational schema. Parsing or regenerating dsh schemas in the
adapter would create a second tool-definition engine.

Projection runs on every assembly. A world file change therefore changes the next
visible surface. A model may retain an earlier schema in its context, but the
current `tools/pre-execute` call recompiles the current world and denies the stale
name as `ABSENT`; discovery never grants authority.

Code Mode also publishes an SDK catalog in the `tools:sdk` prompt section. The
adapter does not parse or rewrite host prompt text. Unless the world explicitly
projects dsh's reserved `run_code` transport, it removes that SDK and transport
instruction; code-only presentation consequently narrows to an empty surface,
while `both` retains projected native tools. This is a deliberate fail-closed
degradation, not a claim that an unstructured SDK was safely filtered.

## The seven-step host-adapter contract (Gate ABI §6)

The plugin does exactly the host-neutral steps, nothing more:

1. Receive the `tools/pre-execute` event.
2. Restore monotonic taint + budget usage for `session_id` from the sidecar
   (`AI2RULES_STATE`; defaults: taint `clean`, usage all-zero).
3. Build a `GateRequest` — **raw** dsh tool name as `tool`, `exec.arguments`,
   `mode`, explicit `taint` + `source_channel` + `usage`. (Path-scoped file
   roots are not enabled in this world, so `path` is `null`; adding roots is a
   world change, not an adapter change.)
4. Run `harness gate --world deepseek-world.yaml` with the request on stdin.
5. Persist `response.context.taint` (monotonic; never lowers).
6. Persist `response.context.usage` — a counter that fails to persist fails the call.
7. Map `response.decision` → `PreToolDecision`; **fail closed** on any error.

Command classification (`bash` → `bash_network` / `bash_destructive` /
`bash_unclassified`) is the **kernel's**, driven by the world's `command_classes`
(D36) — the adapter sends the raw `bash` name and reads back `response.action`.

## Correlation evidence

Every decision emits one write-through line tagged `[ai2rules-gate]`:

```
[ai2rules-gate] session=S1 tool=bash -> ALLOW action=bash_network rule=none manifest=6d48cdef96eb
[ai2rules-gate] session=S1 tool=web_fetch -> DENY action=web_fetch rule=no_tainted_network manifest=6d48cdef96eb
[ai2rules-gate] session=S1 discovery offered=10 visible=9 absent=1 manifest=6d48cdef96eb schema=98d3c40b21aa
[ai2rules-gate] session=S1 tool=read -> DENY (fail-closed): governance gate unavailable …
```

Discovery evidence binds session → offered/visible/ABSENT counts → manifest and
schema identities. Invocation evidence binds session/tool → GateRequest → verdict
(decision, effective action, rule, the same manifest hash) → host decision →
effect/no-effect.

## Running the spike

Build the kernel, then run the spec against it (the spec self-skips if the two env
vars are unset, so it never runs in unrelated CI):

```bash
# 1. build the gate binary from this repo
cargo build -p cli-harness            # -> target/debug/harness

# 2. drop the adapter + spec beside the dsh runner's tests at the pinned commit
#    (deepseek-harness @ 141eb6f), then:
AI2R_HARNESS=<ai2rules>/target/debug/harness \
AI2R_WORLD=<ai2rules>/docs/demos/deepseek-harness/deepseek-world.yaml \
  npx vitest run packages/extensions/cordis-host-runner/tests/ai2rules-gate.spec.ts
# → 13 passed (invocation mapping/outage + discovery/revocation/Code Mode/outage)
```

To govern a real dsh session instead of the test harness, add the plugin to the
composition (e.g. a `cordis.yml` entry) with `AI2RULES_HARNESS` and
`AI2RULES_WORLD` set. `bash` and the other actions are governed per call; the
model's own tool loop is untouched.

## Cross-host acceptance (issue #55) — status

- [x] **One compiled `WorldManifest` governs dsh and another host.**
      `deepseek-world.yaml` uses the same lowercase tool vocabulary as the shipped
      OpenCode adapter, so the identical compiled artifact governs both; its D36
      patterns are now pinned with every host manifest in `tests/one_kernel.rs`.
- [x] **`ABSENT` is removed before prompt delivery.** The real dsh assembly is
      filtered on every step; the test changes the configured world, proves the
      next assembly drops `read`, then proves an invocation from the stale
      assembly still receives binding `ABSENT` with no effect.
- [x] **Projection evidence is joinable.** Discovery and invocation record the
      same 12-hex manifest identity; discovery additionally records the exact
      visible schema hash.
- [x] **The dsh adapter is translation/plumbing only** (no independent rules, no
      duplicate shell classification; imports nothing from dsh).
- [x] **A kernel `DENY` produces no downstream effect** even where dsh-native
      policy would allow — verified against the real dsh registry pipeline.
- [x] **Gate-outage behaviour is explicit and tested** (fail-closed table above).
- [x] **Routes tested through the adapter:** ordinary tool (`read`/`write`),
      shell/subprocess-backed (`bash`, kernel-classified), and a serialized
      sub-call path shares the same `tools/pre-execute` seam (the registry runs
      the waterfall for sub-dispatches too — see the #54 E5 probe).

### Residual host mismatches (not hidden by the completed slice)

- **Code-only presentation degrades to an empty surface.** dsh puts the native
  catalog inside generated SDK prose, not in structured `PromptAssembly.tools`.
  The adapter removes it instead of parsing host text. A usable *filtered* Code
  Mode needs dsh to expose the SDK's source schemas structurally at the assembly
  waterfall; direct Code Mode sub-dispatches remain governed by pre-execute.
- **`REPLAN`** has no faithful dsh pre-execute mapping (denied best-effort); a
  faithful mapping would need a host primitive that returns a non-effecting
  "continue the loop without executing", recorded as an ABI/host mismatch.
- **Path-scoped roots** (`context.path`) are not enabled in this world; a
  filesystem-roots world is a manifest change, then the adapter must attach a
  symlink-aware canonical absolute path for file actions.
- **`MCP` path** and **background jobs / subagents** beyond the shared
  `tools/pre-execute` seam are not separately exercised here (handed to #54's
  mandatory-path follow-up).
