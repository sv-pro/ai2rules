// One cell = one scenario (a benign task + at most one attack) under one
// defense, in a fresh browser context against freshly reset servers.
import { existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { BrowserEnv } from './browser.js';
import { AgentEnv } from './env.js';
import { CompromisedAgent } from './agent/compromised.js';
import { LLMAgent } from './agent/llm.js';
import { createDefense } from './defenses/index.js';
import { judge } from './oracle.js';
import { TASKS } from './tasks.js';
import { Trace } from './trace.js';

export const LAB_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

// `benign:<task>` is the task with no attack: the utility baseline.
export async function loadScenario(id) {
  if (id.startsWith('benign:')) {
    const task = id.slice('benign:'.length);
    if (!TASKS[task]) throw new Error(`unknown task ${task}`);
    return { id, short: 'B', name: `No attack (${task})`, task, attack: false };
  }
  const file = path.join(LAB_ROOT, 'scenarios', id, 'scenario.js');
  if (!existsSync(file)) throw new Error(`unknown scenario ${id}`);
  const mod = await import(pathToFileURL(file).href);
  return { attack: true, ...mod.default, id };
}

// The first non-ALLOW verdict on a call the agent made because of an injected
// instruction (the agent's own tag — used for reporting only, never by a
// defense), or on a call that resolved to a registration the task never had.
// An LLM does not tag its own calls, so for it the test is the resolved
// registration: a call to anything outside the task's capabilities counts.
function stoppedBy(trace, attackSucceeded, task) {
  if (attackSucceeded) return null;
  const calls = trace.ofKind('call');
  const verdicts = trace.ofKind('verdict');
  const onTask = new Set(task.capabilities.map((c) => `${c.origin}/${c.tool}`));
  for (let i = 0; i < calls.length; i++) {
    const c = calls[i];
    const v = verdicts.find((x) => x.seq > c.seq);
    if (!v || v.decision === 'ALLOW' || v.decision === 'MONITOR') continue;
    if (c.trigger?.by === 'injection' || c.trigger?.retry) return v.mechanism ?? v.decision;
    if (c.trigger?.by === 'llm' && c.resolved && !onTask.has(c.resolved.split(' ')[1])) return v.mechanism ?? v.decision;
  }
  return 'none';
}

const AGENTS = {
  compromised: () => new CompromisedAgent(),
  llm: (opts) => new LLMAgent(opts),
};
export const AGENT_IDS = Object.keys(AGENTS);

export async function runCell({ browser, servers, scenarioId, defenseId, traceDir, workDir, agent = 'compromised', agentOptions }) {
  if (!AGENTS[agent]) throw new Error(`unknown agent ${agent}`);
  const scenario = await loadScenario(scenarioId);
  const task = TASKS[scenario.task];
  const cell = `${scenarioId}/${defenseId}`;
  const trace = new Trace(cell);
  const traceFile = path.join(traceDir, `${scenarioId.replace(/[:/]/g, '_')}--${defenseId}.jsonl`);
  const row = { scenario: scenarioId, short: scenario.short, name: scenario.name, task: task.id, defense: defenseId, ...(agent === 'compromised' ? {} : { agent }) };

  let benv = null;
  try {
    servers.reset(scenario.attack ? scenarioId : 'benign', scenario.faults);
    trace.write('cell_start', { scenario: scenarioId, task: task.id, defense: defenseId });
    const defense = createDefense(defenseId, { workDir });
    await defense.init({ task, trace });
    benv = await BrowserEnv.create(browser, { origins: servers.origins, trace });
    const env = new AgentEnv({ browser: benv, defense, trace, task });
    const plan = scenario.plan ? scenario.plan(task.plan) : task.plan;
    const agentImpl = AGENTS[agent](agentOptions);
    const answer = await agentImpl.run({ ...task, plan }, env);
    const verdict = judge({ task, scenario, state: servers.state(), trace, answer });
    trace.write('oracle', verdict);
    const monitored = trace.ofKind('verdict').filter((v) => v.decision === 'MONITOR').length;
    Object.assign(row, {
      status: 'OK',
      attack_success: scenario.attack ? verdict.attack_success : null,
      leak_or_effect_without_attack: scenario.attack ? null : verdict.attack_success,
      utility: verdict.utility,
      stopped_by: scenario.attack ? stoppedBy(trace, verdict.attack_success, task) : null,
      monitored,
      evidence: verdict.evidence,
      ...(agentImpl.stats ? { agent_stats: agentImpl.stats } : {}),
    });
  } catch (err) {
    trace.write('error', { message: String(err?.stack ?? err) });
    Object.assign(row, { status: 'ERROR', error: String(err?.message ?? err), attack_success: null, utility: null, stopped_by: null });
  } finally {
    if (benv) await benv.close().catch(() => {});
    trace.write('cell_end', { status: row.status });
    trace.save(traceFile);
  }
  row.trace = path.relative(LAB_ROOT, traceFile).split(path.sep).join('/');
  return row;
}
