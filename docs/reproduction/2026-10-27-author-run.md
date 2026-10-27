# Author's clean run of the quickstart — 2026-10-27

A dry run of [`QUICKSTART.md`](QUICKSTART.md) by the author's side (Claude Code,
for AI2-9 / #62), before an outside tester tries it. **This is not the external
reproduction** that AI2-9 asks for: it only checks that the guide works as
written on one clean setup, and records the friction found on the way.

**Result: the guide works end to end on Windows.** The governed session's `curl`
was refused with `taint_invariant` in all four governed runs; the ungoverned folder ran
it. The friction below concerns what the tool *prints* on Windows, not what it
*decides*.

## Environment

| | |
|---|---|
| OS | Windows 11 Pro 10.0.26300, Git Bash |
| Node / npm | v24.21.0 / 11.19.0 |
| Claude Code | 2.1.311 |
| ai2rules | `ai2rules-harness@0.7.0` from npm, installed into an empty npm prefix (`npm install -g --prefix <empty dir>`), first on `PATH` |
| Project | a new empty folder with `git init`, nothing else |
| User settings | no user-level hooks; runs used `--setting-sources project,local` |
| Network | the run's sandbox had no DNS: `WebFetch` and the baseline `curl` failed to resolve `example.com` (see "What this run could not show") |

## Steps and results

| Step | Result |
|---|---|
| 1. Install 0.7.0 | `cli-harness 0.7.0`, about 14 s |
| 2. `harness init` | Four writes, additive mode, as documented |
| 3. `harness demo` | Five verdicts as in the guide; `Replay: 5/5 decisions reproduced` |
| 4. Governed session | `PreToolUse:Bash hook error: tainted context cannot reach an externally-effectful action (taint_invariant)` in 4 of 4 runs (three with `claude-sonnet-5-5`, one with the default model); `curl` never ran |
| 4a. Same `curl`, clean session | Asked of the hook directly with a fresh session id and no fetch before it: no output, so it passes to Claude Code's own permissions |
| 5. Ungoverned folder | `curl` ran (exit 6, resolve failure, in this sandbox); `taint_invariant` count `0` |

The guide was then run **literally**, command by command as printed, in a new
folder: the same results. That run is why step 4 pipes `grep` through
`head -1`: the error string occurs more than once in the transcript.

## Friction found

Ranked by how likely each one stops or misleads an outside tester.

1. **Windows: `harness doctor` says `INSTALLATION BROKEN` on a working
   install.** `harness demo` ends with `Next: harness doctor`, so a tester who
   follows the tool's own advice is told it is broken. The findings were
   `CC_INTERPRETER_MISSING: bash interpreter not found in PATH` and
   `CC_CLI_NOT_FOUND: Claude CLI not found on PATH`. Both were on `PATH`
   (Git Bash's `bash`, npm's `claude` shim); the hook ran in every session.
   The doctor's fixture suite is Unix-only (`AGENTS.md`), which fits this.
   **Highest impact.** The guide tells the tester to skip `doctor` for now.
2. **Windows: the commands `init` prints do not paste into Git Bash.** The
   proof command and the kernel path carry the verbatim prefix `\\?\C:\…`;
   pasted, bash drops the backslashes (`No such file or directory`, exit 127).
   The kill-switch line `touch C:\Users\…\off\<key>` fails the same way, and
   would fail even with a correct path, because `~/.claude/ai2rules/off/` does
   not exist yet. Still true on `main` at `35dd9e2`: a current debug build
   prints the same `\\?\` paths. `init --user` / `--managed` already strip the
   prefix (`init_machine.rs`), project `init` does not.
3. **The README's hand-run proof is Unix-shaped.** `Write /etc/passwd` answers
   ASK (`path_scope_ask`) on Windows, not the DENY (`path_scope_readonly`) the
   README shows, because `/etc` is not a Windows path and falls to the roots
   default. And `CLAUDE_PROJECT_DIR=$PWD` in Git Bash gives a `/c/…` path the
   kernel does not match to the project, so a write *inside* the project also
   answers ASK instead of staying silent. With the Windows form of the path
   (`cygpath -w`) both behave as documented. Real sessions are not affected:
   Claude Code sets `CLAUDE_PROJECT_DIR` itself.
4. **A `.claude/` write is a weak demo.** ai2rules denies it
   (`path_scope_readonly`), but Claude Code also protects `.claude/` on its
   own: the same request in an ungoverned folder is refused by Claude Code
   ("a sensitive file"). It shows the kernel works, not what it adds. The guide
   uses the fetch-then-`curl` case instead, where the ungoverned run goes
   through.
5. **The released package is 179 commits behind `main`.** A tester gets 0.7.0;
   flow contracts, the decision log's outcome join and the newer demos are not
   in it. The guide is pinned to 0.7.0 on purpose.

Smaller notes, not friction for a tester:

- Claude Code validates an edit to `settings.json` before `PreToolUse` hooks
  run, so a test prompt that writes invalid JSON there never reaches the kernel.
- The hook command `init` writes is `bash '//?/C:/…/world-gate.sh'`. It looks
  fragile but ran in every session here.

## What this run could not show

- **macOS and Linux.** Not run here. CI covers the kernel on Linux, not this
  guide.
- **A run with working DNS.** `WebFetch` failed to resolve, yet the session was
  still marked, because the mark comes from reaching for the web, not from the
  page's content. With a normal network, the baseline's `curl` should print
  `200` and `WebFetch` should return the page title; neither was observed here.
- **Interactive `claude`.** Only headless `claude -p` was run.
- **An outside person.** That is the next step of AI2-9.

## Suggested fix before the tester's run

Friction 1 first: make `harness doctor` find `bash` and `claude` on Windows
(`PATHEXT`, `.cmd` shims). That is
[#178](https://github.com/sv-pro/ai2rules/pull/178). Its review found a second
half: on Windows, the bash that matters is the Git Bash Claude Code finds itself
(`CLAUDE_CODE_GIT_BASH_PATH`, else the Git install), not any `bash` on PATH,
which from PowerShell is often the WSL launcher. #178 does both. Until a release
carries it, 0.7.0 testers skip `doctor`. Then friction 2: strip the `\\?\` prefix in project `init`'s
printed commands, as `init_machine.rs` already does, and print the kill switch
with the `mkdir -p` it needs.
