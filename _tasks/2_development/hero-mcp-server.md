# Task: hero-image generation as a governed MCP capability

**Owner:** Claude Code (build). **Status:** spec + scaffold done (branch
`feat/hero-mcp-server`); **backend = the `agy` (Antigravity) CLI** (no API key) after a live
probe confirmed it renders on-brief, cinematic **1376×768** art from a single prompt.
Typecheck green. Remaining: wire into `.mcp.json` / `harness mcp-gateway` and use it.

## Why

Today a new blog hero is made by **open agent-to-agent handoff**: a human-written brief is
dropped into [`_tasks/3_advocacy/hero-illustrations.md`](../3_advocacy/hero-illustrations.md)
for Antigravity to pick up in its own session. That's wide-open — no contract, no governance
seam.

Replace it with a **focused, contracted capability**: an MCP server exposing one tool,
`generate_hero`, that bakes in this blog's dir + house style so the caller supplies *only
intent*. This is the thesis applied to our own tooling — a narrow, declared capability the
`WorldManifest` can `ALLOW` / scope / taint, instead of an ungoverned agent channel. Once
wired, **Claude calls it directly** and the loop closes.

## The contract (v1 — one tool)

```
tool  generate_hero
  in    concept : string      # the SCENE ONLY (palette/dims/paths are baked in, not passed)
        name    : string      # kebab theme slug, e.g. "permission-taint-gate"
        labels? : string[]    # short legible labels allowed, e.g. ["ALLOW","DENY"]
  out   { asset: string, dims: "1376x768" }
  does  compose(house_style, concept) -> `agy -p` (renders the PNG) -> sharp crop/resize
        to EXACTLY 1376x768 JPG -> write <assets_dir>/<name>.jpg
```

The "specific blog dir + specific preferences" live in a **profile**
(`tools/hero-mcp/hero-profile.json`), lifted from the hero-illustrations house spec, so the
same server serves other blogs by swapping the profile.

## Design (chosen)

| Choice | Decision | Why |
|---|---|---|
| Runtime | local **TypeScript stdio MCP** server (`tools/hero-mcp/`) | writes into `blog/src/assets`; keeps it on the npm+Rust local plane (no Python) |
| Backend | shell out to the already-authenticated **`agy` CLI** (`agy -p`) | reuses your Google auth — the server needs **no key of its own**; a live probe made a cinematic 1376×768 hero on the first real prompt |
| Delivery | front with **`harness mcp-gateway`** (governed) | recursion/dogfood: `generate_hero` becomes an `Mcp` action the manifest governs |
| Post-process | **sharp** → exact 1376×768 JPG | enforce the hard spec deterministically; convert PNG→JPG |

**Auth is a non-issue.** `agy` is already signed in with the user's Google account, so the
whole `@google/genai` + API-key / Vertex / ADC path is dropped. The one prompt lever that
matters: it must explicitly demand *generative* art (not a programmatic drawing) — the
profile's `generation_directive` does that (the probe: a forced-generative prompt made a
1.7 MB render; a lax "cyan circle" prompt made a 1.2 KB programmatic sketch).

## Open decision

- **Governed vs raw** — ship v1 behind `harness mcp-gateway` (recommended) or a plain
  `.mcp.json` server, promote to governed once it's in daily use?

## v1 scope / non-goals

- **In:** compose prompt → `agy -p` → resize/convert → write ONE JPG; return the path.
- **Out (the caller does these):** setting `heroImage` frontmatter and running
  `npm run build` — Claude does those with Edit/Bash. Multi-variant is also out for v1
  (each `agy` call is a full agent run; add it later by looping if wanted).

## Acceptance

- `generate_hero({concept, name})` writes `blog/src/assets/<name>.jpg` at exactly
  **1376×768 JPG**, cinematic house palette, no baked-in body text.
- Requires only **`agy` on PATH and authenticated** — no API key.
- Reachable by Claude via `.mcp.json` (v1) and, once promoted, via `harness mcp-gateway`.
- **Backend proven:** the probe rendered `permission-taint-gate.png` (1.7 MB, 1376×768,
  on-brief) — the parked *"Your Permission List Can't See Taint"* post's hero.

## Verify

`cd tools/hero-mcp && npm install && npm run build`; with an authenticated `agy` on PATH, a
smoke call produces a correctly-sized JPG in `blog/src/assets/`.
