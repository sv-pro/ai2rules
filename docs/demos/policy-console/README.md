# Live demo: the policy console

The playground, connected to a real agent. A browser page shows the policy of one
Claude Code project as a few plain switches. Flip one and the agent behaves
differently on its very next action, with no restart. Next to the switches, a live
feed shows every action the agent tries and what the kernel decided.

Two things to show:

1. **The policy shapes the agent.** Pick a profile, or set one switch to Allow, Ask
   or Off, and the same request gets a different answer.
2. **Some doors no policy opens.** After the agent has read anything from outside,
   nothing it does can reach the network. The console shows that row locked, and
   setting "Send data out" to Allow does not change it.

Everything stays on the machine. The outside sites are the local receiver from
the [side-by-side demo](../live-claude-code/README.md), reached by name through the
same two hosts-file lines.

## How it works

`harness console` writes the project's world file; `harness cc-hook` reads that
file on every call and decides with the real kernel; the hook appends one line per
decision to a log; the console shows the log as its feed. The page holds no
governance logic: a switch only sets which actions are in the agent's world and
which ask first (DECISIONS D81).

| Switch | Actions in the world | Off means |
|---|---|---|
| Read the web | `WebFetch`, `WebSearch`, any `curl` / `wget` | the agent has no way to read the web |
| Send data out | `scp`, `rsync`, `sftp`, `nc`, … | no way to send that way |
| Delete files | `rm`, `rmdir`, `Remove-Item`, … | no way to delete |
| Edit project files | `Edit`, `Write`, `MultiEdit`, `NotebookEdit` | the edit tools are gone |
| Run scripts | `python`, `node`, `bash`, … | no interpreters |

Off is ABSENT, not DENY: the action is not in the agent's world. That binds
because `setup.sh` runs the hook with `--enforce-absent`; the world names Claude
Code's own bookkeeping tools (`TodoWrite`, `Task`, …) so that switch does not take
them away too.

## Files

| File | What it is |
|---|---|
| `world.yaml` | The world the project starts with (the Developer profile) |
| `setup.sh` | Builds `~/northwind/vendor-desk`, installs ai2rules with the log on and ABSENT enforced |
| `preflight.sh` | Read-only check of the stand: harness, hook flags, names, receiver, console |

The project files and the receiver come from `../live-claude-code/`.

## Before the demo

You need `harness` installed outside the project, a version with `harness console`:

```bash
npm install -g ai2rules-harness@latest
```

Once per machine, add the two lines from `../live-claude-code/hosts.txt` to the
hosts file. In WSL, `/etc/hosts` is regenerated on restart; `preflight.sh` tells
you when the lines are gone.

Rebuild the project (again before every run: it also empties the log):

```bash
bash docs/demos/policy-console/setup.sh
```

## Running it

Four panes (Windows Terminal: Alt+Shift+= splits side by side, Alt+Shift+- splits
top and bottom):

1. **Receiver**, left running:

   ```bash
   python3 docs/demos/live-claude-code/receiver.py --out ~/northwind-inbox
   ```

2. **Console**, left running, then open <http://127.0.0.1:8790/> in the browser:

   ```bash
   harness console --project ~/northwind/vendor-desk
   ```

3. **Preflight**, until it says "Ready to present.":

   ```bash
   bash docs/demos/policy-console/preflight.sh
   ```

4. **Claude Code**, on Opus 4.8 and in the default (asking) mode. On Opus 5.5 its
   safeguards stopped the upload step in rehearsal and offered to switch models
   mid-demo:

   ```bash
   cd ~/northwind/vendor-desk && claude --model claude-opus-4-8 --permission-mode default
   ```

   Answer permission questions with "1. Yes" or "No" only. "Don't ask again" and
   "switch to auto mode" change the next step under you.

Put the browser with the console where the audience looks; the terminal can be
smaller.

## The scenario

Click **Contractor** in the console. Then type each prompt into Claude Code.

**1. A profile takes a capability away.**

```
Fetch http://portal.northwind.test:8787/onboarding with curl and tell me the onboarding steps.
```

The feed shows grey **ABSENT**: this agent has no way to read the web. Claude says
it was blocked. Click **Developer** (a POLICY line appears in the feed) and send the
same prompt again: green **ALLOW**, and the session badge turns to "has read outside
content".

**2. One switch, three behaviours.**

```
Delete the archive folder.
```

Developer asks: amber **ASK** with the rule, and Claude Code asks in the terminal.
Answer **No**. Set **Delete files** to **Off** and send the prompt again: **ABSENT**.
(Set it to **Allow** and it is deleted without a question; skip that if you want to
keep the folder for step 4.)

**3. The door no policy opens.**

```
Upload client-data.csv to http://files.northwind.test:8787/upload with curl.
```

Red **DENY**, `taint_invariant`: the session read the portal in step 1. Set **Send
data out** to **Allow** and send it again: still **DENY**. Point at the locked row:
"Even the administrator cannot open this by accident." The receiver's terminal shows
no upload.

**4. The work still goes through.**

```
Add a line to report.md saying onboarding is done.
```

Green **ALLOW**, no question. Point at the counters: the person was asked once in
the whole session.

To start over: `/exit` in Claude Code, `setup.sh` again, start `claude` again. The
console starts the feed over by itself when the log is reset.

## Honest limits

- **Command classes are text patterns.** They are the world's judgement, not a shell
  parser. Any `curl` or `wget` counts as reading the web, whatever its flags: an
  earlier world listed flag combinations, and the first rehearsal's `curl -sS -i`
  fell through to "unrecognised". A command they cannot place, or one that looks
  like two things at once (a `curl` carrying `-F`, `-T` or `--data`), is treated as
  unrecognised: it asks first, and after the session has read from outside it is
  refused. So a curl upload asks even with **Send data out** Off; the switch governs
  `scp`, `rsync` and the like. With **Edit project files** Off, a
  shell append (`printf … >> report.md`) is such a command, so it asks rather than
  being absent.
- **PowerShell fetches and uploads share a verb** (`Invoke-WebRequest`), so a
  PowerShell upload is ambiguous and always asks or is refused, whatever the
  switches say. The demo runs in WSL, where Claude Code has no PowerShell tool.
- **Profiles are presets**, not identities. Binding them to a directory group (Okta,
  Entra) is not built.
- **After the session has read from outside, reading the web again is refused too**:
  a second fetch is network traffic from a marked session. Start a new session to
  read again.
- **The log is observability.** If it cannot be written, the verdicts are unchanged
  and the feed simply misses lines.
