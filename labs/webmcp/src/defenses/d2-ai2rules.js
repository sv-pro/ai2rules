// D2 — ai2rules. The real kernel, reached through its wire ABIs:
// `harness project` decides what the agent can see (D72), `harness gate`
// decides each call (D24). This adapter adds only what the kernel has no
// concept of for WebMCP: mapping a registration to an action name, and pinning
// a registration's identity. Everything here is written from the task alone —
// the task's allowed origins and the capabilities its workflow needs — never
// from an attack (CONTRACTS.md §8).
//
// Mechanisms reported:
//   projection — the action is not in the compiled world (kernel: ABSENT)
//   pinning    — a manifest action whose live registration is not the one first
//                seen for it (adapter: ABSENT, CONTRACTS.md §8.2)
//   taint      — the kernel's taint floor / capability matrix after the session
//                ingested a taint source (kernel: DENY or ABSENT, verbatim)
//   flow:*     — D3 only: the kernel's flow-contract rules (D95), verbatim
//                (`confidentiality_violation` → `flow:confidentiality`, …)
import { execFile } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..', '..', '..');

export function harnessBin() {
  if (process.env.HARNESS_BIN) return process.env.HARNESS_BIN;
  const exe = process.platform === 'win32' ? 'harness.exe' : 'harness';
  for (const profile of ['release', 'debug']) {
    const p = path.join(REPO_ROOT, 'target', profile, exe);
    if (existsSync(p)) return p;
  }
  return 'harness';
}

export const actionName = (origin, tool) => `${origin}__${tool}`;

// Tool class (from the task) → kernel action type + side-effect class.
const CLASS_MAP = {
  read_only: { action_type: 'Read', side_effect: 'Read' },
  internal_write: { action_type: 'Write', side_effect: 'PersistentWrite' },
  external_side_effect: { action_type: 'Mcp', side_effect: 'External' },
};

// The compiled world for a task, as YAML. Only the task's capabilities exist;
// every other registration — any origin, any name — is ABSENT by omission.
//
// With `labels` (D3), the same actions also declare the kernel's flow contracts
// (D95, CONTRACTS.md §8.4): a read on a sensitive origin is a SOURCE whose whole
// result is Trusted/Private, and every argument of an action on any other
// origin is a SINK that admits only Public values. The task's `args` become the
// action's schema, because a sink must name an argument the schema declares.
// Without `labels` the world is byte-for-byte what D2 has always compiled.
export function worldYaml(task, { labels = false } = {}) {
  const sensitive = new Set(task.sensitiveOrigins ?? []);
  const lines = [
    `world_id: webmcp-${task.id}`,
    '',
    'channels:',
    '  - { name: user_prompt, trust: Trusted, taint: false }',
    '  - { name: mcp_output, trust: Untrusted, taint: true }',
    '',
    'capabilities:',
    '  - { trust: Trusted, actions: [Read, Write, Mcp] }',
    '  - { trust: Untrusted, actions: [Read] }',
    '',
    'base_actions:',
  ];
  for (const cap of task.capabilities) {
    const m = CLASS_MAP[cap.class];
    if (!m) throw new Error(`task ${task.id}: unknown tool class ${cap.class}`);
    lines.push(`  - name: ${actionName(cap.origin, cap.tool)}`);
    lines.push(`    action_type: ${m.action_type}`);
    lines.push(`    side_effect: ${m.side_effect}`);
    if (!labels) {
      // A sensitive origin's reads are declared as remote ingress, which is how
      // today's kernel marks an action's output as a taint source (gate.rs:
      // McpServer backing ⇒ post-call taint). See CONTRACTS.md §8.3.
      if (sensitive.has(cap.origin)) {
        lines.push(`    backing: !McpServer { server: ${cap.origin}, tool: ${cap.tool} }`);
      }
      continue;
    }
    const args = Object.entries(cap.args ?? {});
    if (args.length) {
      lines.push('    schema:');
      lines.push('      type: object');
      lines.push('      properties:');
      for (const [name, type] of args) lines.push(`        ${name}: { type: ${type} }`);
    }
    if (sensitive.has(cap.origin)) {
      // The source. Declared local, not as remote ingress: the CRM is one of the
      // task's own origins, and what its read returns is private, not untrusted.
      // The contract says so explicitly, where D2 had only the taint bit.
      if (m.action_type === 'Read') {
        lines.push('    flow_contract:');
        lines.push('      output: { integrity: Trusted, confidentiality: Private }');
      }
    } else if (args.length) {
      // A sink on every argument: a private value may not reach another origin
      // through any of them, whether the action reads or writes.
      lines.push('    flow_contract:');
      lines.push('      sinks:');
      for (const [name] of args) lines.push(`        ${name}: { confidentiality: Public }`);
    }
  }
  return lines.join('\n') + '\n';
}

