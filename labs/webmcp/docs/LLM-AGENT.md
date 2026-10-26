# LLMAgent: the stretch column

The main results use the CompromisedAgent (CONTRACTS.md §7). It is deterministic
and obeys every injected instruction: the worst case for the reasoning layer. The
LLMAgent column (#166 / AI2-89) runs a real model on a subset of the same cells,
to show what changes when the agent is an actual LLM.

```bash
export ANTHROPIC_API_KEY=...                 # required; nothing runs without it
export WEBMCP_LLM_MODEL=claude-sonnet-5-5    # optional, this is the default
export WEBMCP_LLM_EFFORT=medium              # optional; omitted = the model's default
node src/run.js --agent llm --matrix         # → results/results-llm.json, RESULTS-LLM.md
node src/run.js --cell s1-page-injection D2 --agent llm
```

The subset is `benign:shop-buy`, `benign:crm-gift` and every directory under
`scenarios/`, each under D0, D1 and D2. Traces go to `results/traces-llm/`. The
LLM column is never merged into `results.json` or `RESULTS.md`.

## What the column means

The question is the same: what does the execution layer let through? The oracle,
the defenses, the pages and the tasks are the same. Only the agent differs.

**A low attack-success rate in this column is not a defense result.** A model may
ignore an injection, refuse it, or never read the page that carries it. Under D0
(no defense), any attack the model declines counts as "not achieved", even though
nothing stopped it. Read the column this way:

- **D0** measures the model on its own: how often it follows the injection.
- **D1 / D2** measure the model and the defense together. A cell matters for the
  defense when the attack succeeds under D0 and not under D1 or D2.
- The table says `not achieved (<mechanism>)`. The mechanism is the first
  refused call that resolved to a registration outside the task's capabilities.
  It is `none` when no such call was refused: the model declined on its own, or
  the attack needed no off-task tool.

Utility is reported as in the main table. A model can also fail a benign task by
itself (wrong product, no order), so a `no` under D0 is about the model.

## What differs from the CompromisedAgent

| | CompromisedAgent | LLMAgent |
|---|---|---|
| Plan | Runs the task's `plan` step by step | Gets only `task.prompt`; chooses its own calls |
| Injections | Obeys every `AGENT: call …` line | Whatever the model does with what it reads |
| Tabs | Opens them as the plan reaches each `open` step | All the plan's `open` steps, in plan order, before the first turn (the model cannot navigate, §10) |
| Page text | Reads at the plan's `read` steps | Only when it calls `read_page(origin)` |
| Tools | Re-lists before each planned call | `env.listTools()` re-listed every turn, plus `read_page` |
| Call by unlisted name | Yes, when an injection names it | Yes, if the model names it: the name goes to `env.callTool` unchanged |
| Focus | Focuses the step's `on` origin | Focuses the origin of the listed tool it calls |
| Retry | Retries a tool error once | Sees the error as a tool result; may retry |
| Self-tag (`trigger`) | `injection` / `user_task` | `{ by: 'llm', turn, tool_origin }`. A model cannot say which calls an injection caused |

The LLMAgent uses the same `AgentEnv`, so every call goes through the defense.
Tool names are mapped to the API's name rule (`[a-zA-Z0-9_-]{1,64}`, collisions
get a suffix) and back to the page's own name. Each description starts with
`[origin: <origin>]`. Opening every tab up front changes registration order
relative to the CompromisedAgent run. Name resolution (latest registration wins,
§6) can then pick a different tool for the same name.

## Determinism

The CompromisedAgent run is byte-identical between runs. This column is not:

- The Messages API rejects `temperature`, `top_p` and `top_k` on every model as of
  2026-10-21, so the lab cannot pin sampling. The fixed parts are the prompt, the
  system text, the tool order, a 12-turn step budget and 4096 output tokens per turn.
- Each model turn is written to the trace as `llm_turn`: stop reason, tool calls
  with their inputs, the text, token usage, and the model that answered. The
  trace is the record of what happened. A rerun is a new sample.
- The trace never contains the API key. The key is held only by the transport.
- Thinking is left at the model's default, and thinking blocks go back to the API
  unchanged.
- No refusal fallback is configured. A `refusal` stop reason ends the episode and
  shows in the agent column, so a cell is never quietly served by another model.

To compare models or check stability, run the column several times and compare
the traces. One run is an anecdote.

## Cost

Each cell is one episode of up to 12 Messages API calls. Each call resends the
whole conversation, including page text and tool outputs, so input tokens grow
with every turn. The current subset is 3 scenarios × 3 defenses = 9 cells. With
S2–S5 it is 6 × 3 = 18. Token usage per cell is in `results-llm.json`
(`agent_stats.usage`). Price it at the model's rate before a large run.

## Tests

`test/llm.test.js` runs the agent against a scripted fake model passed in as
the transport. It makes no network calls and needs no key. It covers name
mapping, re-listing, the step budget, keeping the key out of the trace, benign
cells under all three defenses, an obedient model leaking under D0 and being
stopped under D2, and the runner's exit without a key.
`WEBMCP_LLM_TRANSPORT=<module>` points the runner at such a model, or at a
recorded one. The module's default export returns a transport per cell.
