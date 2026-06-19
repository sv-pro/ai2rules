# Correcting review — E12 blog launch

**Artifact:** `blog/` (Astro site + 4 articles) handed off from Advocacy (Antigravity).
**Reviewer:** Claude Code (Correcting Reviewer role — see `docs/FLYWHEEL.md` §3).
**Disposition:** corrected in place; no rework re-queued to the owner.

## Findings & fixes applied

### Blocker — articles presented invented APIs/commands as the real kernel
Corrected the technical content to match the actual code (binary `harness`,
subcommand `serve`, flags `--world/--simulate/--background`; real
`default_world.yaml` schema; `decide()` → `KernelOutcome`; `projected_actions()`).
- `running-claude-safely.md`: removed non-existent `cli-harness compile` /
  `cli-harness run -- npx claude-code`; rewrote to the real authoring-tool preview
  (`harness serve`) + governed `--world` loop + the actual approval prompt; fixed
  the manifest YAML to the real schema; framed external-agent MCP proxying as
  roadmap (not shipped).
- `why-deny-is-dangerous.md`: replaced fictional `project_ontology`/`allowed_roles`
  and the "Invalid schema formatting"/"gaslight" deception with the real
  projection + `ABSENT`/`UnknownToOntology` semantics.
- `ai-aikido.md`: real manifest fragment + real `decide()`/`build_execution_spec`
  flow (labeled simplified).
- `the-zombieagent-threat.md`: the cross-session `xattrs` mechanism was not
  implemented — reframed in-session monotonic taint as shipped and the persistent
  cross-session taint as roadmap; named the real `DENY`/`no_tainted_network` rule.

### High — SEO / Discover gaps + one real bug (`blog/src/components/BaseHead.astro`)
- Added `Article`/`TechArticle` JSON-LD, `robots: max-image-preview:large`,
  dynamic `og:type=article` + `article:published_time`.
- Fixed `og:image` always being the fallback — `BlogPost.astro` now passes the
  post's `heroImage` (verified per-post in the build output).
- Added `SITE_AUTHOR` (consts) for author/publisher E-E-A-T.

### Medium / low
- Fixed frontmatter/body typos ("AI's", "attacker's", "isn't", "Persistence").

## Verified
`npm run build` green — 7 pages + RSS + sitemap; images optimize to WebP
(31–114 kB); JSON-LD/robots/og:type render in built HTML.

## Deferred (needs owner / user input — not blocking)
- **Real `site` domain** in `astro.config.mjs` (placeholder `https://example.com`
  flagged with TODO); canonical/OG/JSON-LD URLs depend on it.
- **WebSub pinging** + automated OG-image generation (E12.2 remainder).
- **AVIF** output (WebP only today); inline architecture diagrams (E12.3).
- **Image source weight**: custom hero JPGs are ~0.8–1 MB committed under
  `blog-placeholder-*` names — recompress/rename (served output is already tiny).
- Confirm `SITE_AUTHOR` display name.
