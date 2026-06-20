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
world_id: claude-code-safe-world

# Which action *types* each trust level may perform.
capabilities:
  - { trust: Trusted, actions: [Read, Patch] }
  - { trust: Untrusted, actions: [Read] }

# The base actions that exist at all in this world.
base_actions:
  - name: read_workspace
    action_type: Read
    side_effect: Read
    schema: { type: object, properties: { path: { type: string } } }
  - name: apply_patch
    action_type: Patch
    side_effect: FilesystemWrite
    approval_required: true            # human-in-the-loop before any write
    schema: { type: object, properties: { patch: { type: string } } }

# The narrowed capabilities the agent actually gets (arguments come from the actor).
scoped_capabilities:
  - name: read_repo_file
    base_action: read_workspace
    args: { path: ActorInput }
  - name: apply_workspace_patch
    base_action: apply_patch
    args: { patch: ActorInput }

# Hard floor: tainted input can never drive an effectful boundary.
transition_policies:
  - { from_taint: Tainted, side_effect: Network, decision: Deny, rule: no_tainted_network }
  - { from_taint: Tainted, side_effect: PersistentWrite, decision: Deny, rule: no_tainted_persistent_write }
```

### Step 2: Preview the World Before You Trust It

There's no separate "compile" command to memorize — the harness compiles the manifest into an immutable, hash-addressed `CompiledWorld` whenever it loads it. What you *do* want is to **see** what a manifest actually permits before running anything against it. Launch the built-in World Authoring Tool:

```bash
$ cargo run --bin harness -- serve
World Authoring Tool: http://127.0.0.1:8787  (Ctrl-C to stop)
```

Open the page and paste your `claude-world.yaml`. The tool compiles it through the **real** kernel and shows two things live: the projected tool surface, and a clean-vs-tainted decision matrix for every action. No guessing — you can see that `apply_workspace_patch` resolves to `ASK`, that a tainted `fetch_web` is `DENY`, and that anything you didn't declare is simply `ABSENT`.

### Step 3: Run the Agent Loop Under Governance

Today the harness drives a governed agent loop directly: every proposed tool call passes through the one gate before anything touches your machine.

```bash
$ cargo run --bin harness -- --world claude-world.yaml
CLI Agent Harness initialized.
World: claude-code-safe-world
Mode: Interactive | Effect: Execute
```

Each turn, the model may only propose from the projected surface; the kernel turns each proposal into a sealed `IntentIR`, evaluates it against the compiled world, and lowers **only** an `ALLOW` to an `ExecutionSpec` that crosses into real execution. Add `--simulate` to dry-run with no side effects, or `--background` to make approval-required actions fail closed (`ASK → DENY`).

> **On wrapping Claude Code itself:** transparently proxying an external `claude-code` (or Aider/Codex) process over MCP — so you keep its native UX while the harness governs its tool traffic — is the direction the `safe-mcp-proxy` work points at. It is *not* what the `harness` binary does today; today the binary runs its own governed loop. The MCP-proxy path is a follow-up.

### The Approval Prompt

When the agent proposes an approval-required action (here, `apply_workspace_patch`), the kernel returns `ASK` and the harness pauses to prompt you — printing the action and its exact arguments, and waiting, defaulting to *No*:

```text
[APPROVAL REQUIRED]
Action: apply_workspace_patch
Arguments: {
  "patch": "--- a/src/styles/global.css\n+++ b/src/styles/global.css\n@@\n+  --glass-bg: rgba(15, 23, 42, 0.6);"
}
? Approve this action? (y/N)
```

Type `y` and a durable `ApprovalToken` is minted, bound to that exact call — if the call drifts, the token is void. Type `n` and the action collapses to `DENY` and the loop moves on. Either way, you keep deterministic control over every side effect.

You maintain absolute deterministic control over the agent's side-effects, allowing you to use cutting-edge AI tools without betting your entire local environment on their reliability.
