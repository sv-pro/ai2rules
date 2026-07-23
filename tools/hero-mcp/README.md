# hero-mcp

A focused, governable MCP capability: **generate one house-style blog hero image.** The
caller supplies only the *scene* (`concept` + `name`); the blog's asset dir, palette,
1376×768 dimensions, JPG output, and the no-body-text rule are baked in from
[`hero-profile.json`](hero-profile.json). Spec:
[`_tasks/2_development/hero-mcp-server.md`](../../_tasks/2_development/hero-mcp-server.md).

Backed by the already-authenticated **`agy` (Antigravity) CLI** — the server shells out to
`agy -p "<task>"`, which renders the image with *your* Google auth. **No API key, no Vertex,
no ADC.** This replaces the open agent-to-agent hero handoff with a narrow, contracted tool
the WorldManifest can govern.

## Requirements

- **`agy` on PATH and signed in** (the Antigravity CLI) — that's the only auth.
- Node 20+.

## Setup

```bash
cd tools/hero-mcp
npm install
npm run build
export HERO_REPO_ROOT=/ABS/PATH/TO/ai2rules   # so blog/src/assets resolves
```

## The tool

```
generate_hero(concept, name, labels?)
  -> agy -p "<house-style prompt>"        (renders a PNG with your Google auth)
  -> sharp -> blog/src/assets/<name>.jpg  (EXACTLY 1376x768 JPG)
  -> returns { asset, dims }
```

Only the scene is the caller's job — everything in `hero-profile.json` (palette, dims, JPG,
and the "force generative art, not a diagram" directive) is baked in.

## Wire it to Claude

**v1 (plain) — `.mcp.json` at the repo root:**

```json
{
  "mcpServers": {
    "hero": {
      "command": "node",
      "args": ["/ABS/PATH/TO/ai2rules/tools/hero-mcp/dist/index.js"],
      "env": { "HERO_REPO_ROOT": "/ABS/PATH/TO/ai2rules" }
    }
  }
}
```

Claude then calls `mcp__hero__generate_hero`. **No secrets in the config** — auth lives in
`agy`.

**Governed (recommended)** — front it with `harness mcp-gateway` so the WorldManifest gates
the call: declare `generate_hero` in the manifest, pin its output to `blog/src/assets`, and
it becomes an `Mcp` action subject to allow/deny/taint like any other tool. The capability
arrives *pre-governed* — the whole point.

## Env

| var | meaning |
|---|---|
| `HERO_REPO_ROOT` | repo root, so `assets_dir` resolves (default: cwd) |
| `HERO_AGY_BIN` | agy binary name/path (default: `agy`) |
| `HERO_AGY_FLAGS` | agy permission flags (default: `--sandbox --dangerously-skip-permissions`) |
| `HERO_ELICIT` | ASK posture: `auto` (default — ask when the host can elicit, else proceed), `require` (**fail closed** when the host can't ask a human), `off` (never ask) |
| `HERO_AGY_TIMEOUT_MS` | max time for one agy generation (default: 420000) |
| `HERO_ASSETS_DIR` | override the output dir |
| `HERO_PROFILE` | path to a different profile (serve another blog) |

## Notes

- **One image per call.** Each `agy -p` is a full agent run (~1–3 min); multi-variant is
  out for v1 — loop if you want candidates later.
- The prompt **explicitly forces generative art** ("call an actual image-generation model,
  do not draw programmatically") — without that, a coding agent may sketch a diagram
  instead. See `generation_directive` in the profile.
- The intermediate PNG is written to a throwaway temp dir and deleted; only the final JPG
  lands in `blog/src/assets/`.
- **`concept` is untrusted** — it becomes an agent prompt, so agy runs with `--sandbox`
  to fence the worst outcome (no arbitrary shell). `--dangerously-skip-permissions` is
  still needed for the non-interactive flow (`--mode accept-edits` alone generates
  *nothing*). For an untrusted caller, run it inside the OS-level container (`docker/`) —
  the deeper fence. Tune via `HERO_AGY_FLAGS`.
- **It asks first** (`ASK` → the present human). If the host supports MCP elicitation
  (Claude Code does, no config), `generate_hero` surfaces the `concept` for a human
  proceed/deny *before* launching agy — so untrusted input gets a glance. Default `auto`
  falls through when the host can't elicit (the sandbox is the backstop); set
  `HERO_ELICIT=require` to **fail closed** instead — no human channel ⇒ no run (the right
  posture behind `mcp-gateway` with non-human callers) — or `off` for unattended runs.
