# Principles, Arbitrary Choices, and a Falsification Program

Companion to [`THESIS.md`](THESIS.md). The thesis *asserts*; this doc
*interrogates*. Three jobs:

1. **Distill the principles** the whole project rests on.
2. **Separate forced from arbitrary** — what the threat model dictates vs. free
   parameters we happened to pick.
3. **Make each falsifiable** — a program of **Trials** (experiments), **Proofs**
   (corroborating results), and **Rejects** (results that would kill or bound the
   claim). Popperian on purpose: a principle we cannot imagine rejecting is not a
   principle, it is a slogan.

Status legend per principle: **[Forced]** (follows from the threat model — abandon
it and the project's security claim collapses), **[Partly forced]** (the goal is
forced, the mechanism is a choice), **[Arbitrary]** (a defensible pick with live
alternatives), **[Unproven]** (a load-bearing *hypothesis* dressed as a principle).

---

## Part A — The principles

### P1 — No LLM in the trust path *(Design-Time Stochastic, Runtime Deterministic)*  **[Forced]**
The runtime enforcement decision is a pure function of `(intent, context, compiled
world)`; LLMs act only at design/ingestion time.
*Why forced:* if a stochastic classifier gates security it can be tricked, and the
guarantee is only as strong as the classifier is un-jailbreakable — i.e., not a
guarantee. **Caveat that bites:** the *location* of the "trust path" is itself a
choice (see C-bound below), and `context-engine`'s `brain-react` runs an LLM
online — so the knowledge layer *violates P1 as stated*. Either P1 is "no LLM
decides **effects**" (narrow, holds) or "no LLM in the loop" (broad, false). The
project currently equivocates. **Sharpen or the thesis leaks.**

### P2 — Closed action ontology; `ABSENT ≠ DENY`  **[Partly forced]**
Dangerous actions don't exist in the agent's world; *absence* is distinct from
*refusal* (invariants 2, 3).
*Forced part:* you cannot exploit a capability that was never projected — absence
is a real security primitive. *Arbitrary part:* the claim that ABSENT also yields
*better model behavior* than DENY (no refusal to argue with, no retries) is an
**empirical** claim, not an axiom. Collapsing the two is a viable design.

### P3 — Monotonic taint + a hard taint floor  **[Arbitrary]**
Taint only joins/increases (invariant 6); tainted data may never reach
Network/External/Credential/Memory/PersistentWrite surfaces (invariant 7).
*The arbitrary core:* **there is no declassification/endorsement primitive.** Real
information-flow-control systems let trusted code lower taint deliberately. Pure
monotonicity trades availability for safety — a legitimate "fetch web → summarize →
write summary" dies because the summary is tainted. The *absence* of a
declassifier is the single most consequential free choice in the project.

### P4 — Sealed intent (unforgeable `IntentIR`)  **[Partly forced]**
Only `IRBuilder::build` can construct an `IntentIR`; its existence is a proof that
representability checks passed (invariant 1).
*Forced part:* "validity-as-a-type" is the cleanest way to make "checks passed" a
non-bypassable fact. *Arbitrary part:* the *mechanism* (Rust module privacy) is
language-specific; the principle (a validity witness) is general.

### P5 — Provenance on every value  **[Forced]**
Origin, trust, lineage, content-id travel with each value; taint is meaningless
without it. *Arbitrary sub-choices:* **granularity** (per-value vs per-message vs
line-span) and the **trust lattice itself** (see C1).

### P6 — Capability projection: per-actor worlds  **[Partly forced]**
A tool surface is projected per actor, not exposed wholesale. *Forced:* least
authority. *Arbitrary:* the specific trust→action matrix (see C2).

### P7 — Determinism ⇒ replay & audit  **[Forced, given P1]**
Same `(inputs, world version)` → same decision (invariant 14); every decision is
logged and replayable. Falls out of P1 for free; the value is real and cheap.

### P8 — Nothing untrusted silently becomes authoritative  **[Forced]**
Knowledge layer: an investigation is descriptive; promotion to a normative
fact/rule is gated. Action layer: taint can't be laundered. Same principle, two
layers. *Arbitrary:* the promotion *thresholds* (see C5).

