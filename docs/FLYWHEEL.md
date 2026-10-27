# The Engineering Flywheel: Discovery, Development, and Advocacy

This document defines the architecture of a reciprocal-feeding system (a "flywheel") designed to continuously generate high-quality research, code, and content. By interconnecting these three pillars, the output of one phase automatically becomes the input for the next, eliminating context-switching and preventing "writer's block."

## The Flywheel Architecture

### 1. Content Discovery (The "Radar")
The goal of this phase is to automate the ingestion of the state-of-the-art in AI security, filtering strictly for signal over noise.

*   **The Setup:** An autonomous agent (built dogfooding the harness or `safe-mcp-proxy`) running on a daily `cron` schedule.
*   **The Action:** The agent fetches top posts from the Hacker News API, new papers from the `cs.CR` (Cryptography and Security) category on arXiv, and specific security blogs via RSS.
*   **The Filter:** The LLM evaluates the feed against a strict prompt: *"Does this relate to prompt injection, agent sandboxing, capability security, or LLM vulnerabilities? Score 1-10."*
*   **The Output:** A daily summary appended to `_research/DISCOVERY_LOG.md`. *(As built: twice a week, scored findings filed as Linear issues plus a digest; see [Cadence](#cadence-the-clock).)*
*   **The Reciprocal Link:** When an article highlights a brand new attack vector (e.g., a novel MCP supply chain vulnerability), it bypasses the backlog and instantly becomes a **Development** ticket.

### 2. System Development (The "Engine")
Instead of building theoretical features, development is driven strictly by neutralizing the real-world threats identified by the Discovery layer.

*   **The Setup:** Take the vulnerability found in the Discovery phase and write a failing integration test in the codebase.
*   **The Action:** Update the `WorldManifest` logic, refine the monotonic taint engine, or patch the execution boundary to neutralize the specific threat.
*   **The Output:** Committed Rust/Python code that solves a zero-day problem in agentic AI.
*   **The Reciprocal Link:** Because you just solved a highly relevant, real-world problem, the commit *is* the foundation of the blog post. This flows seamlessly into **Content Creation**.

### 3. Content / Demo Creation (The "Megaphone")
Translate the engineering victory into an easily digestible, visually appealing format optimized for Google Discover and social networks.

*   **The Setup:** A standardized terminal-recording pipeline using [VHS](https://github.com/charmbracelet/vhs) (`.tape` files).
*   **The Action:** Write a `.tape` script that shows the new attack succeeding *without* the harness, and failing (resulting in `DENY`) *with* the harness. Render a high-res GIF.
*   **The Output:** An MDX blog post (following the `BLOG_PLAN.md` framework) featuring the GIF, the architectural reasoning, and a link to the code.
*   **The Reciprocal Link:** When the article is seeded to Reddit (`r/LocalLLaMA`, `r/rust`) or Hacker News, senior engineers will attempt to poke holes in the logic (e.g., *"But what if the attacker uses technique X?"*). Those comments flow directly back into the **Discovery** layer as the next research topic.

---

## MVP Setup (Bootstrap Checklist)

To get this flywheel spinning quickly, implement these three automation steps:

1.  [x] **Automate Discovery (Cron Script):** Write a 50-line script that hits the Hacker News/arXiv APIs for target keywords, pipes the JSON to an LLM for summarization, and appends it to a Markdown file. *Built differently (2026-10-27): a scheduled cloud routine that files findings in Linear rather than a log file; see [Cadence](#cadence-the-clock).*
2.  [ ] **Standardize the Demo Pipeline:** Create a template `.tape` file. Ensure that whenever a new security test case is written, running `make demo` automatically spits out a polished, high-res GIF of the terminal output.
3.  [ ] **Template the Blog:** Create a `DeepDive.md` template in the Astro setup with pre-filled headers: *The Threat*, *Why Current Defenses Fail*, *The Deterministic Solution*, and *The Demo*.

## The Daily Loop (Filesystem Kanban)
1. Check `_tasks/1_discovery/` for new vulnerabilities found by Codex.
2. Review the task and assign it to Claude Code for Development (`_tasks/2_development/`).
3. Claude builds the defense, tests it, and moves it to `_tasks/3_advocacy/`.
4. Claude Code, as the Megaphone, monitors `3_advocacy/`, generates the `.tape` proof and writes the `DeepDive.md` blog (hero images still come from Antigravity, the only agent with image generation); a correcting-review pass then audits the result for accuracy/SEO and fixes it in place before the task lands in `_tasks/4_done/`.
5. Review social feedback to seed tomorrow's `1_discovery/`.

## Cadence (the clock)

Everything above says *what* each phase does. This section says *when* it runs, so the
question "what should I do now?" never has to be asked. Each phase has a clock (a
schedule or an event), a runner, and an output that lands where the maintainer already
looks.

**The inbox rule.** Every automated phase ends in a pull request or in a Linear issue
assigned to the maintainer, labelled `flywheel`. "A file appeared in a folder" is not an
output: nobody watches folders, which is how findings sat in `1_discovery/` for three
months.

**Prompts live in the repository.** Each scheduled job is a one-line pointer: "follow
`docs/flywheel/<phase>.md`". The prompt is in [`docs/flywheel/`](flywheel/), versioned and
reviewed like code; changing how a phase behaves is a PR, not an edit in a web console.

| Clock | Phase | Runner | Output | Status |
|---|---|---|---|---|
| Mon + Thu, 08:10 Asia/Jerusalem | **Radar** ([`radar.md`](flywheel/radar.md)) | Claude Code cloud routine *Flywheel Radar* | ≤ 3 scored findings as Linear issues (label `radar`, state Backlog) + a `[Radar digest]` issue due next day | live |
| any time, ~5 min | **Triage** | maintainer | finding → Todo (accept) or Canceled (decline; Radar won't bring it back) | live |
| Sun, around 19:00 Asia/Jerusalem (the app adds a few minutes to the 18:50 cron) | **Pulse** ([`pulse.md`](flywheel/pulse.md)) | local scheduled task in the Claude desktop app | the *Flywheel Pulse* dashboard page, a `[Pulse]` Linear issue, one desktop notification naming **the one next step** | live |
| Tue + Fri, if WIP < 2 | **Engine** | cloud routine | top accepted finding → branch, failing test, defense, draft PR | planned |
| a `flywheel` PR merges | **Megaphone** | routine on a GitHub webhook | `.tape` demo + post + `blog/social/<slug>.md` as a draft PR | planned |
| a post is published | **Seed** | routine | `[Seed]` issue with ready copy and a due date (posting stays human-only) | planned |
| 3 and 7 days after seeding | **Harvest** | routine | objections from HN/Reddit comments → new `radar` findings | planned |

**Where a finding lives.** A Radar finding starts in Linear (Backlog = untriaged), not as
a file, so declined findings never touch the repository. On acceptance the issue carries
a ready discovery-file draft; whoever picks it up commits it to `_tasks/1_discovery/`
(needs more research) or straight to `_tasks/2_development/` (ready to build), and from
there the Filesystem Kanban above applies unchanged. Findings filed by hand keep starting
as files.

**Rules that keep it moving** (Pulse checks them every week):

- **WIP limit 2.** No more than two items in `2_development/` or two open non-draft PRs.
- **Aging 14 days.** An item unchanged in one column for two weeks needs a decision: do
  it now, park it in `docs/RESEARCH-BACKLOG.md`, or close it.
- **Noise cap.** Radar files at most three findings per run, only those scoring ≥ 7.
- **Human-only stays human.** Triage, merges, tags and social posting are the
  maintainer's (`.uh/brief.md`); the engine prepares and asks, it never publishes.

**How it reaches the maintainer.** Three channels, so one silent channel is not a silent
engine. Each run ends with a one-line push notification (cloud routines can send one to
the phone; the first Radar run confirmed it). Linear adds a reminder through the due date
on the digest and Pulse issues (issues you create for yourself raise no other
notification). And the *Flywheel Pulse* page is the standing dashboard. Cloud routines
run with the computer off; the Pulse task runs only while the desktop app is open (a
missed Sunday runs on the next launch).

**Network.** Cloud routines reach only the hosts allowlisted in their environment. Radar
lists any source it could not resolve in its digest under *Broken sources*; allowing those
hosts in the environment's network settings is a one-time maintainer step.

The scheduled Radar runs as Claude Code, the role's fallback (`radar: codex →
claude-code`), because cloud routines are what can run on a clock today; Codex can still
make ad-hoc Radar passes into the same Linear inbox.

---

## Multi-Agent Parallel Workflow (No-Conflict Architecture)

It is entirely possible to have several agent sessions running simultaneously on your machine, each driving a phase of the Flywheel: **Claude Code**, **Codex** and **Antigravity**, or two Claude Code sessions in different roles (the Engine and the Megaphone are both Claude Code now). Roles, not agents, own the domains below. To prevent git conflicts or file clobbering, they must adhere to a strict **Isolation and Handoff Protocol**.

### 1. Agent Roles & Directory Boundaries

Agents are restricted to specific directories. They are not allowed to modify files outside their domain.

*   **Codex (The Radar - Discovery)**
    *   **Domain:** `_tasks/1_discovery/`, `scripts/`
    *   **Task:** Runs continuous background scripts querying Hacker News/arXiv. Summarizes findings into individual markdown files.
    *   **Write Access:** Strictly restricted to creating new files in `_tasks/1_discovery/`.
*   **Claude Code (The Engine - Development)**
    *   **Domain:** `crates/`, `src/`, `tests/`, `_tasks/2_development/`
    *   **Task:** Polls `_tasks/2_development/`. When a new task appears, it writes failing tests, implements the Rust/Python defense, and ensures the suite goes green.
    *   **Write Access:** Modifies core code. Upon success, it appends a technical summary to the task file and uses `mv` to send it to `_tasks/3_advocacy/`.
*   **Claude Code (The Megaphone - Advocacy)** — fallbacks: Codex, then Antigravity
    *   **Domain:** `docs/`, `demos/`, `blog/`, `_tasks/3_advocacy/`
    *   **Task:** Polls `_tasks/3_advocacy/`. When a task arrives, it generates `.tape` files, scaffolds the Astro MDX post, and formats architecture diagrams. Hero images are delegated to Antigravity (see `_tasks/3_advocacy/hero-illustrations.md`). Antigravity held this role until 2026-10; it is rarely used now.
    *   **Write Access:** Modifies documentation/blog. Uses `mv` to archive the completed task to `_tasks/4_done/`.
*   **Codex (The Critic — Correcting Review)** — *cross-cutting role*; fallback: Claude Code
    *   **Domain:** reads everything; may correct in `crates/`, `src/`, `tests/`, `docs/`, `blog/`.
    *   **Task:** Audits a handed-off artifact before it reaches `_tasks/4_done/` — code for correctness and regressions, content for **technical accuracy** (does the prose match the *real* kernel, commands, and manifest schema?) and for Google Discover / SEO hygiene.
    *   **Write Access:** Fixes defects **in place** — the "correcting reviewer" pattern — then appends a short *Review* note to the task file describing what changed. Codex reviews first because the Megaphone is Claude Code, so posts get a reviewer other than their author; the procedure is in `.claude/agents/correcting-reviewer.md` and `.claude/commands/review-blog.md` (Claude Code runs it as `/review-blog`). It runs as a *serialized pass* on an artifact whose owner is idle (never concurrently), so the no-conflict guarantee holds even though it crosses domains. It re-queues to `2_development/` or `3_advocacy/` only when a fix genuinely needs the owner's rework.

### 2. The Handoff Mechanism (Filesystem Kanban)

The golden rule for parallel agent execution: **Agents never communicate by modifying the same files concurrently.** 

Instead of an append-only log that causes race conditions, use a **Filesystem Kanban Board**. 

The structure:
```text
_tasks/
  ├── 1_discovery/           (Inbox for Codex)
  ├── 2_development/         (Inbox for Claude Code)
  ├── 3_advocacy/            (Inbox for the Megaphone: Claude Code)
  └── 4_done/                (Archive)
```

1. **Codex** creates `prompt-injection-bypass.md` in `1_discovery/`. 
2. When ready, the file is moved to `2_development/`.
3. **Claude Code** takes ownership, writes the code, and runs `mv prompt-injection-bypass.md ../3_advocacy/`. 
4. **Claude Code**, as the Megaphone, takes ownership, writes the blog post, and runs `mv prompt-injection-bypass.md ../4_done/`.

**Why this works:** The `mv` command in Linux is an atomic operation. Moving a file instantly transfers "ownership" of the task to the next agent, ensuring zero concurrency conflicts while running totally in parallel.

### 3. Review: a role, not (yet) a phase

Quality control is deliberately a *role*, not an extra Kanban column. A formal review↔fix loop (its own inbox, tickets bouncing back and forth) buys accuracy at the cost of throughput, and that ping-pong overhead isn't worth it until bounce-backs are frequent enough to pay for themselves.

So the **Correcting Reviewer** (above) instead makes a single in-place pass on an artifact as it heads to `4_done/`: it fixes what's wrong and records what it changed, rather than returning a list of complaints. The handoff stays linear — no new phase, no new queue.

Promote this to a real `review ↔ fix` loop **only** when the data justifies it — e.g. if correcting-review passes routinely surface rework an owner must redo. Until there's real profit in the bureaucracy, correct-in-place wins.
