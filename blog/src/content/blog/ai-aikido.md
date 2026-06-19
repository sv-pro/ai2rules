---
title: 'AI Aikido: Neutralizing Prompt Injection with Determinism'
description: 'How to use an attackers momentum against them by replacing LLM filters with deterministic tables.'
pubDate: 'Jun 18 2026'
heroImage: '../../assets/blog-placeholder-2.jpg'
---

Prompt injection is the defining vulnerability of the LLM era. The traditional defense mechanism adopted by most enterprises is to place another LLM in front of the execution layer to ask: *"Is this prompt malicious?"*

This is a losing battle. LLMs are inherently non-deterministic; they can always be tricked, cajoled, or confused into bypassing their own guardrails. If your security relies on an LLM making a correct classification 100% of the time, your system is already compromised.

### The Aikido Philosophy

In martial arts, Aikido uses the attacker's momentum against them. In the **CLI Agent Harness**, we use the attacker's reliance on the LLM against them by entirely removing the LLM from the security path.

We call this architectural pattern **Design-Time Stochastic, Runtime Deterministic**.

#### 1. Design Time (Stochastic)
We acknowledge that LLMs are incredible at synthesis and creativity. We use them at *design time* to draft complex `WorldManifests`. We let the stochastic model do the heavy lifting of deciding what capabilities *might* be useful for a specific workflow, generating the YAML definitions for tools and policies.

```yaml
# A generated manifest for a CI/CD agent
id: "ci-agent-world"
capabilities:
  - name: "read_repo"
    action: "read_file"
    constraints:
      path_prefix: "/workspace/src"
  - name: "trigger_build"
    action: "http_post"
    constraints:
      url_whitelist: ["https://ci.internal.corp/build"]
```

#### 2. Compile Time (Deterministic)
A human engineer reviews the manifest, and the harness compiles it into an immutable, hashed `CompiledWorld`. This compilation step transforms human-readable constraints into strict, heavily optimized execution graphs.

#### 3. Runtime (Deterministic)
When an attacker injects a prompt into the agent, the agent attempts to execute a malicious `ToolCall`. The kernel evaluates this call against the deterministic `CompiledWorld`.

There is no LLM evaluating the action. There is no fuzzy logic. It is a pure, `O(1)` table lookup.

```rust
// The execution engine is purely deterministic
fn evaluate_action(
    compiled_world: &CompiledWorld, 
    action: &IntentIR
) -> Result<ExecutionSpec, SecurityError> {
    
    // O(1) hash map lookup
    let policy = compiled_world.policies.get(&action.tool_name)
        .ok_or(SecurityError::ToolAbsent)?;

    // Strict regex and prefix matching, no AI involved
    for constraint in &policy.constraints {
        if !constraint.validate(&action.args) {
            return Err(SecurityError::ConstraintViolation);
        }
    }

    Ok(ExecutionSpec::from(action))
}
```

### The Result

If the taint policy forbids network egress for tainted variables, the action fails. If the requested URL isn't in the exact whitelist, the action fails. 

By shifting the stochastic elements to design time, we guarantee deterministic safety at runtime. The attacker's clever, multi-layered, jailbreaking prompts crash against a mathematically inflexible wall. They are trying to hack a neural network, but they are actually fighting a compiled Rust binary that doesn't understand English.
