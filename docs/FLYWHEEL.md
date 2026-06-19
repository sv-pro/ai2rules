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

## The Daily Loop
1. Wake up and read the automated `DISCOVERY_LOG.md`.
2. Pick an interesting vulnerability and build the defense (Development).
3. Run `make demo` to capture the proof.
4. Fill out the `DeepDive.md` template and publish (Advocacy).
5. Read social feedback to seed tomorrow's Discovery.

---

## Multi-Agent Parallel Workflow (No-Conflict Architecture)

It is entirely possible to have **Claude Code**, **Codex**, and **Antigravity** running simultaneously on your machine, each driving a phase of the Flywheel. To prevent git conflicts or file clobbering, they must adhere to a strict **Isolation and Handoff Protocol**.

### 1. Agent Roles & Directory Boundaries

Agents are restricted to specific directories. They are not allowed to modify files outside their domain.

*   **Codex (The Radar - Discovery)**
    *   **Domain:** `_research/`, `scripts/`
    *   **Task:** Runs continuous background scripts querying Hacker News/arXiv. Summarizes findings.
    *   **Write Access:** Strictly restricted to appending to `_research/DISCOVERY_LOG.md`.
*   **Claude Code (The Engine - Development)**
    *   **Domain:** `crates/`, `src/`, `tests/`
    *   **Task:** Polls `_research/DISCOVERY_LOG.md`. When a new attack vector is logged, it writes failing tests, implements the Rust/Python defense, and ensures the suite goes green.
    *   **Write Access:** Modifies core code. Upon success, it appends a technical summary of the fix to `_research/DEFENSE_IMPLEMENTED.md`.
*   **Antigravity (The Megaphone - Advocacy)**
    *   **Domain:** `docs/`, `demos/`, `blog/`
    *   **Task:** Polls `_research/DEFENSE_IMPLEMENTED.md`. When a defense is ready, Antigravity generates the `.tape` files for the VHS terminal recording, scaffolds the Astro MDX blog post, and formats the architecture diagrams.
    *   **Write Access:** Restricted to documentation, media assets, and the blog directory.

### 2. The Handoff Mechanism (Files as Queues)

The golden rule for parallel agent execution: **Agents never communicate by modifying the same code files.** 

Instead, they use append-only Markdown files as asynchronous event queues:
1. Codex pushes to `DISCOVERY_LOG.md`.
2. Claude Code pulls from `DISCOVERY_LOG.md`, writes code, and pushes to `DEFENSE_IMPLEMENTED.md`.
3. Antigravity pulls from `DEFENSE_IMPLEMENTED.md`, generates the content, and pushes to `docs/BLOG_PLAN.md` / social channels.

By using physical files as handoff queues and strictly bounding their working directories (which can be enforced by the `cli-agent` harness itself!), all three agents can spin the flywheel concurrently without a single merge conflict.
