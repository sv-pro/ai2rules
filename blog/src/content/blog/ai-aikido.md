---
title: 'AI Aikido: Neutralizing Prompt Injection with Determinism'
description: "How to use an attacker's own momentum against them by replacing LLM filters with deterministic tables."
pubDate: 'Jun 18 2026'
heroImage: '../../assets/ai-aikido.jpg'
---

Prompt injection is the defining vulnerability of the LLM era. The traditional defense mechanism adopted by most enterprises is to place another LLM in front of the execution layer to ask: *"Is this prompt malicious?"*

This is a losing battle. LLMs are inherently non-deterministic; they can always be tricked, cajoled, or confused into bypassing their own guardrails. If your security relies on an LLM making a correct classification 100% of the time, your system is already compromised.

### The Aikido Philosophy

In martial arts, Aikido uses the attacker's momentum against them. In the **CLI Agent Harness**, we use the attacker's reliance on the LLM against them by entirely removing the LLM from the security path.

We call this architectural pattern **Design-Time Stochastic, Runtime Deterministic**.

#### 1. Design Time (Stochastic)
We acknowledge that LLMs are incredible at synthesis and creativity. We use them at *design time* to draft complex `WorldManifests`. We let the stochastic model do the heavy lifting of deciding what capabilities *might* be useful for a specific workflow, generating the YAML definitions for tools and policies.

```yaml
# A drafted manifest fragment: base actions, plus a scoped capability that
# locks an argument to a literal — so the agent gets `run_tests`, never `run_command`.
base_actions:
  - name: read_workspace
    action_type: Read
    side_effect: Read
  - name: run_command
    action_type: Command
    side_effect: Process

scoped_capabilities:
  - name: run_tests
    base_action: run_command
    args:
      command: !Literal pytest        # the actor can invoke it, never re-arg it to "rm -rf /"

# Hard floor: tainted input may never cross an effectful boundary.
transition_policies:
  - { from_taint: Tainted, side_effect: Network, decision: Deny, rule: no_tainted_network }
```

#### 2. Compile Time (Deterministic)
A human engineer reviews the manifest, and the harness compiles it into an immutable, hashed `CompiledWorld`. This compilation step transforms human-readable constraints into strict, heavily optimized execution graphs.

#### 3. Runtime (Deterministic)
When an attacker injects a prompt into the agent, the agent attempts to execute a malicious `ToolCall`. The kernel evaluates this call against the deterministic `CompiledWorld`.

There is no LLM evaluating the action. There is no fuzzy logic. It is a pure, `O(1)` table lookup.

```rust
// Runtime is a pure function of (call, context, compiled world) — no LLM on the path.
// Representability checks seal an `IntentIR`; disposition then applies the contextual
// rules (taint floor, approval, budgets). `decide()` is the single entry point.
// (Simplified, but this is the real shape.)
match decide(&world, &call, provenance, &ctx) {
    KernelOutcome::Evaluated { disposition, intent } if disposition.decision == Decision::Allow => {
        // An ALLOW is the *only* outcome that can be lowered to an ExecutionSpec.
        Ok(build_execution_spec(&intent, &env))
    }
    // Everything else — ABSENT, DENY, ASK, REPLAN — stops here, deterministically.
    outcome => Err(outcome),
}
```

### The Result

If the taint policy forbids network egress for tainted variables, the action fails. If the requested URL isn't in the exact whitelist, the action fails. 

By shifting the stochastic elements to design time, we guarantee deterministic safety at runtime. The attacker's clever, multi-layered, jailbreaking prompts crash against a mathematically inflexible wall. They are trying to hack a neural network, but they are actually fighting a compiled Rust binary that doesn't understand English.
