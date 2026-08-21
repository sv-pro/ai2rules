# DeepSeek Harness × ai2rules — `tools/pre-execute` gate adapter (issue #55)

**The point:** the *same* deterministic kernel and Gate ABI that govern Claude Code
(`cc-hook`), OpenCode, and the MCP gateway also govern **DeepSeek Harness (dsh)
native tool calls** — through a thin cordis plugin that subscribes to dsh's
`tools/pre-execute` waterfall and calls `harness gate`. No policy, taint, or
command classification lives in the plugin; world policy stays in the compiled
`WorldManifest` and the kernel (the one-kernel / thin-adapter rule, D35).

This is the **P1 adapter spike** for [issue #55], the implementation companion to
the [#54 governor-integrity benchmark](../../benchmarks/deepseek-harness/). It
deliberately closes the failure-semantics gap that the #54 review flagged: an
**engaged** gate that becomes unavailable must **fail closed**, not fall through.

[issue #55]: https://github.com/sv-pro/ai2rules/issues/55

```
dsh model → tools/pre-execute waterfall
          → ai2rules-gate.ts  (build GateRequest, run `harness gate --world W`, map verdict)
          → external deterministic kernel + compiled deepseek-world.yaml
          → PreToolDecision (allow / deny / ask) back into dsh's own pipeline
```

## Files

| File | What it is |
|---|---|
| [`ai2rules-gate.ts`](ai2rules-gate.ts) | The cordis plugin. Subscribes to `tools/pre-execute`, builds a `GateRequest`, shells to `harness gate` (D34, out-of-process wire ABI), maps the verdict to dsh's `PreToolDecision`, and persists monotonic taint + budget usage. Imports **nothing** from dsh — the `apply(ctx)` contract is structural. |
| [`deepseek-world.yaml`](deepseek-world.yaml) | A **real** `WorldManifest` compiled by the same kernel as `cc-world.yaml`. Its `command_classes` block is copied byte-identical from the pinned host worlds (`cc-world.yaml` / `opencode-world.yaml` / the one-kernel demo world, which `tests/one_kernel.rs` pins); adding this world to that pin is follow-up. Action names are dsh's real registered tool names (`bash`, `edit`, `glob`, `grep`, `read`, `read_image`, `write`, `web_fetch`, `web_search`). |
| [`ai2rules-gate.spec.ts`](ai2rules-gate.spec.ts) | Runs the adapter against the **real** dsh `ToolRegistry` pipeline **and** the real `harness gate` binary + this world. 9 cases: the verdict mapping and the fail-closed contract. |

## Verdict mapping (issue #55)

The adapter maps existing ai2rules outcomes onto dsh's native pre-execute channel
without inventing a second policy vocabulary:

| Kernel decision | dsh `PreToolDecision` | Notes |
|---|---|---|
| `ALLOW` | `next()` → `{kind:'allow'}` | dsh runs the tool |
| `DENY` | `{kind:'deny', reason}` | binding rejection before any effect |
| `ASK` | `{kind:'ask', reason}` | resolved by dsh's own `ctx.approval` one-shot prompt (the narrowest primitive that preserves exact-call binding) |
| `ABSENT` | `{kind:'deny', reason}` | invocation denied. Shaping model-visible **discovery** from the same world (so the model never sees the tool) is the #55 discovery follow-up, not this invocation slice. |
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
[ai2rules-gate] session=S1 tool=read -> DENY (fail-closed): governance gate unavailable …
```

session/tool → GateRequest → verdict (decision, effective action, rule,
manifest hash) → host decision → effect/no-effect.

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
# → 9 passed  (ALLOW×2, DENY, ASK allow/reject, ABSENT, FAIL-CLOSED×2, BYPASS)
```

To govern a real dsh session instead of the test harness, add the plugin to the
composition (e.g. a `cordis.yml` entry) with `AI2RULES_HARNESS` and
`AI2RULES_WORLD` set. `bash` and the other actions are governed per call; the
model's own tool loop is untouched.

## Cross-host acceptance (issue #55) — status

- [x] **One compiled `WorldManifest` governs dsh and other hosts.** `deepseek-world.yaml`'s
      `command_classes` is copied byte-identical from the pinned host worlds
      (`cc-world.yaml` / `opencode-world.yaml`); equivalent normalized actions get
      equivalent kernel decisions independent of host. (Adding this world to the
      `tests/one_kernel.rs` conformance pin is follow-up.)
- [x] **The dsh adapter is translation/plumbing only** (no independent rules, no
      duplicate shell classification; imports nothing from dsh).
- [x] **A kernel `DENY` produces no downstream effect** even where dsh-native
      policy would allow — verified against the real dsh registry pipeline.
- [x] **Gate-outage behaviour is explicit and tested** (fail-closed table above).
- [x] **Routes tested through the adapter:** ordinary tool (`read`/`write`),
      shell/subprocess-backed (`bash`, kernel-classified), and a serialized
      sub-call path shares the same `tools/pre-execute` seam (the registry runs
      the waterfall for sub-dispatches too — see the #54 E5 probe).

### Deferred (honest follow-up, not claimed done here)

- **`ABSENT` discovery shaping.** This slice denies ABSENT at *invocation*; shaping
  the model-visible tool schema from the same compiled world (so an absent tool is
  never offered, and revocation stays consistent between discovery and invocation)
  is the #55 "Discovery / ABSENT follow-up" and needs the dsh tool-schema/scoped-
  registry seam.
- **`REPLAN`** has no faithful dsh pre-execute mapping (denied best-effort); a
  faithful mapping would need a host primitive that returns a non-effecting
  "continue the loop without executing", recorded as an ABI/host mismatch.
- **Path-scoped roots** (`context.path`) are not enabled in this world; a
  filesystem-roots world is a manifest change, then the adapter must attach a
  symlink-aware canonical absolute path for file actions.
- **`MCP` path** and **background jobs / subagents** beyond the shared
  `tools/pre-execute` seam are not separately exercised here (handed to #54's
  mandatory-path follow-up).
