# Task: blog hero illustrations — Antigravity + Gemini

**Owner:** Google Antigravity (Gemini) — it generated the existing house-style heroes,
so it should generate the new ones. **Status:** fix #1 done; images #2/#3 done. **#4 requested
(below)** — hero for the forthcoming *"Your Permission List Can't See Taint"* post.

## Problem

Two published posts lack a proper dedicated hero, and one is mis-wired:

| Post | Was | Issue |
|---|---|---|
| `running-claude-safely.mdx` | `blog-placeholder-4.jpg` | ✅ **fixed** — a dedicated `running-claude-safely.jpg` already existed, unused; rewired to it. No new art. |
| `false-positive-in-our-own-demo.md` | `blog-placeholder-4.jpg` | ✅ **fixed** — generated `false-positive-argument-taint.jpg` |
| `programmatic-tool-calling-governance.md` | *(empty)* | ✅ **fixed** — generated `programmatic-tool-calling.jpg` |

## House style + hard specs

- **Dimensions:** 1376×768, **JPG**, in `blog/src/assets/`. (Every hero in the repo is this size.)
- **Aesthetic:** dark neon / cyberpunk, cinematic. Cyan primary; magenta/orange accents.
  Circuit-board, glass-containment, dome, and "gate/boundary" motifs. Reference the existing
  heroes for style: `assets/taint-subagent-boundary-16x9.jpg` (labeled left→boundary→right concept),
  `assets/running-claude-safely.jpg` (brain in a glowing dome), `assets/zombieagent-threat.jpg`,
  `assets/harness-blocked-itself.jpg`.
- **Text:** avoid baked-in body text — AI-rendered paragraph text comes out garbled. A short, clean
  title is acceptable; **no text is safer**. A couple of short labels (ALLOW / DENY) are fine if legible.
- **Naming:** name after the post theme, matching the existing convention
  (`taint-subagent-boundary-16x9.jpg`, `zombieagent-threat.jpg`, …).

## #2 — `false-positive-argument-taint.jpg`

For `false-positive-in-our-own-demo.md` — *"We Found a False Positive in Our Own Flagship Demo"*
(the PACT / argument-level-taint post).

**Concept: the granularity mismatch.** A single tool call arrives at a gate. A coarse **flat floor**
stamps the *whole call* **DENIED** (red) even though its payload is clean — while a finer
**argument-level gate** inspects each argument separately: a clean argument glowing cyan (**ALLOW**),
a poisoned one glowing red (**DENY**). The story is *"same call, opposite verdict — the coarse floor
over-blocks a benign call; the per-argument gate allows it while still blocking the real exfil."*
Cyan = clean/allow, red-magenta = tainted/deny.

Then set `heroImage: '../../assets/false-positive-argument-taint.jpg'` in that post.

## #3 — `programmatic-tool-calling.jpg`

For `programmatic-tool-calling-governance.md` — *"Your Agent Just Learned to Write Programs. Can You
Still Govern It?"*

**Concept: the governance boundary moving from single calls to a generated program.** An agent emits
a glowing block of JavaScript that branches out to several tool nodes (loops/conditions between
them). A governance ring/shield now encircles the *whole generated plan* rather than each individual
call. Optionally contrast the fading old linear chain (`reason → call → result`) with the new
orchestrated program. House neon palette; the code block can carry a green/cyan glow.

Then set `heroImage: '../../assets/programmatic-tool-calling.jpg'` in that post.

## #4 — `permission-taint-gate.jpg`  *(requested 2026-07-21)*

For the forthcoming post *"Your Permission List Can't See Taint"* — replacing Claude's
`settings.json` permission pile with a governed manifest; the manifest sees **taint**, the flat
pile can't. Draft is written (parked, not yet in `blog/`), so this hero can be generated in parallel.

**Concept: same call, opposite verdict — decided by provenance, not by the call.** Two
**identical** glowing tool-call tokens approach a gate/boundary side by side. Each trails a fine
**data-lineage thread** back to its origin. The left token's thread is clean **cyan**, running
straight back to a trusted source (a keyboard / calm human-origin node) → the gate passes it
(**ALLOW**). The right token's thread passes through a **corrupted, poisoned document node** partway
back and turns **magenta/red** from there on (tainted) → the gate stops it (**DENY**). The two tokens
are visually identical; only the *thread colour* differs — that's the whole point. Optional: off to
one side, a dim redundant stack of `settings.json` rule-cards (the "pile") the gate has bypassed.
House neon: cyan = clean/allow, magenta/red = tainted/deny; circuit + gate motif. Keep it **distinct
from #2** — #2 splits *one* call by argument; this splits *two identical* calls by *history*. Short
ALLOW / DENY labels are fine; no body text.

Then, once the post lands in `blog/src/content/blog/`, set
`heroImage: '../../assets/permission-taint-gate.jpg'`.

## Verify

`cd blog && npm run build` passes, and each of the three posts renders its own distinct hero
(no post shares `blog-placeholder-4.jpg` anymore).
