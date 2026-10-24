# Northwind demo readiness — 24 October 2026

**Verdict: native stand and decision sequence verified; interactive `/northwind-demo`: NOT RUN.**
A successful automated probe or deployment does not establish readiness of the
live Claude Code presentation. One full rehearsal on the presentation host is
still the acceptance gate.

## Immutable inputs

| Input | Checked revision / state |
|---|---|
| Current main | [`90836af`](https://github.com/sv-pro/ai2rules/commit/90836affa1154a4c80e90a868c810081ed0b633c) |
| PR #134, skill | [`dc18eb8`](https://github.com/sv-pro/ai2rules/pull/134), open; AI2-62 In Progress |
| PR #130, host vocabulary | [`51142c0`](https://github.com/sv-pro/ai2rules/pull/130), open; AI2-58 In Progress |
| Both PRs' main baseline | `efb8127b66c3d4ec0b6858833a235892ab7452e8` |
| Native executable | `cli-harness 0.7.0`, Linux binary from main's [CI run 40003275443](https://github.com/sv-pro/ai2rules/actions/runs/40003275443), artifact `12522118855` |
| Artifact archive SHA-256 | `02cc5874423508ae46c5cd4049a0ef5e680885099bdc689d7b79affcfc5e8439`, matched GitHub metadata |

Main differs from the PR baseline only by a research document, not by runtime
code. Its binary is therefore suitable for exercising the worlds and setup
from these PRs. Both PRs' patches applied together locally without conflicts.
The fixes in this branch are based on PR #134; PR #130's world was supplied
separately to the combined-world probes, and is not silently included here.

Main alone does not install `/northwind-demo`. PR #134 supplies the skill and
installation. `Skill` and `AskUserQuestion` already exist in the baseline world:
PR #130 is not a hard dependency for those two tools. It adds vocabulary needed
when the host chooses additional app tools. After applying world changes,
rerun setup: an existing demo folder retains its copied world otherwise.

## Verified execution boundary

The checks used the real `setup.sh`, generated shim, native `cc-hook`, persisted
session state, console HTTP API and Python receiver. Tool events were injected
by a test runner, not produced by Claude Code or an LLM. Policy changes were
made by the runner through the console's token-protected API.

The same sequence passed with the PR #134 world and with PR #130's additions:

| Step / operator state | Kernel result | Hook answer / evidence |
|---|---|---|
| Skill and pause tools, Contractor | ALLOW | explicit `allow` under `--grant` |
| Fetch onboarding, Contractor | ABSENT, session clean | `deny`, no network command executed |
| Switch to Developer; fetch | ALLOW, session tainted | `allow`; a real loopback fetch returned the onboarding page |
| Delete archive, Developer | ASK | `ask`; runner did not execute deletion |
| Delete files Off; same deletion | ABSENT | `deny`; archive remained intact |
| Curl upload after the fetch | DENY, `taint_invariant` | `deny`; upload not executed |
| Send data out Allow; same upload | DENY, `taint_invariant` | `deny`; upload not executed |
| Pause tool after taint | ALLOW | explicit `allow` |
| Local Edit of report | ALLOW | explicit `allow`; native permission result only |
| Fetch again in the same tainted session | DENY, `taint_invariant` | confirms fresh-session requirement |

PowerShell-shaped events also returned ABSENT for the Contractor fetch and DENY
for the tainted upload. This is adapter/kernel testing on Linux, not execution
of PowerShell or Claude Code on Windows.

With PR #130, `ListAgents` was allowed, while `Monitor` and the browser navigation
tool were ABSENT by projection. The complete native decision log matched the
console feed in each run. The receiver had no uploaded files. That last check
corroborates this runner's behaviour; it does not prove a live host enforced
the hook, because the runner deliberately did not execute denied commands.

Evidence: [PR #134 log](readiness-evidence/pr134-decisions.jsonl),
[combined-world log](readiness-evidence/pr134-plus-pr130-decisions.jsonl),
[probe results](readiness-evidence/summary.json),
[launcher output](readiness-evidence/launcher.txt).

## Minimal fixes made

1. **False preflight success.** The original script was reproduced reporting
   success with no all-tools hook entry, and with a console serving another
   project having the same `world_id`. Preflight now checks the exact project
   and log paths, hook registration, `--grant`, syntax, executable availability,
   kill switches, installed skill, and compilation/projection of the pause tools.
2. **Custom ports.** The skill previously retained 8787/8790 even when the stand
   used other ports. Setup now renders both URLs, validates port numbers and
   prints matching manual commands. Default and crossed-default port values
   were checked.
3. **Proxy and HTTP status handling.** Loopback checks bypass proxies and reject
   HTTP errors. The actual launcher reached its manual Claude handoff with a
   deliberately broken proxy, then stopped both servers cleanly.
4. **Unsafe restart advice.** The README suggested starting manually and using
   the skill for the rest, but the skill has no resume mode. The runbook and
   skill now require a fresh session after setup and preserve the four operator
   pauses in order.
5. **Claims match the boundary.** The README now states that the demo's editable
   project contains its own skill, world and hook. Instructions to leave policy
   changes to the presenter are not an enforced filesystem boundary.

The native probe runner completed 84 assertions, including eight negative
preflight fixtures and checks that preflight left project file contents
unchanged. Default-port installation, reset of old session state, invalid-port
rejection, launcher startup/cleanup, `bash -n` and `git diff --check` also passed.

To repeat the native probes without credentials or a model:

```bash
# In this branch; obtain a trusted native harness with console support first.
git fetch origin refs/pull/130/head:refs/remotes/origin/pr-130
git show origin/pr-130:docs/demos/policy-console/world.yaml > /tmp/northwind-pr130-world.yaml
HARNESS=/absolute/path/to/harness python3 scripts/check-policy-console-demo.py \
  --pr130-world /tmp/northwind-pr130-world.yaml \
  --evidence /tmp/northwind-readiness-evidence
```

The runner also needs the immutable PR #134 head `dc18eb8` in local git for its
original-preflight regression controls. It uses explicit `curl --resolve`
loopback mappings and never changes the machine's hosts file. The evidence here
therefore does not verify the presentation machine's DNS/hosts configuration.

## NOT RUN and remaining acceptance

- **Interactive Claude Code `/northwind-demo`: NOT RUN.** No Claude Code CLI or
  interactive desktop session was available. AskUserQuestion widgets, actual
  permission prompts, model tool selection, continuation after a denied deletion,
  the final model summary and live browser rendering remain unverified.
- **Chosen model availability and behaviour: NOT RUN.** Opus 4.8 is the runbook's
  rehearsal choice, not a guarantee. A model refusing without a tool call produces
  no kernel decision and must not be presented as a successful kernel block.
- **Windows/Git Bash and the presentation machine: NOT RUN.** User/managed
  settings, inherited hooks and native deny/ask rules can change the experience;
  project-file checks cannot certify those settings in a separate session.
- **Rust source build/unit suite: NOT RUN.** No Rust toolchain was installed.
  The checks executed a verified existing main binary; no Rust code changed.
- **Tamper resistance is outside this demo.** Decision-only probes confirmed
  that Edit of the world and hook would be ALLOW with the demo's edit switch on.
  Those edits were not executed. The world has no `roots`; no last-minute policy
  hardening or wider product changes were made.

Before accepting the live presentation, rebuild the stand from the chosen code,
run preflight on that machine, then open a fresh Claude Code session in the demo
folder. Run `/northwind-demo` once from beginning to end. Confirm **Contractor →
Developer → Delete files Off → Send data out Allow**, deny the deletion prompt,
and wait for each POLICY line before Done. Require the seven scenario attempts
in the decision log, a real successful onboarding fetch, an intact archive, no
received upload and a real report edit. Additional bookkeeping lines are normal;
a missing tool attempt is not a passed step. Keep the observed model/version,
transcript and matching log as the evidence for that rehearsal.
