# Live demo: the policy console

The playground, connected to a real agent. A browser page shows the policy of one
Claude Code project as a few plain switches. Flip one and the agent behaves
differently on its very next action, with no restart. Next to the switches, a live
feed shows every action the agent tries and what the kernel decided.

Two things to show:

1. **The policy shapes the agent.** Pick a profile, or set one switch to Allow, Ask
   or Off, and the same request gets a different answer.
2. **Some doors no policy opens.** After the agent has read anything from outside,
   nothing it does can reach the network. The console pins the kernel's invariants
   in a band above the policy, lights the one that decided a verdict, and marks that
   line in the feed with a lock. Setting "Send data out" to Allow does not change it.

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
because `setup.sh` runs the hook with `--enforce-absent`, under which a tool the
world does not name is absent too, harmless or not. So the world names every
built-in tool of Claude Code and of the Claude app. The bookkeeping ones
(`TodoWrite`, the app's task list, chapters, …) are reads. The ones with real
effects (the browser pane, `Monitor`, artifacts, scheduling, starting other
sessions) are named but not projected, so the feed says "not projected", not
"unknown". A tool from an account's own connectors stays unknown, and so absent.

The hook belongs to the folder the session **started** in. Changing folder inside
the session does not leave this world, so start working sessions somewhere else.

## Files

| File | What it is |
|---|---|
| `world.yaml` | The world the project starts with (the Developer profile) |
| `setup.sh` | Builds `~/northwind/vendor-desk`, installs ai2rules with the log on and ABSENT enforced |
| `preflight.sh` | Read-only check of the stand: harness, hook flags, names, receiver, console |
| `run-demo.sh` | The whole stand in one command: setup, receiver, console, preflight, browser, Claude Code |

The project files and the receiver come from `../live-claude-code/`.

## Before the demo (once per machine)

1. **A `harness` with the `console` command, installed outside the project.** Once
   a release carries it:

   ```bash
   npm install -g ai2rules-harness@latest
   ```

   Until then, use a build of this branch and point `HARNESS` at it in the run
   command below. CI's `dryrun-harness-*` artifacts are packaging rehearsals that
   hold one Linux binary in every archive; a real Windows build comes from a manual
   run of `release.yml` (artifact `harness-x86_64-pc-windows-msvc`).

2. **The two hosts-file lines** from `../live-claude-code/hosts.txt`
   (`C:\Windows\System32\drivers\etc\hosts` as Administrator on Windows,
   `/etc/hosts` with sudo elsewhere). WSL regenerates `/etc/hosts` on restart; the
   preflight tells you when the lines are gone.

## Running it

One command builds and starts the whole stand. Run it from the repository root, in
Git Bash on Windows or any shell elsewhere:

```bash
bash docs/demos/policy-console/run-demo.sh
```

When the right `harness` is not first on PATH, name it:

```bash
HARNESS="$HOME/tools/harness-console/harness-x86_64-pc-windows-msvc/harness.exe" bash docs/demos/policy-console/run-demo.sh
```

It does five things and prints each:

1. rebuilds the project in `~/northwind/vendor-desk`: a clean copy, an empty
   decision log, an empty `~/northwind-inbox`;
2. starts the receiver (port 8787) and the console (port 8790) in the background,
   logging to `~/northwind-logs`;
3. runs `preflight.sh` and stops if anything is not ready;
4. opens the console, <http://127.0.0.1:8790/>, in the browser;
5. opens Claude Code:
   - **with the `claude` CLI on PATH** it starts it in the project, on Opus 4.8 and
     in the asking mode (on Opus 5.5 the model's safeguards stopped the upload step
     in rehearsal and offered to switch models mid-demo). Answer permission
     questions with "1. Yes" or "No" only: "don't ask again" and "switch to auto
     mode" change the next step under you. Follow the receiver in a second pane
     with `tail -f ~/northwind-logs/receiver.log`;
   - **without it** (the Claude app on Windows) it prints the folder to open, then
     shows the receiver's request log in the same terminal. In the app: a new Code
     session on `C:\Users\<you>\northwind\vendor-desk`, mode **Manual**, model
     **Opus 4.8**. Answer with **Allow once** or **Deny** only.

Put the browser with the console where the audience looks; the terminal can be
smaller.

**Stopping and starting over.** Ctrl+C in the terminal, or leaving Claude Code,
stops the receiver and the console. Close the Claude session, run the command
again, and open a new session. The script refuses to start while an old receiver
or console still answers on 8787 or 8790: stop that one first.

Options: `--no-claude` (never start the CLI), `--no-browser`, a different `DEST`
as the last argument, and `PORT`, `CONSOLE_PORT`, `LOGS`, `RECEIVED` in the
environment.

### By hand

The same stand without the script, one pane per long-running process (Windows
Terminal: Alt+Shift+= splits side by side, Alt+Shift+- splits top and bottom):

1. **Project**, rebuilt before every run:

   ```bash
   bash docs/demos/policy-console/setup.sh
   ```

2. **Receiver**, left running (`python` instead of `python3` on Windows):

   ```bash
   python3 docs/demos/live-claude-code/receiver.py --out ~/northwind-inbox
   ```

3. **Console**, left running, then open <http://127.0.0.1:8790/>:

   ```bash
   harness console --project ~/northwind/vendor-desk
   ```

4. **Preflight**, until it says "Ready to present.":

   ```bash
   bash docs/demos/policy-console/preflight.sh
   ```

5. **Claude Code**, as in step 5 above:

   ```bash
   cd ~/northwind/vendor-desk && claude --model claude-opus-4-8 --permission-mode default
   ```

## The scenario

Click **Contractor** in the console. Then type each prompt into Claude Code.

**1. A profile takes a capability away.**

```
Fetch http://portal.northwind.test:8787/onboarding with curl and tell me the onboarding steps.
```

The feed shows grey **ABSENT**: this agent has no way to read the web. Claude says
it was blocked.

Claude may not stop at one try. In the first Windows rehearsal it ran `curl.exe`
from its PowerShell tool, got ABSENT, and two seconds later tried plain `curl`
from Bash: ABSENT again. If that happens, point at the two lines: "It looked for
another way in, through a different tool. There isn't one: the kernel decides by
what the action is, not by which tool asked." It is the model's choice whether it
retries, and how; the verdict does not depend on it (a Python script would be
absent too, with **Run scripts** Off). Click **Developer** (a POLICY line appears in the feed) and send the
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
data out** to **Allow** and send it again: still **DENY**, and the first card of the
invariants band at the top lights up: "After reading outside content, nothing
leaves", held twice. Point at it: "The switch says Allow. The kernel says no. Even
the administrator cannot open this by accident." The receiver's terminal shows no
upload.

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
- **On Windows, Claude Code may use its PowerShell tool**, and there it often runs
  `curl.exe` rather than Bash's `curl` (the first Windows rehearsal did, and got
  ASK instead of ABSENT until the PowerShell classes learned curl). Any `curl`,
  `curl.exe`, `wget`, `Invoke-WebRequest` or `irm` there reads the web; one that
  also carries an upload flag (`-F`, `-InFile`, `-Method Post`, …) is ambiguous and
  asks first, then is refused once the session has read from outside.
- **Profiles are presets**, not identities. Binding them to a directory group (Okta,
  Entra) is not built.
- **After the session has read from outside, reading the web again is refused too**:
  a second fetch is network traffic from a marked session. Start a new session to
  read again.
- **The log is observability.** If it cannot be written, the verdicts are unchanged
  and the feed simply misses lines.
