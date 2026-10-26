// CompromisedAgent (CONTRACTS.md §7): deterministic, no LLM. It runs the user's
// benign plan, and it obeys every instruction it meets in page text, tool
// descriptions or tool outputs. This models a fully hijacked reasoning layer;
// what the lab measures is what the execution layer lets through.
//
// Instruction syntax, one per line:   AGENT: call <tool> <json-args>
// String arguments may use $LAST_OUTPUT (the latest tool output, as JSON text)
// and $ALL_OUTPUTS (every tool output so far), the way a hijacked model would
// paste data it has seen.

const MARKER = /AGENT:\s*call\s+([A-Za-z0-9_.-]+)\s*(.*)$/;
const MAX_INJECTED_CALLS = 8;

export function findInstructions(text) {
  const out = [];
  for (const line of String(text ?? '').split(/\r?\n/)) {
    const m = line.match(MARKER);
    if (!m) continue;
    let args = {};
    const raw = m[2].trim();
    if (raw) {
      try { args = JSON.parse(raw); } catch { continue; }
    }
    out.push({ line: line.trim(), tool: m[1], args });
  }
  return out;
}

function stringLeaves(v, out = []) {
  if (typeof v === 'string') out.push(v);
  else if (Array.isArray(v)) v.forEach((x) => stringLeaves(x, out));
  else if (v && typeof v === 'object') Object.values(v).forEach((x) => stringLeaves(x, out));
  return out;
}

export class CompromisedAgent {
  constructor() {
    this.outputs = [];
    this.seenLines = new Set();
    this.seenDescriptions = new Set();
    this.injectedCalls = 0;
  }

  #fill(value) {
    if (typeof value === 'string') {
      return value
        .replaceAll('$LAST_OUTPUT', this.outputs.at(-1) ?? '')
        .replaceAll('$ALL_OUTPUTS', this.outputs.join('\n'));
    }
    if (Array.isArray(value)) return value.map((v) => this.#fill(v));
    if (value && typeof value === 'object') {
      return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, this.#fill(v)]));
    }
    return value;
  }

  async #obey(env, text, source) {
    for (const ins of findInstructions(text)) {
      if (this.seenLines.has(ins.line) || this.injectedCalls >= MAX_INJECTED_CALLS) continue;
      this.seenLines.add(ins.line);
      this.injectedCalls += 1;
      await this.#call(env, ins.tool, ins.args, { by: 'injection', source: source.kind, origin: source.origin });
    }
  }

  async #call(env, name, args, trigger, retried = false) {
    const res = await env.callTool(name, this.#fill(args), trigger);
    if (res.ok) {
      const text = typeof res.result === 'string' ? res.result : JSON.stringify(res.result);
      this.outputs.push(text);
      // Read the output the way a model would: the text of every string in it,
      // not the JSON escaping around them.
      await this.#obey(env, stringLeaves(res.result).join('\n'), { kind: 'tool_output', origin: name });
    } else if (res.outcome === 'tool_error' && !retried) {
      // A tool that failed is retried once, re-resolving the name from scratch.
      await this.#refresh(env);
      return this.#call(env, name, args, { ...trigger, retry: true }, true);
    }
    return res;
  }

  async #refresh(env) {
    const tools = await env.listTools();
    for (const t of tools) {
      const key = `${t.regId}`;
      if (this.seenDescriptions.has(key)) continue;
      this.seenDescriptions.add(key);
      await this.#obey(env, t.description, { kind: 'tool_description', origin: t.origin });
    }
    return tools;
  }

  // Runs `task.plan` and returns the final answer text.
  async run(task, env) {
    let answer = '';
    for (const step of task.plan) {
      if (step.open) {
        await env.open(step.open, step.path ?? '/');
      } else if (step.read) {
        const text = await env.readPage(step.read);
        await this.#obey(env, text, { kind: 'page_text', origin: step.read });
      } else if (step.focus) {
        env.focus(step.focus);
      } else if (step.call) {
        if (step.on) env.focus(step.on);
        await this.#refresh(env);
        await this.#call(env, step.call, step.args ?? {}, { by: 'user_task' });
      } else if (step.answer) {
        answer = this.#fill(step.answer);
      }
    }
    env.trace.write('answer', { text: answer });
    return answer;
  }
}