function run(bin, args, input) {
  return new Promise((resolve, reject) => {
    const child = execFile(bin, args, { maxBuffer: 1 << 20 }, (err, stdout, stderr) => {
      if (err && err.code !== 0 && !stdout) return reject(new Error(`${args[0]} failed: ${stderr || err.message}`));
      try { resolve(JSON.parse(stdout)); } catch { reject(new Error(`${args[0]}: bad JSON: ${stdout} ${stderr}`)); }
    });
    child.stdin.end(JSON.stringify(input));
  });
}

// ---- D3: the host side of the kernel's flow labels (gate ABI §6) ----

const TRUSTED_PUBLIC = { integrity: 'trusted', confidentiality: 'public' };

// Labels only ever go up: joining is the only operation a host has.
const joinLabel = (a, b) => ({
  integrity: a.integrity === 'untrusted' || b.integrity === 'untrusted' ? 'untrusted' : 'trusted',
  confidentiality: a.confidentiality === 'private' || b.confidentiality === 'private' ? 'private' : 'public',
});
const showLabel = (l) => `${l.integrity}/${l.confidentiality}`;

// A value's identity, for binding a label to it: its text, or its JSON for
// anything that is not a string. A label follows the whole value, by digest,
// and nothing less: a quote of part of it, or a transform of it, is a value the
// host has never seen (CONTRACTS.md §8.4).
const identity = (v) => (typeof v === 'string' ? v : JSON.stringify(v));
const digest = (v) => createHash('sha256').update(identity(v)).digest('hex');

// Every primitive in a value tree.
function primitives(v, out = []) {
  if (Array.isArray(v)) v.forEach((x) => primitives(x, out));
  else if (v && typeof v === 'object') Object.values(v).forEach((x) => primitives(x, out));
  else if (v !== undefined && v !== null) out.push(v);
  return out;
}

// The kernel's flow rules, reported under the lab's mechanism names.
const FLOW_RULES = {
  confidentiality_violation: 'flow:confidentiality',
  integrity_violation: 'flow:integrity',
  missing_flow_context: 'flow:missing_context',
  invalid_flow_context: 'flow:invalid_context',
};

