# ai2rules-harness

**Govern a local coding agent in one command.**

```bash
npm install -g ai2rules-harness
harness init
```

That writes a governance manifest, a `PreToolUse` shim, and the host settings entry
into the current project. It is safe to run twice, it never replaces a manifest you
have tuned, and turning it off is one file.

> **Try it without installing:** `npx ai2rules-harness init` works and is the
> fastest way to see a verdict — but **prefer the global install for real use.**
> The shim records the absolute path of the binary that ran `init`, and under
> `npx` that path is inside npm's transient `_npx` cache. When the cache is
> cleaned or evicted the binary disappears, and the shim **fails open** — your
> tool calls go back to the host's normal permissions with no error. That is the
> correct behaviour for a missing kernel and the wrong place to be surprised by
> it. After a global install, re-run `harness init` to re-bake the stable path.

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
