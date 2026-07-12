---
title: 'Your Agent Just Learned to Write Programs. Can You Still Govern It?'
description: 'GPT-5.6 Programmatic Tool Calling makes tool-heavy agents faster. It also moves the governance boundary from individual calls to generated execution plans.'
pubDate: 'Jul 12 2026'
---

GPT-5.6 introduces a deceptively small change to tool use: **Programmatic Tool
Calling**. Instead of returning to the model after every tool result, the model can
write JavaScript that calls several eligible tools, passes results between them,
runs loops and conditions, and emits a compact result.

That is a major efficiency improvement. It is also a change in the shape of agent
execution.

The old mental model was:

```text
reason → tool call → result → reason → tool call → result
```

The new model can be:

```text
reason → generated program → call A
                           → call B
                           → call C … N
                         → reduced result → reason
```

The model no longer decides only *which tool to call next*. It can generate a
small orchestration program that derives future calls from earlier results.

This does not make agents ungovernable. But it does invalidate governance systems
that treat one model turn as one authorization decision.

> **The unit of intent just became larger. The unit of enforcement must remain
> the individual effect.**

## The runtime is constrained — the effects are not automatically safe

OpenAI runs generated programs in a fresh isolated V8 runtime. It has no Node.js,
package installation, direct network access, general-purpose filesystem,
subprocess execution, console, or persistent JavaScript state. A program can
reach the outside world only through tools enabled in the request.

That is a good sandbox boundary. It prevents the generated JavaScript from
quietly becoming an unrestricted host process.

But sandboxing the orchestrator is not the same as governing the tools it
orchestrates.

Consider a program like this:

```javascript
const issues = await tools.search_issues({ project: "ABC" });

for (const issue of issues) {
  if (issue.status === "Draft") {
    await tools.update_issue({
      id: issue.id,
      status: "Ready"
    });
  }
}
```

The program itself performs no network access. The tools do. It may produce ten,
a hundred, or a thousand externally visible mutations from one generated plan.

A secure runtime answers:

> What can this JavaScript process access directly?

Execution governance must answer:

> Which effects may this program cause, with which data, how many times, under
> whose authority, and with what audit trail?

Those are different questions.

## A new capability dimension: who may call the tool?

Programmatic Tool Calling introduces `allowed_callers`. An application can mark a
tool as callable directly by the model, programmatically from generated code, or
through either route.

```json
{
  "type": "function",
  "name": "search_issues",
  "allowed_callers": ["direct", "programmatic"]
}
```

That is more than API configuration. It is a new capability attribute.

A sensible Jira surface might look like this:

```yaml
tools:
  jira.search_issues:
    callers: [direct, programmatic]
    effects: [read]

  jira.get_issue:
    callers: [direct, programmatic]
    effects: [read]

  jira.create_issue:
    callers: [direct]
    effects: [external_write]
    decision: ASK

  jira.delete_issue:
    visibility: ABSENT
```

Generated programs can batch reads, join results, deduplicate records, and perform
deterministic validation. Mutations remain direct calls with a clear approval
boundary. Destructive capabilities never enter the model's world at all.

OpenAI's own guidance recommends this split: use Programmatic Tool Calling for
bounded, predictable reduction workflows, and prefer direct calls for writes or
approval-sensitive actions. That is a sound default. But a prompt saying “do not
perform writes programmatically” is not enforcement. The caller restriction must
be compiled into the actual tool surface.

This is **Capability Shaping** applied to invocation mode.

## Approval can no longer mean only “approve this call”

Even read-heavy programs can cross an action boundary. A program may pause on an
MCP tool whose policy requires approval, but approving a long loop one call at a
time is neither usable nor meaningful.

The missing abstraction is a **bounded delegation envelope**:

```yaml
approval:
  principal: user:alice
  tool: jira.update_issue
  where:
    project: ABC
    current_status: Draft
  arguments:
    allowed_fields: [status]
    status: Ready
  limits:
    max_calls: 20
    max_parallelism: 2
    expires_after: 5m
```

The user is not approving arbitrary execution. They are delegating a narrowly
described capability with explicit argument constraints, cardinality, concurrency,
and expiry.

This is the programmatic equivalent of an object capability: authority is carried
as a bounded artifact, not inferred from a vague “yes” earlier in the
conversation.

## Caller provenance becomes a first-class security primitive

There is an encouraging detail in the API design. A generated `program` has its
own `call_id`; each nested tool call has another `call_id`; and the nested
call's `caller` field points back to the program that produced it.

The execution graph is therefore observable:

