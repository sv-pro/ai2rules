// The lab's D2/D3 adapter (src/defenses/d2-ai2rules.js), on the kernel in WASM
// instead of the `harness` binary (DESIGN.md §3–4). The decisions are the
// kernel's: `project` (D72) shapes what the agent sees, `gate` (D24) decides
// each call, and the flow-contract rules (D95) are the kernel's own. What this
// file adds is what the lab's adapter adds, line for line: an action name per
// registration, pinning a registration's identity, the session taint from
// untrusted page text, and the host side of the flow labels.
//
// Differences from the lab's adapter, all mechanical:
//   - the kernel is `kernel.gate(worldYaml, json)` / `kernel.project(…)`, not a
//     child process; the world is YAML compiled at build time from the same
//     `worldYaml(task)` (build.mjs), never written here;
//   - all state is one plain object (`freshState`), so the service worker can
//     persist it across restarts (chrome.storage.session) — MV3 stops idle
//     workers;
//   - digests are WebCrypto (async), so every hook is async;
//   - post-call state is keyed by a call id, not held on `this`: the gate and
//     the result arrive as two messages from the page.
// The build's test (test/extension.test.js) holds this file to the lab's
// verdicts on the same call sequence.

export const actionName = (origin, tool) => `${origin}__${tool}`;

const TRUSTED_PUBLIC = { integrity: 'trusted', confidentiality: 'public' };

const joinLabel = (a, b) => ({
  integrity: a.integrity === 'untrusted' || b.integrity === 'untrusted' ? 'untrusted' : 'trusted',
  confidentiality: a.confidentiality === 'private' || b.confidentiality === 'private' ? 'private' : 'public',
});
export const showLabel = (l) => `${l.integrity}/${l.confidentiality}`;

const identity = (v) => (typeof v === 'string' ? v : JSON.stringify(v));

async function sha256(text) {
  const buf = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(text));
  return [...new Uint8Array(buf)].map((b) => b.toString(16).padStart(2, '0')).join('');
}
const digest = (v) => sha256(identity(v));

function primitives(v, out = []) {
  if (Array.isArray(v)) v.forEach((x) => primitives(x, out));
  else if (v && typeof v === 'object') Object.values(v).forEach((x) => primitives(x, out));
  else if (v !== undefined && v !== null) out.push(v);
  return out;
}

export function canonicalJson(v) {
  if (Array.isArray(v)) return `[${v.map(canonicalJson).join(',')}]`;
  if (v && typeof v === 'object') {
    return `{${Object.keys(v).sort().map((k) => `${JSON.stringify(k)}:${canonicalJson(v[k])}`).join(',')}}`;
  }
  return JSON.stringify(v ?? null);
}
export const descriptorHash = ({ description, inputSchema, annotations }) => sha256(canonicalJson({ description, inputSchema, annotations }));

const FLOW_RULES = {
  confidentiality_violation: 'flow:confidentiality',
  integrity_violation: 'flow:integrity',
  missing_flow_context: 'flow:missing_context',
  invalid_flow_context: 'flow:invalid_context',
};

export const POLICIES = {
  D0: 'none',
  D2: 'ai2rules (compiled world + pinning + taint)',
  D3: 'ai2rules + flow contracts (kernel flow labels, D95)',
};

export function freshState(policy) {
  return { policy, taint: 'clean', pins: {}, ambient: TRUSTED_PUBLIC, valueLabels: {}, pending: {}, calls: 0 };
}

function call(kernel, fn, world, req) {
  const res = JSON.parse(kernel[fn](world, JSON.stringify(req)));
  if (res.ok === false) throw new Error(`kernel ${fn}: ${res.error}`);
  return res;
}

