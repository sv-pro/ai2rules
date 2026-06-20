---
description: Correcting-review the blog (optionally one article) against the real harness + SEO, fixing issues in place.
argument-hint: "[article slug or file path] — optional; defaults to blog files changed on this branch"
allowed-tools: Read, Edit, Bash, Grep, Glob
---

Run a **correcting-review** pass on the blog. Prefer delegating the work to the
`correcting-reviewer` subagent.

Target: $ARGUMENTS
If empty, review the blog files changed on this branch:
!`git diff --name-only main...HEAD -- blog/`

Steps:
1. **Technical accuracy** — verify every command, snippet, manifest, and mechanism
   against the real harness: binary `harness`, `serve` subcommand, flags
   `--world/--simulate/--background`; the manifest schema in
   @crates/compiler/assets/default_world.yaml; `decide()` / `KernelOutcome`.
   Fix fabricated commands/APIs in place; mark unshipped mechanisms as roadmap.
2. **SEO/Discover** — confirm Article/TechArticle JSON-LD, canonical, per-post
   `og:image`, and `robots: max-image-preview:large` in
   @blog/src/components/BaseHead.astro and @blog/src/layouts/BlogPost.astro.
3. **Polish** — fix typos in titles/descriptions and any wrong outbound links.
4. **Verify** — `cd blog && npm run build` must pass.
5. **Report** severity-ordered: fixed / deferred (with reason) / how verified.
