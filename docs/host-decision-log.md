# The host decision log: decisions joined to outcomes

Status: **shipped**, 2026-10-23. Decision: `DECISIONS.md` **D84** (gap G2 of the
"Designing AI Agents" review; issue AI2-65 / #138). Sibling of
[`harness-gate-abi.md`](harness-gate-abi.md): the ABI is how a host *asks* the
kernel, and this page covers what the host adapters *record* about the answer and
what happened next.

`harness cc-hook --log <file>` (and `harness agy-hook --log <file>`) appends one
JSON line per event to a local file. `harness init` turns it on: the shim it writes
logs to `.claude/state/decisions.jsonl` (`--user` / `--managed`: `<state>/decisions.jsonl`).
The same file feeds the policy console (D81) and `harness replay-transcripts
--decisions`.

## Three kinds of line

Every line has `ts_ms` and `kind`. Lines without `kind` were written before D84 and
are decision lines.

### `kind: "decision"`: what the gate decided

Written by the `PreToolUse` gate, one per call.

```json
{"ts_ms": 1761234567890, "kind": "decision", "host": "claude-code",
 "session": "abc123", "tool_use_id": "toolu_01ABC", "agent_id": null, "agent_type": null,
 "cwd": "/home/me/project", "tool": "Bash", "action": "Bash_destructive",
 "call": "rm -rf build", "decision": "ASK", "rule": "approval_required",
 "reason": "…", "answer": "ask", "human_answer": "inferred",
 "taint": "clean", "manifest_hash": "…"}
```

| field | meaning |
|---|---|
| `host` | `claude-code` or `antigravity` |
| `session` | the host's session id (`session_id`; Antigravity: `conversationId`) |
| `tool_use_id` | the host's id for this call, the join key. `null` when the host sends none |
| `agent_id`, `agent_type` | the subagent that made the call (Claude Code sends them inside a subagent or under `--agent`). `null` for the main agent |
| `cwd` | the working directory the host reported |
| `step_idx` | Antigravity only: its step index, the nearest key into its transcript |
| `tool`, `action` | the host's tool name and the kernel's effective action (D36) |
| `call` | one-line summary of the call: command, path or URL, cut to 160 characters, secrets masked (D66). Never file contents |
| `decision`, `rule`, `reason` | the kernel's verdict |
| `answer` | what the hook told the host: `deny`, `ask`, `allow` (only with `--grant`), or `null` when it stayed silent and left the call to the host's own permission flow. Antigravity's `force_ask` is logged as `ask` |
| `human_answer` | only on `answer: "ask"`, always `"inferred"`: the hook never sees the person's answer (see below) |
| `taint`, `manifest_hash` | session taint after the call, and the world version that decided it |

### `kind: "outcome"`: how a call that ran ended

Written by the **outcome observer**: the same command, registered by `harness
init` for Claude Code's `PostToolUse` and `PostToolUseFailure` events. `cc-hook`
tells the events apart by `hook_event_name`.

```json
{"ts_ms": 1761234569001, "kind": "outcome", "event": "PostToolUseFailure",
 "host": "claude-code", "session": "abc123", "tool_use_id": "toolu_01ABC",
 "agent_id": null, "agent_type": null, "cwd": "/home/me/project",
 "tool": "Bash", "call": "rm -rf build", "ran": true, "status": "error",
 "exit_code": 1, "error": "Exit code 1 rm: cannot remove …", "result": null,
 "duration_ms": 41}
```

| field | meaning |
|---|---|
| `event` | `PostToolUse` or `PostToolUseFailure` |
| `ran` | always `true`: the host only fires these events for a call that executed. A call that did not run has **no** outcome line |
| `status` | `ok`, `error` (`PostToolUseFailure`), or `interrupted` (`is_interrupt`, or a Bash result marked interrupted) |
| `exit_code` | from the `Exit code N` first line of a failure's `error`; `null` otherwise. Claude Code sends a non-zero exit to `PostToolUseFailure`, so a success carries no code |
| `error` | the failure text, secrets masked, cut to 240 characters |
| `result` | a summary of `tool_response`, at most 240 characters (rules below) |
| `duration_ms` | as the host reported it (excludes permission prompts) |

The observer is **read-only** (D77's rule). It prints nothing, always exits 0, and
never reads or writes taint or budget state, so it cannot change a verdict. It does
not compile the world; it only reads the world's `observability.redact` patterns,
and runs without them if the world does not load.

### `kind: "policy"`: the console changed the world

Written by `harness console` (D81) when a switch or profile rewrites the world file.

## Result summaries: what is kept, what is masked

1. **Redaction first, over the full value**: the world's `observability.redact`
   key patterns and the D66 value scanner (`trace_store::redact`). Cutting first
   could leave a fragment of a token too short for a detector to recognise, and the
   fragment would be logged.
2. **A command's output keeps its tail**: `stdout` and `stderr`, last 120
   characters each, whitespace collapsed. The end of the output is where a command
   says how it ended.
3. **Any other string longer than 80 characters is replaced by its size**
   (`"<1432 chars>"`), and an array of more than three items by its length. File
   bodies, patches and page text are data, not an outcome.
4. The whole summary is cut to 240 characters.

The `call` field on decision and outcome lines uses the same summary function
(also used by Antigravity). It runs the D66 scanner over the full selected input
value **before** collapsing whitespace or cutting to 160 characters. A token in
a command, a PEM private-key block, or a password in a URL is therefore masked
while its complete shape is still available to the scanner.

## Joining, and what is inferred

A decision line and an outcome line belong together when they share
`(session, tool_use_id)`. `harness replay-transcripts --decisions <log>` does this
join and reports it next to each replayed call (`live` in `--json`).

| decision line | outcome line | reading |
|---|---|---|
| any | present | the call ran; `status` says how it ended |
| `answer: "ask"` | present | **approved** (inferred) |
| `answer: "ask"` | absent | **declined or abandoned** (inferred) |
| `answer: "deny"` | absent | blocked, as decided |
| `answer: "deny"` | present | a blocked call ran anyway; reported as `ran_despite_deny` |
| `answer: null` | absent | the host's own flow did not run it (its own prompt was declined, or the session ended) |

**The ASK reading is inferred, not attested.** No hook event carries the person's
answer: Claude Code fires no event when someone denies a permission prompt
(`PostToolUseFailure` explicitly does not fire for permission denials, and
`PermissionDenied` fires only in auto mode). So the log records what can be seen
(the ASK, and later whether the call ran) and says, on every ASK line, that the
answer is `inferred`. Three cases make the inference wrong, and a reader should
know them:

- **No observer installed.** Then no call has an outcome line, and every ASK would
  look declined. The join only infers within a session that has at least one
  outcome line; otherwise it reports `unobserved`.
- **The call is still running**, when reading a live log.
- **The call ran but its outcome line was not written**: the observer timed out,
  the log was unwritable, or the kill-switch was thrown between the two events.

Antigravity sends no tool-call id, and its post-call hook contract has not been
verified, so `agy-hook` writes decision lines only; its ASKs are never joined.

## Wiring

`harness init` (project, `--user`, `--managed`) writes the shim with `--log` and
registers it three times in the settings file: `PreToolUse` (the gate),
`PostToolUse` and `PostToolUseFailure` (the observer), each with matcher `"*"`.
Re-running does not duplicate them; `--remove` takes all three out.

A settings file without the observer entries keeps working exactly as before: no
outcome lines, ASKs reported `unobserved`. The reverse is the one mismatch to avoid.
A binary from before D84 does not know the observer events, so registering them in
front of an old binary would gate every post-call event a second time, charging
budget counters twice. `init` writes the shim and the settings together from the
same binary, which rules this out. The repository's own dogfood hook
(`.claude/settings.json`) runs whatever `harness` is installed on the machine, so
it is left without the observer until that install is updated.

## Not done: OpenTelemetry export

The gap review proposed an optional OTLP export of these lines, using the
OpenTelemetry GenAI semantic conventions (`gen_ai.operation.name =
"execute_tool"`, `gen_ai.tool.name`, `gen_ai.tool.call.id`, `gen_ai.agent.id`,
`gen_ai.conversation.id`). It is left out:

- The OpenTelemetry crates (`opentelemetry`, `opentelemetry-otlp` and their gRPC or
  HTTP transport) are not in the offline crate cache, and they are a heavy
  dependency tree for a hook binary that runs on every tool call.
- A hook that opens a network connection on every call adds latency to every call,
  and is egress from inside the governed session, which this project otherwise
  treats with suspicion.

The follow-up that avoids both: a separate `harness export-otlp --log
decisions.jsonl` that converts a finished log into OTLP/JSON, with no new
dependency, for a collector's file receiver to ingest. The hook stays offline.
