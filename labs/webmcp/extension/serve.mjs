#!/usr/bin/env node
// Serves the lab's own pages as static demo pages for the extension
// (DESIGN.md §5): one origin per lab origin on 127.0.0.1, scenario files layered
// over the base pages exactly as src/servers.js layers them, and a simulated
// server (demo/sim-server.js) injected as each page's first script in place of
// the lab's Node API. Nothing here decides anything; it only hosts pages.
//
//   node extension/serve.mjs [scenario]     # default: s5b-read-sink-exfil
import http from 'node:http';
import { readFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { CATALOG, CRM_RECORDS, ORIGIN_NAMES, SHOP_ACCOUNT, originsFor } from '../src/servers.js';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const LAB_ROOT = path.resolve(HERE, '..');

// Not the lab's 4100: a matrix run and the demo can share a machine.
export const DEMO_BASE_PORT = 4300;
export const DEMO_ORIGINS = originsFor(DEMO_BASE_PORT);

const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.json': 'application/json' };

// What each origin's simulated server holds: its own data only. The CRM's
// records (with the canary) never reach another origin's page.
const SIM_DATA = {
  shop: { catalog: CATALOG, account: SHOP_ACCOUNT },
  crm: { records: CRM_RECORDS },
};

export async function startDemoServers({ scenario = 's5b-read-sink-exfil', origins = DEMO_ORIGINS } = {}) {
  const simSource = await readFile(path.join(HERE, 'demo', 'sim-server.js'), 'utf8');
  const servers = await Promise.all(ORIGIN_NAMES.map((origin) => new Promise((resolve, reject) => {
    const srv = http.createServer(async (req, res) => {
      const url = new URL(req.url, origins[origin]);
      if (url.pathname === '/__sim.js') {
        const prefix = `globalThis.__AI2RULES_SIM = ${JSON.stringify({ origin, data: SIM_DATA[origin] ?? {} })};\n`;
        res.writeHead(200, { 'content-type': 'text/javascript', 'cache-control': 'no-store' });
        return res.end(prefix + simSource);
      }
      const rel = url.pathname === '/' ? 'index.html' : url.pathname.replace(/^\/+/, '');
      const file = rel.includes('..') ? null : [
        path.join(LAB_ROOT, 'scenarios', scenario, 'pages', origin, rel),
        path.join(LAB_ROOT, 'pages', origin, rel),
      ].find((f) => existsSync(f));
      if (!file) {
        res.writeHead(404, { 'content-type': 'text/plain' });
        return res.end('not found');
      }
      let body = (await readFile(file)).toString('utf8');
      body = body.replace(/\{\{ORIGIN_([A-Z]+)\}\}/g, (m, n) => origins[n.toLowerCase()] ?? m);
      if (file.endsWith('.html')) body = body.replace(/<head>/i, '<head><script src="/__sim.js"></script>');
      res.writeHead(200, { 'content-type': TYPES[path.extname(file)] ?? 'application/octet-stream', 'cache-control': 'no-store' });
      res.end(body);
    });
    srv.once('error', reject);
    srv.listen(Number(new URL(origins[origin]).port), '127.0.0.1', () => resolve(srv));
  })));
  return { origins, close: () => Promise.all(servers.map((s) => new Promise((r) => s.close(r)))) };
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const scenario = process.argv[2] ?? 's5b-read-sink-exfil';
  const { origins } = await startDemoServers({ scenario });
  console.log(`demo pages for ${scenario}:`);
  for (const [name, o] of Object.entries(origins)) console.log(`  ${name.padEnd(8)} ${o}`);
  console.log('Ctrl+C to stop.');
}
