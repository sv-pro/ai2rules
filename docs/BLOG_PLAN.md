# Blog Strategy & Publishing Plan: CLI Agent Harness

This document outlines a comprehensive plan for launching and maintaining a technical blog centered around the **CLI Agent Harness**. The plan is specifically optimized for **Google Discover**, which favors high-engagement, visually appealing, fast-loading, and authoritative content (E-E-A-T).

## 1. The Stack (Optimized for Google Discover)

Google Discover does not rely on search queries; it relies on an algorithmic feed. To succeed, the technical foundation must prioritize **Core Web Vitals (speed)**, **mobile-first design**, and **structured data**.

*   **Framework:** **Astro** (Highly recommended for content-heavy sites). It ships zero JavaScript by default, ensuring perfect Lighthouse scores and instant mobile load times, which is critical for Discover visibility. Alternative: **Next.js** (App Router) if you plan to build complex interactive features directly into the blog.
*   **Content Management:** **MDX** (local markdown files with React/UI components) or a headless CMS like **Sanity.io** / **Contentful**. Given the target audience (developers), an MDX-based workflow tied to a Git repository is usually best.
*   **Styling:** **Tailwind CSS** or modern Vanilla CSS. Use a visually striking dark-mode aesthetic (glassmorphism, vibrant gradients) to reflect the cutting-edge nature of AI tools.
*   **Image Pipeline (CRITICAL):** Google Discover *requires* large, high-quality images. The stack must automatically optimize and serve WebP/AVIF formats. 
    *   *Rule:* Max-image preview must be set to `large` in meta tags (`<meta name="robots" content="max-image-preview:large">`). 
    *   *Rule:* Images must be at least 1200px wide.
*   **SEO & Discovery:** 
    *   Automated generation of `Article` and `TechArticle` JSON-LD Schema markup.
    *   **WebSub / RSS:** Implement RSS and WebSub pinging so Google's crawlers are instantly notified of new posts.
    *   Automated OpenGraph and Twitter Card image generation (e.g., using Vercel OG).

## 2. Content Framework (Ongoing Engine)

To sustain a feed like Google Discover, you need a repeatable framework. The "Hub and Spoke" model works best, divided into three recurring content types:

### A. The "Thought Leadership / Hot Take" (Spike Traffic)
*   **Purpose:** To capture attention on Discover feeds by addressing current pain points, trends, or controversial topics in the AI space. 
*   **Format:** Strong opinions, high-level architecture thoughts, trend analysis.
*   **Cadence:** 1x per month.

### B. The "Deep Dive / Engineering" (Authority & Trust)
*   **Purpose:** To establish E-E-A-T (Experience, Expertise, Authoritativeness, Trustworthiness). Google Discover ranks content higher when the author is a proven expert.
*   **Format:** Behind-the-scenes engineering logs, architectural decision records (ADRs translated to prose), code walkthroughs.
*   **Cadence:** 2x per month.

### C. The "Showcase / Tutorial" (Utility)
*   **Purpose:** To show the harness in action and provide immediate value to developers building local AI tooling.
*   **Format:** "How to build X securely," "Securing your Aider/Claude Code setup," etc.
*   **Cadence:** 1x per month.

## 3. The Publishing Plan: First 5 Articles

These initial articles are designed to hook developers exploring AI agents, leverage current trending keywords (Prompt Injection, AI Agents, Secure CLI), and establish the core philosophy of the CLI Agent Harness.

### Article 1: "Why Prompt Injection Isn't Just a Prompt Problem—It's an Authority Boundary Problem"
*   **Type:** Thought Leadership
*   **Angle:** Most developers are trying to solve prompt injection with better LLM rules. This article argues that you can't filter out danger with an LLM; you must compile the physics of the world the agent lives in. 
*   **Discover Hook:** The title challenges a common developer assumption.
*   **Visual:** A stark diagram contrasting a "Wrapper/Filter" architecture vs. the "CLI Agent Harness Kernel" architecture.

### Article 2: "Stop Giving Local AI Ambient Authority: Introducing the CLI Agent Harness"
*   **Type:** Showcase / Launch
*   **Angle:** A formal introduction to the project. Explaining the danger of giving Codex, Claude Code, or Aider ambient developer authority (SSH keys, `rm -rf /`), and how the Harness provides a deterministic virtualization layer.
*   **Discover Hook:** Addresses a silent fear every developer has when running autonomous agents locally.

### Article 3: "Monotonic Taint in AI Execution: How We Track Untrusted Context"
*   **Type:** Deep Dive / Engineering
*   **Angle:** Based on `docs/harness-architecture.md`, exploring the concept of "Taint" in the execution boundary. How data enters, gets tainted, and how the kernel ensures tainted data never drives network egress or memory writes.
*   **Discover Hook:** Highly technical, appeals to security engineers and systems developers. Use code snippets and flowcharts.

### Article 4: "Designing a Deterministic Kernel for Non-Deterministic AI"
*   **Type:** Deep Dive / Engineering
*   **Angle:** Discussing the "Design-time stochastic, runtime deterministic" philosophy. Why the LLM does *not* sit on the runtime policy enforcement path, and how the `WorldManifest` translates into an immutable `CompiledWorld`.
*   **Discover Hook:** Explores the intersection of Rust's safety guarantees with unpredictable LLM outputs.

