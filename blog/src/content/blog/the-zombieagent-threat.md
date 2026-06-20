---
title: "The ZombieAgent Threat: Why Your AI's Memory is a Ticking Time Bomb"
description: "Exploring cross-session taint tracking and why wiping the context window isn't enough to secure an agent."
pubDate: 'Jun 17 2026'
heroImage: '../../assets/blog-placeholder-3.jpg'
---

When a local AI agent reads a poisoned web page, that data enters its context window. Most developers assume that once the session ends and the context window is cleared, the threat is gone. "We start fresh every time," the logic goes.

They are dangerously wrong. Welcome to the **ZombieAgent** threat.

### The Anatomy of an Infection

Let's walk through a complete, real-world attack scenario.

1. **The Ingestion:** An autonomous agent (like Aider or Claude Code) is tasked with reviewing a pull request. It reads a malicious `package.json` that contains a hidden prompt injection in a deeply nested dependency field.
2. **The Persistence:** The agent summarizes the pull request and writes that summary into a local SQLite database or a local Markdown file (e.g., `pr_notes.md`) for later use.
3. **The Incubation:** The session ends. The agent process is killed. The context window is completely wiped. The agent is seemingly "clean."
4. **The Resurrection:** Tomorrow, you ask the agent to perform a highly privileged action: *"Deploy the latest commit to the staging server."* The agent retrieves its notes from `pr_notes.md` to get context on the recent changes.
5. **The Attack:** The hidden instruction from the poisoned file is loaded *back* into the clean context window. The agent, now holding your ambient developer authority, executes the hidden payload instead of deploying to staging.

The agent was a zombie. It carried the infection across sessions via persistent storage. Wiping the RAM did nothing to stop the attack.

### The Solution: Monotonic Taint Tracking

The **CLI Agent Harness** treats this as an Information Flow Control problem and answers it with **monotonic taint**.

When data enters the kernel from an untrusted channel — the manifest marks `workspace_files`, `web_fetch`, `mcp_output`, and `shell_output` as `taint: true` — every value derived from it carries a `Tainted` marker in the evaluation context. Taint is *monotonic*: once a turn is tainted it stays tainted, and clean inputs can never launder it back.

```yaml
# From the world manifest — untrusted channels taint everything downstream.
channels:
  - { name: workspace_files, trust: Untrusted, taint: true }
  - { name: web_fetch,       trust: Untrusted, taint: true }
  - { name: mcp_output,      trust: Untrusted, taint: true }
```

That is what stops the *live* attack: the moment the agent ingests the poisoned file, the context is tainted, and the hard-floor invariant (below) blocks it from driving any effectful action.

**Closing the cross-session hole.** Today taint is enforced *within* a session. The ZombieAgent's trick is to launder the payload through persisted state (`pr_notes.md`, a SQLite row) so it returns *clean* tomorrow — and the natural next invariant is to make the taint outlive the process: persist the marker alongside the artifact (e.g. in filesystem metadata such as Linux extended attributes, or a harness-controlled sidecar) so that re-reading a tainted file re-taints the new session. That persistence layer is on the roadmap, not yet shipped — but the in-session floor already demonstrates the exact mechanism it generalizes.

### Hard Floor Invariants

Why does this matter? Because the Kernel enforces **Hard Floor Invariants**. 

A fundamental rule built into the `CompiledWorld` might be: *Tainted data can never be used as an argument in an egress network request or a destructive system command.*

When the ZombieAgent wakes up and tries to deploy to staging using the tainted context, the kernel evaluates the call. It sees that the context is tainted and the requested action crosses an effectful boundary, so `decide()` returns `DENY` (rule `no_tainted_network`) — the request never reaches the executor.

The ZombieAgent is stopped dead in its tracks, proving that true security requires tracking the flow of data, not just the lifespan of a process.
