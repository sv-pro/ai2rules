# D54 — `agentic-coding-lab` is a second outbound instrument: capability-first distribution, governed by the same one-way rule


**Date:** 2026-08-06. **Extends** D52 (which added the *outbound instrument* as a fifth
role in D40's topology) by putting a **second** artifact in that role rather than
inventing a sixth. Constrained by D40 (one live implementation), `STRATEGY.md` (witness
over depth), and THESIS §3.

- **Context.** D52 established that an artifact built to be *found* — live, public, aimed
  at an audience that has never heard of the thesis, never a dependency of the
  implementation — is a legitimate fifth role. `detbench` occupies it by attacking the
  detection question. A second gap is larger and closer to home: the people who use
  Claude Code, Codex CLI, OpenCode and Antigravity every day are searching for
  *effectiveness* — skills, subagents, MCP setups, workflows — not for governance. The
  cluster has a governance kernel and no capability-first surface at all. Every artifact
  that makes an agent more effective also makes it reach further, so the two subjects are
  the same subject approached from the side the audience is actually standing on.
- **Decision.** `agentic-coding-lab` is **its own repo in D52's fifth role**: experiments,
  articles and copyable artifacts (skills, plugins, subagents, hooks, MCP configs) for
  agentic software development, each shipped with an honest account of its reach. It is a
  **consumer of `ai2rules`** and nothing else. Created locally 2026-08-06 (`e53f047`); no
  remote yet, deliberately — see the residual on publishing below.
- **The one-way rule, again the load-bearing half.** `agentic-coding-lab` may depend on,
  cite and configure `ai2rules`. **`ai2rules` may never depend on it**, and **nothing
  produced there may become an input to a governance decision** — not a runtime-written
  rule, not a heuristic feeding a verdict, not a "trusted artifact" list the kernel
  consults. Identical in shape to D52's detector prohibition and justified identically:
  the kernel's claim is that a decision is a pure function of `(intent, context, compiled
  world)`, and a convenience artifact with a vote converts a deterministic decision into a
  negotiable one.
- **The differentiator, which constrains the build and so is recorded here.** The
  contract is *"an artifact may not describe itself as safer than it is."* Three tiers —
  **0 unenforced** (prose advice, nothing checks it), **1 overlay** (ships deny/ask rules
  atop the host's permissions; **fails open**), **2 structural** (the capability is
  `ABSENT` from the compiled world, or mediated by the fail-closed `mcp-gateway`). A tier
  ≥ 1 claim must ship the file that does the enforcing, and CI rejects it otherwise. An
  artifact nobody has run must carry `> **Unverified.**` on its own front page.
  This is D52's referee discipline applied to a different sport: the honest label is the
  product, and "hardened"/"sandboxed"/"safe" are exactly the words that would earn stars
  and forfeit the position.
- **Why not inside `ai2rules`.** Rejected for D52's reasons plus one of its own: a
  capability-first artifact buried in a Rust governance workspace inverts the funnel. The
  reader arrives for the skill and meets the thesis on the way out; that only works if the
  skill is the front door. It would also drag a content repo's cadence into a workspace
  whose CI gates a kernel.
- **Why not a directory in `detbench`, or a shared "content" repo.** Different audience,
  different instrument. D52 archived-by-precedent the habit of merging things that merely
  rhyme.
- **Alternatives rejected.**
  - *Publish the recipes as blog posts only.* The upstream blog is argument-first and its
    reader is being persuaded of a position. A recipe's reader wants to go faster today.
    Recorded as an explicit article split in the new repo's charter: if deleting the
    governance section leaves a useful article, it belongs there; if it leaves nothing, it
    belongs on the blog.
  - *Ship a starter pack of artifacts immediately, to look alive.* Rejected by the repo's
    own contract on its first commit — none had been run. `artifacts/` therefore holds a
    template and nothing else, which is the honest state and is said plainly in its README.
  - *Make the tier check advisory.* Rejected: an unenforced honesty rule in a repo whose
    entire pitch is enforced honesty is the failure this cluster spends its time naming in
    others.
- **What was verified, and what was not.** The template's `world.yaml` is a real manifest
  compiled by the real kernel; five verdicts were checked against `harness gate` on both a
  debug and a release build with identical results (`ALLOW Read`; `ASK Bash_destructive`;
  `ABSENT WebFetch / unknown_to_ontology`; `DENY Bash_network / taint_invariant` when
  tainted; `ALLOW Bash_network` when clean), manifest hash `8629c6be6c12`. **Not
  verified:** any end-to-end artifact, because none exists yet. Manifest verification is
  local-only — that repo's CI does not build Rust, by design.
- **Known residual: nothing is published.** *(Amended 2026-08-06, hours after this entry
  landed — the original text said "the repo is local, has no remote".)* It now has one:
  `sv-pro/agentic-coding-lab`, **private**, `origin` only, no GitLab mirror — matching
  `ai-detector-bench` rather than this repo's dual-remote setup. Private is not published,
  so the residual stands rather than closes: **an outbound instrument that is not outbound
  is not yet doing its job.** It still has no LICENSE, and a repo whose point is that
  people copy things out of it is unusable without one. Both remain open calls, not
  oversights; the license one is cluster-wide (`ai2rules` and `ai-detector-bench` carry
  none either). The two come due together — going public is the gate that forces the
  license question, and the first verified artifact is the event that should trigger it.
- **✅ Residual closed 2026-08-08. Both halves, together, as this entry predicted — but the
  trigger was not the one named.** `sv-pro/agentic-coding-lab` is **public**, and all three
  repos carry **MIT** (`ai2rules` `44d6b0c`, `ai-detector-bench` `c97956f`,
  `agentic-coding-lab` `6b49c18`; copyright Sergey Vlasov). The event that forced it was
  **D55**, not a verified artifact: the Governability Index shipped publicly one day earlier
  with its *Contributing a measurement* section pointing at procedures inside a private repo.
  That is not a broken link — **it is D55's conflict-of-interest firewall failing in
  practice**, since the index's entire defence is that a stranger can check it without our
  tooling, and no stranger could read the procedures at all. Verified after the change: an
  unauthenticated fetch of the probe README returns 200.
  **Two things worth keeping from how this closed.** First, D54 and D55 landed a day apart
  and *neither noticed that the second made the first's open residual load-bearing* — an
  entry can convert another entry's known gap into a live defect without either author
  seeing it, which is an argument for reconciling the decision log against the artifacts, not
  only against itself. Second, licensing was never the small half: `ai2rules` had declared
  `MIT OR Apache-2.0` in `Cargo.toml` since E0, inherited by all ten crates, **with neither
  license file ever in the tree** — a grant asserted in metadata with no text to accept.
  *(Amended later the same day — the first pass narrowed this to MIT-only to match the
  siblings; see the correction below.)*
- **Correction, 2026-08-08: `ai2rules` keeps the dual grant. The cluster is not
  single-licensed, and that asymmetry is the decision.** The MIT-only narrowing was reverted
  on the owner's call: `Cargo.toml` declares `MIT OR Apache-2.0` again, and the repo now ships
  **both texts** — `LICENSE-MIT` and `LICENSE-APACHE` — per the Rust ecosystem convention that
  the metadata always implied. The siblings stay **MIT-only**, deliberately.
  **The split follows the roles D52 and D54 already assigned.** The outbound instruments are
  content and tools meant to be *copied out of*, where one permissive license with a minimal
  attribution burden is the entire point; two license files at the root of a recipe repo is
  friction against its only job. `ai2rules` is the one artifact someone might build a product
  **on top of**, and that reader is the one who needs an explicit patent grant — which MIT does
  not provide and Apache-2.0 does. **A licensing choice is a distribution choice, so it should
  follow the topology rather than be applied uniformly for tidiness.** Uniformity was the whole
  argument for MIT-everywhere, and it was the wrong axis.
  **Known cost, measured rather than predicted, because it will look like a regression:**
  GitHub resolves the pair to **`Apache-2.0`** — its API returns that single SPDX id, sourced
  from `LICENSE-APACHE`, and the repo page reports "Apache-2.0, MIT licenses found". So the
  badge *understates* the grant by naming the stricter option and hiding the MIT one. **This
  is the ecosystem-standard outcome, not a misconfiguration: `rust-lang/rust` resolves
  identically** (verified 2026-08-08 — same SPDX id, same source file). A first draft of this
  entry predicted `NOASSERTION`; that was a guess and it was wrong, which is recorded here
  because the number of times this project has been bitten by an unverified assumption is now
  its own pattern. Not a reason to collapse back to one file.
- **Known residual: this is a *seventh* live thing in a cluster that archived eight repos
  to stop exactly this.** So the kill condition is stated up front, per D52's practice:
  **if there is no verified artifact and no published experiment by 2027-02-06, archive it**
  with a README pointer to `ai2rules`.
- **Related:** D24, D33, D40, D47, D48, D51, D52; THESIS §3, §6; `STRATEGY.md` (witness
  over depth); `scripts/install-governance.sh`; `docs/harness-gate-abi.md`.
