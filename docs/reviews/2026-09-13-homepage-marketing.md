# Homepage: messaging, first proof and discoverability

Reviewed on 2026-09-13 against main at
`997275480dde4fa58de6ba2aa69838fa624a650a`.

Related: [AI2-22](https://linear.app/ai2rules/issue/AI2-22/docs-publish-progressive-90-second-10-minute-and-45-minute-onboarding).
This is the homepage portion of onboarding, not completion of the three guided
paths or a measurement of conversion improvement.

## Method and scope

Adapted the copy/landing review questions from
[ai-marketing-claude](https://github.com/zubair-trabzada/ai-marketing-claude)
and the technical discoverability checks from
[geo-seo-claude](https://github.com/zubair-trabzada/geo-seo-claude).
No third-party skills or installers were installed. The implementation is
original site copy and configuration; no vendor scores are treated as evidence.

Audience: developers evaluating execution governance, starting with Claude Code
CLI. Desired next action: inspect the real kernel in the browser, then follow a
working package installation and direct hook check.

## Three changes

| Priority | Observed problem | Change | Evidence / boundary |
|---|---|---|---|
| 1 | Homepage and shared description say every tool call is decided; the host-neutral section implies universal coverage. | Describe calls routed through a connected hook or MCP gateway, name Claude Code CLI as the starting point, distinguish conformance from coverage. | `SECURITY.md`, `docs/harness-architecture.md`, `docs/one-kernel-many-hosts.md`. A hook is not process containment. |
| 2 | The primary browser demonstration is below installation and several explanatory sections. | Move the single primary button to the introduction; add a secondary installation anchor, fresh-directory setup and the existing manual CLI guide. | Existing `/playground` runs the committed WASM engine. Published npm 0.5.0 supports `init` and direct hook probes; it does not support `demo`. |
| 3 | Homepage title is only `ai2rules`; no source-controlled robots file advertises the sitemap. | Add a descriptive homepage title, qualify the shared description and publish a sitemap-only robots file. | Canonical, Open Graph, JSON-LD, RSS and sitemap generation already exist; no duplicate schema or speculative ranking claims are added. |

The README's npm paragraph is also aligned with the homepage: npm downloads need
network access; the package itself has no install script. Homepage CSS keeps the
existing style and bounds padding/width for the earlier CTA on narrow screens.

## Release discrepancy: do not advertise `harness demo` yet

`crates/cli-harness/src/main.rs` registers `Demo` and `demo.rs` implements five
decision/replay scenarios on the reviewed main commit. However, a clean npm
installation on Linux x64 returned:

```text
$ harness --version
cli-harness 0.5.0
$ harness init
[starter manifest, shim, settings and gitignore written; exit 0]
$ harness demo
error: unrecognized subcommand 'demo'
[exit 2]
```

`npm view ai2rules-harness version` returned `0.5.0`. Validation installed that
exact package using `npm install --global --prefix <temporary-tools-dir>
--ignore-scripts --no-audit --no-fund ai2rules-harness@0.5.0`; the binary was outside
the fresh fixture project. Node 24.19.0 / npm 11.9.0. This checks the packaged
Linux binary, not macOS, Windows or a running Claude session.

The initial proposed homepage command sequence included `demo`; it was removed
after this failure. Release the command and rerun the packaged clean-install
proof before promoting the 90-second CLI path. Do not use main-only tests or the
Done status of AI2-24 as evidence that the npm package contains the command.

## Direct hook checks on npm 0.5.0

Each event was passed on stdin to `bash .claude/hooks/world-gate.sh`, with
`CLAUDE_PROJECT_DIR` set to the fresh project. These are decision probes, not
executed writes or web fetches. All hook processes exited 0.

| Proposed event | Observed response |
|---|---|
| `Write` to `/etc/passwd` | `permissionDecision: deny`, `path_scope_readonly` |
| `Write` to a new file inside the fixture | Empty stdout (additive pass-through); target file was not created |
| First `WebFetch` to `https://example.com` | Empty stdout; session taint raised |
| Second identical `WebFetch` | `permissionDecision: deny`, `taint_invariant` |

These results support the manual hook-check link. They do not establish that
Claude Code has loaded the generated settings or enforced a verdict in a live
agent session. No time-to-first-governed-session metric was measured.

## Live crawling observation

Direct HTTPS requests from this environment received HTTP 403 for `/` and
`/sitemap-index.xml`. `/robots.txt` returned HTTP 200 with Cloudflare-managed
content signals, a general Allow rule and specific bot restrictions, including
GPTBot and ClaudeBot. No Sitemap directive appeared in that response.

This is an environment-specific observation, not proof that ordinary visitors
or verified search crawlers are blocked. Training-bot restrictions do not by
themselves establish search crawler access. No Cloudflare permissions or bot
restrictions were changed. The new origin robots file only advertises the
sitemap; it does not override edge policy.

After deployment, inspect Cloudflare security events for the failing requests
and verify the served robots/sitemap plus search-engine indexing evidence before
changing crawler policy. Do not broadly disable bot protections or infer a GEO
ranking benefit from this source change.

## Validation

- Astro production build passed (25 pages).
- Existing `check:heroes`, `check:drafts`, `check:spacing` passed.
- Built HTML has one primary CTA; all homepage local links and the setup anchor
  resolve. Canonical, description and JSON-LD agree. Homepage, playground and
  manual guide are present in the generated sitemap; robots.txt is copied.
- Published npm binary `init` and the four direct hook probes passed as recorded.
- Published npm binary `demo` failed as recorded; it is excluded from homepage instructions.
- Browser visual QA was unavailable: the local Chromium executable was absent
  and its download timed out. Layout was inspected in source, not visually certified.
- No runtime/kernel changes; no release, deployment or live-session claim.

## Next decision

Review and merge the homepage change, then verify the served site. Keep AI2-22
open: the released CLI first-proof command, guided ten-minute path, cold-start
timings and broader platform evidence remain separate work. Add the social
repurposing step only when a release/demo has verified evidence to publish.