### P9 — Correctness > completeness  **[Arbitrary]**
Prefer "unknown" to a hallucinated/over-permissive answer. This is a **values
choice**, reversible — a recall-first system is a coherent alternative with a
different risk profile. It implies a measurable precision/recall trade we have not
quantified.

### P10 — One primitive kit governs all four layers  **[Unproven]**
The coherence claim: taint, sealed intent, ABSENT≠DENY, capability projection
govern actions, capabilities, knowledge, *and* intent. **This is the project's
central bet, and it is currently a hypothesis with one datapoint** (the
poisoned-knowledge demo). It is the most important thing to either prove or reject.

### P11 — The human reviews the manifest  **[Arbitrary / hidden assumption]**
"Design-time stochastic" is only safe if a human meaningfully reviews the
LLM-drafted manifest. If manifests are generated and rubber-stamped, the stochastic
process re-enters through the back door. This assumption is **unstated and
untested**, and it may be the softest point in the whole argument.

---

## Part B — Arbitrary-choice register (free parameters)

Each is a knob we set without forcing. Column 4 = what breaks if we set it wrong.

| # | Choice | Current value | Live alternatives | Cost of being wrong |
|---|---|---|---|---|
| C1 | Trust lattice cardinality | 4 (Trusted/SemiTrusted/Untrusted/Derived) | 2 (trusted/untrusted); a full lattice | Over- or under-fitting; dead distinctions |
| C2 | Trust→action capability matrix | SemiTrusted gets Write/Patch/Command; Untrusted only Read | any assignment | Too tight = unusable; too loose = leaks |
| C3 | Taint-floor effect set | Network, External, Credential, Memory, PersistentWrite | add/remove effects | Missing one = a real egress channel |
| C4 | **Declassification** | none (pure monotonic) | endorsement primitive, quota, human-declassify | False-deny death *or* a laundering hole |
| C5 | Promotion thresholds (knowledge) | human / multi-source / N-over-Y | tune N, Y; auto-promote | Knowledge stale *or* poisoned |
| C6 | Pseudo-model split | `brain-rag` vs `brain-react` | one adaptive model | Needless complexity or lost capability |
| C7 | ASK fails **closed** in background | deny + no token | fail-open with audit | Blocks legit automation *or* silent risk |
| C8 | Retrieval hyperparameters | chunk 500/overlap 50; sim≥0.27; PPR damping 0.5; synonymy τ 0.8 | any | Recall/precision swings; mostly tuning |
| C9 | Intent unit = Triad | Task\|Context\|Outcome (3 slots) | n-ary; free schema | Mis-models intents that aren't triadic |
| C10 | Approval-token binding granularity | exact call | per-action / per-session | Replay holes *or* approval fatigue |

C4 and C5 are the high-stakes ones; C8 is mostly tuning; C1/C2/C6 are likely
over-engineered until a trial says otherwise.

---

## Part C — The Trial / Proof / Reject program

Ordered by leverage. Tag: **[now]** runnable in this repo today; **[needs X]**
requires a fragment or corpus we don't yet have wired.

### T1 — Does the deterministic gate actually stop injection? *(tests P1, P3)* **[now]**
- **Trial:** replay a corpus of prompt-injection payloads through the harness loop
  (extend `poisoned_knowledge_demo` into a table-driven test); for each, assert no
  effectful action executes.
- **Proof:** 100% of injections that *attempt* egress/cred/memory resolve to
  DENY/ABSENT, decided by the pure gate.
- **Reject:** a single payload reaches a real effect through an *allowed* path →
  P1/P3 as implemented is incomplete; locate the leak channel (likely a missing C3
  effect).

### T2 — Is `ABSENT ≠ DENY` more than cosmetic? *(tests P2)* **[needs live model]**
- **Trial:** same task, two worlds — a dangerous tool ABSENT vs present-but-DENY.
  Measure retries, jailbreak attempts, tokens, task completion.
- **Proof:** ABSENT measurably reduces retries/jailbreak attempts at equal task
  success → the distinction earns its keep.
- **Reject:** no behavioral delta → collapse ABSENT into DENY; keep the audit
  distinction only.

