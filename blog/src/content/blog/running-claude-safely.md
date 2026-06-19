---
title: 'Running Claude Code Safely: A Sandbox Setup Guide'
description: 'A practical guide to wrapping existing CLI agents with deterministic physics.'
pubDate: 'Jun 16 2026'
heroImage: '../../assets/blog-placeholder-4.jpg'
---

Claude Code and Aider are incredible developer tools, but giving them ambient access to your SSH keys and entire filesystem is a massive security risk. 

Here is how you can use the **CLI Agent Harness** to sandbox them without losing their utility.

### 1. Define the Physics
Instead of letting the agent dictate what it can do, you define its universe using a `WorldManifest`.

```yaml
id: "claude-code-safe-world"
capabilities:
  - name: "read_repo_file"
    action: "read_file"
    constraints:
      cwd: "./src"
  - name: "apply_patch"
    action: "write_file"
    policy: "ASK"
```

### 2. Compile the World
The harness compiles this YAML into a `CompiledWorld`, sealing the definitions behind a SHA-256 descriptor hash. The agent is now physically bound by this world. 

### 3. The Interactive TUI
When Claude Code decides it wants to write a file (`apply_patch`), the harness interrupts the execution boundary. Because the policy is set to `ASK`, a terminal UI prompts the developer.

```text
[ASK] Claude Code is attempting to execute `apply_patch`
Reason: Update index.astro with new CSS.
Effect: Modifies local filesystem.

Approve? (y/N)
```

If you approve, a durable `ApprovalToken` is minted, and the execution proceeds. If you deny, the action collapses, and Claude Code is forced to re-plan. You maintain absolute deterministic control over the agent's side-effects.
