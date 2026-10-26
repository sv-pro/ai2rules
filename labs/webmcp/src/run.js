#!/usr/bin/env node
// Runner. `npm run matrix` runs every scenario × every defense, plus the
// no-attack baseline for each task, and writes results/results.json and
// results/RESULTS.md. `npm run cell -- <scenario> <defense>` runs one cell.
import { mkdirSync, readdirSync, existsSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { launchBrowser } from './browser.js';
import { startServers } from './servers.js';
import { runCell, LAB_ROOT } from './cell.js';
import { DEFENSE_IDS } from './defenses/index.js';
import { TASKS } from './tasks.js';
import { renderResults } from './report.js';

function attackScenarios() {
  const dir = path.join(LAB_ROOT, 'scenarios');
  if (!existsSync(dir)) return [];
  return readdirSync(dir, { withFileTypes: true })
    .filter((d) => d.isDirectory() && existsSync(path.join(dir, d.name, 'scenario.js')))
    .map((d) => d.name)
    .sort();
}

async function main(argv) {
  const outDir = path.join(LAB_ROOT, 'results');
  const traceDir = path.join(outDir, 'traces');
  const workDir = path.join(os.tmpdir(), `webmcp-lab-${process.pid}`);
  mkdirSync(traceDir, { recursive: true });

  let cells;
  if (argv[0] === '--cell') {
    const [scenario, defense] = argv.slice(1);
    if (!scenario || !defense) throw new Error('usage: --cell <scenario|benign:task> <D0|D1|D2>');
    cells = [[scenario, defense]];
  } else {
    const only = argv.includes('--scenarios') ? argv[argv.indexOf('--scenarios') + 1].split(',') : null;
    const scenarios = [...Object.keys(TASKS).map((t) => `benign:${t}`), ...attackScenarios()]
      .filter((s) => !only || only.includes(s));
    cells = scenarios.flatMap((s) => DEFENSE_IDS.map((d) => [s, d]));
  }

  const started = Date.now();
  const servers = await startServers();
  const browser = await launchBrowser();
  const rows = [];
  try {
    for (const [scenarioId, defenseId] of cells) {
      const row = await runCell({ browser, servers, scenarioId, defenseId, traceDir, workDir });
      rows.push(row);
      const mark = row.status !== 'OK' ? 'ERROR' : row.attack_success === null ? '—' : row.attack_success ? 'ATTACK SUCCEEDED' : 'attack stopped';
      console.log(`${scenarioId.padEnd(28)} ${defenseId}  ${mark.padEnd(16)} utility=${row.utility}  stopped_by=${row.stopped_by ?? '-'}${row.error ? `  ${row.error}` : ''}`);
    }
  } finally {
    await browser.close();
    await servers.close();
  }

  if (argv[0] !== '--cell') {
    writeFileSync(path.join(outDir, 'results.json'), JSON.stringify({ lab: 'webmcp-attack-lab', agent: 'CompromisedAgent', rows }, null, 2) + '\n');
    writeFileSync(path.join(outDir, 'RESULTS.md'), renderResults(rows));
  }
  console.log(`${rows.length} cells in ${((Date.now() - started) / 1000).toFixed(1)}s`);
  return rows.some((r) => r.status !== 'OK') ? 1 : 0;
}

main(process.argv.slice(2)).then((code) => process.exit(code), (err) => { console.error(err); process.exit(2); });
