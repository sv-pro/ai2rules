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
import { execFile } from 'node:child_process';
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
export function worldYaml(task) {
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
    // A sensitive origin's reads are declared as remote ingress, which is how
    // today's kernel marks an action's output as a taint source (gate.rs:
    // McpServer backing ⇒ post-call taint). See CONTRACTS.md §8.3.
    if (sensitive.has(cap.origin)) {
      lines.push(`    backing: !McpServer { server: ${cap.origin}, tool: ${cap.tool} }`);
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

export function createD2({ workDir }) {
  const bin = harnessBin();
  let worldPath = null;
  let task = null;
  let taint = 'clean';
  const pins = new Map(); // action name -> { regId, descriptorHash }

  const channel = () => (taint === 'tainted' ? 'mcp_output' : 'user_prompt');

  function pinned(tool) {
    const pin = pins.get(actionName(tool.origin, tool.name));
    return pin && pin.regId === tool.regId && pin.descriptorHash === tool.descriptorHash;
  }

  return {
    id: 'D2',
    label: 'ai2rules (compiled world + pinning + taint)',

    async init({ task: t, trace }) {
      task = t;
      mkdirSync(workDir, { recursive: true });
      worldPath = path.join(workDir, `world-${task.id}.yaml`);
      writeFileSync(worldPath, worldYaml(task));
      taint = 'clean';
      pins.clear();
      trace.write('defense_init', { defense: 'D2', world: path.basename(worldPath), capabilities: task.capabilities.length });
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
      const res = await run(bin, ['gate', '--world', worldPath], {
        v: 1,
        tool: a,
        arguments: args ?? {},
        context: { session_id: 'webmcp-lab', mode: 'background', taint, source_channel: channel() },
      });
      // Persist the kernel's post-call taint (monotonic) only for a call that
      // will run; a refused call ingests nothing.
      // The kernel's verdict is passed through as-is (ASK/REPLAN count as DENY:
      // there is no human in the loop). Only the mechanism label is ours.
      const decision = res.decision === 'ALLOW' ? 'ALLOW' : res.decision === 'ABSENT' ? 'ABSENT' : 'DENY';
      const mechanism = decision === 'ALLOW' ? null
        : ['unknown_to_ontology', 'absent'].includes(res.rule) ? 'projection'
          : taint === 'tainted' ? 'taint' : `kernel:${res.rule}`;
      if (decision === 'ALLOW') this._pendingTaint = res.context.taint;
      return { decision, mechanism, reason: res.reason ?? null, kernel: { decision: res.decision, rule: res.rule, manifest_hash: res.manifest_hash } };
    },

    // The kernel's post-call taint is persisted for every ALLOWed call, whether
    // or not the tool then succeeded: an error message is ingested text too.
    async afterCall(_call, _res, { trace }) {
      if (this._pendingTaint === 'tainted' && taint !== 'tainted') {
        taint = 'tainted';
        trace.write('taint', { state: 'tainted' });
      }
      this._pendingTaint = undefined;
    },
  };
}
