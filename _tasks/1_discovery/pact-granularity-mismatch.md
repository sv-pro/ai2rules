# Discovery: PACT — the granularity mismatch is in our own taint floor

**Paper:** Fan, Li, Tian, Wang, Li, Wang (2026), *"The Granularity Mismatch in Agent Security:
Argument-Level Provenance Solves Enforcement and Isolates the LLM Reasoning Bottleneck."*
[arXiv:2605.11039](https://arxiv.org/abs/2605.11039)

**Status:** ✅ **implemented** — the L2 fix designed below is **merged to `main`** (squash
`977330c`, "PACT L2 argument-level taint — carrier + data-flow producer"). This document is now the
discovery record for a shipped feature; the "Next steps" checklist reflects what landed and what
remains.

**Runnable proof (the original witness, standalone L2 vs the real flat floor):**
`cargo run -p world-kernel --example pact_witness --offline`

**The shipped behavior** (kernel + orchestrator now do this for real):
`cargo test -p world-kernel l2_ && cargo test -p agent-core l2_producer`

---

## The claim, in one line

Indirect prompt injection is dangerous not when untrusted content is *in context*, but when it
**determines an authority-bearing argument**. So a monitor that decides per *tool call* is at the
wrong granularity; the right unit is the *argument*.

## Why it lands on us specifically

Our taint floor is, almost verbatim, PACT's **Definition 1 (flat tool-level monitor)**:
one label per call, block iff the action is externally effectful and the value is tainted.

| PACT | ai2rules | Location |
|---|---|---|
| flat monitor: one label per call | `check_taint(action, taint, side_effect)` | `crates/world-kernel/src/invariants.rs:21` |
| "the value is tainted" = one scalar | `TaintContext { taint: Taint }` (a single scalar) | `crates/harness-types/src/provenance.rs:142` |
| "any tainted input taints the call" | `from_outputs` = monotonic **join** of every prior output | `crates/harness-types/src/provenance.rs:155` |
| block iff externally effectful | `externally_effectful(side_effect)` | `crates/world-kernel/src/taint.rs:22` |
| the whole-intent taint fed to the check | `let taint = taint_context.taint()` then `check_taint(...)` | `crates/world-kernel/src/intent.rs:129` |

**PACT Theorem 3:** any flat monitor must incur a false positive or a false negative in a
mixed-trust environment. **PACT Theorem 4:** refining L0→L3 (argument roles) monotonically
shrinks the blocked set while preserving the authority-binding property. We sit at ~L1
(one threshold for the whole capability); L2 is *proven* strictly better.

## The witness — and it's already in our flagship demo

`poisoned_knowledge_demo` session 2 runs three steps after retrieving a poisoned doc:

- **step 2** — `fetch_web` to `http://attacker.evil/...` (the exfil). Correctly denied.
- **step 3** — the *identical* `fetch_web https://docs.example/guide` from session 1, with a
  **hardcoded clean constant URL**. Also denied — purely because the ambient session is tainted.

The demo frames step 3 as the feature ("same call, opposite verdict"). Through PACT's lens **step 3
is a false positive**: the authority-bearing argument (`url`) is clean-origin; only the *ambient*
context is tainted. The flat monitor blocks it because `TaintContext` is a scalar join — it cannot
tell "the tainted value bound the URL" (step 2) from "the URL is a clean constant, the session is
merely tainted" (step 3).

`examples/pact_witness.rs` drives the **real kernel** on all three and runs a minimal L2 check on
the per-argument provenance the flat monitor discards:

```
scenario                           flat       L2           note
1. legit fetch, clean session      ALLOW      ALLOW        both allow — agree
2. exfil: URL derived from poison  DENY       BLOCK        both block — agree (no false negative)
3. legit fetch, tainted session    DENY       ALLOW        ◀ FLAT BLOCKS, L2 ALLOWS — Theorem-3 witness
```

## Answering the task's central question

**Does adopting PACT put an LLM in the trust path (violating THESIS §3)?** Read the paper (§3.4):

- **Enforcement** (Algorithm 1) is deterministic and design-time — "only lattice comparisons, set
  intersections, and certificate-scope checks at execution time." **No LLM.**
- **Deployment inference** *does* use "an LLM classifier for remaining ambiguous arguments" at
  runtime — a §3 violation — costing them **87.1% role / 77.4% provenance accuracy** on 20 MCP tools.

**The move (PACT's own §3.5): adopt the enforcement layer, refuse the inference pipeline.** Our
roles come from the human-authored `WorldManifest` at design time — contracts are specified *by
construction*, so PACT's 77.4%-accuracy weakness is exactly the move we already make. Verdict:
**adoption candidate, not a §8 contrast case** (unlike CaMeL/AGT).

## What L2 costs us — the real finding

The gap is **structural, not a missing `if`**. Per-argument provenance is destroyed *before*
`build()` ever runs: `TaintContext` carries one scalar, and `ToolCall.arguments` is an opaque
`serde_json::Value` with no per-key lineage. To reach L2 we need to:

1. **Carry per-argument taint** — thread a `arg_path → Taint` map (or `TaintedValue`-per-arg)
   alongside the scalar, computed where outputs are joined today. The scalar stays as the L0/L1
   fallback (PACT's fail-closed default when arguments are unavailable — Theorem 4 keeps that safe).
2. **Assign roles at design time** — add a `role` to each argument in the manifest schema
   (`crates/compiler/assets/default_world.yaml`); `target`/`command`/`credential` are
   authority-bearing. This is a manifest field, not code, and not an LLM.
3. **Check authority-bearing args only** — the L2 rule in the example: externally effectful **and**
   some authority-bearing argument tainted → block. Content/selector taint flows freely.
4. **Preserve taint on outputs** (OutputSpec) so a stored/returned tainted value stays tainted —
   this is what makes allowing tainted *content* safe: it cannot later bind an authority-bearing arg.

Note the plumbing is half-built: E7/D16 scoped capabilities (`scoped_capabilities` in the default
world) already pin authority-bearing args as design-time literals (`command: !Literal pytest`).
That's argument-level *constraint*; L2 adds argument-level *taint contract*.

## The honest caveat

The flat monitor's block is **sound** — no false negatives; it over-approximates because it cannot
*prove* the URL is clean-origin. L2 is only as good as the per-argument provenance feeding it, and
today that provenance is thrown away. So this is not "invariant 7 is wrong" — it is "invariant 7 is
maximally conservative because it is starved of the data that would let it be precise." Recovering
that data is the experiment.

## Status of the plan (merged in `977330c`)

- [x] **Per-argument carrier** — `TaintContext` gained an `arg_taint` map (`with_arg_taint` /
      `arg_taint`); the ambient scalar stays as the L0/L1 fallback. (`harness-types/src/provenance.rs`)
- [x] **`role:` as manifest data** — `ArgRole` on `BaseActionDef`/`Descriptor`; `fetch_web` opts in
      with `url: Target`. Not code, not an LLM. (`harness-types`, `compiler`, `default_world.yaml`)
- [x] **L2 check in `world-kernel`** — `effective_floor_taint()` feeds the **unchanged** floor rule a
      per-argument value; `IntentIR.floor_taint` keeps disposition consistent. The floor stays the
      non-overridable L0/L1 physics; L2 refines *within* it. (`world-kernel/src/invariants.rs`)
- [x] **Data-flow producer** — `agent-core::arg_provenance` computes `arg_taint` from real perceptions
      (the deterministic, no-LLM half of PACT §3.4); wired into `run()` via `SessionConfig.user_request`.
- [x] **Regression tests** — 9 kernel tests (`l2_*`, incl. the step-3 recovery through `decide()`) +
      7 producer tests (`arg_provenance` + `l2_producer_recovers_*` through `run()`). The demo's
      false-positive is now an explicit, passing assertion.

**Remaining (not yet built):**

- [ ] **Transformation-aware provenance.** The producer only catches *verbatim* data flow;
      paraphrased/summarized tainted content falls through to the ambient floor (that is exactly the
      case PACT hands to its LLM classifier, and we refuse it). A deterministic step up: track taint
      through the executor's actual value transformations rather than string matching.
- [ ] **OutputSpec read-taint** so memory/MCP can opt into L2. Today they stay on the ambient floor —
      relaxing tainted→memory before taint is preserved on *read* would be a real false negative (the
      ZombieAgent threat).
- [ ] **Promote to `PLAN.md`** as a real epic, and **measure** on a benign-mixed-trust suite: false
      positives recovered vs. false negatives introduced (target: >0 recovered, 0 introduced).

## Flywheel advocacy hook

The blog post writes itself and it's honest about our own code: *"We found a false positive in our
own flagship demo."* Ties directly to the published `the-zombieagent-threat` post — the flat floor's
answer to ZombieAgent ("never store tainted data") is the over-block; PACT's ("store it, keep the
taint, block only the authority-bearing use") is the utility recovery.
