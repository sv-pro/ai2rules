# D76 — A governance benchmark publishes per-scenario evidence, and asserts its baseline still fails


**Context.** E18 needed a shape for the Public MCP Governance Benchmark's
executable seed. Governance benchmarks fail in two characteristic ways. They
produce a **score**, which lets a target trade a line it holds against a line it
loses. And their **baseline silently improves** — someone fixes the deliberately
weak reference implementation, every run goes green, and the suite keeps reporting
success while measuring nothing.

**Decision.** Four rules, enforced structurally rather than by convention:

1. **No aggregate score, anywhere.** Results are a scenario × target matrix of
   PASS/FAIL plus the observed downstream effect count. Neither `results.json` nor
   the generated report contains a total.
2. **PASS requires an observed decision, an observed effect count, *and*
   evidence the oracle actually reads.** The effect counter lives in the runner's
   mock upstream, not in either target, and is read before and after every step,
   so a target that answers `DENY` and calls the upstream anyway fails the second
   half. Evidence is the third, and it is checked structurally on every step of
   every run rather than left to each scenario: a refusal must name a rule, a
   grant must say what it covers, a call presenting a handle must say which
   identity it checked and — on a refusal — give a structured reason, and the
   ledger's record must be the call the target was handed. The checked identities
   are comparable across steps (`binding_distinguishes`), which is what turns
   "an approval is bound to the exact effect" into a check: a target binding to a
   tool name reports the same string for the approved call and for one with a
   mutated argument, and fails on its own evidence before any effect is counted.

   Without this the field was decorative. The first cut of the oracle read
   verdicts, visibility, handles and effect counts and never opened `evidence`,
   so a target could answer `{}` everywhere and pass all three scenarios — which
   is not what issue #64 asks for. `a_correct_verdict_without_evidence_is_not_a_pass`
   is the standing proof it now bites.
3. **Neither side of the judgement knows the other.** Scenarios are data; the
   oracle sees observations and expectations but no target identity; targets
   implement three operations and never learn which case is running. "Expected to
   fail here" is not expressible.
4. **Acceptance is two-directional, and requires the contrast to have run.**
   `--assert-contrast` (and the CI job) first insists on exactly one result for
   every `scenario × {weak, ai2rules}` cell, then fails when ai2rules fails a
   scenario *and* when the weak reference gateway passes one. Judging only the
   runs that happen to exist let `--target ai2rules --assert-contrast` succeed
   having never executed the baseline: an assertion about a comparison, passed
   without the comparison. The check lives outside the oracle (`accept.rs`),
   because it is the one layer that legitimately knows a benchmark has two
   targets and that one of them is meant to fail.

5. **The measured target is named for what it is.** ai2rules does not ship the
   consume-then-invoke boundary as a command, so the benchmark supplies it; the
   target is therefore `ai2rules-reference-host`, and its recorded identity
   carries a `composition` block naming what ai2rules ships and what the
   benchmark wired. A result proves that *composition* holds the line, not that a
   shipped command does. Calling it `ai2rules` would have been the single most
   load-bearing inaccuracy in the pack — precisely the overclaim the
   no-aggregate-score rule exists to prevent, one level up.

The full verdict vocabulary — `ABSENT`, `ALLOW`, `DENY`, `ASK`, `ERROR_CLOSED`,
`ERROR_OPEN`, `UNKNOWN` — is preserved in the schema even where no scenario yet
produces the last three, because a governor that *broke* must never be recorded as
a governor that *decided*.

**Alternatives.** (a) A single "governance score" per target: rejected as above; it
is the format that made every prior comparison unciteable. (b) Encoding expected
failures per target in the scenario files: rejected — the oracle would then be
grading against its own answer key, and a real regression in ai2rules would be
indistinguishable from an intended weak-baseline failure. (c) Letting each target
report whether an effect occurred: rejected; that is the one fact a target under
test must not be trusted for. (d) Skipping the weak baseline entirely and asserting
only that ai2rules passes: rejected — without a target that fails, a green suite is
evidence of nothing, which is precisely finding #18's lesson applied to evidence
rather than to artifacts.

**Consequence.** The seed pack is small (three scenarios) and honest about it: its
README states what it does not claim, including that ai2rules does not yet ship the
trusted authorization boundary as a command, so a host that omits ~40 lines of
wiring gets the weak gateway's third defect from a correct kernel. Rules 2 and 4
above were added in review (PR #69): both guarantees were true of the pack as
written but were asserted in prose rather than enforced, which is the same defect
class the benchmark measures — a control nothing checks is not a control. The committed
`results/` directory is a build output in git — the shape that let the WASM artifact
rot for seven weeks (finding #18) — so the `governance-bench` CI job regenerates it
and fails on drift.
