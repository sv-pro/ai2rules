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
import { renderResults, renderResultsSvg } from './report.js';
import { renderLlmResults, LLM_TASKS } from './report-llm.js';
import { harnessBin } from './defenses/d2-ai2rules.js';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';

// Local settings (API key, ports, binary path) from `labs/webmcp/.env`, which is
// gitignored; `.env.example` lists the names. A variable already set in the
// environment wins over the file. WEBMCP_NO_DOTENV=1 skips the file, so tests
// stay hermetic on a machine that has a real key in it.
const DOTENV = path.join(LAB_ROOT, '.env');
if (process.env.WEBMCP_NO_DOTENV !== '1' && existsSync(DOTENV)) {
  const before = new Set(Object.keys(process.env));
  process.loadEnvFile(DOTENV);
  // A name left blank in the template (`WEBMCP_BASE_PORT=`) means "not set",
  // not "the empty string": drop it so the default applies.
  for (const [k, v] of Object.entries(process.env)) if (v === '' && !before.has(k)) delete process.env[k];
}

function harnessVersion() {
  try { return execFileSync(harnessBin(), ['--version']).toString().trim(); } catch { return null; }
}

// The attacks the epic requires. A matrix run that is missing one is not the
// matrix (verifier finding E04/E05): it fails instead of silently shrinking.
export const REQUIRED_ATTACKS = ['s1-page-injection', 's2-poisoned-description', 's3-tool-substitution', 's4-reregister-after-failure', 's5-cross-tab-exfil'];

const sha256 = (buf) => createHash('sha256').update(buf).digest('hex');
const sha256Text = (file) => sha256(readFileSync(file, 'utf8').replace(/\r\n/g, '\n'));

// Policy provenance, deterministic across OSes (committed with the results):
// the exact D1 rules and the tasks D2 compiles its worlds from.
function policyProvenance() {
  return {
    d1_rules_sha256: sha256Text(path.join(LAB_ROOT, 'src', 'defenses', 'd1-rules.json')),
    tasks_sha256: sha256Text(path.join(LAB_ROOT, 'src', 'tasks.js')),
    d2_adapter_sha256: sha256Text(path.join(LAB_ROOT, 'src', 'defenses', 'd2-ai2rules.js')),
  };
}

// Run provenance, machine-specific (written next to the results, not committed).
function runProvenance(browserVersion) {
  const bin = harnessBin();
  let binSha = null;
  try { binSha = sha256(readFileSync(bin)); } catch { /* on PATH only */ }
  let head = null;
  try { head = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: LAB_ROOT }).toString().trim(); } catch { /* not a checkout */ }
  return { git_head: head, harness_bin: bin, harness_sha256: binSha, harness_version: harnessVersion(), node: process.version, chromium: browserVersion, platform: process.platform };
}

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
  let partial = false; // a filtered run must not overwrite the canonical results
  if (argv[0] === '--cell') {
    const [scenario, defense] = argv.slice(1);
    if (!scenario || !defense) throw new Error('usage: --cell <scenario|benign:task> <D0|D1|D2>');
    cells = [[scenario, defense]];
  } else {
    const only = argv.includes('--scenarios') ? argv[argv.indexOf('--scenarios') + 1].split(',') : null;
    partial = Boolean(only);
    const tasks = llm ? LLM_TASKS : Object.keys(TASKS);
    const scenarios = [...tasks.map((t) => `benign:${t}`), ...attackScenarios()]
      .filter((s) => !only || only.includes(s));
    cells = scenarios.flatMap((s) => DEFENSE_IDS.map((d) => [s, d]));
    const missing = only ? [] : REQUIRED_ATTACKS.filter((s) => !scenarios.includes(s));
    if (missing.length) throw new Error(`matrix is missing required attacks: ${missing.join(', ')}`);
  }

  const started = Date.now();
  const servers = await startServers();
  const browser = await launchBrowser();
  const chromiumVersion = browser.version();
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

  if (partial) {
    // A `--scenarios` subset ran; leave the committed results.json / RESULTS.md
    // untouched so a partial run can never overwrite the canonical table.
    console.log('(partial run: canonical results not rewritten)');
  } else if (argv[0] !== '--cell' && llm) {
    writeFileSync(path.join(outDir, 'results-llm.json'), JSON.stringify({ lab: 'webmcp-attack-lab', agent: 'LLMAgent', rows }, null, 2) + '\n');
    writeFileSync(path.join(outDir, 'RESULTS-LLM.md'), renderLlmResults(rows));
  } else if (argv[0] !== '--cell') {
    const meta = { harness: harnessVersion(), ...policyProvenance() };
    writeFileSync(path.join(outDir, 'provenance.local.json'), JSON.stringify(runProvenance(chromiumVersion), null, 2) + '\n');
    writeFileSync(path.join(outDir, 'results.json'), JSON.stringify({ lab: 'webmcp-attack-lab', agent: 'CompromisedAgent', ...meta, rows }, null, 2) + '\n');
    writeFileSync(path.join(outDir, 'RESULTS.md'), renderResults(rows, meta));
    writeFileSync(path.join(outDir, 'RESULTS.svg'), renderResultsSvg(rows));
  }
  console.log(`${rows.length} cells in ${((Date.now() - started) / 1000).toFixed(1)}s`);
  return rows.some((r) => r.status !== 'OK') ? 1 : 0;
}

main(process.argv.slice(2)).then((code) => process.exit(code), (err) => { console.error(err); process.exit(2); });
