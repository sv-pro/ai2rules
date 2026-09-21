---
title: 'Your Agent Has a Policy. Which Tools Does It Actually Cover?'
description: 'An MCP policy covers calls through its gateway. Native tools may take another route. How to verify the actual coverage of an AI coding agent’s controls.'
pubDate: 'Sep 07 2026'
heroImage: '../../assets/your-agent-has-a-policy.png'
---

A denied tool call is useful evidence. It tells you that one request reached a control, the control rejected it, and—if you checked the downstream effect—the operation did not happen.

How far can you extend that claim?

While working on ai2rules, I keep returning to this question. A policy can be correct, its decision engine deterministic, and its integration functional, while another route to the same effect remains available. Understanding those routes is part of installing governance.

Consider an illustrative setup: Claude Code connects to a ticketing service through an MCP gateway. The gateway permits reading tickets and rejects ticket creation. The same session also has a native shell tool.

If that shell can reach the service API and use credentials authorized to create tickets, the agent has another route to the effect. Whether it succeeds depends on shell permissions, credential access, network restrictions, and the service's authorization checks. The MCP policy alone cannot answer that question.

| Route to ticket creation | Does an MCP-only gateway evaluate it? | What else must be checked? |
| --- | --- | --- |
| MCP call through the gateway | Yes | Policy decision and downstream effect |
| API request from the native shell | No | Shell controls, credentials, network access |
| Direct connection to the upstream MCP server | No | Whether that connection is available and authorized |

This is a coverage example, not a reported exploit or a claim about Claude Code's default configuration. Claude Code has its own permission system, including controls for native tools and MCP calls. Those controls belong in the assessment. [Claude Code permissions documentation](https://code.claude.com/docs/en/permissions).

The distinction matters because “blocked by this gateway,” “blocked elsewhere,” and “not checked” describe different system states.

In ai2rules, there are already two relevant integration paths. The MCP gateway shapes discovery and evaluates calls passing through it. The Claude Code adapter connects the same Rust kernel to `PreToolUse`, allowing governance of native tool invocations as well. Installing only the gateway does not install the native-tool hook. Installing the hook still leaves questions about registration, configuration, supported mappings, and failure behavior. [Integration contracts](https://github.com/sv-pro/ai2rules/blob/main/docs/one-kernel-many-hosts.md).

Even the meaning of a decision depends on the integration point. The gateway can omit a tool from MCP discovery. The ai2rules `PreToolUse` integration cannot remove a native tool from the model's tool list; its optional absence enforcement blocks the attempted invocation. Both use the same kernel, but they expose different capabilities to the host.

Failure behavior is equally concrete. The documented ai2rules Claude Code adapter fails open on process failure, leaving the host's own controls in charge. The MCP gateway fails closed for calls passing through it: an unevaluated call is not forwarded upstream. Neither property describes every route available in the session. [Adapter failure behavior and limitations](https://github.com/sv-pro/ai2rules/blob/main/docs/one-kernel-many-hosts.md#fail-open-vs-fail-closed-explicit-per-adapter).

That is why a green “installed” indicator would tell me too little.

I want a coverage report that identifies the running binary, the active manifest and its hash, the installed hook or gateway, and any active kill switch. It should show which tools pass through that integration and which execution paths remain outside it. Where it cannot establish the answer, it should say **unknown**.

There is also a difference between finding configuration and observing enforcement. A hook entry on disk establishes configuration. A controlled session that reaches the hook, receives a denial, and produces no forbidden effect establishes something stronger. Neither proves that every possible route has been examined.

This is the direction of the planned `harness doctor` command. As of September 7, 2026, it is still planned work. The initial target is a read-only report for Claude Code CLI. Automatic diagnosis and the guided configuration journey should not be read as shipped features; the existing manual setup is documented in the [repository README](https://github.com/sv-pro/ai2rules#govern-a-project-in-one-command).

There is already a narrower proof readers can inspect. From a repository checkout with the Rust toolchain, the documented governed coding example runs with:

```bash
cargo run -p agent-core --example governed_coding_workflow
```

It uses a deterministic scripted model in the real orchestration loop. A workspace read taints the session; an external-help request is denied; the next step uses that feedback to select a permitted local patch. The example checks the fixture's final file contents and replays the recorded governance decisions. It is an offline workflow, and replay reproduces policy decisions rather than model sampling or side effects. [Workflow and evidence contract](https://github.com/sv-pro/ai2rules#five-minute-governed-coding-workflow).

That proof has a useful, bounded claim: this workflow carries a refusal back into the loop and continues through an allowed action. Full coverage of a user's Claude Code environment requires separate evidence.

For a real installation, I would start with one harmless forbidden effect against a disposable fixture. Exercise the mediated route, inspect the decision, and check the target state. Then examine other routes to that same effect. Record whether each is blocked by ai2rules, blocked by another control, available, or unverified.

Attach the host version, integration mode, and manifest identity to that result. Repeat the relevant checks when those inputs change.

The question I want onboarding to answer is precise: **in this session, which routes to an effect must cross an active control, and what evidence supports that answer?**

*Related: [What Your Coding Agent Actually Lets You Control](/blog/what-your-coding-agent-lets-you-control/) and [Governed Is Not Confined](/blog/governed-is-not-confined/).*

*Written with AI assistance; technical claims checked against the repository and the linked documentation. The ticketing scenario is illustrative.*
