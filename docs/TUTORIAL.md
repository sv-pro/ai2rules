# Tutorial — see it decide, then let it govern your project

A guided tour of what actually works today, in the order that makes it make sense.
Nine stops, about **45 minutes** end to end, each ending in a verdict you can read.
Most stops are a single command; stops 4, 5 and 7 are a short sequence.

Stops 1–8 run **offline** — no API key, no credentials, no network, no containers.
Stop 0 is the exception, and the opposite: it's a hosted web page and needs nothing
installed at all. Between stops 4 and 5 there's a short interlude with nothing to run.

Stops 1–6 change nothing outside a temp directory (stop 2 works inside a throwaway
sandbox, stop 3 writes a trace — both `tempfile::tempdir()`). Stop 7 installs a hook
into a project you choose, and tells you how to remove it. Stop 8 is a local web UI.

> **What you are looking at.** One Rust kernel decides every verdict in this
> tutorial. The decision is a pure function of *(what was proposed, the context it
> was proposed in, the compiled world)* — no model is on the *decision* path, at any
> stop. A model may legitimately propose (that's stop 4); none may ever adjudicate. The
> design behind it: [`docs/harness-architecture.md`](harness-architecture.md); the
> thesis it serves: [`docs/THESIS.md`](THESIS.md); the vocabulary
> (ALLOW / ASK / DENY / ABSENT / REPLAN, taint, world, manifest):
> [`docs/GLOSSARY.md`](GLOSSARY.md).

**Prerequisites for stops 1–8:** a stable Rust toolchain (developed against 1.87) and
a clone of this repo. Also `python3`, which `demo-one-kernel-many-hosts.sh` uses to
pretty-print (stop 5), and `jq`, which the tutorial's own commands use at stops 5
and 7. Build once:

```bash
cargo build --workspace          # first build takes a few minutes; after that, seconds
```

---

## Stop 0 — No install: the kernel in your browser (2 min)

Open **<https://ai2rules.dev/playground/>**.

That page ships the *real* kernel compiled to WebAssembly (a 480 KB bundle) — not a
JavaScript imitation of it. Pick a preset, flip the session between **Clean** and
**Tainted**, and watch every tool sort itself into ALLOWED / ASK / DENIED / ABSENT.

**What it proves:** the governance logic is small enough and pure enough to run
anywhere — same code in your browser, in the CLI, and in the hook you'll install at
stop 7.

---

## Stop 1 — The five verdicts (3 min)

```bash
cargo run -p world-kernel --example kernel_demo
```

You'll see six proposed calls and what the kernel does with each:

```
• Developer reads a workspace file
    verdict  : ALLOW + Execute  (rule: default_allow)
• Model proposes an action the world never defined
    verdict  : UNKNOWN_TO_ONTOLOGY — `send_email` is not in this world's ontology
• Untrusted web content tries to write to the repo
    verdict  : ABSENT  (rule: capability)
• Tainted web data tries to drive a network fetch (exfil attempt)
    verdict  : DENY  (rule: taint_invariant)
• Agent asks for an interactive shell (PTY)
    verdict  : ASK  (rule: approval_required)
• Agent runs one command past its budget
    verdict  : REPLAN  (rule: max_commands_per_task)
```

Six calls, five verdicts: `UNKNOWN_TO_ONTOLOGY` is `ABSENT`'s stricter sibling. The
kernel keeps it a distinct outcome so "this action was never defined" stays
separable from "this actor may not have it" (invariant 3), but it renders as ABSENT
— the model is told the same thing either way.

**What it proves:** these are five *different* answers, not one blocklist. The
interesting one is `ABSENT` — untrusted content asking to write doesn't get
"denied", it gets told the action doesn't exist here. There is nothing to argue
with and nothing to jailbreak.

**Read the manifest that produced this** — the whole world is one YAML file:
[`crates/compiler/assets/default_world.yaml`](../crates/compiler/assets/default_world.yaml).

---

## Stop 2 — The boundary refuses (3 min)

A verdict is only worth as much as the layer that carries it out. This one does
real work in a throwaway temp directory, then refuses three things:

```bash
cargo run -p world-kernel --example execution_demo
```

```
• Write outside the sandbox (escape attempt)
    REFUSED  : write outside writable roots: /tmp/escape.txt
• Stale descriptor (rug-pull) blocked before the handler
    REFUSED  : descriptor drift — handler never ran
• Command that overruns its timeout is killed
    REFUSED  : killed after 200ms
```

**What it proves:** the executor refuses on its own — it re-checks the spec instead
of trusting that the kernel already approved it. The middle refusal is the subtle
one: the file changed *between* the decision and the execution, so the approval no
longer applies to what's actually there.

---

## Stop 3 — The audit trail, and what "deterministic" buys you (3 min)

```bash
cargo run -p trace-store --example trace_demo
```

```
2) secrets are redacted before disk
     contains the secret token? false
3) replay against the same world (determinism)
     reproduced 5/5 decisions — drift: 0
4) replay against a changed manifest (Trusted loses Read)
     1 decision(s) now differ:
       seq 0: read_workspace   Deny -> Absent
```

**What it proves:** this is the payoff for keeping the model off the decision path.
Every decision replays exactly, so you can ask a counterfactual — *"if I tighten
this rule, what would have changed?"* — and get an exact answer instead of a vibe.
Try that with a policy that an LLM adjudicates.

---

## Stop 4 — A model driving the loop (5 min)

Three examples, increasingly pointed. All use a scripted stand-in for the model, so
they're deterministic and offline.

```bash
cargo run -p agent-core --example agent_loop              # propose → decide → execute → perceive
cargo run -p agent-core --example tools_demo              # scoped capabilities, MCP, web
cargo run -p agent-core --example poisoned_knowledge_demo # the cross-layer one
```

The last one is the thesis in miniature — two sessions that differ by one step:

```
Session 1 — BASELINE (no retrieval):
  1. legit fetch     fetch_web   ALLOW

Session 2 — POISONED (retrieve from KB first):
  1. retrieve from KB  call_known_mcp_tool  ALLOW
  2. exfil attempt     fetch_web            Deny (taint_invariant)
  3. same as baseline  fetch_web            Deny (taint_invariant)
```

**What it proves:** the *identical* fetch is allowed in one session and denied in
the other. Nothing about the request changed — only where the session had been. A
permission list can't express that, because it has no memory of provenance. Note
line 3: the poisoned session loses a legitimate capability too. That's the honest
cost of a taint floor, and it's the tradeoff to argue about.

Also worth reading in `tools_demo`: `run_tests` proposed with
`command: "rm -rf /"` lowers to argv `["pytest"]`. The dangerous argument isn't
detected and rejected — it's *not representable*, so it's dropped on the way down.

---

## Interlude — two crates, two different jobs

*Nothing to run here. Two minutes of reading, between the loop and the hosts.*

Stop 4 is where people conflate `world-kernel` and `agent-core`, because both are
Rust and both just ran deterministically. They are not peers.

|  | `world-kernel` | `agent-core` |
|---|---|---|
| Is | a decision function | a loop that calls it |
| Entry point | `decide(intent, context, world)` | `run(SessionConfig)` |
| Depends on | `harness-types` + two pure helpers. No I/O, no clock, no network | the kernel *plus* compiler, executor, trace-store, provider-adapters |
| Deterministic | **by construction** — this is the property everything else rests on | **only here**, because a `ScriptedModel` is plugged into its model seam |
| Can build an `ExecutionSpec` | yes — it is the only thing that can | no; it has to ask the kernel |

The determinism you just watched is two different things wearing one word. The
kernel's is architectural: same inputs, same verdict, in a hook or in the browser
— that's what makes stop 3's replay and stop 0's playground possible. The loop's
is a **test fixture**: `agent-core`'s `ModelClient` is the seam a real LLM plugs
into, and `ScriptedModel` (48 lines in
[`crates/agent-core/src/model.rs`](../crates/agent-core/src/model.rs)) is a canned
stand-in so these examples run offline. Swap in a live model and the loop stops
being deterministic — while every verdict in it stays exactly as deterministic as
before. That gap is the whole design.

So only the stub stands in for an LLM. The kernel isn't a stand-in for anything —
it exists so no model is ever asked to be the judge. A model may legitimately
produce `agent-core`'s *proposals*; nothing but the kernel may produce a *verdict*.
That's enforced by the crate graph, not by discipline: `agent-core` cannot
construct an `ExecutionSpec`, so it has to go through `decide()` to get anything
done at all.

Which raises the obvious question — what happens when the loop isn't ours? That's
the next stop.

---

## Stop 5 — One kernel, many hosts (5 min)

```bash
bash scripts/demo-one-kernel-many-hosts.sh
```

Eight blocks, numbered 1 to 7 (step 6 splits in two). The first three shape an MCP
server's tool list — a real `tools/list` goes from 7 tools to 4, so the three
destructive ones are never offered to the model at all. Then the same `rm -rf` gets
`ASK` interactively and `DENY` in background; a tainted `curl` is denied (6a) and so
is a tainted MCP call through the gateway (6b); and the last beat sends the *same*
request in Claude Code's shape and OpenCode's shape:

```
Claude Code shape : DENY rule=taint_invariant taint=tainted action=bash_network hash=e765a236c5fd
OpenCode shape    : DENY rule=taint_invariant taint=tainted action=bash_network hash=e765a236c5fd
✅ decision/rule/taint/action/manifest_hash identical — one kernel, many hosts
```

**What it proves:** hosts translate shapes; they never hold policy. The matching
`manifest_hash` is the load-bearing part — both hosts decided against the byte-identical
compiled world.

And note what is *absent* from that path: `agent-core`. When Claude Code or
Antigravity runs the loop, our loop isn't in the picture at all — the host goes
straight to `harness_preview::gate()` → `world_kernel::decide()`. Grep the
workspace for `agent_core` and you'll find exactly one importer,
`cli-harness/src/main.rs`, the interactive REPL. `agent-core` is the in-house loop
for when the harness hosts the agent itself; the kernel is what every host needs.

### Drive the gate yourself

The host-neutral port is JSON in, JSON out
([schema](harness-gate-abi.md)):

```bash
W=docs/demos/one-kernel/demo-world.yaml
echo '{"tool":"bash","arguments":{"command":"rm -rf /tmp/x"},
       "context":{"session_id":"me","mode":"interactive","taint":"clean",
                  "source_channel":"user_prompt"}}' \
  | cargo run -q --bin harness -- gate --world $W | jq -c '{decision,rule,action}'
# {"decision":"ASK","rule":"approval_required","action":"bash_destructive"}
```

Now change **one field at a time** and watch the verdict move:

| Change | Verdict |
|---|---|
| `"mode":"background"` | `DENY` / `background_denies_ask` — nobody's there to approve, so it fails closed |
| `"taint":"tainted"` + `curl https://x.example` | `DENY` / `taint_invariant` |
| drop `"source_channel"` entirely | `DENY` / `missing_source_channel` |

That last row is worth pausing on: an incomplete request is not a request the
kernel guesses about. Missing context fails closed.

---

## Stop 6 — Replace the permission pile (5 min)

```bash
bash docs/demos/replace-permissions/demo.sh
```

```
clean Read               (manifest ALLOW -> grant) -> allow
clean curl example.com   (ALLOW -> grant; taints)  -> allow
curl again, now TAINTED  (taint floor)             -> deny
rm -rf /tmp/x            (destructive -> ask)      -> ask
SomeUnknownTool          (not in manifest -> ABSENT) -> deny
```

**What it proves:** same `curl`, allow then deny — the only thing that changed is
that the session read from the network in between. And because the hook runs with
`--grant`, the manifest isn't decorating Claude Code's permission list; it *is* the
permission list.

Read [the demo's README](demos/replace-permissions/README.md) before you rely on
this — it documents where the seams are (native `deny`/`ask` rules still fire, the
hook governs calls but can't remove a tool from the model's view, behavior is
sensitive to the Claude Code version).

---

## Stop 7 — Govern a real project (10 min)

This is the one that changes something on your machine. Use a scratch project first.

```bash
mkdir -p /tmp/toyproj && cd /tmp/toyproj && git init -q
bash /path/to/ai2rules/scripts/install-governance.sh .    # your checkout
```

It does two things: installs the `harness` binary at a trusted absolute path
(`~/.local/bin` by default — `--bin-dir DIR` to move it), and drops four files into
the project:

```
.claude/hooks/world-gate.sh   # 23-line shim; execs the kernel, holds no policy
.claude/cc-world.yaml         # the starter manifest — this is your policy
.claude/settings.json         # registers the PreToolUse hook
.gitignore                    # appends .claude/state/ and .claude/gate-off
```

The install is *additive* by default: the manifest can add `deny`/`ask`, never
grant. Add `--grant` for the stop-6 replace mode once you trust it.

### Verify it without launching an agent

The hook is a program that reads JSON on stdin. You can be the agent:

```bash
cd /tmp/toyproj && export CLAUDE_PROJECT_DIR=$PWD
H=.claude/hooks/world-gate.sh

# 1) a clean fetch — silent (exit 0 = the normal flow proceeds), and it taints the session
echo '{"session_id":"t","tool_name":"WebFetch","tool_input":{"url":"https://example.com"}}' | bash $H

# 2) the identical fetch, now that the session is tainted
echo '{"session_id":"t","tool_name":"WebFetch","tool_input":{"url":"https://example.com"}}' | bash $H | jq -r .hookSpecificOutput.permissionDecisionReason
# tainted context cannot reach an externally-effectful action (taint_invariant)

# 3) a destructive command
echo '{"session_id":"t2","tool_name":"Bash","tool_input":{"command":"rm -rf /tmp/x"}}' | bash $H | jq -r .hookSpecificOutput.permissionDecision
# ask

# 4) a write outside the project
echo '{"session_id":"t3","tool_name":"Write","tool_input":{"file_path":"/etc/passwd","content":"x"}}' | bash $H | jq -r .hookSpecificOutput.permissionDecisionReason
# the target path is read-only under the roots policy (path_scope_readonly)
```

Call 1 leaves a taint sidecar at `.claude/state/taint-t` — that file is why call 2
is denied. Delete it and call 2 is allowed again.

Then do it for real: start Claude Code in that directory, ask it to fetch a web
page, then ask it to fetch another. The second one is denied. An ungoverned session
would just prompt you.

### Turning it off

```bash
touch .claude/gate-off        # off for this project, next call, no restart
touch ~/.claude/gate-off      # panic switch, everywhere
rm .claude/gate-off           # back on
rm -rf .claude                # full uninstall
```

Note the installer **appends** to `.gitignore` rather than owning it, so uninstalling
means deleting the two lines it added (`.claude/state/`, `.claude/gate-off`) — not
the file.

The kill switch is checked per call, which is what makes it safe to try this on
something you care about.

### Then tune the manifest

Open `.claude/cc-world.yaml`. That file is the whole policy: which actions exist,
which paths are writable, what counts as destructive, what taint forbids. Changing
it changes the verdicts on the next call — no restart, no redeploy.

---

## Stop 8 — Author a world with a live preview (5 min)

```bash
cargo run --bin harness -- serve          # http://127.0.0.1:8787
```

Edit a manifest on the left; the right side shows the projected tool surface and a
clean-vs-tainted decision matrix. The preview goes through the **real** compiler and
kernel (`POST /api/preview`) — there's no JavaScript reimplementation of the rules,
so what you see is what the hook will do.

Also: `cargo run --bin harness` with no subcommand gives you an interactive session
where you play the model — propose calls from the projected surface and watch each
one get decided.

---

## What is *not* done yet

So you don't mistake a demo for a guarantee:

- **OS-level isolation is not there.** Writes are confined to declared roots and
  commands are killed on timeout, but network egress is *declared* in the spec, not
  enforced by the OS. That's E8, planned. The containerized setup in `docker/` is
  the current floor.
- **`ABSENT` is fully real only at the MCP gateway.** There, a destructive tool is
  never advertised (stop 5). In the Claude Code hook, `ABSENT` still has to be
  reported as a deny, because a `PreToolUse` hook governs calls and cannot remove a
  tool from the model's view.
- **MCP and web transports in the crate examples are mocks.** The real MCP path is
  the `mcp-gateway` (stop 5) and the Atlassian skin in
  [`docs/demos/jira-copilot/`](demos/jira-copilot/).
- **The MCP gateway speaks an older protocol than the current spec.** It declares
  protocol version `2024-11-05` and does not implement `server/discover` or the
  required `resultType` — legal, because the spec's backward-compatibility rules
  accommodate older servers, but it is a legacy server by choice, not a current one.
  The gap that matters: revision `2026-07-28` introduced **MRTR**, where a server
  answers a tool call with a result that *demands* input (an elicitation, or an LLM
  completion) which the client must supply before retrying. The gateway forwards such
  a result unexamined, and the kernel has no verdict shape for "the result asked for
  something." Nothing in the loop can produce one today — there is no modern upstream
  — so this is design debt, not a live hole. See `DECISIONS.md` **D49** and
  [issue #40](https://github.com/sv-pro/ai2rules/issues/40).
- **Trust pins were consciously dropped** in the Rust cutover (D29) until they land
  as typed manifest fields.

Milestone-level state is in the [README status table](../README.md#status); the
reasoning behind every choice is in [`DECISIONS.md`](../DECISIONS.md).

## Where to go next

| You want | Go to |
|---|---|
| The argument, not the code | [`docs/THESIS.md`](THESIS.md) |
| How the pieces fit | [`docs/harness-architecture.md`](harness-architecture.md) |
| Wiring a new host | [`docs/harness-gate-abi.md`](harness-gate-abi.md) + [`docs/one-kernel-many-hosts.md`](one-kernel-many-hosts.md) |
| A realistic end-to-end scenario | [`docs/demos/jira-copilot/`](demos/jira-copilot/) |
| OpenCode / Antigravity CLI instead of Claude Code | [`docs/demos/opencode/`](demos/opencode/), [`docs/demos/antigravity/`](demos/antigravity/) |
| Why a decision was made | [`DECISIONS.md`](../DECISIONS.md) |
