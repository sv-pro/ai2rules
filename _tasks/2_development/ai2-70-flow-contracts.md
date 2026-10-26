# Task: typed source/sink flow contracts in the kernel (AI2-70 / #151)

**Owner:** kernel (build). **Status:** ✅ **v0 built** (`claude/ai2-70-flow-contracts`, D95) —
the build end of the WebMCP lab's D3 result ([D94](../../DECISIONS.md), `labs/webmcp/CONTRACTS.md`
§8.4). Per-value `FlowLabel` (integrity × confidentiality) → opt-in per-action `flow_contract` on
the `Descriptor` → `check_flow` / `result_label_floor` as step 5b of `IRBuilder::build`, before the
taint floor → `context.flow` on the gate ABI, additive in v1 → a scripted flow-capable host proving
the slice with counted effects. 48 new tests (646 workspace, green on Windows and Linux); the
benchmark results, the demo markers and every pre-D95 hash are byte-identical.

## Why

The kernel's taint is one integrity bit per **session**. D94's lab measured the cost: after
the agent reads anything sensitive, every write and external action becomes ABSENT — which
stops exfiltration (S5) but breaks the benign task (`crm-gift` places no order) and still misses
a leak through a *permitted read* sink (S5b). The bit cannot say "this value is private but
came from a trusted place". D3 showed that labelling **values** recovers all of that utility and
closes S5b with no security regression — as a JavaScript heuristic in the adapter. This task
makes it real in the kernel, as a typed, opt-in contract, independent of any demo.

## The contract

### 1. Labels — `FlowLabel { integrity, confidentiality }`

`Trusted|Untrusted` × `Public|Private`. A product lattice (no `Ord`); the only operation is a
pointwise monotonic `join`. No declassify, no endorse, no `Default`: an unknown label is `None`,
and every consumer falls back to the conservative **ambient** label, never to Trusted/Public.

### 2. Manifest — `flow_contract:` on a base action (opt-in; absent ⇒ today's behavior)

```yaml
- name: read_document            # a SOURCE: a floor the host may only raise
  flow_contract:
    output: { integrity: Trusted, confidentiality: Private }
- name: summarize_fixture        # a TRANSFORM: the result inherits its body's label
  flow_contract:
    inherits: [body]
- name: publish                  # a SINK: body must be Public to enter it
  flow_contract:
    sinks:
      body: { confidentiality: Public }
```

