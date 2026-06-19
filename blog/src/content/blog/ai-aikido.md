---
title: 'AI Aikido: Neutralizing Prompt Injection with Determinism'
description: 'How to use an attackers momentum against them by replacing LLM filters with deterministic tables.'
pubDate: 'Jun 18 2026'
heroImage: '../../assets/blog-placeholder-2.jpg'
---

Prompt injection is the defining vulnerability of the LLM era. The traditional defense is to place another LLM in front of the execution layer to ask: *"Is this prompt malicious?"*

This is a losing battle. LLMs are non-deterministic; they can always be tricked.

### The Aikido Philosophy

In martial arts, Aikido uses the attacker's momentum against them. In the **CLI Agent Harness**, we use the attacker's reliance on the LLM against them by entirely removing the LLM from the security path.

We call this **Design-Time Stochastic, Runtime Deterministic**.

1. **Design Time:** We use LLMs to draft complex `WorldManifests`. We let the stochastic model do the creative heavy lifting of deciding what capabilities *might* be useful.
2. **Compile Time:** A human reviews the manifest, and the harness compiles it into an immutable, hashed `CompiledWorld`.
3. **Runtime:** When an attacker injects a prompt into the agent, the agent attempts to execute a malicious `ToolCall`. The kernel evaluates this call against the deterministic `CompiledWorld`.

There is no LLM evaluating the action. It is a pure, `O(1)` table lookup. If the taint policy forbids network egress for tainted variables, the action fails. 

By shifting the stochastic elements to design time, we guarantee deterministic safety at runtime. The attacker's clever prompts crash against a mathematically inflexible wall.
