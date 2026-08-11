# ai2rules-harness

**Govern a local coding agent in one command.**

```bash
npm install -g ai2rules-harness
harness init
```

That writes a governance manifest, a `PreToolUse` shim, and the host settings entry
into the current project. It is safe to run twice, it never replaces a manifest you
have tuned, and turning it off is one file.

> **Install it globally, not into a project.** `harness init` **refuses** to run
> when the binary lives inside the project it would govern — a local
> `node_modules` install puts the kernel in the agent's own writable root, where
> replacing one file turns governance off silently. `npx ai2rules-harness init`
> is fine: its cache is outside your project.

## How it installs — and why there is no install script

The binary arrives as a **platform package** (`ai2rules-harness-linux-x64` and
friends), listed in `optionalDependencies`. npm resolves exactly one by `os`/`cpu`
and skips the rest.

That means installing this package runs **no install script, makes no network
request, executes no shell, and chmods nothing**. Earlier versions downloaded a
binary in a `postinstall` and verified a checksum fetched from the same host — which
proves the file arrived intact, not that it is the right file. Now the binary is
covered by the integrity hash npm writes into *your* lockfile, and the tarballs
carry [npm provenance](https://docs.npmjs.com/generating-provenance-statements)
signed against the workflow and commit that built them.

Practical upshots: installs are reproducible, work offline from a cache, and work
behind a proxy or a mirrored registry — none of which was true before.

If your platform has no prebuilt binary, nothing silently breaks: the CLI tells you
how to build one, or you point `AI2RULES_HARNESS` at an existing binary.

## What it does

A local coding agent inherits your whole shell: credentials, git remotes, package
managers, write access to everything. It also reads untrusted text on every turn —
web pages, issue bodies, tool results. Prompt injection is therefore a question
about *authority*, not about prompts.

This installs a deterministic kernel between the agent and the world. No model
takes part in the decision: a verdict is a pure function of the proposed call, the
session's context, and a compiled manifest.

## Prove it in five seconds

After `init`, ask the kernel directly — no agent session needed:

```bash
echo '{"tool_name":"Write","tool_input":{"file_path":"/etc/passwd"}}' \
  | CLAUDE_PROJECT_DIR=$PWD bash .claude/hooks/world-gate.sh
```

You get a deny with a reason. The identical write *inside* your project stays
silent. That is a path-scope policy, and it is the thing a list of allowed command
strings cannot express — it is about where the path points, not what the command
looks like.

## Modes

| Command | Behaviour |
|---|---|
| `harness init` | **Additive.** Adds deny/ask on top of the host's own permissions. Cannot lock you out. |
| `harness init --grant` | **Replace.** The manifest becomes the allowlist and grants directly, so the host stops asking. |
| `harness init --dry-run` | Print the plan, write nothing. |

## Turning it off

Effective on the very next tool call, no restart:

```bash
touch ~/.claude/ai2rules/off/<your-project>   # this project
touch ~/.claude/gate-off                      # everywhere
```

`harness init` prints the exact per-project path. Both live **outside** the
governed project on purpose: a switch inside it is one the agent can throw itself.
Delete the file to re-enable. The shim runs per call, so the toggle is immediate.

## Honest boundaries

- **Governed is not sandboxed.** This decides what an agent may *do*; it does not
  contain where it runs. Both matter and they are different properties.
- **Depth is asymmetric.** Deep on hosts that expose a pre-execution hook; at the
  MCP seam only on hosts that do not. What each host actually permits is measured
  in [the Agent Governability Index](https://github.com/sv-pro/ai2rules/blob/main/docs/GOVERNABILITY-INDEX.md),
  including the cells where the answer is unknown.
- **This installs a policy you should read.** `.claude/cc-world.yaml` is a starter,
  not a finished judgement about your project.

MIT OR Apache-2.0 · [source](https://github.com/sv-pro/ai2rules)
