#!/usr/bin/env node
// Runner. `npm run matrix` runs every scenario × every defense, plus the
// no-attack baseline for each task, and writes results/results.json and
// results/RESULTS.md. `npm run cell -- <scenario> <defense>` runs one cell.
// `--agent llm` runs the LLMAgent stretch column instead (docs/LLM-AGENT.md):
// its own subset, traces and output files, never mixed into the main table.
import { mkdirSync, readdirSync, existsSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { pathToFileURL } from 'node:url';
import { launchBrowser } from './browser.js';
import { startServers } from './servers.js';
import { runCell, LAB_ROOT, AGENT_IDS } from './cell.js';
import { DEFENSE_IDS } from './defenses/index.js';
import { TASKS } from './tasks.js';
import { renderResults } from './report.js';
import { renderLlmResults, LLM_TASKS } from './report-llm.js';

function attackScenarios() {
  const dir = path.join(LAB_ROOT, 'scenarios');
  if (!existsSync(dir)) return [];
  return readdirSync(dir, { withFileTypes: true })
    .filter((d) => d.isDirectory() && existsSync(path.join(dir, d.name, 'scenario.js')))
    .map((d) => d.name)
    .sort();
}

// `--agent <id>` is taken out of argv; the rest parses as before.
function takeAgent(argv) {
  const i = argv.indexOf('--agent');
  if (i < 0) return { agent: 'compromised', argv };
  const agent = argv[i + 1];
  if (!AGENT_IDS.includes(agent)) throw new Error(`--agent must be one of ${AGENT_IDS.join('|')}`);
  return { agent, argv: [...argv.slice(0, i), ...argv.slice(i + 2)] };
}

// WEBMCP_LLM_TRANSPORT names a module whose default export makes a transport
// per cell (a recorded or scripted model); otherwise the real API is used.
async function llmOptions() {
  if (process.env.WEBMCP_LLM_TRANSPORT) {
    const mod = await import(pathToFileURL(path.resolve(process.env.WEBMCP_LLM_TRANSPORT)).href);
    return () => ({ transport: mod.default() });
  }
  if (!process.env.ANTHROPIC_API_KEY) return null;
  return () => ({});
}

async function main(rawArgv) {
  const { agent, argv } = takeAgent(rawArgv);
  let agentOptions = () => undefined;
  if (agent === 'llm') {
    agentOptions = await llmOptions();
    if (!agentOptions) {
      console.error('--agent llm needs ANTHROPIC_API_KEY (model: WEBMCP_LLM_MODEL, default claude-sonnet-5-5). Nothing was run.');
      return 2;
    }
  }
  const llm = agent === 'llm';
  const outDir = path.join(LAB_ROOT, 'results');
  const traceDir = path.join(outDir, llm ? 'traces-llm' : 'traces');
  const workDir = path.join(os.tmpdir(), `webmcp-lab-${process.pid}`);
  mkdirSync(traceDir, { recursive: true });

  let cells;
  if (argv[0] === '--cell') {
    const [scenario, defense] = argv.slice(1);
    if (!scenario || !defense) throw new Error('usage: --cell <scenario|benign:task> <D0|D1|D2>');
    cells = [[scenario, defense]];
  } else {
    const only = argv.includes('--scenarios') ? argv[argv.indexOf('--scenarios') + 1].split(',') : null;
    const tasks = llm ? LLM_TASKS : Object.keys(TASKS);
    const scenarios = [...tasks.map((t) => `benign:${t}`), ...attackScenarios()]
      .filter((s) => !only || only.includes(s));
    cells = scenarios.flatMap((s) => DEFENSE_IDS.map((d) => [s, d]));
  }

  const started = Date.now();
  const servers = await startServers();
  const browser = await launchBrowser();
  const rows = [];
  try {
    for (const [scenarioId, defenseId] of cells) {
      const row = await runCell({ browser, servers, scenarioId, defenseId, traceDir, workDir, agent, agentOptions: agentOptions() });
      rows.push(row);
      const mark = row.status !== 'OK' ? 'ERROR' : row.attack_success === null ? '—' : row.attack_success ? 'ATTACK SUCCEEDED' : llm ? 'not achieved' : 'attack stopped';
      console.log(`${scenarioId.padEnd(28)} ${defenseId}  ${mark.padEnd(16)} utility=${row.utility}  stopped_by=${row.stopped_by ?? '-'}${row.error ? `  ${row.error}` : ''}`);
    }
  } finally {
    await browser.close();
    await servers.close();
  }

  if (argv[0] !== '--cell' && llm) {
    writeFileSync(path.join(outDir, 'results-llm.json'), JSON.stringify({ lab: 'webmcp-attack-lab', agent: 'LLMAgent', rows }, null, 2) + '\n');
    writeFileSync(path.join(outDir, 'RESULTS-LLM.md'), renderLlmResults(rows));
  } else if (argv[0] !== '--cell') {
    writeFileSync(path.join(outDir, 'results.json'), JSON.stringify({ lab: 'webmcp-attack-lab', agent: 'CompromisedAgent', rows }, null, 2) + '\n');
    writeFileSync(path.join(outDir, 'RESULTS.md'), renderResults(rows));
  }
  console.log(`${rows.length} cells in ${((Date.now() - started) / 1000).toFixed(1)}s`);
  return rows.some((r) => r.status !== 'OK') ? 1 : 0;
}

main(process.argv.slice(2)).then((code) => process.exit(code), (err) => { console.error(err); process.exit(2); });
