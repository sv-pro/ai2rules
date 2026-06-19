---
title: 'Running Claude Code Safely: A Sandbox Setup Guide'
description: 'A practical guide to wrapping existing CLI agents with deterministic physics.'
pubDate: 'Jun 16 2026'
heroImage: '../../assets/blog-placeholder-4.jpg'
---

Claude Code, Aider, and Codex are incredible developer tools. They can refactor entire codebases, write tests, and debug complex race conditions. But out of the box, they run with **Ambient Authority**. They inherit your SSH keys, your production AWS credentials, and root-level write access to your hard drive. 

If they hallucinate—or worse, if they read a poisoned `README.md`—the blast radius is your entire machine.

Here is how you can use the **CLI Agent Harness** to sandbox them without losing their utility.

### Step 1: Define the Physics

Instead of letting the agent dictate what it can do, you define its universe using a `WorldManifest`. This file acts as the constitutional law for the agent.

Create a `claude-world.yaml` file in your workspace:

```yaml
id: "claude-code-safe-world"
version: "1.0.0"

# What the agent can see
percepts:
  - type: "filesystem"
    path: "./src"
    access: "read_only"

# What the agent can do
capabilities:
  - name: "read_repo_file"
    action: "read_file"
    constraints:
      cwd: "./src"
      max_bytes: 50000

  - name: "apply_patch"
    action: "write_file"
    policy: "ASK" # The critical human-in-the-loop setting
    constraints:
      allowed_extensions: [".ts", ".tsx", ".css", ".astro"]
```

### Step 2: Compile the World

The harness takes this human-readable YAML and compiles it into an immutable, hashed binary representation that the execution engine uses.

```bash
$ cli-harness compile claude-world.yaml
Compiling WorldManifest...
Success. Hash: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
World is sealed.
```

### Step 3: Launching the Agent via the Proxy

You don't run Claude Code directly anymore. You run it *through* the Harness, which intercepts all MCP (Model Context Protocol) traffic.

```bash
$ cli-harness run --world claude-world.yaml -- npx @anthropic-ai/claude-code
```

The Harness boots up a `safe-mcp-proxy` instance. It translates Claude's tool requests into `IntentIR` objects, validates them against the compiled world, and executes them if permitted.

### The Interactive TUI

When Claude Code decides it wants to write a file (`apply_patch`), the harness interrupts the execution boundary. Because we set the policy to `ASK` in the manifest, the Harness pauses the agent and renders a Terminal UI (TUI) prompt for the developer.

```text
=========================================================
 🛡️ HARNESS INTERCEPTION: ACTION REQUIRES APPROVAL
=========================================================
Agent:    Claude Code
Action:   apply_patch (write_file)
Target:   ./src/styles/global.css

Payload Diff:
+  --glass-bg: rgba(15, 23, 42, 0.6);
+  --glass-border: rgba(255, 255, 255, 0.1);

Approve this execution? (y/N)
```

If you type `y`, a durable `ApprovalToken` is minted, and the execution proceeds. If you type `n`, the action collapses. The Harness returns a simulated error to Claude Code, forcing it to re-plan or explain itself.

You maintain absolute deterministic control over the agent's side-effects, allowing you to use cutting-edge AI tools without betting your entire local environment on their reliability.
