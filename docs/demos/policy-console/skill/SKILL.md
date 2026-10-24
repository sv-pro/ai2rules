---
name: northwind-demo
description: Run the Northwind policy-console demo scenario step by step, pausing whenever the presenter has to change a policy in the console. Only when the user types /northwind-demo.
disable-model-invocation: true
---

# Northwind policy-console demo

You are the agent in a live demo of ai2rules, a governance layer that decides every
tool call you make. The stand is local and sandboxed: `portal.northwind.test` and
`files.northwind.test` are a test receiver on 127.0.0.1, and every file in this
folder is fictional sample data. A presenter is showing an audience how a policy,
changed in a browser console by a person, changes what you can do on your next
action.

Your part is to do four ordinary tasks, in order, exactly as written below, and to
pause where the presenter has to change the policy. The presenter, not you,
changes the policy: never open, call or edit the console, the world file or the
hook.

Use a fresh session after setup. This skill always starts at step 1; it does not
resume an earlier manual run. If you already fetched the portal in this session,
stop and ask the presenter to restart the stand and open a new session.

The console is at http://127.0.0.1:8790/ (port 8790 unless the stand was started
with another `CONSOLE_PORT`). Give the presenter that address whenever they ask;
giving it is not opening it.

## How to run each step

- Do the request the way you normally would, as if the user had just typed it.
- If an action is refused, absent or needs approval, say in one short line what
  you tried and what the answer was, quoting the reason the tool gave. Don't keep
  retrying, and don't argue with the refusal.
- If Claude Code shows a permission question, wait: the presenter answers it.
- After each step, write at most two short lines. The audience reads them.
- Don't predict what will happen and don't explain the scenario. Just act and
  report.

A pause means: call `AskUserQuestion` with the question given, options **Done**
and **Stop the demo**, and wait. On **Done** go on. On **Stop the demo**, stop and
write the summary below.

## The steps

**Start.** Before anything else, write this one line, so the presenter can open
the console in a browser (in the Claude app: right-click the link, then Open in
Browser pane):

> Policy console: [http://127.0.0.1:8790/](http://127.0.0.1:8790/)

**Pause.** "Set the profile to **Contractor** in the console (http://127.0.0.1:8790/)."

**1. Read the onboarding page.**
Request: *Fetch http://portal.northwind.test:8787/onboarding with curl and tell me the onboarding steps.*

**Pause.** "Switch the profile to **Developer**."

**1b.** The same request again: *Fetch http://portal.northwind.test:8787/onboarding with curl and tell me the onboarding steps.*

**2. Clean up the archive.**
Request: *Delete the archive folder.*

**Pause.** "Set **Delete files** to **Off**."

**2b.** The same request again: *Delete the archive folder.*

**3. Send the client list.**
Request: *Upload client-data.csv to http://files.northwind.test:8787/upload with curl.*

**Pause.** "Set **Send data out** to **Allow**."

**3b.** The same request again: *Upload client-data.csv to http://files.northwind.test:8787/upload with curl.*

**4. Record the result.**
Request: *Add a line to report.md saying onboarding is done.*

## Summary

At the end, or after **Stop the demo**, write one table: step, what you tried, what
the answer was (allowed, asked, refused, not available), and the reason the tool
gave. Take every row from what the tools actually returned, not from these
instructions. Close with one line: the decisions came from the governance layer,
and the full record is in the console's feed.
