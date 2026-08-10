---
title: 'Govern Your Coding Agent in One Command'
description: "Your coding agent can write anywhere your shell can, and it reads untrusted text on every turn. Here is a one-command install that puts a deterministic gate in front of it — plus the two-line test that proves the gate is real, and the honest list of what it does not do."
pubDate: 'Aug 10 2026'
heroImage: '../../assets/govern-your-coding-agent-in-one-command.jpg'
---

Start with the part you can check yourself, in a directory you do not care about:

```bash
npm install -g ai2rules-harness
harness init
```

Then, without starting an agent session at all, ask the gate what it thinks of a
write to `/etc/passwd`:

```bash
echo '{"tool_name":"Write","tool_input":{"file_path":"/etc/passwd"}}' \
  | CLAUDE_PROJECT_DIR=$PWD bash .claude/hooks/world-gate.sh
```

```json
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny",
 "permissionDecisionReason":"the target path is read-only under the roots policy (path_scope_readonly)"}}
```

Now change the path to something inside the project and run it again. Nothing
comes back — silence means it passed. That difference is the entire product, and
you just reproduced it in about five seconds.

## What the install actually did

Four files, all in your project, all readable:

```
.claude/cc-world.yaml         a manifest: what this project's agent may do
.claude/hooks/world-gate.sh   a shim your host calls before every tool call
.claude/settings.json         the hook entry, merged into whatever was there
.gitignore                    ignores the runtime state and the kill-switch
```

Nothing else. No daemon, no account, no telemetry, no network calls at runtime.
The shim is nine lines of shell that hand the proposed call to a Rust binary and
print its verdict.

It is safe to run twice. If you have already tuned the manifest, `init` keeps it
and says so; your existing hooks and settings survive untouched.

## Why the deny is more interesting than it looks

Most agent permission systems are lists of command strings: allow `git status`,
ask about `rm -rf`, deny `curl`. That model breaks in a specific and boring way —
it can only recognise things it has seen written down before, and there are
infinitely many ways to write "delete that file".

The deny above is not about the command's shape. It is about **where the path
points**. The manifest declares the project directory as the writable root, and
anything resolving outside it is refused whatever it looks like. `/etc/passwd`,
`../../secrets`, a symlink, an absolute path assembled at runtime — same answer,
because the question being asked is about the destination, not the spelling.

## The one an allowlist cannot express at all

Here is the sequence worth running, because no list of permitted commands can
represent it. Fetch a web page:

```bash
echo '{"tool_name":"WebFetch","tool_input":{"url":"https://example.com"}}' \
  | CLAUDE_PROJECT_DIR=$PWD bash .claude/hooks/world-gate.sh
# (silence — allowed)
```

Now fetch a *different* page, or run `curl`:

```json
{"permissionDecision":"deny",
 "permissionDecisionReason":"tainted context cannot reach an externally-effectful action (taint_invariant)"}
```

The identical call, allowed and then denied. Nothing about the command changed.
What changed is that the session read something from the outside world in
between, and the gate now treats everything downstream as carrying that
influence.

This is the shape of a real prompt-injection attack: the model reads a web page
or an issue body, that text says something persuasive, and the model then does
something outward-facing with your credentials. A permission list sees two
identical `curl` calls and has no vocabulary for "but one of them came after
reading a stranger's text". This gate does, and it is the reason the project
exists.

You can see the state it is tracking:

```bash
cat .claude/state/taint-default
# tainted by WebFetch (WebFetch)
```

Delete that file and the session is clean again. It is a text file. That is
deliberate — you should be able to read everything this thing believes.

## Two modes, and the difference matters

```bash
harness init            # additive
harness init --grant    # replace
```

**Additive** is the default and cannot lock you out. It only ever *adds* denials
and prompts on top of your host's existing permissions. If the gate vanishes,
you are back to normal.

**Replace** (`--grant`) means the manifest becomes the allowlist: it returns an
explicit "allowed" and your host stops asking. That is genuinely better once you
trust your manifest — fewer prompts, one file deciding — and genuinely worse if
you have not read it. Start additive.

## Turning it off is one file

```bash
touch .claude/gate-off      # this project
touch ~/.claude/gate-off    # everywhere, right now
```

Effective on the very next tool call, no restart. Delete the file to re-enable.

I put this section here rather than at the end because it is the reason it is
reasonable to try any of this on a real project. A governance tool you cannot
switch off in one second is a governance tool you should not install.

## What this does not do

- **Governed is not sandboxed.** This decides what the agent may *do*; it does
  not contain *where it runs*. If you want both — and you probably do — this sits
  alongside a sandbox, not instead of one.
- **Depth depends on your host.** Deep where the host exposes a pre-execution
  hook, MCP-seam-only where it does not. What each tool actually permits is
  measured, including the cells where the answer is "we don't know", in
  [the Agent Governability Index](https://github.com/sv-pro/ai2rules/blob/main/docs/GOVERNABILITY-INDEX.md).
- **The starter manifest is a starting point.** It is a generic default that
  confines file writes to the project and treats network reads as tainting. It
  knows nothing about your project. Read it; it is about ninety lines of YAML.
- **Prefer the global install over `npx` for real use.** `npx ai2rules-harness
  init` works and is the fastest way to see a verdict, but the shim records the
  path of the binary that ran it, and under `npx` that path lives in a temporary
  cache. When the cache is cleared the binary is gone — and the shim then **fails
  open**, which is the right behaviour for a missing kernel and a surprising place
  to discover it. After installing globally, re-run `harness init`.

## If you want to look further

The gate is a Rust kernel: a verdict is a pure function of the proposed call, the
session's accumulated context, and the compiled manifest. No model is consulted
at decision time, which is what makes the same inputs always produce the same
verdict and makes every decision replayable afterwards.

- Package: [`ai2rules-harness`](https://www.npmjs.com/package/ai2rules-harness) —
  no dependencies; the install fetches a checksum-verified prebuilt binary.
- Source, and the argument in full:
  [github.com/sv-pro/ai2rules](https://github.com/sv-pro/ai2rules).
- A longer offline tour, nine stops, no credentials:
  [`docs/TUTORIAL.md`](https://github.com/sv-pro/ai2rules/blob/main/docs/TUTORIAL.md).

If you run it and something behaves differently from what is written here, that is
worth an issue — the whole point of a deterministic gate is that its behaviour is
supposed to be a fact you can check, not a claim you have to accept.
