# One kernel, many hosts

Status: **shipped**, 2026-07-12. Decisions: **D24** (gate ABI), **D34**
(in-process vs wire), **D35** (OpenCode), **D36** (manifest-declared command
classification), **D37** (Claude Code live-hook cutover). Verified by
`crates/cli-harness/tests/one_kernel.rs` against
`docs/demos/one-kernel/{demo-world.yaml,cases.yaml}`; demonstrated by
`scripts/demo-one-kernel-many-hosts.sh`.

The thesis this increment makes real and testable: **every host decides through
the one Rust kernel**, reached through thin adapters —

```
Claude Code ─┐
OpenCode    ─┼─→ thin adapter → GateRequest → one Rust kernel
MCP Gateway ─┘                        ↓
                                 GateResponse
```

```
host event → thin host adapter → GateRequest → harness_preview::gate()
→ world_kernel::decide() → GateResponse → host-specific response
```

## Where the single governance implementation lives

- **`world-kernel`** — `decide()`: ontology (ABSENT), capability matrix, the
  taint × side-effect floor, approval (ASK, collapsing to DENY in background),
  budgets. Pure; no I/O.
- **`harness-preview`** — `gate()` (the D24 request/response mapping, post-call
  monotonic taint, and since D36 the **effective-action classification**) and
  `host.rs` (`host_outcome()`: the one verdict→host-obligation mapping,
  `ABSENT`/`DENY`/`REPLAN` kept distinct, unknown verdicts fail closed).
- **The compiled `WorldManifest`** — actions, side effects, approval flags,
  transition policies, **and `command_classes`** (D36): the bash-shape pattern
  lists are world *data*, byte-identical across hosts (pinned by test).

## What stays host-specific (all of it shape, none of it policy)

A host adapter may only: translate the host event to/from the ABI, restore and
persist session taint, pass the execution mode, call the real kernel, and apply
its documented fail-open/fail-closed strategy.

| Adapter | Translation it owns |
|---|---|
| `harness cc-hook` (Rust, in-process `gate()`) | PreToolUse JSON ↔ `permissionDecision`; tool-name normalization (exact ontology name, else lowercase, else unchanged); taint sidecar `.claude/state/taint-<sid>`; `--mode`; `--enforce-absent` |
| `.opencode/plugin/ai2rules-gate.ts` (TS, wire ABI `harness gate`) | `tool.execute.before` ↔ throw-to-block; taint in `.opencode/ai2rules-state.json`; `AI2RULES_MODE` |
| `harness mcp-gateway` (Rust, in-process `gate()`) | MCP `tools/list` shaping (ABSENT never offered) + `tools/call` ↔ `isError` with the decision label; in-process monotonic session taint; `--mode` |
| `harness gate` (CLI) | stdin/stdout JSON marshalling only |

## Duplication survey (before → after this increment)

| Location | Language | Responsibility | Duplicated? | Action taken |
|---|---|---|---|---|
| `.claude/hooks/world-gate.py` | Python | full gate: ABSENT, taint floor, ASK, trust pins | **yes — a second engine** | archived to `superseded/`; file is now a 15-line bootstrap shim exec'ing `harness cc-hook` (D37) |
| `.claude/hooks/_gatelib.py` | Python | taint ledger + trust pins | yes | archived; trust pins consciously dropped until a typed manifest field lands |
| `.claude/hooks/world-gate-adapter.py` | Python | D26 adapter POC | superseded | archived |
| `.claude/cc-world.json` | JSON | bespoke world schema | yes | archived; `.claude/cc-world.yaml` (real manifest) is the world |
| `cc_hook.rs` `classify()`/`word_match()` + pattern consts | Rust | bash-shape classification | **yes — copy #2** | removed; kernel classifies from `command_classes` (D36) |
| `ai2rules-gate.ts` `classify()`/`wordMatch()` + pattern consts | TypeScript | bash-shape classification | **yes — copy #3** | removed; plugin sends the raw tool name |
| decision→host mapping in `cc_hook.rs` / `mcp_gateway.rs` | Rust | verdict handling | drifting strings | unified behind `harness_preview::host_outcome()` |

## Fail-open vs fail-closed (explicit, per adapter)

A **process failure is never an outcome** — an adapter that couldn't evaluate
does not synthesize a verdict; it applies its documented strategy:

| Entry point | On process failure | Why |
|---|---|---|
| `harness cc-hook` | **fail-open** (exit 0, no output) | a broken hook must never brick a live host session |
| OpenCode plugin | **fail-open** (warn + allow) | same; only an explicit kernel verdict blocks |
| `harness mcp-gateway` | **fail-closed** (an unevaluated call is never forwarded upstream) | the gateway *is* the surface; nothing passes around it |
| `harness gate` CLI | exit codes `0/1/2` report evaluation vs process error and **never encode a verdict** (D24) | verdict→convention mapping is the adapter's job |

On the verdict channel itself, an **unknown decision string** maps to
`Block{Deny}` (fail-closed) in `host_outcome()`.

## The parity guarantee

Host worlds are separate manifests (`cc-world.yaml`, `opencode-world.yaml`,
`demo-world.yaml`), so hashes *across worlds* differ by design. The guarantee
is: **same manifest + same request ⇒ same decision / rule / post-call taint /
manifest_hash on every entry point** — in-process `gate()`, the `harness gate`
CLI, the cc-hook event contract, the OpenCode wire shape, and the MCP gateway
(`tests/one_kernel.rs`).

## Limitations (this increment)

- **No OS sandbox / physics floor here** — the kernel decides; the E13.7
  container + egress proxy remains the enforcement backstop (D21).
- **Trust pins (D29) are not in the compiled world** — dropped at cutover until
  a typed `trust_pins` manifest field lands in the kernel.
- **No path-based read-taint yet** — the archived `demo-injection-egress.sh`
  depended on it; taint now enters via network/MCP/external outputs (D25's
  recorded follow-up).
- **Claude Code's native seam cannot make tools ABSENT** — a PreToolUse hook
  can't remove tools from the surface, hence `--enforce-absent` (deny with an
  `ABSENT:` prefix) as an explicit opt-in; default stays additive.
- **OpenCode has no structured ask channel** — ASK surfaces as a block (throw);
  pair with OpenCode `permission` rules for an approval UX (D35).