```text
user request
└── generated program
    ├── search_issues(project=ABC)
    ├── get_issue(ABC-17)
    └── update_issue(ABC-17, status=Ready)
```

A flat tool log could tell us that `update_issue` happened. A provenance graph
can also tell us:

- which user request initiated the run;
- which generated program derived the action;
- which earlier results influenced its arguments;
- which policy and approval authorized the effect;
- which program output summarized the run.

That graph should be preserved even when intermediate tool results are reduced
before they return to the model.

Efficiency for the model must not become opacity for the governor.

## The dangerous part is often the edge between two allowed tools

Programmatic orchestration makes cross-tool data flow ordinary:

```javascript
const customer = await tools.crm_get_customer({ id });
await tools.send_email({
  to: externalAddress,
  body: JSON.stringify(customer)
});
```

Neither tool is necessarily forbidden in isolation:

- reading a customer record may be allowed;
- sending an email may be allowed.

The prohibited operation is the **flow**:

```text
CRM / PII → external email
```

Allow/deny lists cannot express this. RBAC cannot express it either. The decision
depends on the provenance and classification of the value crossing the boundary.

This is where calculated taint stops looking like an advanced research feature
and starts looking like a production requirement:

```yaml
labels:
  crm.get_customer.output: [internal, pii]
  email.send.body: [external_sink]

flows:
  - from: pii
    to: external_sink
    decision: DENY
```

The taint must survive transformations inside the generated program. Filtering,
joining, formatting, or summarizing a value does not make its provenance
disappear.

## Programs need policy too

Governing the reachable tools is necessary, but not sufficient. The orchestration
itself consumes authority and resources. A generated program needs an execution
budget:

```yaml
programmatic_execution:
  enabled: true
  permitted_effects: [read, compute]
  max_tool_calls: 50
  max_parallel_calls: 5
  max_retries_per_call: 1
  max_duration: 30s

  forbidden_flows:
    - from: crm.get_customer
      to: email.send
```

The policy surface now includes:

- eligible tools and invocation modes;
- maximum call count and concurrency;
- retry and time budgets;
- allowed tool sequences;
- data-flow constraints;
- approval handoffs;
- incomplete-program behavior;
- evidence and audit requirements.

This is not a guardrail around model text. It is governance of an executable plan.

## What a World Manifest should compile

A World Manifest should remain provider-neutral and compile into the mechanisms
of each host:

```yaml
tools:
  jira.search_issues:
    visibility: PRESENT
    effects: [read]
    callers: [direct, programmatic]
    output_labels: [jira_internal]

  jira.create_issue:
    visibility: PRESENT
    effects: [external_write]
    callers: [direct]
    decision: ASK

  jira.delete_issue:
    visibility: ABSENT

programmatic_execution:
  permitted_effects: [read, compute]
  max_calls: 40
  max_parallelism: 5
  max_retries: 1

flows:
  - from_label: jira_internal
    to_effect: external_write
    decision: DENY

audit:
  record_program: true
  record_nested_calls: true
  record_caller_chain: true
  record_policy_decisions: true
```

For GPT-5.6, the compiler can emit `allowed_callers`, MCP approval settings,
runtime budgets, and trace requirements. For another host, it may emit gateway
policies or lifecycle hooks. The policy intent stays stable while the enforcement
mechanism changes.

This is exactly why the manifest must describe the agent's world rather than the
configuration format of one vendor.

## The real shift

Programmatic Tool Calling is good architecture. Deterministic code is often a
better place than repeated model turns for filtering, batching, aggregation, and
validation. It reduces latency, tokens, and unnecessary exposure of intermediate
data to the model.

But it also makes the execution boundary impossible to ignore.

A model-generated program is neither “just reasoning” nor a trusted application.
It is an untrusted orchestration artifact operating over explicitly delegated
capabilities. Its code can be inspected, its calls can be traced, and every
external effect can still pass through a deterministic gate.

The governance rule is simple:

> **Authorize the plan narrowly. Enforce every effect independently. Preserve the
> provenance between them.**

Programmatic Tool Calling does not reduce the need for Agent Execution
Governance. It makes that missing layer visible.

---

**References**

- OpenAI, [Programmatic Tool Calling](https://developers.openai.com/api/docs/guides/tools-programmatic-tool-calling)
- OpenAI, [Prompting guidance for GPT-5.6 Sol](https://developers.openai.com/api/docs/guides/prompt-guidance-gpt-5p6)
- OpenAI, [GPT-5.6 model guidance](https://developers.openai.com/api/docs/guides/latest-model)