### T3 — Does monotonic taint kill real work? *(tests P3, C4)* **[now]**
- **Trial:** run realistic dev workflows (web→summarize→write; mcp→patch) and
  measure the **false-deny rate** — legitimate tasks blocked purely by taint with
  no declassifier.
- **Proof:** false-deny rate low enough that the harness is usable as-is →
  pure monotonicity is justified.
- **Reject:** high false-deny rate → **monotonicity without declassification is
  rejected**; a controlled endorsement primitive (C4) becomes required, not
  optional. This is the most likely principle to fall.

### T4 — Does the primitive kit *transfer*, or just rhyme? *(tests P10)* **[needs context-engine wired]**
- **Trial:** drive the *same* primitive (taint/provenance) across all four
  resources with **no new mechanism** — the generalized cross-layer demo, with the
  real `context-engine` retriever behind MCP (the PLAN next-step).
- **Proof:** one taint rule governs action *and* knowledge with zero new code →
  coherence claim corroborated.
- **Reject:** a governed resource (likely *intent*) where taint is meaningless and
  a different primitive is required → P10 weakens to "shared vocabulary, not shared
  mechanism." State that honestly if so.

### T5 — Is the 4-level trust lattice doing any work? *(tests C1)* **[now]**
- **Trial:** ablation — collapse the lattice to {trusted, untrusted} and re-decide a
  representative trace set.
- **Proof:** some decisions flip → the extra levels carry information.
- **Reject:** no decision changes → the 4-level lattice is over-engineered; cut it.

### T6 — Does knowledge governance catch what recall misses? *(tests P8, P9)* **[needs corpus]**
- **Trial:** seed a corpus with poisoned/contradictory docs; compare
  `context-engine` (governed) vs HippoRAG-2-style recall on answer correctness.
- **Proof:** governance rejects poisoned facts that recall surfaces → the shell pays
  for itself.
- **Reject:** governance adds latency/complexity but catches nothing recall didn't →
  governance is theater here; demote it.

### T7 — Does the human actually catch a malicious manifest? *(tests P11)* **[needs study]**
- **Trial:** insert a subtly over-permissive fragment into an LLM-drafted manifest;
  measure reviewer catch-rate under realistic review time.
- **Proof:** reviewers reliably catch it → the design-time boundary holds.
- **Reject:** most miss it → "human reviews the manifest" is rejected as a safety
  assumption; the compiler needs *automated* manifest linting (a non-LLM check for
  over-permissioning), or P1's boundary leaks at design time.

### T8 — Is the determinism claim real under replay & drift? *(tests P7)* **[now]**
- **Trial:** replay the trace corpus against the same world (must reproduce) and a
  drifted world (must diff); the WASM↔native fidelity guard (E14.4) is the same
  assertion across runtimes.
- **Proof:** bit-identical verdicts on same world; clean diffs on drift.
- **Reject:** any nondeterministic verdict → a hidden stochastic input exists; find
  and seal it.

### T9 — Is the Triad the right intent unit? *(tests C9)* **[needs intent corpus]**
- **Trial:** encode a sample of real intents; count how many fit Task\|Context\|
  Outcome without distortion.
- **Proof:** high fit → Triad is a good primitive.
- **Reject:** frequent distortion → the 3-slot schema is arbitrary and lossy; widen it.

---

## Part D — Claims not yet falsifiable (sharpen before trusting)

- **"The layers compose."** Vague. Sharpened form is T4: *one primitive, four
  resources, zero new mechanism.* Until stated that way it's unfalsifiable.
- **"Governed recall = best of both."** Needs a metric (T6) or it's a slogan.
- **"No LLM in the trust path."** Ambiguous between "no LLM decides effects" and
  "no LLM in the loop" (P1). Pick one; they have different truth values.

---

## How this feeds the Flywheel

Each **Reject** is a Discovery item and a failing test; each **Proof** is a blog
post + a hardened invariant. The honest ones to lead with are the trials most
likely to *reject* something — **T3** (monotonic taint may be too brittle), **T7**
(human review may not hold), and **T4** (coherence may be vocabulary, not
mechanism). A research program that only runs trials it expects to pass isn't one.

The immediate runnable starts are **T1, T3, T5, T8** (all **[now]**); **T4** unlocks
once the PLAN next-step (real `context-engine` behind MCP) lands.
