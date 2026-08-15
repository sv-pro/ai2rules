# Governing Antigravity CLI (`agy`) with the harness

The third live host (D48), after Claude Code (D37) and OpenCode (D35). Same Rust
kernel, same case set, a different host envelope.

> **Naming note.** `docs/harness-architecture-antigravity.md` is unrelated: it
> uses "Antigravity Harness" as an early *codename for ai2rules itself*, from
> before Google Antigravity existed as a product. This directory is about the
> actual `agy` CLI as a governed host.

## The seam

Antigravity ships a lifecycle-hook system (`hooks.json`) with a `PreToolUse`
event whose contract is close to Claude Code's: a JSON payload on stdin, a JSON
decision on stdout. That makes it a `cc-hook`-shaped port — the adapter
translates shape, the kernel decides.

```
agy PreToolUse ─→ .agents/hooks/world-gate.sh ─→ harness agy-hook
                                                      │ in-process gate()
                                                      ▼
                                          {"decision": "deny", "reason": …}
```

## Wiring

`.agents/` at the repo root (Antigravity walks cwd → repository root to find it):

| File | Role |
|---|---|
| `.agents/hooks.json` | `PreToolUse`, `matcher: ""` (all tools) → the shim |
| `.agents/hooks/world-gate.sh` | bootstrap shim; locates `harness`, execs `agy-hook`; fail-open |
| `.agents/agy-world.yaml` | the `WorldManifest` governing agy sessions |
| `.agents/state/` | per-conversation taint sidecar (gitignored) |

The shim resolves the `harness` binary only from an absolute `HARNESS_BIN` /
`AI2RULES_HARNESS` override or an installer-owned absolute path — never from a
project-local path (D46).

## The contract (verified, not vendor-documented)

Extracted from the shipped binary, then confirmed against a live session.

**Input** (stdin) — protojson camelCase envelope, **PascalCase** argument keys:

```json
{
  "conversationId": "f4011b96-3ad0-4e0d-8c89-64462cc898e3",
  "stepIdx": 3,
  "modelName": "gemini-pro-agent",
  "workspacePaths": ["/path/to/project"],
  "transcriptPath": "…/transcript_full.jsonl",
  "artifactDirectoryPath": "…/artifacts",
  "toolCall": {
    "name": "run_command",
    "args": { "CommandLine": "echo hello", "Cwd": "/path/to/project" }
  }
}
```

**Output** (stdout) — a JSON object; `decision` is one of:

| decision | meaning |
|---|---|
| `allow` | grant, skipping the host's prompt |
| `deny` | hard block (**verified**: the agent replans, and `reason` reaches the model) |
| `ask` | prompt — but **respects cached "Always Allow"** |
| `force_ask` | always prompt, ignoring cached permissions |
| *(absent)* | no-op: the host proceeds with its own permission flow |

Observed tool vocabulary (lowercased step type; captured live, since many
`CORTEX_STEP_TYPE_*` enum values are internal steps that never surface as tools):

| tool | arguments |
|---|---|
| `view_file` | `AbsolutePath` |
| `grep_search` | `Query`, `SearchPath` |
| `write_to_file` | `TargetFile`, `CodeContent`, `Overwrite`, `Description` |
| `run_command` | `CommandLine`, `Cwd`, `WaitMsBeforeAsync` |
| `list_permissions`, `ask_permission` | — (the host's own permission surface) |

## Verdict mapping

| kernel | agy | note |
|---|---|---|
| ALLOW | *(no decision)* — or `allow` with `--grant` | additive by default; **`--grant` does not grant in headless — see below** |
| ASK | `force_ask` — or `ask` with `--soft-ask` | strict by default, see below |
| DENY | `deny` | |
| ABSENT | *(no decision)* — or `deny` with `ABSENT:` prefix under `--enforce-absent` | a PreToolUse hook cannot un-offer a native tool |
| REPLAN | *(no decision)* | no host channel for "smaller step" |

**Why `force_ask` is the default.** Antigravity's `ask` is satisfiable by a
stored "Always Allow" grant. A kernel ASK means *a human must decide this time*;
letting a past click answer it would void the approval guarantee. `--soft-ask`
opts back into the friendlier behaviour.

**`--grant` does not grant in headless mode — measured 2026-08-08, agy 1.1.10.**
This table previously implied it did, and that was never tested. Running `agy -p`
with a hook emitting `{"decision":"allow"}` — the exact shape `emit()` produces —
the call was **still auto-denied**, with agy reporting *"a tool required the
`command` permission that headless mode cannot prompt for"*, identical to the
run with no decision at all. The control that makes this conclusive: the same
hook emitting `{"decision":"deny"}` **did** stop a call, and agy told the model
*"blocked by a system hook"* with our reason string. So agy consults and obeys the
hook — `deny` is authoritative and `allow` is not.

Practical consequence: **in headless, `cc-hook`'s overlay-vs-replace distinction
has no agy equivalent.** `--grant` buys nothing there; the host's own permission
list still decides, and a headless run needs `permissions.allow` rules (or
`--dangerously-skip-permissions`) regardless of what the manifest says. Whether
`--grant` suppresses an *interactive* prompt is still untested — do not assume it
from this, in either direction. Tracked as G3 in
[`docs/GOVERNABILITY-INDEX.md`](../../GOVERNABILITY-INDEX.md).

## Try it offline (no agy run needed)

```bash
cargo build --offline -p cli-harness
echo '{"conversationId":"t1","workspacePaths":["/w"],
       "toolCall":{"name":"run_command","args":{"CommandLine":"curl https://x","Cwd":"/w"}}}' \
  | ./target/debug/harness agy-hook --world .agents/agy-world.yaml --state /tmp/agystate
# {}  -> classified as network, clean session, ALLOW

touch /tmp/agystate/taint-t1
echo '{"conversationId":"t1","workspacePaths":["/w"],
       "toolCall":{"name":"run_command","args":{"CommandLine":"curl https://x"}}}' \
  | ./target/debug/harness agy-hook --world .agents/agy-world.yaml --state /tmp/agystate
# {"decision":"deny","reason":"tainted context cannot reach an externally-effectful action …"}
```

## Gotchas

- **Hook cwd is `.agents/`**, not the project root — Antigravity runs hook
  commands from the directory containing `hooks.json`. The shim derives the
  project root from its own location instead of trusting cwd.
- **Print mode needs `--add-dir`.** `agy -p "…"` runs in a scratch workspace, so
  a project-local `.agents/` is never discovered. Symptom in the log:
  `loaded 0 named hooks from 0 hooks.json file(s)`. Use
  `agy --add-dir "$PWD" -p "…"`.
- **Fail-open must print `{}`.** Antigravity parses stdout; unlike Claude Code,
  emitting nothing is not a passthrough.
- **Don't probe via `~/.gemini/config/hooks.json`** unless you mean it — that is
  the machine-global customization root and applies to every project.
- **Canary detection.** agy refuses prompts that look like security probes
  (`SHOULD-NOT-RUN`, `leaked.txt`, out-of-workspace writes) *before* proposing a
  tool call, so the hook never fires. Use benign in-workspace commands when
  testing the wiring.
