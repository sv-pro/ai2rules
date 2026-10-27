# ai2rules

> Deterministic virtualization for local CLI developer agents.
> We don't filter what the agent does — we compile the physics of the world the
> agent lives in. Dangerous actions aren't blocked by a rule; they don't exist.

**This repository is the umbrella / companion codebase for the
[ai2rules.dev](https://ai2rules.dev) site** — *Agentic Governance at the
stochastic–deterministic border*. It unifies several projects across five layers
(action · capability · knowledge · intent · substrate) under one thesis; the spine
is [`docs/THESIS.md`](docs/THESIS.md). The flagship and most mature layer is the
governance **harness** described below (which was formerly this whole repo's name,
"CLI Agent Harness").

A governance **kernel** that sits underneath a local CLI developer agent (Claude
Code, OpenCode, Codex CLI, Gemini CLI, Aider, …) and controls what the agent can perceive,
what actions it can represent, and what validated specs may cross into real
execution. The model never touches raw reality: it sees a virtualized world
defined by a compiled manifest and can only *propose* typed intents into it.

This repository is **early-stage**, but the **deterministic core (Milestone 1) is
complete**: the manifest compiler, the governance kernel, the execution boundary,
and the audit/replay layer are all in place — a proposed tool call flows all the
way to a real or simulated result, and every decision is logged, redacted, and
replayable. See [Status](#status).

> **New here? Take the tour: [`docs/TUTORIAL.md`](docs/TUTORIAL.md).** Nine stops,
> ~45 minutes, all offline and credential-free — from the five verdicts, to one
> kernel deciding for four different hosts, to governing a project of your own.

The [policy-console demo](docs/demos/policy-console/README.md) includes the
`/northwind-demo` skill and read-only stand checks. Automated probes cover setup,
preflight and launcher against the current runtime. Start a fresh session and
rehearse interactively; passing preflight does not establish live-agent readiness.

---

## Govern a project in one command

```bash
npm install -g ai2rules-harness   # or: cargo install --git https://github.com/sv-pro/ai2rules cli-harness
harness init                      # additive: adds deny/ask on top of the host's permissions
harness init --grant              # replace: the manifest becomes the allowlist
harness init --dry-run            # print the plan, write nothing
harness init --user               # every session this account opens, in any folder
harness init --managed            # every session on the machine; admin-installed, the user cannot turn it off
harness demo                      # decision-only proof: 5 cases through the real kernel, no execution
harness demo --story law-firm     # the same kind of proof as a story, for non-engineers; no init needed
```

**Trying it on a clean machine, or reproducing it for us?** Follow
[`docs/reproduction/QUICKSTART.md`](docs/reproduction/QUICKSTART.md): 15 minutes,
pinned to 0.7.0, with the expected output of every step and a real Claude Code
session refused by the kernel next to an ungoverned one that is not.

Published as [`ai2rules-harness`](https://www.npmjs.com/package/ai2rules-harness) —
**no install script.** The binary ships in a per-platform package that npm
resolves by `os`/`cpu`. Downloading the npm packages requires network access;
there is no package install script that downloads or modifies a binary. The
binary is covered by the integrity hash npm writes into your lockfile.

> **Install globally, not into a project.** `harness init` **refuses** when the
> binary sits inside the project it would govern — a local `node_modules` install
> puts the kernel in the agent's own writable root, where replacing one file turns
> governance off silently. `npx ai2rules-harness init` is fine; its cache is
> outside your project.

That is the whole install. No checkout, no `cargo`, no `jq` — the starter manifest
is compiled into the binary, and the shim bakes the absolute path of the executable
you just ran. It writes four things and is safe to run twice:

```
.claude/cc-world.yaml         the manifest   (kept if you have tuned one; --force to replace)
.claude/hooks/world-gate.sh   the PreToolUse shim, kill-switch baked in
.claude/settings.json         the hook entry, merged — your other hooks are untouched
.gitignore                    .claude/state/   (the runtime taint state)
```

**Prove it is real**, without starting an agent session or supplying credentials:

```bash
harness demo
# Runs 5 cases through the real kernel over this project's world: a local write
# (ALLOW), an out-of-root write in background (DENY), a network call from a tainted
# session (DENY), and `rm -rf` interactive (ASK) and in background (DENY). It records
# each verdict and replays them to prove determinism. Decision-only, no tool execution.
```

For someone who does not write code, tell it as a story instead. An AI legal
assistant reads a client email with a hidden instruction, and every way out of the
firm closes. It runs from any folder, over its own embedded world; the runbook and
talking points are in [`docs/demos/law-firm/`](docs/demos/law-firm/):

```bash
harness demo --story law-firm
```

**Before you turn it on, see what it would have done in your own sessions.** This
replays every tool call in your past Claude Code transcripts through the kernel,
exactly as the hook would have decided them, and reports how often it would have
asked or denied, and why. Nothing is executed:

```bash
harness replay-transcripts                       # every session under ~/.claude/projects
harness replay-transcripts path/to/session.jsonl --world .claude/cc-world.yaml --json
```

Run it on the OS the sessions ran on: path rules resolve paths against today's
filesystem, so Windows paths replayed on Linux, and folders deleted since, show
up as `missing_path`.

Or ask the kernel directly about one proposed call:

```bash
echo '{"tool_name":"Write","tool_input":{"file_path":"/etc/passwd"}}' \
  | CLAUDE_PROJECT_DIR=$PWD bash .claude/hooks/world-gate.sh
# {"hookSpecificOutput":{…,"permissionDecision":"deny",
#   "permissionDecisionReason":"the target path is read-only under the roots policy (path_scope_readonly)"}}
```

The same write *inside* the project stays silent. That is the roots policy doing
what no allowlist of command strings can do — it is about where the path is, not
what the command looks like.

**Turning it off is one file**, effective on the very next call, no restart. Both
paths live **outside** the governed project, because a switch the agent can reach
is a switch the agent can throw — `harness init` prints the exact path for your
project:

```bash
touch ~/.claude/ai2rules/off/<your-project>   # this project
touch ~/.claude/gate-off                      # panic switch, everywhere
```

The manifest is compiled by the real compiler *before* anything is written, so what
lands in your project is known to build. `init` supersedes the per-project half of
[`scripts/install-governance.sh`](scripts/install-governance.sh), which is still
there for the per-machine half (putting a binary on `PATH` from a checkout).

**Every folder, not one.** `harness init --user` puts the same hook into
`~/.claude/settings.json`, so every session you open is governed, wherever it
starts; its world and taint state live under `~/.claude/ai2rules/user/`. It is a
personal seatbelt: a project can still switch user hooks off with
`disableAllHooks`. For an organisation, `harness init --managed` (elevated)
installs into Claude Code's **managed settings**, which nothing below an
administrator can disable, with no user-side kill-switch and the kernel copied
into the admin-owned directory. Both take `--remove`. The difference, an example
policy, and what neither scope stops yet are in
[`docs/machine-wide-governance.md`](docs/machine-wide-governance.md).

---

## Why

A local CLI agent inherits the developer's full ambient authority — credentials,
SSH keys, git remotes, package managers, write access to the working tree — while
ingesting untrusted text on every turn (file contents, issue bodies, web pages,
MCP tool results). Prompt injection is therefore an *authority-boundary* problem,
not just a prompt problem. The harness closes that gap **structurally**:

```
raw reality            → typed Perception
model output           → typed ToolCall
ToolCall               → sealed IntentIR     (only via the kernel)
IntentIR               → ExecutionSpec        (only after deterministic evaluation)
ExecutionSpec          → the execution boundary (nothing else crosses)
```

No LLM participates in runtime policy enforcement. Decisions are a pure function
of `(intent, context, compiled world)` — deterministic and replayable.

Read the full design in **[`docs/harness-architecture.md`](docs/harness-architecture.md)**.

This harness is the **action layer** of a larger thesis — *Agentic Governance at
the stochastic–deterministic border* — that unifies it with sibling projects
(governed memory, capability projection, intent). The umbrella spine is
**[`docs/THESIS.md`](docs/THESIS.md)**.

---

## Status

| Milestone | Theme | State |
|---|---|---|
| **M1** Deterministic Core | kernel works in simulation | ✅ done (E0–E4) |
| **M2** Live Agent | a real model drives the loop | ✅ done (E5–E6) |
| **M3** Full Tool Surface | MCP, web, scoped capabilities, CLI/TUI | ✅ done (E7, E9) |
| M4 Isolation & Hardening | sandbox + acceptance + benchmarks + authoring UI + tech blog + dogfooding | 🚧 E11, E12, E13 started; **E18 governance benchmark seed delivered**; E8, E10 planned |
| M5 Interactive Advocacy | the real kernel in the reader's browser (WASM) + a TF-Playground-class visualization suite | 🚧 E14 engine done (643 KB wasm); structured `/playground` live on the blog; E15 suite (timeline / attack / graph visualizations) planned |

**Done so far:**

- **Typed flow contracts in the kernel (#151 / AI2-70, D95):** the taint is one integrity bit per session; it cannot say "private, but from a trusted place", so a world that reads a secret had to choose between losing every later benign write and leaking through a permitted read. Now a value carries a `FlowLabel` on two axes (integrity × confidentiality, a product lattice with join as its only operation — no declassify, no implicit Trusted/Public), an action may declare an opt-in `flow_contract` (an output floor, inherited arguments, per-argument sinks) that is part of its descriptor identity, and the kernel runs an independent confidentiality admission *before* the taint floor that no approval can override. The host carries the labels (`context.flow` on the gate ABI, additive in v1) and binds the returned `result` floor to the value it gets back. Proven on a vertical slice with counted effects: a private summary cannot be published by a trusted caller in a clean session (0 effects, before and after the human says yes) while a public body is published exactly once. Worlds without contracts keep identical decisions and identical manifest/descriptor/world hashes — pinned by test, by the benchmark diff gate and by the demo guard.
- **WebMCP attack lab (#160 / AI2-83, `labs/webmcp/`):** 5 epic attacks plus 3 variants against no defense (D0), a rule-based per-call baseline (D1) and ai2rules (D2, the real kernel via `harness project` / `harness gate`), under a deterministic compromised agent. Attacks are judged from effects only, and D1 and the tasks were frozen before any attack. Result: D1 lets 6 of 8 through; D2 lets 2 through (S3b, handler tampering in a shared realm; S5b, exfiltration through a permitted read tool) and loses utility wherever per-session taint fires. A fourth column, D3, runs on the kernel's typed flow contracts (D95; AI2-70 / #151): the compiled world declares the CRM read a Trusted/Private source and every other-origin argument a Public sink, the adapter carries the gate ABI's `context.flow` sidecar, and the kernel's `confidentiality_violation` does the refusing. It recovers all the utility D2 loses, closes S5b, and lets through no attack D2 stops in the shipped scenarios. That last part is not true in general: independent checkpoint 6 built an injected order from the task's own values that D2 refuses and D3 allows, because D3 trades D2's post-read integrity floor for utility. Its other cost is that labels are whole-result and bound by digest, so after a private read any value the adapter cannot vouch for is refused, a transformed secret and a fresh benign input alike. Independently verified (`labs/webmcp/verification/`), plus a real-model column. `cd labs/webmcp && npm run matrix`; CI job `webmcp-lab` fails on result drift. `npm run demo` opens a replay page for showing it: pick a defense, run S5b, watch the agent's steps and the verdict. `npm run live` is the live stand: Claude Code (no API key) or a hijacked model working in a visible browser through an MCP bridge, with switches for the policy, the widget and which sites hold private data, and a labels panel.
- **E0 — Foundations & Core Contracts:** the Cargo workspace, the language-neutral
  contracts in `harness-types`, the sealed `IntentIR` / `IRBuilder` in
  `world-kernel`, the `BuildError` taxonomy, and CI.
- **E1 — Manifest & Compiler:** a `WorldManifest` compiles into an immutable,
  hash-addressed `CompiledWorld` (`compiler`) — YAML/JSON loader + validator, real
  SHA-256 descriptor/manifest hashing, and a default CLI world
  (`crates/compiler/assets/default_world.yaml`).
- **E2 — World Kernel:** the deterministic heart. `IRBuilder::build` runs the
  representability checks (ontology → projection → capability → schema →
  descriptor → hard taint invariant) to seal an `IntentIR`; `disposition::evaluate`
  applies the contextual rules (manifest taint policy, approval, budgets) to a
  built intent; and `decide()` is the single pure entry point returning a
  `KernelOutcome`. Honors acceptance invariants 1, 2, 3, 6 (and the invariant-7
  taint floor).
- **E3 — Execution Boundary:** the kernel lowers an `ALLOW` to an `ExecutionSpec`
  (`build_execution_spec` + `ExecEnv`), and the `executor` runs it behind a
  closed registry — refusing unregistered actions and descriptor drift, applying
  `Execute` / `Simulate` / `Truncate`, constraining writes to writable roots, and
  returning a `TaintedValue`. Read / patch / command handlers run in both real
  and simulated modes. Honors invariants 4, 5, 8, 11, 13, 16.
- **E4 — Trace, Audit & Replay:** an append-only JSONL trace (`trace-store`)
  records every decision with secrets redacted *before* disk; `replay` reproduces
  decisions against the same world (determinism), `drift_report` diffs them
  against a changed manifest, and a `Bundle` packages a trace with its manifest
  for offline replay. Honors invariants 14, 15 — **completing Milestone 1.**
- **E5 — Provider Adapters & Orchestrator:** a model now drives the loop. The
  `provider-adapters` Anthropic adapter normalizes `tool_use`/`tool_result`/tool
  defs ↔ the neutral `ToolCall`; `agent-core` exposes only the projected tool
  surface, then runs propose → adapt → `decide` → execute (simulated) → perceive
  (tainted) → return the correlated provider `tool_result` → repeat, recording
  every decision to the trace. Non-executed `DENY`/`ABSENT`/`ASK` outcomes use
  that same first-class feedback path, with structured decision/rule evidence;
  observers remain presentation-only. A `ModelClient` trait + deterministic
  `ScriptedModel` keep it fully offline (a live HTTP client is a later,
  feature-gated add). Reinforces invariants 3 and 4 — starting Milestone 2.
- **E6 — Approvals & Execution Modes:** human-in-the-loop as durable state. The
  kernel branches on `ExecutionMode` — an approval-required action `ASK`s
  interactively but **fails closed to `DENY` in background**; a durable
  `ApprovalStore` (`trace-store`) mints/persists single-use
  `AuthorizationInstance`s (`pending → approved → consumed → executed`). Each
  is bound to the trusted principal, canonical effect, resource, world/schema
  epochs, provenance, effect mode, and expiry. Consumption is atomic and durable
  before execution, so substitution, replay, concurrent reuse, and restart fail
  closed; the orchestrator then re-decides the `ASK` under the unchanged world
  ceiling. Honors invariants 9, 10 — **completing Milestone 2.**
- **Staged external finality (AI2-8 / D74):** `StagedEffect` is a sealed,
  explicitly non-effective artifact between admission and an external actuator.
  Its independent durable commit path revalidates the artifact, current
  world/schema epochs, consequence metadata, actuator operation, and the exact
  `AuthorizationInstance`; reserves the idempotency key before actuation; and
  emits a causal `ExecutionReceipt`. Known completion, simulation, failure,
  duplicate, rejection, and ambiguous finality remain receipt states rather than
  new kernel verdicts. See [`docs/staged-effect-commit.md`](docs/staged-effect-commit.md).
- **E7 — MCP, Web & Scoped Capabilities:** broader reach through the one gate.
  Scoped capabilities narrow a base action — `build_execution_spec` strips
  locked/unknown args and injects literals (so `run_tests` always runs `pytest`,
  invariant 12); MCP calls dispatch via a pluggable `McpTransport` through the
  same descriptor/drift path (invariant 11); web fetch is an always-tainted
  channel (invariant 7). MCP/web use deterministic **mock** transports (real
  stdio/HTTP deferred). Part of Milestone 3. Since D79 a scoped capability can
  also rename an argument and limit it to a list of values or a prefix, a base
  action can be kept off the surface (`projected: false`) while scoped capabilities
  wrap it, and `mcp_surface` names the server a gateway will present. With a
  surface declared, `harness mcp-gateway` offers scoped capabilities under their
  own names and forwards the kernel's lowered call (renamed arguments mapped back,
  literals injected) rather than the call as proposed — E13.4a/b. `harness
  mcp-author` is the authoring page for such a surface (E13.4c, below).
- **E9 — CLI / TUI:** `cargo run --bin harness` is now an
  interactive session — `clap` flags (`--world`/`--simulate`/`--background`), a
  human-driven `ModelClient` that proposes from the projected tool surface via
  `inquire`, and approval prompts through an `ApprovalPolicy::Interactive`
  callback; each step streams through the loop's observer with structured
  `Decision`, `Rule`, `Effect`, and `Feedback` fields for
  `ABSENT`/`DENY`/`ASK`/`REPLAN` outcomes — **completing Milestone 3.**
- **E11 — World Authoring Tool (in progress):** `harness serve` launches a local
  browser editor for world manifests — a single embedded page (no build step)
  whose `POST /api/preview` endpoint compiles a draft manifest through the
  **real** compiler + kernel and returns the projected tool surface plus a
  clean-vs-tainted decision matrix, so the live preview is faithful to what the
  harness would actually do — no governance logic reimplemented in JS (E11.1–E11.3;
  manifest export and the LLM-assist/trace explainer are the pending E11.4–E11.5).
  See `DECISIONS.md` D17/D18.
- **E13 — Claude Code integration (dogfooding; live hook cut over to the Rust
  kernel, D37):** the `PreToolUse` hook is now a bootstrap shim
  (`.claude/hooks/world-gate.sh`, canonical; `world-gate.py` kept as the same
  shim for snapshotted sessions) that `exec`s **`harness cc-hook`** — the real
  kernel deciding in-process against the real `WorldManifest`
  `.claude/cc-world.yaml`: ABSENT (opt-in `--enforce-absent`), the monotonic
  **taint floor**, **ASK** on destructive commands (collapsing to DENY under
  `--mode background`), with Bash classified **kernel-side** from the manifest's
  `command_classes` (D36). Additive (only ever `deny`/`ask`) and fail-open when its
  binary is missing; a world that does not compile follows `--on-broken-world`
  (`last-good` by default, D87). The
  Python engine is archived under `.claude/hooks/superseded/`; the cutover
  consciously drops trust pins (D29) and path-based read-taint until they land
  as typed manifest fields. Cross-agent taint still follows the shared session
  sidecar (E13.6, D20), and the containerized governed SUT with an
  egress-allowlist proxy supplies the E8 enforcement floor (E13.7, D21).
  Instruction and memory files (`CLAUDE.md`, `AGENTS.md`, `.claude/skills/**`,
  auto-memory, Antigravity's equivalents) are classified by `path_classes` on the
  canonical path as durable-memory writes, so a tainted session cannot leave
  instructions for the next one (E13.10, D83).
  See `DECISIONS.md` D19–D21, D29, D36, D37, D83.
  **Mods coverage finding (2026-10-03):** on Claude Code 2.1.288 with released
  harness 0.6.0, a user-tier mod overrode a project PreToolUse `deny` and the
  native Write tool created the denied temporary marker. The kernel correctly
  denied; the host did not retain that decision as final. See the
  [reproducible probe and evidence](docs/benchmarks/claude-mods/).
  **Managed comparison (2026-10-24, AI2-41 / #99):** in an isolated Linux
  container, the genuine system-file hook denied the same native Write with
  mod off and on; neither marker was created. sec-default loaded in prepend
  and explicitly admitted the user-tier test mod. See the
  [managed report and limits](docs/benchmarks/claude-mods/MANAGED.md).
  This is a versioned native-tool result, not a sandbox or arbitrary-mod guarantee.
- **E14 — In-browser kernel (WASM engine, started):** the real `preview(yaml) →
  {surface, decision matrix}` is now a shared pure crate (`harness-preview`) used
  by both `harness serve` and a new `wasm-bindgen` crate (`harness-wasm`) — so the
  same governance runs natively *and* in the browser, with no reimplementation
  (D22). A spike compiles the whole stack to `wasm32` and a Node smoke test proves
  the kernel decides client-side (clean `fetch_web` → Allow, tainted → Deny by
  `taint_invariant`). This is the engine under the visualization suite (M5 / E15):
  a size-optimized `--target web` bundle (`wasm-opt -Oz`, **643 KB**) ships on the
  blog at **`/playground`**, where a structured `KernelPlayground` editor — preset +
  Clean/Tainted switch + per-tool toggles — sorts every tool live into
  ALLOWED / ASK / DENIED / ABSENT buckets, all decided by the real kernel in-browser.
  Remaining E14 work: the native↔wasm fidelity CI guard (E14.4).

- **Gate ABI — the host-neutral integration port (D24):** `harness gate --world
  <manifest>` reads a `GateRequest` JSON on stdin and writes the kernel's verdict
  (`ABSENT/ALLOW/DENY/ASK/REPLAN` + rule + post-call taint) on stdout — so any
  host (Claude Code, a Hermes agent, Codex CLI, an MCP proxy) integrates through a
  **thin adapter** that calls the *real* kernel, never a reimplementation. The
  decision is the pure `harness_preview::gate()`, shared native and WASM. Schema:
  [`docs/harness-gate-abi.md`](docs/harness-gate-abi.md).

- **Discovery projection ABI (D72):** `harness project --world <manifest>` reads
  the exact schemas a host is about to advertise, removes names the compiled world
  does not project, and returns joinable manifest/schema identities. DeepSeek's
  adapter runs it at `system-prompt/assemble`, then uses `gate` at
  `tools/pre-execute`; a revoked tool disappears on the next model step and a call
  retained from stale context still has no authority.

The same gate also powers the **governability demo** (`docs/demos/jira-copilot/`,
DECISIONS D33): `harness mcp-gateway` fronts an MCP server — shaping its `tools/list`
(ABSENT) and gating each `tools/call` on the real kernel — over a self-contained
`harness mock-jira` upstream, and `harness cc-hook` is the Claude Code `PreToolUse`
adapter in Rust (governing native tools, replacing the Python hook). The **real
Atlassian skin** (E16.E) swaps that mock upstream for the Atlassian Rovo MCP Server
via an `mcp-remote` OAuth bridge with no kernel change — and its shaping is proven
offline (no creds) by `harness mock-jira --rovo`, which advertises the genuine Rovo
tool names (see [`docs/demos/jira-copilot/REAL-ATLASSIAN.md`](docs/demos/jira-copilot/REAL-ATLASSIAN.md)).

The [Confluence authoring world](docs/demos/confluence-docs/) confines creation to
one tenant/space and updates to one tenant/page (ASK, not forwarded by the gateway).
All five remote reads taint the session, preventing subsequent external writes.
Five offline regressions cover the pins, read-to-write taint and gateway sequence;
the corrected manifest has not been revalidated against live Atlassian.

**Author your own MCP surface (D79, E13.4):** `harness mcp-author` connects to an MCP
server and opens a local page where you name the surface the gateway will present
(say `My_Jira`), tick the tools that exist (everything else is ABSENT), rename them,
fix arguments to hidden values, and limit the rest to a list of values or a prefix.
"Try it" runs a call through the real gate and shows exactly what the gateway would
send upstream; it calls the upstream only when you choose to send. Save writes a
validated manifest, which the gateway then serves:

```bash
harness mcp-author --world my-jira.world.yaml -- harness mock-jira --rovo   # http://127.0.0.1:8788/
harness mcp-gateway --world my-jira.world.yaml -- harness mock-jira --rovo
```

**Shape a live agent from a policy console (D81, E15.7):** `harness console` opens a
local page for one Claude Code project governed by `cc-hook`. Its profiles and
Allow / Ask / Off switches (read the web, send data out, delete files, edit project
files, run scripts) rewrite the project's world file atomically, and the hook reads
that file on every call, so the agent's very next action gets the new answer, with no
restart. `cc-hook --log` feeds the page a live list of what the agent tried and what
the kernel decided. The same log joins each decision to its outcome (D84): every line
carries the host's `tool_use_id` and agent, and a read-only `PostToolUse` observer,
which `harness init` wires in, records how each call that ran ended. That is enough
to tell an approved ASK from a declined one, inferred from whether the call ran
([`docs/host-decision-log.md`](docs/host-decision-log.md)). `harness export-otlp`
turns a finished log into OpenTelemetry traces (OTLP/JSON, GenAI `execute_tool`
spans, one trace per session) for an existing collector, offline. Repeated hook
decisions retain distinct, stable span IDs and their original call ID for correlation.
Call summaries mask
recognized secrets before whitespace normalization and truncation, so shortening
cannot expose a credential fragment. Off takes the action out of the agent's world (ABSENT, with
`--enforce-absent`); the taint floor has no switch. A **Stop** button (D85) takes every
effectful base action away at the agent's next call, including custom names and shell
processes; actions declared Read/None stay as configured. Worlds with effectful scoped
capabilities show Stop as unavailable, because those capabilities cannot be hidden
independently. Restore puts the exact previous projection/approval flags back;
the page says "Stopped" only after the hook has refused a call. Runbook and scenario:
[`docs/demos/policy-console/`](docs/demos/policy-console/README.md).

```bash
bash docs/demos/policy-console/setup.sh                       # ~/northwind/vendor-desk
harness console --project ~/northwind/vendor-desk               # http://127.0.0.1:8790/
```

**One kernel, many hosts (shipped — D36/D37/D48, `docs/one-kernel-many-hosts.md`):**
Claude Code, OpenCode, Antigravity CLI, and the MCP gateway all decide through the one Rust kernel
via thin adapters that hold no policy, no taint algebra, and no command
classification — bash shapes are classified by the *kernel* from the manifest's
`command_classes` (D36), and verdict→host mapping is the shared `host_outcome()`
layer (ABSENT stays distinct from DENY everywhere):

```
Claude Code    ─┐
OpenCode       ─┤
Antigravity CLI ┼─→ thin adapter → GateRequest → one Rust kernel
MCP Gateway    ─┘                        ↓
                                    GateResponse
```

The parity guarantee — same manifest + same request ⇒ same decision / rule /
post-call taint / manifest_hash on every entry point — is pinned by the
conformance suite (`crates/cli-harness/tests/one_kernel.rs` over
`docs/demos/one-kernel/{demo-world.yaml,cases.yaml}`) and demonstrated offline in
seconds by:

```bash
bash scripts/demo-one-kernel-many-hosts.sh
```

OpenCode (E17 / DECISIONS D35) is dogfooded the same way: the
`.opencode/plugin/ai2rules-gate.ts` `tool.execute.before` adapter (plus OpenCode
`permission` rules) calls the same `harness gate` wire ABI, sending raw tool
names — see `docs/demos/opencode/`.

DeepSeek Harness (issue #55 / D71/D72) is the fourth host and the first one here
whose plugin shapes both sides: the authoritative prompt assembly is projected
before the model sees tools, and the pre-execute waterfall gates every call. The
same `deepseek-world.yaml` can govern OpenCode because both hosts use the same
lowercase tool vocabulary; revocation, stale invocation, Code Mode fail-closed,
and projection/gate outage behavior are pinned in the real dsh service spec.

**Antigravity CLI (`agy`, DECISIONS D48)** is the third live host: `.agents/hooks.json`
runs a bootstrap shim that `exec`s **`harness agy-hook`**, which links `gate()`
in-process like `cc-hook`. Its hook ABI is not vendor-published — the contract was
extracted from the shipped binary and then verified against a live session — so the
adapter absorbs a protojson camelCase envelope, `conversationId`, and PascalCase
argument keys (`CommandLine`, `TargetFile`) aliased into the neutral vocabulary the
shared `command_classes` reads. See `docs/demos/antigravity/`.

The MCP gateway supports out-of-band approvals with `--approvals <store>
--principal <operator-assigned-id>` and `harness approvals list|approve|reject`.
Consent is exact-bound, expires after five minutes, and is durably consumed before
forwarding; uncertain upstream outcomes require operator reconciliation. See the
[approval workflow](docs/approval-capable-hosts.md).

Builds clean offline with `clippy -D warnings`; **656 tests** green on Linux (3 specification tests explicitly ignored). Unix machine-hook
tests feed prepared events through stdin so a disabled hook's early exit cannot
race the fixture writer.
The Unix-only doctor integration suite includes **40 passing tests and 3 explicitly
ignored specification gaps** (#87 / #95); how it finds `bash` and `claude` on
`PATH` is tested on every OS, Windows included. It uses command-hook fixtures, verifies
the external per-project switch separately from the global switch, and rejects a
non-command registration as evidence of a working shim. The remaining #87 matrix
rows and three diagnostic divergences are still open.


The epic-by-epic plan, with task checklists and acceptance-invariant traceability,
is in **[`PLAN.md`](PLAN.md)**.

---

## Repository layout

```
Cargo.toml            Rust workspace (resolver 2)
crates/               the harness implementation
  harness-types/      language-neutral core contracts (pure data; no I/O, no LLM)
  world-kernel/       sealed IntentIR + IRBuilder; policy, taint, budgets (E2)
  compiler/           WorldManifest → immutable CompiledWorld (E1)
  executor/           execution boundary: subprocess/PTY/fs/patch/MCP/web (E3)
  trace-store/        append-only audit, redaction, replay (E4)
  provider-adapters/  provider tool-call → neutral ToolCall (E5)
  agent-core/         context packing, projected tool surface, model loop (E5)
  cli-harness/        terminal entrypoint + `serve`/`gate`/`project`/`mcp-gateway`/`mcp-author`/host adapters (binary `harness`) (E9, E11, E13.4, E16, D48, D72, D79)
  harness-preview/    pure preview + runtime gate() ABI + discovery projection, shared by serve, wasm, `harness gate` and `harness project` (E11/E14, D24, D75)
  harness-wasm/       the real compiler + kernel compiled to WASM, callable from JS (E14)
  govbench/           MCP governance benchmark runner: 3 scenarios, weak baseline vs ai2rules (E18)
docs/                 architecture (harness-architecture.md is canonical; one-kernel-many-hosts.md = cross-host parity)
  benchmarks/mcp-governance/  the governance smoke pack + its generated report (issue #64)
  benchmarks/ecc/             planned external target: ECC + AgentShield on the host-hook path (E18.14)
scripts/              runnable demos (demo-one-kernel-many-hosts.sh, run-governance-bench.sh)
blog/                 Astro blog — Discover-optimized advocacy site (E12; Node sub-project)
labs/webmcp/          WebMCP attack lab: 5 attacks × D0/D1/D2, CompromisedAgent, Playwright (#160 / AI2-83; Node sub-project)
addons/               Add-ons around the kernel, never inside it (D96): exfiltration/ (AI2-101), experiment 1 of AI2-104
PLAN.md               epic-level execution plan
AGENTS.md             repo conventions (canonical; shared across AI assistants)
CLAUDE.md             Claude Code pointer → AGENTS.md
repos/                reference projects (separate git repos, not workspace members)
  agent-hypervisor/   research kernel
  safe-mcp-proxy/     productized MCP control plane
  mcp-tool-projection/ declarative MCP projections + authoring-UI pattern (informs E11)
```

`harness-types` is the foundation: every other crate depends inward on it.
`IntentIR` lives in `world-kernel` (not `harness-types`) so Rust's privacy can
**seal** it — only `IRBuilder::build` can construct one, making the existence of
an `IntentIR` a proof that representability checks passed.

---

## Build & test

Requires a stable Rust toolchain (developed against 1.87).

```bash
cargo build --workspace
cargo test  --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run --bin harness        # interactive harness (flags: --world <yaml> --simulate --background)
cargo run --bin harness -- serve   # World Authoring Tool at http://127.0.0.1:8787 (flag: --port)
echo '{"tool":"fetch_web","context":{"taint":"tainted"}}' \
  | cargo run --bin harness -- gate --world crates/compiler/assets/default_world.yaml  # host-neutral gate ABI (D24)
```

CI runs all four checks on every push and PR
([`.github/workflows/ci.yml`](.github/workflows/ci.yml)), clippy and the tests
again on Windows, plus a `demos` job that
runs `scripts/check-demos.sh` — every example and demo script, asserting the
verdict lines each one claims to show. The unit tests cover the kernel; that
script covers what a reader actually sees.

Windows path-scope regressions check rooted and drive-prefixed targets, reject
`..` after missing components before canonicalization, and verify actual hook
verdicts. Linux tests preserve filesystem resolution of symlink/`..` paths (D88).

The blog CI job also checks the homepage-to-playground journey in Chromium at
390px and 1440px widths, including the real WASM taint/tool-toggle transitions.
The `homepage-browser-qa` artifact retains screenshots and JSON results for
14 days. Browser tooling is version-pinned and installed outside the blog; it
is not a production dependency.

### The governance benchmark

```bash
bash scripts/run-governance-bench.sh      # offline, no LLM, ~10s
```

Three scenarios — cross-principal discovery-cache leakage, approval argument
substitution and replay, cross-principal handle reuse — run against an
intentionally weak reference gateway and against this kernel, under one
deterministic oracle. The weak gateway turns one human "yes" into three
irreversible transfers; the ai2rules target refuses the substitution
(`authorization_effect_mismatch`), the replay (`authorization_exhausted`) and the
stolen handle (`authorization_principal_mismatch`), and applies exactly one.

That target is named `ai2rules-reference-host`, deliberately: it is kernel
components plus this benchmark's own trusted-host wiring, because ai2rules does
not yet ship the consume-then-invoke boundary as a command (PLAN.md E18.10).

PASS needs all three halves — the verdict, the downstream effect count the runner
watched, and evidence the oracle actually checks — and there is no aggregate
score. Read
[`docs/benchmarks/mcp-governance/`](docs/benchmarks/mcp-governance/), including
what the pack does **not** claim. CI job `governance-bench` re-runs it and fails
if the committed report has drifted, or if the weak baseline stops failing.

### Release binaries (no local toolchain needed)

Pushing a `v*` tag triggers [`.github/workflows/release.yml`](.github/workflows/release.yml),
which builds the `harness` binary on native GitHub runners and attaches the archives
to the matching GitHub Release: **Linux x86_64-musl** (static), **macOS arm64**,
**macOS x86_64**, and **Windows x86_64**. `workflow_dispatch` runs the same build for
testing without publishing a Release.

```bash
git tag v0.1.0 && git push gh v0.1.0   # → cross-OS binaries on the v0.1.0 Release
```

### See the kernel decide

The demos below are the raw list; [`docs/TUTORIAL.md`](docs/TUTORIAL.md) walks the
same ground in a deliberate order, with what each one is meant to prove.

### Five-minute governed coding workflow

Run one deterministic, credential-free scenario from a fresh fixture:

```bash
cargo run -p agent-core --example governed_coding_workflow
```

The example starts with a broken `calc.py`, then drives the real `agent-core`
loop through four dependent tool calls:

```text
read_repo_file → fetch_web (DENY) → apply_workspace_patch → read_repo_file
```

The first read adds workspace-derived taint. The model then asks for external
help; the kernel returns `DENY (taint_invariant)` as a correlated Anthropic
`tool_result`. The next model turn reads that structured result and replans to a
permitted local patch. The example prints the final diff, verifies the fixture
moved from `FAIL` to `PASS`, reads its JSONL trace back, and replays every
recorded decision against the same compiled world.

| Workflow behavior | Evidence in the run |
|---|---|
| Multi-step orchestration | Four dependent calls complete before the final answer |
| Tool use and projection | The full projected tool surface and each selected call are printed |
| State / observation feedback | The first read taints the session; each result returns to the next model turn |
| Governance boundary | External help is not executed: `DENY`, rule `taint_invariant` |
| Approval / replanning | This shortest path exercises replanning after `DENY`; approval is not invoked because a hard denial is not approvable |
| Evaluation | The expected unified diff is emitted and the fixture test changes `FAIL → PASS` |
| Traceability | The retained `trace.jsonl` path is printed and replay must report `4/4 decisions reproduced` |

The printed artifact directory remains under `target/demo-artifacts/` for
inspection and also contains `final.diff`. Replay deliberately re-runs the pure
governance decisions, not model sampling or side effects. `scripts/check-demos.sh`
runs this same path in CI, so the walkthrough fails if the refusal, replan,
verification, or replay evidence disappears.

For a quick noninteractive demo, run the kernel plus the execution boundary. It
compiles the default world, feeds it a handful of
proposed tool calls — printing `ALLOW` / `ASK` / `DENY` / `ABSENT` / `REPLAN`
for each — and, for an `ALLOW`, lowers the intent to an `ExecutionSpec` and runs
it through the executor in **simulation** (no real side effects):

```bash
cargo run -p world-kernel --example kernel_demo
```

It shows the core idea in action: an undefined action is `UNKNOWN_TO_ONTOLOGY`,
an untrusted writer is `ABSENT` by capability, tainted data into the network is
`DENY` by a hard invariant, a PTY is `ASK`, and an over-budget command is
`REPLAN` — all decided by a pure function, no LLM on the path — while the allowed
read completes an end-to-end round-trip through the boundary.

For the boundary doing **real** work (confined to a throwaway sandbox), run:

```bash
cargo run -p world-kernel --example execution_demo
```

It actually reads a file, writes one, and runs a command — then has the executor
*refuse* a write that escapes the sandbox, a stale (drifted) descriptor, and a
command that overruns its timeout. (Writes are pinned to a temp dir; network
disable is declared in the spec but not yet OS-enforced — that backstop is E8.)

For the audit trail — every decision logged, redacted, replayed, and drift
detected — run:

```bash
cargo run -p trace-store --example trace_demo
```

It records a handful of decisions to an append-only trace (secrets redacted
before disk), replays them against the same world to prove they reproduce
exactly, then replays against a changed manifest to show the drifted verdict.

For a **model driving the loop** (a deterministic scripted stand-in for an LLM —
no network), run:

```bash
cargo run -p agent-core --example agent_loop
```

The model proposes Anthropic tool calls; the harness governs each through the one
gate — a read is `ALLOW`ed and its tainted result feeds back, which then makes a
web fetch `DENY` by taint, an undefined action `UNKNOWN_TO_ONTOLOGY`, and a PTY
`ASK` — every step recorded to the trace, with no LLM on the gate.

For **approvals + fail-closed background**, run:

```bash
cargo run -p agent-core --example approvals_demo
```

A `start_pty` (approval-required) action: interactive + auto-approve resumes it
`ASK → APPROVED → ALLOW` (a durable authorization is minted → approved →
atomically consumed → executed); in background it fails closed to `DENY` with
no authorization minted.

For **scoped capabilities + MCP + web**, run:

```bash
cargo run -p agent-core --example tools_demo
```

`run_tests` proposed with `command: "rm -rf /"` lowers to argv `["pytest"]`
(locked args stripped, literal injected — invariant 12); an MCP call returns a
tainted result, after which a web fetch is `DENY`ed because the context is now
tainted (invariant 7). MCP/web use deterministic mock transports.

For the **cross-layer thesis demo** — a poisoned document cannot escalate into a
forbidden action ([`docs/THESIS.md`](docs/THESIS.md)) — run:

```bash
cargo run -p agent-core --example poisoned_knowledge_demo
```

Two sessions isolate one variable. A knowledge base (an MCP-backed retriever, à
la `context-engine`) returns a poisoned document; because `mcp_output` is
`Untrusted`+tainted, the *identical* `fetch_web https://docs.example/guide` that
is `ALLOW`ed in the baseline session is `DENY`ed in the session that retrieved
first — along with the exfil attempt — by `no_tainted_network` (invariant 7).
Knowledge-layer provenance drives action-layer enforcement; the layers compose.

> Architectural decisions and the alternatives weighed are logged in
> [`DECISIONS.md`](DECISIONS.md).

---

## Reference projects

The harness distills the best ideas from prior projects kept alongside it under
`repos/` (separate git repositories, not Cargo workspace members):

- **`repos/agent-hypervisor/`** — the research kernel: sealed typed intent,
  monotonic taint, the process boundary, design-time HITL, invariants-as-physics.
- **`repos/safe-mcp-proxy/`** — the productized MCP control plane:
  `ABSENT`/`DENY`/`ASK` semantics, descriptor-drift detection, scoped
  capabilities, provider adapters, append-only audit and replay.
- **`repos/mcp-tool-projection/`** — declarative MCP tool projections
  (`verbatim`/`partial`/`simulated`/`absent`) and a browser **authoring tool**
  with a live "effective tool surface" preview — the pattern behind **E11**
  (World Authoring Tool); see `DECISIONS.md` D17.

`docs/harness-architecture.md` attributes each borrowed principle to its source.

A wider set of external projects (governance toolkits, memory protocols, sandboxes,
skill packs) is parked under **`repos/3p/`** for comparison. What to adopt,
incorporate, position against, or ignore — and why — is recorded in
**[`docs/THIRD-PARTY-ADOPTION.md`](docs/THIRD-PARTY-ADOPTION.md)** (with `DECISIONS.md`
D27 on the Agent Governance Toolkit and D28 on MGP).

What you actually install — the product family (a Claude Code governance **plugin**, an
MCP **proxy sidecar**, the `world-kernel` **library**) and who each is for — is in
**[`docs/USE-CASES.md`](docs/USE-CASES.md)**. The delivery model is *infrastructure, not
an app* (governance you plug into a host you already run; the standalone-agent path is
cut) — see `DECISIONS.md` **D31**.

For reviewers who ask which obligation a control is evidence for, and what it leaves
behind to inspect, **[`docs/compliance-evidence-map.md`](docs/compliance-evidence-map.md)**
maps each control to its artifact (a file, a test, a decision) and to the EU AI Act,
NIST AI RMF, ISO/IEC 42001 and OWASP Agentic Top 10 clauses it can support, with the
host limits of the default configuration beside each row. It is not a compliance claim.

Speculative, research-grade directions that aren't committed work (e.g. a cross-host
"super-harness" governance plane) are parked in the idea pool at
**[`docs/RESEARCH-BACKLOG.md`](docs/RESEARCH-BACKLOG.md)**.

---

## License

Dual-licensed under either of

- Apache License, Version 2.0 ([`LICENSE-APACHE`](LICENSE-APACHE))
- MIT license ([`LICENSE-MIT`](LICENSE-MIT))

at your option. This is the Rust ecosystem convention and the terms `Cargo.toml` has
declared since E0 — the license *files* were missing until 2026-08-08, which meant the
grant was asserted in metadata with no text to accept. Apache-2.0 is the option to take
if you need an explicit patent grant.

The sibling repos ([`ai-detector-bench`](https://github.com/sv-pro/ai-detector-bench),
[`agentic-coding-lab`](https://github.com/sv-pro/agentic-coding-lab)) are MIT-only: they
are content and instruments meant to be copied from, where a single permissive license
with a minimal attribution burden is the whole point. This repo is the one someone might
build a product on top of, which is where the patent grant earns its extra file.
