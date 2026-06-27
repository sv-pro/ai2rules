# Research Backlog (the idea pool)

A parking lot for **research-grade topics** worth revisiting when current execution
work runs dry. These are deliberately *not*:

- **`PLAN.md`** — committed, scheduled execution (epics with checkboxes), nor
- **`DECISIONS.md`** — choices already taken (with the alternatives weighed).

Entries here are speculative, carry no schedule, and may be abandoned. The point is
to never lose a good thread just because it arrived mid-flow.

## How to use

- **Append freely** with the template at the bottom; don't prune aggressively.
- When you **pick one up**, promote it: expand into a `PLAN.md` epic (if it's work)
  or a `DECISIONS.md` `D<n>` (if it forces a choice), then mark it here.
- **Status tags:** 🌱 seed (raw) · 🔬 explored (has notes/a spike) · ⏫ promoted ·
  🗑 dropped.

## Index

| # | Topic | Status |
|---|-------|--------|
| **R1** | Super-harness — a cross-host governance/control plane over Claude Code, Codex, Antigravity (…) | 🔬 explored (notes below) |

Plus a few unfleshed one-liners under [Seeds](#seeds).

---

## R1 — Super-harness: a cross-host governance/control plane  🔬

**The question.** Is it feasible to build an infra layer that commands several agent
*hosts* (Claude Code, Codex, Antigravity, …) — or at least their capabilities —
routing subtasks by strength (code → Claude Code, prose → Codex, images → Gemini)
from a single session, instead of switching between them by hand? The switching
itself is cheap; the *value* is what you learn about agent infra by designing the
governed version.

### The key distinction: hosts vs. capabilities

Two different things can be orchestrated, with very different control surfaces:

- **Agent *hosts*** — Claude Code, Codex CLI, Antigravity. A loop + tools +
  permissions wrapped around a model.
- **Model *capabilities*** — Claude, GPT/o-series, Gemini/Imagen. The raw model.

| Host | Programmatic surface | Drivable? |
|------|----------------------|-----------|
| **Claude Code** | headless `claude -p … --output-format stream-json`, **Claude Agent SDK**, MCP client, **`claude mcp serve`** (exposes itself *as* an MCP server), hooks/subagents | Excellent — built to embed |
| **Codex CLI** | non-interactive `codex exec`, JSON output, MCP (both directions), OSS | Very good |
| **Antigravity** | GUI/editor-first (Gemini 3); reads `AGENTS.md` / `.agents/rules`; **no robust public headless API** | Weak as a host |

**Punchline for the image case:** don't orchestrate Antigravity to make images —
call the capability underneath (Gemini / Imagen API) directly. In general you command
a *host at its published surface* or a *model at its API*; you can't reach *inside* a
closed host's subagents, so let each host run its own.

### Cleanest design (achieves "no session switching")

Pick **one host as the conductor** and expose the others to it **as MCP tools**.
Claude Code suits this (MCP client + subagents + our gate):

- `codex-mcp` → wraps `codex exec` for **prose**
- `gemini-image-mcp` → wraps the Gemini/Imagen API for **images**
- Claude itself writes the **code**
- the **shared git working tree is the shared memory**; handoff = artifacts + a task
  spec.

One thread, multi-vendor, capability-routed — buildable in a weekend. Heavier-weight
alternative: an external **orchestrator** (LangGraph / CrewAI / AutoGen / OpenAI
Agents SDK / Google ADK) running the classic **orchestrator–worker (router)** pattern
and dispatching to each host as subprocess or MCP endpoint. Good for vocabulary; the
conductor-as-MCP approach gets ~80% with far less.

### Two protocols that name the space (study these first)

- **MCP** — agent ↔ *tools/data* (Anthropic; broadly adopted).
- **A2A** (Agent-to-Agent; Google → Linux Foundation) — agent ↔ *agent*, cross-vendor.
  This is *exactly* "agents commanding agents"; the spec is the fastest way in.

### Why this is *our* project, not a detour

It's the **host-neutral gate ABI** (D24, [`harness-gate-abi.md`](harness-gate-abi.md))
promoted from a *per-host gate* to a *cross-host control + governance plane*. The
five-layer model maps cleanly (orchestration = intent/capability; the gate = the
enforcement that travels with each host). The genuinely research-grade, unsolved part
— where the thesis has something orchestration frameworks don't:

- **Taint / provenance across handoffs** — when Codex's prose feeds Claude's code
  step, does taint propagate? (The subagent-taint experiment is the same question one
  boundary up.)
- **Capability projection per worker** — each agent sees only its `ABSENT`-minimized
  surface; a meta-orchestrator is a prime **confused-deputy** target, so least-
  capability per worker is a *security* property, not a nicety.
- **One deterministic audit ledger** across vendors.

Frameworks solve *routing*; none solve *governed* routing. That gap is your moat.

### Caveats

Nesting agents multiplies cost, latency, and non-determinism; errors compound across
hops; cross-host observability is hard (you reverse-engineer each host's real
semantics — see the subagent-taint post). GUI-first hosts cap automation, which is
*why* dropping to the model API is right for capabilities gated behind an editor.

### What to research first (if/when picked up)

1. Read the **A2A** spec; map it onto the gate ABI (could `harness gate` answer A2A?).
2. Spike the conductor + `codex-mcp` + `gemini-image-mcp` on a throwaway task; observe
   whether/where context and taint actually cross the boundary.
3. Decide the umbrella *form* question (this connects to the sibling-repo
   consolidation, D30 / THESIS §7).

**Related:** [`harness-gate-abi.md`](harness-gate-abi.md) · [`THESIS.md`](THESIS.md)
(five layers; §7 consolidation) · `DECISIONS.md` D24/D25/D26 · blog
*"Does Taint Cross the Subagent Boundary?"*

---

## Seeds

Unfleshed one-liners — candidates to expand later (some may fold into **R1**):

- 🌱 **Cross-host information-flow control** — generalize the subagent-taint finding
  into a standalone primitive: taint/provenance that survives *any* agent→agent
  handoff, even without a full super-harness.
- 🌱 **A2A-conformant gate** — expose `harness gate` over the A2A protocol so any A2A
  agent network can call our deterministic disposition as a service.
- 🌱 **Governed local image generation** — a local FLUX/SDXL (or Gemini/Imagen)
  capability behind the gate; offline-first synergy with the thesis (no taint-floor
  network friction). Prompted by needing blog hero art.
- 🌱 *(add the next idea here)*

---

## Template

```markdown
## R<n> — <short title>  <status emoji>

**The question.** <one or two sentences: what and why it's interesting.>

### Notes
<key insights, distinctions, prior art, links.>

### What to research first
1. <smallest informative next step / spike.>

**Related:** <links to docs / decisions / repos / posts.>
```
