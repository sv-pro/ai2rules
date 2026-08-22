# D30 — Rename the umbrella project `cli-agent` → `ai2rules`; "harness" stays the action layer

- **Status:** Accepted (rename) · refines/advances D23 · **See:** [`docs/THESIS.md`](../docs/THESIS.md) §7
- **Decision:** Rename the repository / umbrella project from `cli-agent` ("CLI Agent
  Harness") to **`ai2rules`** — repo = site = thesis (the work already publishes at
  **ai2rules.dev**, and "AI → rules" *is* the stochastic→deterministic move). The old
  name had been outgrown: per **D23** the repo became the **umbrella over the five-layer
  thesis** (action · capability · knowledge · intent · substrate), not just the
  action-layer harness, so a name describing only the harness no longer fit. Scope of
  this change: GitHub repo renamed `sv-pro/cli-agent → sv-pro/ai2rules` (GitHub
  auto-redirects old URLs; local `gh` remote re-pointed); in-repo **identity + brand
  surfaces** (`README` title + an explicit "umbrella/companion for ai2rules.dev" note,
  `PLAN`/`DECISIONS`/`AGENTS` headers, `Cargo.toml` `repository`, blog `SITE_TITLE` /
  header / footer / about / index / DEPLOY, the `cli-harness` binary banner) → `ai2rules`.
  The **action-layer component keeps the name "harness"** (the kernel, the `cli-harness`
  binary, `docs/harness-architecture*.md`) because it is accurate and is *one* layer.
  **Crate names are unchanged** (`world-kernel`, `compiler`, `executor`, `cli-harness`,
  `harness-types`, …) — internal and still correct.
- **Why `ai2rules` over the alternatives:** chosen (by the maintainer) over
  `agentic-governance` (descriptive but generic / SEO-flat as a repo slug), `Worldgate`
  (a strong coined brand, but a new name to seed), and `IntentOS` (reads as a product and
  **D23** flagged it overweights the least-mature fragment). `ai2rules` reuses an
  already-owned domain, so repo = site = brand with zero new surface.
- **Alternatives:** keep `cli-agent` — rejected: it names only the action layer and
  actively misleads now that the repo is the umbrella; the three names above.
- **Deferred (the open companions, still from D23 §7.3 / THESIS §7):** (1) the **GitLab**
  mirror rename (`origin`) — done via its web UI, not scripted (a `curl` to the GitLab API
  is itself denied by our taint floor while the session is tainted — a fitting dogfood);
  (2) the **local working-dir** rename `cli-agent/ → ai2rules/` — deferred (renaming the
  CWD mid-session breaks paths/hooks); (3) the **physical consolidation** of the sibling
  repos into one tree (meta-repo with submodules vs. a single Cargo/workspace) — the
  umbrella *form* remains the genuinely open decision, to be recorded when taken.
- **Known limits:** published blog-post *prose* and the `harness-architecture*.md` titles
  still say "CLI Agent Harness" — kept deliberately (they describe the action-layer harness
  / are historical published content); a prose-rebrand pass is optional follow-up. The
  GitLab repo stays named `cli-agent` until renamed in its UI.
