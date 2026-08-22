# D55 — The Governability Index is public and structural; no parameter may require our own tooling to answer


**Date:** 2026-08-07. **Executes** `STRATEGY.md`'s second ranked bet (the governability
index) and **applies D54's split** — definitions here, measurement recipes in the lab.
Constrained by D24 (the host-neutral seam is what makes cross-host comparison legible at
all) and by D52's referee discipline, which this entry extends to a second sport.

- **Context.** `STRATEGY.md` ranks a governability index second among the open bets, on
  two arguments: a ranking travels further than a demo, and whoever defines the benchmark
  defines the category. It stayed unbuilt because its first criterion — "can one portable
  manifest govern it" — is a claim about *our* product, which makes the benchmark look
  like an advertisement. The unlock was noticing that **the presence or absence of a
  pre-execution hook is itself a benchmark parameter**: structural, binary, vendor-owned,
  and answerable by anyone in five minutes. That reframes the whole instrument away from
  "how well does our manifest govern host X" and toward "what does host X let *anybody*
  control".
- **Decision.** The **Agent Governability Index** is a public artifact.
  `docs/GOVERNABILITY-INDEX.md` owns the **parameter definitions, the methodology and the
  results table**; `agentic-coding-lab/artifacts/governability-probe` owns the
  **procedures**. Nine parameters at v0 (G1 intercept · G2 deny · G3 grant · G4 MCP+native
  coverage · G5 cache-satisfiable approval · G6 absent-vs-denied · G7 post-execution
  observation · G8 file-based config · G9 live reload). Shipped `cd37364` / `49d5c88`.
- **The load-bearing constraint — the conflict-of-interest firewall.** **No parameter may
  require `ai2rules` to answer.** We build a governance harness for the hosts this index
  scores; an index whose cells can only be filled with our tooling is marketing wearing a
  table's clothes. If a proposed parameter cannot be checked by someone who has never
  heard of us, it does not go in — *however discriminating it is*. This is the rule that
  makes the rest of the instrument survivable, and it is the first thing to check when
  adding a parameter.
- **Second constraint — the index measures the product, never the model.** Every parameter
  is a yes/no question about what the host *permits*, answerable without running an agent
  task. Admitting one behavioural parameter ("how well does it resist injection") would
  reimport everything the structural design exists to exclude: model non-determinism,
  prompt sensitivity, a shelf life of weeks, and irreproducibility. The value here is
  precisely that a result from March is still a result in November.
- **Third constraint — `?` is a publishable state and a guess is a defect.** Confidence is
  a load-bearing column, as in `MAP.md`: `✓` observed by us, `○` documented but not run,
  `?` unknown. The Codex and Copilot columns are entirely `?` on publication. "We looked
  and did not find a hook" is **not** a measurement and may not be published as a `no`.
- **No composite score. Ever.** A single "governability score" would be exactly the sin
  `detbench` was built to name — AUROC averaging away the decision anyone actually makes
  (D52). Per-parameter cells only. A reader who wants one number wants a number that
  hides which of the nine they should have cared about.
- **The disclosure is structural, not a footer.** The COI statement opens the document,
  before the parameters, with the three constraints above stated as consequences of it.
  Buried, such a disclosure reads as a liability; first, it reads as the reason to trust
  what follows. Also recorded there: a low score is **not** a claim that a host is bad —
  governability is orthogonal to quality, and the index says nothing about how good an
  assistant is.
- **Two parameters were discovered by building the instruments, not by design.** G8
  (file-based config) came from finding that UI-configured connectors are invisible to
  tooling that cannot even report them as missing. G9 (live reload) came from measuring
  that Claude Code reads hook config live rather than snapshotting it at session start —
  the opposite of what we had assumed and stated twice. This is §6's flywheel behaving as
  advertised, and it is the argument for keeping the procedures and the definitions in
  different repos with real users in between.
- **Alternatives rejected.**
  - *Keep it internal.* An index nobody can check is not a benchmark, and witness is the
    binding constraint (`STRATEGY.md`).
  - *One repo for both halves.* Rejected on D54: definitions are positioning and belong
    with the thesis; procedures are practitioner content and belong in the lab. Putting
    the table in the lab would also invert the direction of *authority* — the lab is the
    consumer.
  - *Lead with "can one portable manifest govern it".* That criterion is real and stays in
    `STRATEGY.md` as a product thesis, but as parameter G0 it would fail the COI firewall
    on its first line.
  - *Score the hosts.* See above.
- **Known residual, and the deepest one: we chose the nine questions.** The firewall
  guarantees each parameter is *independently answerable*; it does not guarantee the
  *selection* is neutral. Nine parameters drawn from our own architecture will
  systematically favour hosts shaped like the seam we integrate with. There is no clean
  fix, so the mitigation is procedural: publish proposed parameters that were rejected and
  why, and treat an outside parameter proposal as higher-priority than an outside
  measurement.
- **Known residual: the conventions have no mechanical enforcement.** Dates, host
  versions, `?`-not-guess, observed-vs-documented — all are review discipline. Unlike the
  lab's artifact contract, no CI check can verify that a cell's evidence is real.
- **Known residual: this is the first cluster artifact that names competitors in a scored
  table.** D27 and D53 argue positions against named products, which is ordinary technical
  writing. Assigning cells is a different exposure: a wrong cell is a false claim about
  someone's product, and the correction cost falls on them.
- **Review condition.** If by **2027-02-07** no third party has contributed a measurement
  or contested a cell, the index is a monologue rather than a benchmark — and a benchmark
  nobody engages with is marketing after all. At that point either retire it or hand the
  definitions to someone with no product in the category.
- **Related:** D24, D27, D33, D37, D40, D48 (`force_ask` — the finding behind G5), D51,
  D52 (referee discipline; no composite score), D53, D54; THESIS §6; `STRATEGY.md` (bet 2);
  [`docs/GOVERNABILITY-INDEX.md`](../docs/GOVERNABILITY-INDEX.md);
  <https://github.com/sv-pro/agentic-coding-lab>