Carried on the `Descriptor`, so a contract is descriptor identity (changing one is drift) while
its absence is byte-invisible (pinned). Scoped capabilities inherit their base's contract. The
compiler refuses a contract that names undeclared arguments, a sink that bounds nothing,
duplicate `inherits`, a contract that declares nothing, and a `Trusted` output on an MCP-backed
action (a remote result is untrusted by construction, finding #13).

### 3. Host context — `FlowContext { ambient, arg_labels }` inside `TaintContext`

The host's word, attached with `TaintContext::with_flow`, which joins the labels' integrity axis
into the binary taint one-way (an Untrusted ambient taints the session; an Untrusted argument
taints that argument). Required **per action**: the kernel demands it exactly when the effective
action carries a contract (`missing_flow_context`), as `missing_path` and `missing_usage`.

### 4. Kernel — admission before the floor, result label after

`check_flow` judges every sink against the label bound to it (ambient fallback; `Literal`-pinned
skipped; renamed inputs under the actor's name), confidentiality first, then integrity, as step
5b of `build` — inside representability, so a refusal is `NotRepresentable` and no manifest
policy or approval reaches it. `result_label_floor` = `output` ⊔ inherited labels, `None` when
the contract declares no result. The gate joins `Untrusted` into the result for a remote backing
or a `taint_source` path.

### 5. Gate ABI — `context.flow` in, `context.flow { ambient, result }` out (v1, additive)

Absent from the wire unless carried. Rules: `missing_flow_context`, `invalid_flow_context`,
`confidentiality_violation`, `integrity_violation`. `result` is `null` on anything but ALLOW.

## Design decisions (the real choices)

| # | Decision | Why |
|---|---|---|
| A | Flow context rides **inside `TaintContext`**, not on `EvalContext`. | `TaintContext` has private fields and builders and is already `build`'s argument; `EvalContext` has public fields and fifteen struct-literal constructors across eight crates. Zero callers change. |
| B | Required context is **per action**, not per world. | One contracted action must not break every host on every uncontracted action. Mirrors `missing_path` / `missing_usage`. |
| C | The declared `output` is a **floor the host may only raise**. | "One whole-result label" without a per-fixture classifier. A world with public and private stores declares two source actions. |
| D | A contract with no `output` and no `inherits` yields `result: None`. | Unknown is explicit, never implicit Trusted/Public. |
| E | `with_flow` **joins** integrity labels into the binary taint, never replacing a caller-set taint. | Reaches the existing floor for "Untrusted authority argument" through the gate; can never weaken a decision. |
| F | ABI stays **v1, additive**. | Both fields are `Option` and skipped when absent; a contracted action denies `missing_flow_context` for an old host — the §8 "fail-closed without a bump" precedent. |
| G | Confidentiality before integrity, both **before** `check_taint`. | A disclosure is reported as a disclosure even when the floor would also refuse; the two reasons never mask each other. |
| H | Confidentiality is **not** derived from `DataClass`; `Trusted` output on an MCP backing is a compile error; the gate joins `Untrusted` for remote/taint-source results. | Preserves the mandatory remote-ingress and path taint. |
| I | Scoped capabilities inherit the base contract; `Literal`-pinned args are skipped at check time. | Exactly as `arg_roles`. |

## Scope / non-goals (v0)

- **In:** the label algebra; opt-in contracts; per-action required context; independent
  admission before the floor; the result floor; the ABI sidecar; the vertical slice with counted
  effects; wire and WASM parity; hash stability pinned by test.
- **Out:** default-world rollout; a CaMeL/IFC interpreter or any inference engine;
  declassification; a readers/principals lattice; per-field labels; flow sidecars on
  cc-hook / agy-hook / mcp-gateway (they send none and fail closed on contracted worlds);
  govbench schema changes; UI.

## Crate changes

- **harness-types:** `flow.rs` (`Integrity`, `Confidentiality`, `FlowLabel`, `FlowAxis`,
  `SinkConstraint`, `FlowContract`, `FlowContext`); `TaintContext::{with_flow, flow}`;
  `BaseActionDef.flow_contract`, `Descriptor.flow_contract`, `CompiledWorld::flow_contract`;
  `BuildError::{MissingFlowContext, FlowViolation}`.
- **compiler:** carry the contract into both `Descriptor` constructors; `validate_flow_contract`;
  hash-stability and validation tests.
- **world-kernel:** `invariants::{check_flow, result_label_floor}`; step 5b in `build`;
  `IntentIR::result_label`; `classify` arms; `tests/flow_contract.rs`.
- **harness-preview:** `GateLabel`, `GateFlowContext`, `GateFlowResponse`; `parse_flow`;
  post-call result/ambient; four reason strings; preview-matrix columns labelled;
  `tests/flow_slice.rs` (the `FlowHost`).
- **cli-harness:** `flow: None` in every host constructor; `one_kernel.rs` flow parity.
- **scripts / blog:** `check-wasm-freshness.mjs` `flow_contract` case; rebuilt WASM artifact.
- **docs:** `harness-gate-abi.md` §3/§4/§6/§8; D95; GLOSSARY; PLAN E2.7; README/AGENTS counts.

## Acceptance

- A Private value cannot be published even by a trusted caller in a clean session, with **zero
  observed effects** on the host's ledger, before and after the human says yes
  (`private_body_cannot_be_published`, `ask_does_not_erase_labels`).
- A clean publish still runs **exactly once** (`public_body_is_published_exactly_once`).
- A transform with a Private parent yields a Private result; claims from the model or a tool
  cannot elevate; missing or malformed context fails closed; the run is deterministic.
- Worlds without contracts keep identical decisions and identical manifest / descriptor /
  compiled hashes (`absent_flow_contract_keeps_pre_d95_hashes_stable`,
  `default_world_declares_no_flow_contract`; benchmark and demo guards byte-identical).
- Parity: `every_entry_point_agrees_on_flow_contracts` (in-process vs `harness gate`, every OS)
  and `check-wasm-freshness.mjs` (native vs WASM).

## The honest limit

v0 labels are **whole-result** and **host-bound by digest**. A value the model transforms or
partially quotes is a new value; re-binding it is the host's problem, not an inference the
kernel makes. D94's caveat does not disappear — it moves to where it belongs. And once a
session has read something private, an *unlabelled* value is private too: the host must say,
itself, that what the user typed is Trusted/Public (`an_unlabelled_value_in_a_private_session_is_private`).

## Related

[D94](../../DECISIONS.md) · `labs/webmcp/CONTRACTS.md` §8.4 · `labs/webmcp/docs/TAKEAWAY.md` ·
[roots-path-scope](roots-path-scope.md) (the other opt-in axis, same fail-closed shape) ·
`docs/demos/one-kernel/{flow-world,flow-cases}.yaml` · Linear AI2-70 · GitHub #151.
