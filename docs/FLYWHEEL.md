# The Engineering Flywheel: Discovery, Development, and Advocacy

This document defines the architecture of a reciprocal-feeding system (a "flywheel") designed to continuously generate high-quality research, code, and content. By interconnecting these three pillars, the output of one phase automatically becomes the input for the next, eliminating context-switching and preventing "writer's block."

## The Flywheel Architecture

### 1. Content Discovery (The "Radar")
The goal of this phase is to automate the ingestion of the state-of-the-art in AI security, filtering strictly for signal over noise.

*   **The Setup:** An autonomous agent (built dogfooding the `cli-agent` harness or `safe-mcp-proxy`) running on a daily `cron` schedule.
*   **The Action:** The agent fetches top posts from the Hacker News API, new papers from the `cs.CR` (Cryptography and Security) category on arXiv, and specific security blogs via RSS.
*   **The Filter:** The LLM evaluates the feed against a strict prompt: *"Does this relate to prompt injection, agent sandboxing, capability security, or LLM vulnerabilities? Score 1-10."*
*   **The Output:** A daily summary appended to `_research/DISCOVERY_LOG.md`.
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

1.  [ ] **Automate Discovery (Cron Script):** Write a 50-line script that hits the Hacker News/arXiv APIs for target keywords, pipes the JSON to an LLM for summarization, and appends it to a Markdown file.
2.  [ ] **Standardize the Demo Pipeline:** Create a template `.tape` file. Ensure that whenever a new security test case is written, running `make demo` automatically spits out a polished, high-res GIF of the terminal output.
3.  [ ] **Template the Blog:** Create a `DeepDive.md` template in the Astro setup with pre-filled headers: *The Threat*, *Why Current Defenses Fail*, *The Deterministic Solution*, and *The Demo*.

## The Daily Loop (Filesystem Kanban)
1. Check `_tasks/1_discovery/` for new vulnerabilities found by Codex.
2. Review the task and assign it to Claude Code for Development (`_tasks/2_development/`).
3. Claude builds the defense, tests it, and moves it to `_tasks/3_advocacy/`.
4. Antigravity monitors `3_advocacy/`, generates the `.tape` proof and writes the `DeepDive.md` blog; a correcting-review pass then audits the result for accuracy/SEO and fixes it in place before the task lands in `_tasks/4_done/`.
5. Review social feedback to seed tomorrow's `1_discovery/`.

---

## Multi-Agent Parallel Workflow (No-Conflict Architecture)

It is entirely possible to have **Claude Code**, **Codex**, and **Antigravity** running simultaneously on your machine, each driving a phase of the Flywheel. To prevent git conflicts or file clobbering, they must adhere to a strict **Isolation and Handoff Protocol**.

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
*   **Antigravity (The Megaphone - Advocacy)**
    *   **Domain:** `docs/`, `demos/`, `blog/`, `_tasks/3_advocacy/`
    *   **Task:** Polls `_tasks/3_advocacy/`. When a task arrives, Antigravity generates `.tape` files, scaffolds the Astro MDX post, and formats architecture diagrams.
    *   **Write Access:** Modifies documentation/blog. Uses `mv` to archive the completed task to `_tasks/4_done/`.
*   **Claude Code (The Critic — Correcting Review)** — *cross-cutting role*
    *   **Domain:** reads everything; may correct in `crates/`, `src/`, `tests/`, `docs/`, `blog/`.
    *   **Task:** Audits a handed-off artifact before it reaches `_tasks/4_done/` — code for correctness and regressions, content for **technical accuracy** (does the prose match the *real* kernel, commands, and manifest schema?) and for Google Discover / SEO hygiene.
    *   **Write Access:** Fixes defects **in place** — the "correcting reviewer" pattern — then appends a short *Review* note to the task file describing what changed. It runs as a *serialized pass* on an artifact whose owner is idle (never concurrently), so the no-conflict guarantee holds even though it crosses domains. It re-queues to `2_development/` or `3_advocacy/` only when a fix genuinely needs the owner's rework.

### 2. The Handoff Mechanism (Filesystem Kanban)

The golden rule for parallel agent execution: **Agents never communicate by modifying the same files concurrently.** 

Instead of an append-only log that causes race conditions, use a **Filesystem Kanban Board**. 

The structure:
```text
_tasks/
  ├── 1_discovery/           (Inbox for Codex)
  ├── 2_development/         (Inbox for Claude Code)
  ├── 3_advocacy/            (Inbox for Antigravity)
  └── 4_done/                (Archive)
```

1. **Codex** creates `prompt-injection-bypass.md` in `1_discovery/`. 
2. When ready, the file is moved to `2_development/`.
3. **Claude Code** takes ownership, writes the code, and runs `mv prompt-injection-bypass.md ../3_advocacy/`. 
4. **Antigravity** takes ownership, writes the blog post, and runs `mv prompt-injection-bypass.md ../4_done/`.

**Why this works:** The `mv` command in Linux is an atomic operation. Moving a file instantly transfers "ownership" of the task to the next agent, ensuring zero concurrency conflicts while running totally in parallel.

### 3. Review: a role, not (yet) a phase

Quality control is deliberately a *role*, not an extra Kanban column. A formal review↔fix loop (its own inbox, tickets bouncing back and forth) buys accuracy at the cost of throughput, and that ping-pong overhead isn't worth it until bounce-backs are frequent enough to pay for themselves.

So the **Correcting Reviewer** (above) instead makes a single in-place pass on an artifact as it heads to `4_done/`: it fixes what's wrong and records what it changed, rather than returning a list of complaints. The handoff stays linear — no new phase, no new queue.

Promote this to a real `review ↔ fix` loop **only** when the data justifies it — e.g. if correcting-review passes routinely surface rework an owner must redo. Until there's real profit in the bureaucracy, correct-in-place wins.
