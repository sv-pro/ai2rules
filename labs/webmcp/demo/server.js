#!/usr/bin/env node
// Live demo shell for the lab (AI2-94). `npm run demo`, then open the printed URL.
//
// One page: pick a defense, run a scenario, watch the agent's steps arrive and
// the oracle's verdict at the end. Nothing here decides anything: each run is the
// same `runCell` the matrix uses, against the same servers, with the real
// `harness` binary behind D2 and D3. The page only renders the trace records as
// `runCell` writes them, streamed over server-sent events.
//
// Traces go to a temp directory, so a demo run never touches results/.
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { launchBrowser } from '../src/browser.js';
import { startServers } from '../src/servers.js';
import { runCell, loadScenario } from '../src/cell.js';
import { DEFENSE_IDS, DEFENSE_INFO } from '../src/defenses/index.js';
import { harnessBin } from '../src/defenses/d2-ai2rules.js';
import { CANARY } from '../src/canary.js';
import { TASKS } from '../src/tasks.js';
import { loadLabEnv, harnessVersion } from '../src/local.js';

const HERE = path.dirname(fileURLToPath(import.meta.url));

// Same local settings file as the runner (HARNESS_BIN, ports).
loadLabEnv();

const PORT = Number(process.env.WEBMCP_DEMO_PORT ?? 4180);

// What the page offers. S5b is the story the demo is built around; the others
// are there for a follow-up question, not for the main path.
const SCENARIOS = ['s5b-read-sink-exfil', 's5-cross-tab-exfil', 'benign:crm-gift'];

// The kernel's version, asked once: a synchronous spawn per page load would
// stall a run being streamed.
const HARNESS = harnessVersion();

let browser = null;
let servers = null;
let busy = false;

async function ensureLab() {
  servers ??= await startServers();
  browser ??= await launchBrowser();
}

async function info() {
  const scenarios = [];
  for (const id of SCENARIOS) {
    const s = await loadScenario(id);
    scenarios.push({ id, short: s.short, name: s.name, task: s.task, attack: s.attack });
  }
  const tasks = Object.fromEntries(Object.values(TASKS).map((t) => [t.id, { prompt: t.prompt }]));
  return { scenarios, tasks, defenses: DEFENSE_IDS.map((id) => ({ id, ...DEFENSE_INFO[id] })), canary: CANARY, harness: HARNESS };
}

function sse(res) {
  res.writeHead(200, { 'content-type': 'text/event-stream', 'cache-control': 'no-cache', connection: 'keep-alive' });
  return (event, data) => res.write(`event: ${event}\ndata: ${JSON.stringify(data)}\n\n`);
}

async function run(url, res) {
  const defenseId = url.searchParams.get('defense');
  const scenarioId = url.searchParams.get('scenario') ?? SCENARIOS[0];
  if (!DEFENSE_IDS.includes(defenseId) || !SCENARIOS.includes(scenarioId)) {
    res.writeHead(400, { 'content-type': 'text/plain' });
    return res.end('unknown defense or scenario');
  }
  const send = sse(res);
  // Said as an event, not a 409: EventSource cannot read a status code, so a
  // refusal would reach the page as a lost connection.
  if (busy) {
    send('busy', { message: 'Another run is still going (in another tab, or one started before a reload). Try again when it ends.' });
    return res.end();
  }
  busy = true;
  try {
    await ensureLab();
    const row = await runCell({
      browser,
      servers,
      scenarioId,
      defenseId,
      traceDir: path.join(os.tmpdir(), `webmcp-demo-traces-${process.pid}`),
      workDir: path.join(os.tmpdir(), `webmcp-demo-${process.pid}`),
      onRecord: (rec) => send('record', rec),
    });
    send('result', row);
  } catch (err) {
    send('failure', { message: String(err?.message ?? err) });
  } finally {
    busy = false;
    res.end();
  }
}

const server = http.createServer(async (req, res) => {
  const url = new URL(req.url, `http://${req.headers.host}`);
  try {
    if (url.pathname === '/' || url.pathname === '/index.html') {
      res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
      return res.end(await readFile(path.join(HERE, 'index.html')));
    }
    if (url.pathname === '/api/info') {
      res.writeHead(200, { 'content-type': 'application/json' });
      return res.end(JSON.stringify(await info()));
    }
    if (url.pathname === '/api/run') return run(url, res);
    res.writeHead(404, { 'content-type': 'text/plain' });
    res.end('not found');
  } catch (err) {
    if (!res.headersSent) res.writeHead(500, { 'content-type': 'text/plain' });
    res.end(String(err?.message ?? err));
  }
});

async function shutdown() {
  server.close();
  await browser?.close().catch(() => {});
  await servers?.close().catch(() => {});
  process.exit(0);
}
process.on('SIGINT', shutdown);
process.on('SIGTERM', shutdown);

server.listen(PORT, '127.0.0.1', () => {
  console.log(`WebMCP lab demo: http://127.0.0.1:${PORT}  (kernel: ${HARNESS ?? `not found at ${harnessBin()}`})`);
});
