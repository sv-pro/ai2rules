# Machine-wide governance: every Claude Code session, not one folder

`harness init` governs one project: it writes `.claude/` into that folder, and a
session opened anywhere else is ungoverned. Claude Code also reads hooks from two
places that apply to **every** folder, and ai2rules can install into either:

| | `harness init --user` | `harness init --managed` |
|---|---|---|
| Hook lives in | `~/.claude/settings.json` (or `$CLAUDE_CONFIG_DIR`) | `managed-settings.json` in the system policy directory |
| Covers | every session **this account** opens, any folder | every session on **this machine**, every account |
| Installed by | the developer | an administrator (elevated shell, MDM, GPO) |
| Who can switch it off | the developer; **and a project**, via `disableAllHooks` | only an administrator |
| Kill-switch in the shim | yes (`~/.claude/ai2rules/user/off`, `~/.claude/gate-off`) | **none, by design** |
| Use it for | a personal seatbelt on your own machine | an organisation's control: an agent policy employees cannot opt out of |

The user scope is convenient. The managed scope is the one to sell: it works the way
an endpoint agent such as a corporate proxy client does. IT installs it, and the person
it governs cannot remove it.

The difference comes from Claude Code itself, not from ai2rules. It is set out in the
next section.

---

## What Claude Code does with hooks