// `state` is mutated in place; the caller persists it after every hook.
export function createDefense({ task, worlds, kernel, state }) {
  const policy = state.policy;
  if (policy === 'D0') {
    return {
      id: 'D0',
      label: POLICIES.D0,
      async init({ trace }) { trace.write('defense_init', { defense: 'D0' }); },
      async onIngest() {},
      async listTools(registered) { return registered; },
      async beforeCall() { return { decision: 'ALLOW', mechanism: null, callId: ++state.calls }; },
      async afterCall() {},
    };
  }
  if (policy !== 'D2' && policy !== 'D3') throw new Error(`unknown policy ${policy}`);
  const labels = policy === 'D3';
  const world = worlds[policy];

  const channel = () => (state.taint === 'tainted' ? 'mcp_output' : 'user_prompt');
  const pinned = (tool) => {
    const pin = state.pins[actionName(tool.origin, tool.name)];
    return pin && pin.regId === tool.regId && pin.descriptorHash === tool.descriptorHash;
  };
  async function bind(value, label) {
    const k = await digest(value);
    state.valueLabels[k] = state.valueLabels[k] ? joinLabel(state.valueLabels[k], label) : label;
  }

  // The sidecar for a call: the ambient, plus the label of every argument whose
  // value the host has seen labelled; a declared argument the call does not
  // bind is Trusted/Public on the host's word (see the lab's adapter).
  async function flowFor(action, args) {
    const labelled = {};
    const absent = [];
    for (const [name, value] of Object.entries(args ?? {})) {
      const label = state.valueLabels[await digest(value)];
      if (label) labelled[name] = label;
    }
    const declared = task.capabilities.find((c) => actionName(c.origin, c.tool) === action)?.args ?? {};
    for (const name of Object.keys(declared)) {
      if (!(name in (args ?? {}))) { labelled[name] = TRUSTED_PUBLIC; absent.push(name); }
    }
    return { ambient: state.ambient, args: labelled, absent };
  }

  return {
    id: policy,
    label: POLICIES[policy],

    async init({ trace }) {
      trace.write('defense_init', { defense: policy, capabilities: task.capabilities.length, labels });
      if (labels) {
        const inputs = task.plan.flatMap((step) => (step.call ? primitives(step.args ?? {}) : []));
        for (const v of inputs) await bind(v, TRUSTED_PUBLIC);
        trace.write('flow_label', { source: 'user_task', label: showLabel(TRUSTED_PUBLIC), bound: inputs.length });
      }
    },

    async onIngest({ what, origin }, { trace }) {
      if (what === 'page_text' && !task.origins.includes(origin) && state.taint !== 'tainted') {
        state.taint = 'tainted';
        trace.write('taint', { state: 'tainted', cause: `page_text:${origin}` });
      }
    },

    async listTools(registered, { trace }) {
      const seen = new Set();
      const tools = registered
        .map((t) => ({ name: actionName(t.origin, t.name), description: t.description, parameters: t.inputSchema }))
        .filter((t) => (seen.has(t.name) ? false : seen.add(t.name)));
      const res = call(kernel, 'project', world, { v: 1, context: { source_channel: channel() }, tools });
      const visibleNames = new Set(res.tools.map((t) => t.name));
      const out = [];
      for (const t of registered) {
        const a = actionName(t.origin, t.name);
        if (!visibleNames.has(a)) continue;
        if (!state.pins[a]) {
          state.pins[a] = { regId: t.regId, descriptorHash: t.descriptorHash };
          trace.write('pin', { action: a, regId: t.regId });
        }
        if (pinned(t)) out.push(t);
      }
      return out;
    },

    async beforeCall({ tool, args }) {
      const a = actionName(tool.origin, tool.name);
      const callId = ++state.calls;
      if (!state.pins[a] && task.capabilities.some((c) => actionName(c.origin, c.tool) === a)) {
        state.pins[a] = { regId: tool.regId, descriptorHash: tool.descriptorHash };
      }
      if (state.pins[a] && !pinned(tool)) {
        return { decision: 'ABSENT', mechanism: 'pinning', reason: `${a}: live registration ${tool.regId} is not the pinned identity ${state.pins[a].regId}`, callId };
      }
      const flow = labels ? await flowFor(a, args) : undefined;
      const res = call(kernel, 'gate', world, {
        v: 1,
        tool: a,
        arguments: args ?? {},
        context: {
          session_id: 'webmcp-extension', mode: 'background', taint: state.taint, source_channel: channel(),
          ...(flow ? { flow: { ambient: flow.ambient, args: flow.args } } : {}),
        },
      });
      const decision = res.decision === 'ALLOW' ? 'ALLOW' : res.decision === 'ABSENT' ? 'ABSENT' : 'DENY';
      const mechanism = decision === 'ALLOW' ? null
        : ['unknown_to_ontology', 'absent'].includes(res.rule) ? 'projection'
          : FLOW_RULES[res.rule] ?? (state.taint === 'tainted' ? 'taint' : `kernel:${res.rule}`);
      const pending = { taint: decision === 'ALLOW' ? res.context.taint : undefined, result: null };
      const verdict = { decision, mechanism, reason: res.reason ?? null, callId, kernel: { decision: res.decision, rule: res.rule, manifest_hash: res.manifest_hash } };
      if (flow) {
        const out = res.context.flow ?? null;
        if (out?.ambient) state.ambient = joinLabel(state.ambient, out.ambient);
        pending.result = decision === 'ALLOW' ? out?.result ?? null : null;
        verdict.flow = {
          ambient: showLabel(flow.ambient),
          args: Object.fromEntries(Object.entries(flow.args).map(([k, l]) => [k, showLabel(l)])),
          ...(flow.absent.length ? { absent: flow.absent } : {}),
          result: pending.result ? showLabel(pending.result) : null,
        };
      }
      if (decision === 'ALLOW') state.pending[callId] = pending;
      return verdict;
    },

    async afterCall(callId, res, { trace, label }) {
      const pending = state.pending[callId];
      delete state.pending[callId];
      if (!pending) return;
      if (labels && pending.result) {
        const value = res?.ok ? res.result : res?.error;
        if (value !== undefined) {
          await bind(value, pending.result);
          trace.write('flow_label', { source: label, label: showLabel(pending.result), bound: 1 });
        }
      }
      if (pending.taint === 'tainted' && state.taint !== 'tainted') {
        state.taint = 'tainted';
        trace.write('taint', { state: 'tainted' });
      }
    },
  };
}