### Article 5: "Running Claude Code Safely: A Sandbox Setup Guide"
*   **Type:** Tutorial
*   **Angle:** A practical guide using the `cli-harness` to wrap an existing CLI agent. Shows the interactive approval UI (`ASK` vs `DENY`), demonstrating how destructive commands are halted.
*   **Discover Hook:** Capitalizes on the popularity of "Claude Code" while providing a secure workflow for it.

## 4. Google Discover Optimization Checklist for Every Post

Before publishing any article, ensure it meets these criteria:

1.  [ ] **Hero Image:** Custom-designed, vibrant, and highly communicative. Avoid generic stock photos. Size: >1200px wide. Aspect ratio: 16:9.
2.  [ ] **Title:** Compelling but **not clickbait**. Discover aggressively punishes clickbait. It should accurately reflect the content.
3.  [ ] **Author Bio:** Include a clear author bio with links to GitHub/Twitter to build E-E-A-T.
4.  [ ] **Mobile Readability:** Short paragraphs, plenty of headings, and legible code blocks on small screens.
5.  [ ] **The "Scroll Stopper":** Place a highly engaging diagram, architecture map, or code snippet within the first 2-3 scrolls of the article.
6.  [ ] **Meta Tags:** Ensure `<meta name="robots" content="max-image-preview:large">` is present.

## 5. Promotion & Seeding (Kickstarting Discover)

Google Discover picks up articles that already have a heartbeat. You must seed the articles to get initial velocity:
*   **Hacker News:** Submit the Engineering Deep Dives (Articles 3 & 4) and the Architecture argument (Article 1).
*   **Reddit:** Share specific security insights in `r/LocalLLaMA`, `r/MachineLearning`, and `r/rust`.
*   **X (Twitter):** Thread out the architecture diagram and the core philosophy ("Absence over denial").

## 6. Catchy Article Ideas (From Research Repos)

Drawing inspiration from the `agent-hypervisor`, `safe-mcp-proxy`, and `mcp-tool-projection` repositories, here are 20 catchy, Discover-optimized article titles:

1. **"Why 'Deny' is Dangerous: The Case for Absent Tools in AI"** (Focus: safe-mcp-proxy's ABSENT vs DENY semantics)
2. **"AI Aikido: Using Deterministic Rules to Neutralize Prompt Injection"** (Focus: agent-hypervisor's core design philosophy)
3. **"The ZombieAgent Threat: Why Your AI's Memory is a Ticking Time Bomb"** (Focus: Cross-session taint tracking)
4. **"We Replaced LLM Security Filters with Table Lookups—Here's Why"** (Focus: Deterministic runtime vs LLM evaluation)
5. **"The MCP Supply Chain Crisis (And How to Proxy Your Way Out of It)"** (Focus: safe-mcp-proxy's supply chain mitigation)
6. **"Stop Hardcoding AI Tools: Meet Declarative MCP Projections"** (Focus: mcp-tool-projection's YAML-based virtualization)
7. **"How to Stop a Poisoned Tool Descriptor in Its Tracks"** (Focus: safe-mcp-proxy's SHA256 descriptor drift detection)
8. **"Design-Time vs. Runtime Security: The Fatal Flaw in LLM Guardrails"** (Focus: agent-hypervisor's O(log n) design-time HITL)
9. **"Your Local Agent is Leaking: A Guide to Monotonic Taint Tracking"** (Focus: safe-mcp-proxy's data flow constraints)
10. **"Building an AI Firewall: The 4 Layers of Execution Governance"** (Focus: agent-hypervisor's architectural stack)
11. **"Simulating Reality: Safely Mocking MCP Tools for AI Agents"** (Focus: mcp-tool-projection's `simulated` kind)
12. **"From Open-for-Execution to Closed-for-Execution: A New AI Paradigm"** (Focus: The manifest resolution law)
13. **"The Illusion of Control: Why Agent Wrappers Fail (And Kernels Succeed)"** (Focus: Kernel vs wrapper architectural debate)
14. **"Protecting Jira and Confluence from Your Own Autonomous Agents"** (Focus: The Atlassian MCP proxy use-case)
15. **"The Missing Layer in Model Context Protocol"** (Focus: Securing MCP's runtime vulnerability)
16. **"Why Your AI Needs a 'Hypervisor' (And What That Actually Means)"** (Focus: Introduction to the hypervisor pattern)
17. **"The End of O(n) Runtime Approvals: Scaling Security at Design-Time"** (Focus: Why per-query HITL doesn't scale)
18. **"Don't Let the Agent Decide: Deterministic Proxies for MCP"** (Focus: safe-mcp-proxy's deterministic policy engine)
19. **"How We Achieved a 0% Attack Success Rate on the AgentDojo Benchmark"** (Focus: Empirical results of the hypervisor)
20. **"Taming the Tool Surface: How to Strip and Narrow MCP Capabilities"** (Focus: mcp-tool-projection's `partial` projections)