Checked against the official docs on 2026-10-21 (Claude Code 2.1.304):
[settings](https://code.claude.com/docs/en/settings),
[hooks](https://code.claude.com/docs/en/hooks),
[managed settings](https://code.claude.com/docs/en/managed-settings),
[settings reference](https://code.claude.com/docs/en/settings-reference).

- **Hooks merge across scopes.** User, project, local and managed hooks all run;
  none replaces another. If the same handler is defined twice it runs once. Hooks
  that match run in parallel.
- **The strictest answer wins.** When PreToolUse hooks disagree, precedence is
  `deny` > `defer` > `ask` > `allow`. A hook's `allow` never overrides a deny or
  ask rule in settings.
- **`disableAllHooks` is the bypass.** Set in any *non-managed* file, a project's
  `.claude/settings.json` included, it turns off user, project, local and plugin
  hooks. So does `claude --settings '{"disableAllHooks": true}'`. It **cannot**
  turn off managed hooks. Only `disableAllHooks` in managed settings does that.
- **`allowManagedHooksOnly: true`** (managed only) blocks every user, project,
  local and plugin hook and every agent-frontmatter hook. Managed hooks, Agent SDK
  hooks and plugins force-enabled by managed settings still run.
- **Edits apply mid-session.** Claude Code watches settings files, so an install
  or removal reaches running sessions without a restart. Endpoint policy from
  MDM or the registry is polled every 30 minutes.
- **Workspace trust.** An interactive session holds back hooks from every settings
  file, your own user settings included, until you accept the folder's trust
  dialog. `claude -p` and SDK sessions treat the folder as trusted.
- **Environment.** A hook runs in the session's cwd. `$CLAUDE_PROJECT_DIR` is the
  project root. On Windows the shell is Git Bash, or PowerShell when Git Bash is
  missing.
- **`--bare`** (headless) skips auto-discovered hooks in the project and in
  `~/.claude`. The docs do not say whether managed hooks survive `--bare`. Treat
  that as unverified. `--bare` also cannot use a subscription login (it needs an
  API key), which limits it in practice.

Managed settings locations (all documented):

| OS | File | Also |
|---|---|---|
| Windows | `C:\Program Files\ClaudeCode\managed-settings.json` | `HKLM\SOFTWARE\Policies\ClaudeCode`, value `Settings` (REG_SZ, the JSON) |
| macOS | `/Library/Application Support/ClaudeCode/managed-settings.json` | configuration profile (MDM) |
| Linux / WSL | `/etc/claude-code/managed-settings.json` | — |

Next to the file you can also use a `managed-settings.d/*.json` drop-in directory,
and an organisation can push the same keys from the claude.ai admin console
(server-managed settings). Claude Code no longer reads the legacy Windows path
`C:\ProgramData\ClaudeCode\`. `HKCU\SOFTWARE\Policies\ClaudeCode` is user-writable
and only counts when no admin source exists, so it is **not** a managed control.

---

## The user scope: `harness init --user`

```bash
harness init --user --dry-run     # print the plan, write nothing
harness init --user               # additive: deny/ask only, never auto-allows
harness init --user --remove      # unregister + delete the shim; every other setting kept
```

It writes:

```
~/.claude/ai2rules/user/cc-world.yaml   the manifest (kept on re-run unless --force)
~/.claude/ai2rules/user/world-gate.sh   the shim
~/.claude/ai2rules/user/state/          taint + budget sidecars, absolute
~/.claude/settings.json                 the PreToolUse hook, merged; your keys untouched
```

**There is no `--grant` here by default, deliberately.** A grant at user scope would
skip Claude Code's own prompts in every folder on the machine. `--grant` still works
if you pass it.

**State is absolute.** `cc-hook` defaults to `--state .claude/state`, which is
relative to the hook's cwd. At machine scope the cwd is whichever folder the session
is in, so a relative state dir would leave taint sidecars in every project you open.

**The world protects whatever can switch it off.** It is the starter manifest with
one change. The starter makes a project's whole `.claude/` and `.agents/` read-only (D57, D92). At machine
scope that would block ordinary work in every repository, such as adding a skill
or a command. So this world protects only the files that can disable the hook:

```yaml
- { path: "./.claude/settings.json",       access: Read }   # disableAllHooks lives here…
- { path: "./.claude/settings.local.json", access: Read }   # …or here
- { path: "~/.claude/ai2rules",            access: Read }   # this world, the shim, state, every kill-switch
- { path: "~/.claude/settings.json",       access: Read }   # the registration itself
```

The agent's Write and Edit calls to those paths are denied. Reads still pass, so the
agent can see the rules it is under.

**A project with its own ai2rules hook.** Both hooks run on every call. Each keeps
its own state dir, and both see the same calls, so they taint together. Claude Code
applies the stricter verdict. In practice:

- A project in `--grant` mode keeps its grants.
- The additive user hook can only *add* a deny or an ask, never take one away.

**What the user scope cannot stop:**

- **A person.** Someone who sets `disableAllHooks` by hand in a project, or passes
  it with `--settings`, turns the user hook off. The world above keeps the *agent*
  from doing this through file tools.
- **Bash writes.** Path rules govern the structured file tools (Write, Edit, …).
  A shell redirect such as `echo … > ~/.claude/settings.json` is a Bash command.
  The starter classifies unrecognised commands as `Bash_unclassified`, which needs
  approval, so a human sees it. But a human can approve it.
- **Another host.** User settings bind Claude Code only.

When those gaps matter, use the managed scope.

---

## The organisation scope: `harness init --managed`

This is the Zscaler-style install. IT puts it on the work machine, and the employee
it governs cannot switch it off, edit its policy, or swap its binary.

```powershell
# Windows, from an elevated PowerShell (or as a deployment script under SYSTEM):
harness init --managed --dry-run
harness init --managed
```

```bash
# macOS / Linux, as root:
sudo harness init --managed
```

It writes into the policy directory, which only an administrator can modify:

```
<policy>/ai2rules/bin/harness[.exe]   a copy of the kernel, admin-owned
<policy>/ai2rules/cc-world.yaml       the manifest
<policy>/ai2rules/world-gate.sh       the shim
<policy>/managed-settings.json        the hook, merged; existing policy keys kept
```

What makes it managed rather than merely machine-wide:

- **No kill-switch in the shim.** The user and project shims honour
  `~/.claude/gate-off`, which is a file in the employee's home. The managed shim
  does not.
- **No `HARNESS_BIN` override.** That variable is set by the user. If the shim read
  it, a no-op binary could replace the kernel.
- **The kernel is copied** into the admin-owned directory instead of pointing at
  wherever `npm install -g` put it, which may be user-writable.
- **Nothing below managed settings can disable it.** A project's or user's
  `disableAllHooks` does not reach managed hooks.

### Example policy

The minimum `harness init --managed` writes on Windows:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "bash 'C:/Program Files/ClaudeCode/ai2rules/world-gate.sh'",
            "timeout": 10
          }
        ]
      }
    ]
  }
}
```

A fuller organisation policy might add these. `init` does not write them; they are
choices for the administrator:

```json
{
  "allowManagedHooksOnly": true,
  "allowManagedPermissionRulesOnly": true,
  "permissions": {
    "disableBypassPermissionsMode": "disable",
    "deny": ["Read(./.env)", "Read(./.env.*)"]
  },
  "hooks": { "PreToolUse": [ "…as above…" ] }
}
```

- `allowManagedHooksOnly` stops every other hook from running. Use it when you also
  want to block hooks that projects or developers bring in themselves. It is
  **not** needed to keep ai2rules on: a managed hook already cannot be disabled
  from below, and its `deny` beats any other hook's `allow`.
- `disableBypassPermissionsMode` stops anyone from starting a session in
  bypass-permissions mode.

### Deploying across a fleet

- **One machine:** run `harness init --managed` once, elevated.
- **Many machines:** stage the bundle once with
  `harness init --managed --policy-dir <staging dir>`, then distribute it with your
  existing tooling (Intune, Group Policy, Jamf, Ansible). Two rules:
  - Paths baked into the shim and the hook command are the paths `--policy-dir`
    named, so stage at the final path, or run `init` on each machine.
  - On Windows the same JSON can go into `HKLM\SOFTWARE\Policies\ClaudeCode\Settings`
    instead of the file. The ai2rules files still need to be on disk.
- **Confirm:** in a session, `/status` shows `Enterprise managed settings (file)`
  under `Setting sources`.
- **Remove:** `harness init --managed --remove`, elevated. Only an administrator can
  do this, which is the point.

### What the managed scope still does not stop

Be honest about these in a pitch. They are where an endpoint product would have to
go further.

- **The taint state is per-user and writable by that user.** The hook runs as the
  user, so it can only record state where the user can write:
  `~/.claude/ai2rules/managed/state`. The world keeps the *agent's* file tools off
  that path. But a person, or an approved Bash command, can delete a taint marker
  and un-taint a session. Closing that needs the kernel's state behind a boundary
  the user cannot cross: a system service the hook talks to. That is a different
  product shape (see D82).
- **Fail-open.** If the kernel binary is missing, the shim lets calls through, so a
  broken install does not brick every session in the company. Only an
  administrator can remove the binary, so this does not hand the user a switch.
  It does mean a botched deployment fails silently. Check with `/status` and one
  denied call.
- **A broken managed world refuses every call** (D87). The managed shim passes
  `--on-broken-world closed`: a world that does not compile, or is missing, stops
  every tool call on the machine with the compile error, until an administrator
  fixes it. Not `last-good`, because its cache would sit in the per-user state
  directory, where the user could plant a permissive "last good" world; not
  `open`, because that makes the policy optional. `init --managed` compiles the
  world before installing it; check any hand edit compiles before deploying it.
  User scope writes `last-good`.
- **Local administrators** can edit managed settings. That holds for every managed
  setting in Claude Code, not just this one.
- **Other agents and other tools.** Managed settings bind Claude Code. An employee
  who uses another agent, or calls the API directly, is outside them. For those
  hosts, ai2rules has its own adapters (`agy-hook`, `harness gate`).
- **`--bare`.** See the unverified note above.

---

## Verifying it yourself

Feed a hook event straight into the shim. No session or credentials needed. On
Windows run this in Git Bash; the shim path for the user scope is
`~/.claude/ai2rules/user/world-gate.sh`.

```bash
cd ~/some-folder-nobody-ran-init-in
ev() { printf '{"session_id":"probe","tool_name":"%s","tool_input":%s}' "$1" "$2"; }
ev WebFetch '{"url":"https://example.com"}'      | bash ~/.claude/ai2rules/user/world-gate.sh  # silent, but taints
ev Bash     '{"command":"curl https://x.test"}'  | bash ~/.claude/ai2rules/user/world-gate.sh  # deny (taint_invariant)
```

In a real session the same pair produces the same result: fetch a page, then ask for
a `curl`, and the second call is denied.