// D3 (`labels: true`) is D2 with the per-session taint from a sensitive-origin
// read replaced by the kernel's own flow contracts (D95; CONTRACTS.md §8.4).
// The adapter carries `context.flow` on every gate request — the session's
// ambient label and a label for each argument whose value it has a label for —
// and persists what the kernel returns: the ambient, and the floor it puts on
// each ALLOWed call's result, which the adapter binds to the value the tool
// actually returned. It decides nothing: every DENY here is the kernel's rule.
// Projection, pinning and the integrity taint from untrusted page text are
// unchanged.
export function createD2({ workDir, labels = false }) {
  const bin = harnessBin();
  let worldPath = null;
  let task = null;
  let taint = 'clean';
  const pins = new Map(); // action name -> { regId, descriptorHash }
  // D3 host state (gate ABI §6 step 2): the monotonic ambient label and the
  // digest-keyed table of value labels. Both are the host's word.
  let ambient = TRUSTED_PUBLIC;
  const valueLabels = new Map(); // sha256(identity(value)) -> label

  const channel = () => (taint === 'tainted' ? 'mcp_output' : 'user_prompt');

  function pinned(tool) {
    const pin = pins.get(actionName(tool.origin, tool.name));
    return pin && pin.regId === tool.regId && pin.descriptorHash === tool.descriptorHash;
  }

  // Bind a label to a value, joined with any label it already has: a later
  // claim can raise a label, never lower it.
  function bind(value, label) {
    const k = digest(value);
    valueLabels.set(k, valueLabels.has(k) ? joinLabel(valueLabels.get(k), label) : label);
  }

  // The sidecar for a call: the ambient, plus the label of every argument whose
  // value the host has seen labelled. An argument the host cannot vouch for is
  // left out and falls back to the ambient in the kernel — never to
  // Trusted/Public.
  //
  // One claim the host makes on its own word: a declared argument the call does
  // not bind at all carries no value, so nothing flows through it, and it is
  // labelled Trusted/Public. The kernel judges every declared sink, present or
  // not, by the ambient when it is unlabelled (`check_flow`), which would refuse
  // `submit_order {}` in a session that has read the CRM — for a value that does
  // not exist. The lab reports which arguments this applied to in the trace.
  function flowFor(action, args) {
    const labelled = {};
    const absent = [];
    for (const [name, value] of Object.entries(args ?? {})) {
      const label = valueLabels.get(digest(value));
      if (label) labelled[name] = label;
    }
    const declared = task.capabilities.find((c) => actionName(c.origin, c.tool) === action)?.args ?? {};
    for (const name of Object.keys(declared)) {
      if (!(name in (args ?? {}))) { labelled[name] = TRUSTED_PUBLIC; absent.push(name); }
    }
    return { ambient, args: labelled, absent };
  }

  return {
    id: labels ? 'D3' : 'D2',
    label: labels
      ? 'ai2rules + flow contracts (kernel flow labels, D95)'
      : 'ai2rules (compiled world + pinning + taint)',

    async init({ task: t, trace }) {
      task = t;
      mkdirSync(workDir, { recursive: true });
      worldPath = path.join(workDir, `world-${task.id}.yaml`);
      writeFileSync(worldPath, worldYaml(task, { labels }));
      taint = 'clean';
      pins.clear();
      ambient = TRUSTED_PUBLIC;
      valueLabels.clear();
      trace.write('defense_init', { defense: labels ? 'D3' : 'D2', world: path.basename(worldPath), capabilities: task.capabilities.length, labels });
      if (labels) {
        // The user's own input, labelled explicitly: every value the task's
        // workflow sends is Trusted/Public by the host's word. This is what lets
        // the benign write out of a session that has read something private.
        const inputs = task.plan.flatMap((step) => (step.call ? primitives(step.args ?? {}) : []));
        for (const v of inputs) bind(v, TRUSTED_PUBLIC);
        trace.write('flow_label', { source: 'user_task', label: showLabel(TRUSTED_PUBLIC), bound: inputs.length });
      }
    },

    // Page text from an origin the task did not name is untrusted ingress, the
    // WebMCP analogue of a web fetch: it taints the session. Text on the task's
    // own origins does not (CONTRACTS.md §8.3).
    async onIngest({ what, origin }, { trace }) {
      if (what === 'page_text' && !task.origins.includes(origin) && taint !== 'tainted') {
        taint = 'tainted';
        trace.write('taint', { state: 'tainted', cause: `page_text:${origin}` });
      }
    },

    async listTools(registered, { trace }) {
      // Trust on first use: the first live registration of a manifest action
      // that the agent is shown becomes its pinned identity.
      const req = {
        v: 1,
        context: { source_channel: channel() },
        tools: registered.map((t) => ({ name: actionName(t.origin, t.name), description: t.description, parameters: t.inputSchema })),
      };
      // Duplicate action names are a projection request error; offer the
      // earliest registration of each (later ones are not the pinned identity).
      const seen = new Set();
      req.tools = req.tools.filter((t) => (seen.has(t.name) ? false : seen.add(t.name)));
      const res = await run(bin, ['project', '--world', worldPath], req);
      const visibleNames = new Set(res.tools.map((t) => t.name));
      const out = [];
      for (const t of registered) {
        const a = actionName(t.origin, t.name);
        if (!visibleNames.has(a)) continue;
        if (!pins.has(a)) {
          pins.set(a, { regId: t.regId, descriptorHash: t.descriptorHash });
          trace.write('pin', { action: a, regId: t.regId, descriptorHash: t.descriptorHash });
        }
        if (pinned(t)) out.push(t);
      }
      return out;
    },

    async beforeCall({ tool, args }) {
      const a = actionName(tool.origin, tool.name);
      // A manifest action called before it was ever listed is pinned here, on
      // the same trust-on-first-use terms as the listing.
      if (!pins.has(a) && task?.capabilities.some((c) => actionName(c.origin, c.tool) === a)) {
        pins.set(a, { regId: tool.regId, descriptorHash: tool.descriptorHash });
      }
      if (pins.has(a) && !pinned(tool)) {
        return { decision: 'ABSENT', mechanism: 'pinning', reason: `${a}: live registration ${tool.regId} is not the pinned identity ${pins.get(a).regId}` };
      }
      const flow = labels ? flowFor(a, args) : undefined;
      const res = await run(bin, ['gate', '--world', worldPath], {
        v: 1,
        tool: a,
        arguments: args ?? {},
        context: { session_id: 'webmcp-lab', mode: 'background', taint, source_channel: channel(), ...(flow ? { flow: { ambient: flow.ambient, args: flow.args } } : {}) },
      });
      // Persist the kernel's post-call taint (monotonic) only for a call that
      // will run; a refused call ingests nothing.
      // The kernel's verdict is passed through as-is (ASK/REPLAN count as DENY:
      // there is no human in the loop). Only the mechanism label is ours.
      const decision = res.decision === 'ALLOW' ? 'ALLOW' : res.decision === 'ABSENT' ? 'ABSENT' : 'DENY';
      const mechanism = decision === 'ALLOW' ? null
        : ['unknown_to_ontology', 'absent'].includes(res.rule) ? 'projection'
          : FLOW_RULES[res.rule] ?? (taint === 'tainted' ? 'taint' : `kernel:${res.rule}`);
      if (decision === 'ALLOW') this._pendingTaint = res.context.taint;
      const verdict = { decision, mechanism, reason: res.reason ?? null, kernel: { decision: res.decision, rule: res.rule, manifest_hash: res.manifest_hash } };
      if (flow) {
        // ABI §6 step 5: the post-call ambient is persisted after every call,
        // joined, never lowered. The result floor is kept for `afterCall`,
        // which binds it to the value the tool actually returns.
        const out = res.context.flow ?? null;
        if (out?.ambient) ambient = joinLabel(ambient, out.ambient);
        this._pendingResult = decision === 'ALLOW' ? out?.result ?? null : null;
        verdict.flow = {
          ambient: showLabel(flow.ambient),
          args: Object.fromEntries(Object.entries(flow.args).map(([k, l]) => [k, showLabel(l)])),
          ...(flow.absent.length ? { absent: flow.absent } : {}),
          result: this._pendingResult ? showLabel(this._pendingResult) : null,
        };
      }
      return verdict;
    },

    // The kernel's post-call taint is persisted for every ALLOWed call, whether
    // or not the tool then succeeded: an error message is ingested text too.
    //
    // D3: the floor the kernel returned for this call's result is bound to the
    // value the tool returned (or to its error text: that is a returned value
    // too), as the whole value, by digest. A sink-only action returns no floor
    // and binds nothing.
    async afterCall(call, _res, { trace }) {
      if (labels && this._pendingResult) {
        const value = _res?.ok ? _res.result : _res?.error;
        if (value !== undefined) {
          bind(value, this._pendingResult);
          trace.write('flow_label', { source: `${call.tool.origin}/${call.tool.name}`, label: showLabel(this._pendingResult), bound: 1 });
        }
      }
      if (this._pendingTaint === 'tainted' && taint !== 'tainted') {
        taint = 'tainted';
        trace.write('taint', { state: 'tainted' });
      }
      this._pendingTaint = undefined;
      this._pendingResult = undefined;
    },
  };
}
