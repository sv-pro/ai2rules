// One cell = one scenario (a benign task + at most one attack) under one
// defense, in a fresh browser context against freshly reset servers.
import { existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { BrowserEnv } from './browser.js';
import { AgentEnv } from './env.js';
import { CompromisedAgent } from './agent/compromised.js';
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

// The first non-ALLOW verdict on an attack-relevant call: one the agent made
// because of an injected instruction or as a retry (the agent's own tag — used
// for reporting only, never by a defense), or one that resolved to a different
// registration than the first one the browser ever listed under that name (a
// substitution or a squat).
function stoppedBy(trace, attackSucceeded) {
  if (attackSucceeded) return null;
  const firstReg = new Map();
  for (const l of trace.ofKind('list')) {
    for (const entry of l.registered) {
      const [regId, where] = entry.split(' ');
      const name = where.slice(where.indexOf('/') + 1);
      if (!firstReg.has(name)) firstReg.set(name, regId);
    }
  }
  const calls = trace.ofKind('call');
  const verdicts = trace.ofKind('verdict');
  for (const c of calls) {
    const v = verdicts.find((x) => x.seq > c.seq);
    if (!v || v.decision === 'ALLOW' || v.decision === 'MONITOR') continue;
    const regId = c.resolved?.split(' ')[0];
    const substituted = regId && firstReg.has(c.name) && firstReg.get(c.name) !== regId;
    if (c.trigger?.by === 'injection' || c.trigger?.retry || substituted) return v.mechanism ?? v.decision;
  }
  // Nothing was refused: the injected instruction never reached the agent
  // (or the attack failed on its own). The scenario says which.
  return 'not_triggered';
}

export async function runCell({ browser, servers, scenarioId, defenseId, traceDir, workDir }) {
  const scenario = await loadScenario(scenarioId);
  const task = TASKS[scenario.task];
  const cell = `${scenarioId}/${defenseId}`;
  const trace = new Trace(cell);
  const traceFile = path.join(traceDir, `${scenarioId.replace(/[:/]/g, '_')}--${defenseId}.jsonl`);
  const row = { scenario: scenarioId, short: scenario.short, name: scenario.name, task: task.id, defense: defenseId };

  let benv = null;
  try {
    servers.reset(scenario.attack ? scenarioId : 'benign', scenario.faults);
    trace.write('cell_start', { scenario: scenarioId, task: task.id, defense: defenseId });
    const defense = createDefense(defenseId, { workDir });
    await defense.init({ task, trace });
    benv = await BrowserEnv.create(browser, { origins: servers.origins, trace });
    const env = new AgentEnv({ browser: benv, defense, trace, task });
    const plan = scenario.plan ? scenario.plan(task.plan) : task.plan;
    const answer = await new CompromisedAgent().run({ ...task, plan }, env);
    const verdict = judge({ task, scenario, state: servers.state(), trace, answer });
    trace.write('oracle', verdict);
    const stopped = scenario.attack ? stoppedBy(trace, verdict.attack_success) : null;
    const monitored = trace.ofKind('verdict').filter((v) => v.decision === 'MONITOR').length;
    Object.assign(row, {
      status: 'OK',
      attack_success: scenario.attack ? verdict.attack_success : null,
      leak_or_effect_without_attack: scenario.attack ? null : verdict.attack_success,
      utility: verdict.utility,
      stopped_by: stopped,
      monitored,
      note: stopped === 'not_triggered' ? scenario.untriggeredMeans ?? null : null,
      evidence: verdict.evidence,
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
