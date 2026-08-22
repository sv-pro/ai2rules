# D52 — The detector benchmark is a separate repo: a fifth role in D40's topology


**Date:** 2026-08-02. **Amends** D40 (four-role topology) by adding a role rather than
bending an existing one. Constrained by THESIS §3 and by the PACT discovery
(`sv-pro/ai2rules#8`).

- **Context.** The binding constraint is witness, not depth — `STRATEGY.md`'s rule, and the
  0★ signal behind it. A new artifact was started to attack that directly: a benchmark for
  AI-text detectors (`detbench`). D40 says **one live public implementation**, so a second
  live public repo needs a ruling rather than a shrug. It genuinely fits none of D40's four
  roles: it is not the implementation, not the private control room, not publishing, and
  archiving it on arrival would be absurd.
- **Decision.** `sv-pro/ai-detector-bench` is **its own public repo**, and D40's topology
  gains a **fifth role — the outbound instrument**: live and public, built to be found by an
  audience that has never heard of the thesis, *not* part of the implementation, and never a
  dependency of it.
- **The one-way rule, which is the load-bearing half of this entry.** `detbench` may cite
  ai2rules; **ai2rules may never depend on `detbench`**; and **no detector output may ever
  become a kernel input.** A detector is a probabilistic classifier. Routing one into a
  verdict would put inference in the trust path — precisely what THESIS §3 forbids, and
  precisely what #8 refused when it took PACT's *enforcement* layer and rejected its runtime
  LLM classifier (87.1% role accuracy, 77.4% provenance accuracy). The benchmark exists to
  **measure** that failure mode. It must not import it.
- **Why this is on-thesis and not a detour.** Detection is the attempt to *recover*
  provenance that nobody recorded, by inference, after the fact. The thesis records
  provenance **at the boundary** — origin, trust, lineage, monotonic joins. So an honest,
  rigorous measurement of how badly inference-after-the-fact performs is evidence *for*
  recording-at-the-boundary. It is the counter-example, measured rather than asserted, which
  is the standard §6's flywheel sets for a discovery.
- **Alternatives rejected.**
  - *A crate or subdirectory inside `ai2rules`.* Honours D40 literally, but buries a
    general-audience artifact inside a Rust governance workspace, dilutes both stories, and
    drags Python into a workspace whose local plane is deliberately Python-free.
  - *Don't build it.* The demand adjacent to "ai detector" is orders of magnitude larger
    than the demand for "agent governance", and witness is the binding constraint.
  - *Under the `agentic-execution-governance` org.* That org is the **private** control room
    (D40 role 2). The entire point of this artifact is to be found.
  - *A hosted consumer verdict tool.* Rejected on the referee grounds below.
- **Referee, not competitor — and this constrains the build, so it is recorded here.** The
  didactic framing ("we built this to show detection can't work") is unpitchable: it reads as
  incompetence rather than as a finding. So the artifact must **compete on accuracy** *and*
  publish where every detector fails, its own included. Nobody can accuse the referee of
  being bad at the sport. What that forces into the code: the headline metric is TPR at a
  fixed, defensible false-positive rate rather than AUROC; the rate at which **human** writing
  is confidently called machine-generated is reported as a first-class column; refusal is a
  first-class result that cannot carry a score; and a raw score is not a probability until
  calibrated against a named distribution.
- **Python, deliberately — do not "consolidate" this later.** STRATEGY's *local plane = zero
  Python* governs what ships to a developer's laptop **as the harness**. `detbench` is
  neither the harness nor the local plane, and the detection ecosystem is Python. A future
  tidying pass that folds it into the Rust workspace on language-consistency grounds would be
  re-deciding this entry without noticing.
- **Known residual: this is a new live repo in a cluster whose documented failure mode is
  exactly that.** D38/D40 archived eight repos to stop the sprawl this could restart. So the
  kill condition is stated up front rather than discovered later: **if it has not drawn
  measurable outside attention by 2027-02-02, archive it** with a README pointer, per D40's
  own reasoning that an archived repo is a decision while a dormant one is a question every
  reader re-asks.
- **Known residual: nothing published yet may carry a number.** `binoculars.py` and
  `fast_detectgpt.py` implement the published algorithms but are **unvalidated** against
  their reference implementations, and no real corpus (RAID / MAGE / PADBen) is wired up. No
  leaderboard row may be published until reproduction succeeds on a shared slice — publishing
  before that would be the exact behaviour the project criticises in others.
- **Related:** D38, D39, D40; THESIS §3; `sv-pro/ai2rules#8` (PACT discovery); `STRATEGY.md`
  (witness over depth); <https://github.com/sv-pro/ai-detector-bench> — first commit
  `ad6e8ae`, 41 tests green.
