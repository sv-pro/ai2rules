#!/usr/bin/env node
// The live two-window demo (AI2-95). `npm run live`, then open the control room
// it prints. A visible Chromium shows the lab's tabs; Claude Code works in them
// through the MCP bridge (live/mcp-bridge.js); the control room sets the policy,
// the widget and the sensitive sites, runs the hijacked model, and shows the
// feed, the labels and what the outside world received.
//
// The server holds one LiveSession; the bridge is a thin stdio process that
// forwards Claude Code's MCP requests here. Nothing in this file decides
// anything: the session's defense does, through the lab's own middleware.
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { launchBrowser } from '../src/browser.js';
import { startServers } from '../src/servers.js';
import { LAB_ROOT } from '../src/cell.js';
import { harnessBin } from '../src/defenses/d2-ai2rules.js';
import { CANARY } from '../src/canary.js';
import { LiveSession, POLICIES, WIDGETS } from './session.js';
import { createReplay, STATIC, harnessVersionOnce as harnessVersion } from '../demo/server.js';
import { loadLabEnv } from '../src/local.js';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const DEMO = path.join(HERE, '..', 'demo');

// Local settings (HARNESS_BIN, ports): WEBMCP_DOTENV, else the lab's .env. A
// variable already set wins; a name left blank means "not set".
loadLabEnv(process.env.WEBMCP_DOTENV ?? path.join(LAB_ROOT, '.env'));

// Not 4190: that is on the Fetch standard's list of blocked ports, so Node's
// fetch (the bridge, the hook) and Chrome would refuse to connect to it.
export const LIVE_PORT = Number(process.env.WEBMCP_LIVE_PORT ?? 4290);
const SITES = ['crm', 'shop'];
const BODY_LIMIT = 64 * 1024;

// The MCP name of a page tool: its origin and its own name. The bridge sends
// both back, so the session can focus the right tab; the call itself still
// resolves by the page tool's name (session.js).
const mcpName = (origin, name) => `${origin}__${name}`.replace(/[^A-Za-z0-9_-]/g, '_');

// The agent's window. Playwright's own Chromium first; where the machine refuses
// to start it (some Windows setups block unsigned test builds), an installed
// Chrome, then Edge. WEBMCP_LIVE_CHANNEL picks one outright.
async function launchVisible(headless) {
  if (headless) return launchBrowser();
  const forced = process.env.WEBMCP_LIVE_CHANNEL;
  const tries = forced ? [forced] : [undefined, 'chrome', 'msedge'];
  let last;
  for (const channel of tries) {
    try {
      const browser = await launchBrowser({ headless: false, channel });
      console.log(`agent's window: ${channel ?? "Playwright's Chromium"}`);
      return browser;
    } catch (err) { last = err; }
  }
  throw last;
}

export async function startLive({ port = LIVE_PORT, headless = process.env.WEBMCP_LIVE_HEADLESS === '1', basePort } = {}) {
  const servers = await startServers(basePort ? { basePort } : {});
  // The agent's window, launched again if someone closed the whole browser.
  let browser = await launchVisible(headless);
  let relaunching = null;
  const currentBrowser = async () => {
    if (browser.isConnected()) return browser;
    // A failed launch is not remembered: the next request tries again.
    relaunching ??= launchVisible(headless).then(
      (b) => { browser = b; relaunching = null; return b; },
      (err) => { relaunching = null; throw err; },
    );
    return relaunching;
  };
  const clients = new Set();
  // The current session's events, replayed to a control room that connects
  // late. A world snapshot replaces the previous one (each is the whole world),
  // and the rest is capped: a late control room needs the story, not every byte.
  let history = [];
  const HISTORY_LIMIT = 2000;

  const emit = (event, data) => {
    if (event === 'session' && data.fresh) history = [];
    if (event === 'world') history = history.filter(([e]) => e !== 'world');
    history.push([event, data]);
    if (history.length > HISTORY_LIMIT) history.splice(1, history.length - HISTORY_LIMIT);
    const line = `event: ${event}\ndata: ${JSON.stringify(data)}\n\n`;
    for (const res of clients) res.write(line);
  };
  const session = new LiveSession({ browser: currentBrowser, servers, workDir: path.join(os.tmpdir(), `webmcp-live-${process.pid}`), emit });

  // A request body: JSON, at most BODY_LIMIT bytes. Anything else is the
  // caller's error (400/413), not the stand's.
  const body = async (req) => {
    const chunks = [];
    let size = 0;
    for await (const c of req) {
      size += c.length;
      if (size > BODY_LIMIT) throw Object.assign(new Error('request body too large'), { status: 413 });
      chunks.push(c);
    }
    const raw = Buffer.concat(chunks).toString('utf8');
    try { return raw ? JSON.parse(raw) : {}; } catch { throw Object.assign(new Error('request body is not JSON'), { status: 400 }); }
  };
  const json = (res, status, data) => { res.writeHead(status, { 'content-type': 'application/json' }); res.end(JSON.stringify(data)); };
  const page = (file) => async (_req, res) => { res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' }); res.end(await readFile(file)); };
  const replay = createReplay();

  const routes = {
    // The three parts: how it works, the replay (AI2-94's page), the live room.
    'GET /': page(path.join(HERE, 'index.html')),
    'GET /how': page(path.join(HERE, 'how.html')),
    'GET /replay': page(path.join(DEMO, 'index.html')),
    'GET /replay/api/info': async (_req, res) => json(res, 200, await replay.info()),
    'GET /replay/api/run': (req, res) => replay.run(new URL(req.url, 'http://x'), res),
    ...Object.fromEntries(Object.entries(STATIC).map(([p, [file, type]]) => [`GET ${p}`, async (_req, res) => {
      res.writeHead(200, { 'content-type': `${type}; charset=utf-8` });
      res.end(await readFile(path.join(DEMO, file)));
    }])),
    'GET /api/info': (_req, res) => json(res, 200, {
      policies: Object.entries(POLICIES).map(([id, p]) => ({ id, ...p })),
      widgets: Object.entries(WIDGETS).map(([id, w]) => ({ id, name: w.name, blurb: w.blurb })),
      sites: SITES,
      canary: CANARY,
      harness: harnessVersion(),
      session: session.describe(),
    }),
    'GET /api/events': (req, res) => {
      res.writeHead(200, { 'content-type': 'text/event-stream', 'cache-control': 'no-cache', connection: 'keep-alive' });
      for (const [event, data] of history) res.write(`event: ${event}\ndata: ${JSON.stringify(data)}\n\n`);
      clients.add(res);
      req.on('close', () => clients.delete(res));
    },
    'POST /api/session': async (req, res) => {
      const { policy, widget, sensitive } = await body(req);
      const config = {};
      if (policy) config.policy = policy;
      if (widget) config.widget = widget;
      if (Array.isArray(sensitive)) config.sensitive = sensitive.filter((s) => SITES.includes(s));
      await session.start(config);
      json(res, 200, session.describe());
    },
    'POST /api/prompt': async (req, res) => {
      const { prompt } = await body(req);
      session.addPrompt(prompt);
      json(res, 200, { prompts: session.prompts.length });
    },
    'POST /api/hijacked': async (_req, res) => {
      session.runHijacked().catch((err) => emit('record', { kind: 'error', message: String(err?.message ?? err) }));
      json(res, 202, { started: true });
    },
    // ---- the bridge's side ----
    'GET /bridge/generation': (_req, res) => json(res, 200, { session: session.number }),
    'POST /bridge/tools': async (_req, res) => {
      const { tabs, tools } = await session.tools();
      json(res, 200, {
        session: session.number,
        tools: [
          {
            name: 'read_page',
            description: `Read the visible text of an open tab in the user's browser. Open tabs: ${tabs.join(', ')}.`,
            inputSchema: { type: 'object', properties: { origin: { type: 'string', enum: tabs } }, required: ['origin'] },
          },
          ...tools.map((t) => ({
            name: mcpName(t.origin, t.name),
            description: `[${t.origin}] ${t.description ?? ''}`.slice(0, 4000),
            inputSchema: t.inputSchema && typeof t.inputSchema === 'object' ? { type: 'object', ...t.inputSchema } : { type: 'object' },
          })),
        ],
      });
    },
    'POST /bridge/call': async (req, res) => {
      const { name, arguments: args } = await body(req);
      if (name === 'read_page') {
        const r = await session.readPage(String(args?.origin ?? ''));
        return json(res, 200, { isError: !r.ok, text: r.text });
      }
      const cut = String(name).indexOf('__');
      const origin = cut > 0 ? name.slice(0, cut) : null;
      const tool = cut > 0 ? name.slice(cut + 2) : String(name);
      const r = await session.call(origin, tool, args);
      json(res, 200, r.ok
        ? { isError: false, text: typeof r.result === 'string' ? r.result : JSON.stringify(r.result ?? null) }
        : { isError: true, text: String(r.error) });
    },
  };

  // Only the control room, the bridge and the hook may drive the stand. The
  // agent's window is full of pages, the shop's compromised widget among them,
  // and any page can send a "simple" cross-origin POST (text/plain, no
  // preflight) to localhost. So: the Host must be this stand (no DNS
  // rebinding), a POST must be application/json (a cross-origin page cannot send
  // that without a preflight, which is never answered), and a browser's Origin,
  // when sent, must be this stand's own.
  const own = () => [`127.0.0.1:${server.address().port}`, `localhost:${server.address().port}`];
  function refuse(req) {
    if (!own().includes(req.headers.host)) return [403, 'unknown host'];
    if (req.headers.origin && !own().map((h) => `http://${h}`).includes(req.headers.origin)) return [403, 'cross-origin request'];
    if (req.method === 'POST' && !/^application\/json\s*(;|$)/i.test(req.headers['content-type'] ?? '')) return [415, 'POST bodies must be application/json'];
    return null;
  }

  const server = http.createServer(async (req, res) => {
    const refused = refuse(req);
    if (refused) return json(res, refused[0], { error: refused[1] });
    const url = new URL(req.url, `http://${req.headers.host}`);
    const route = routes[`${req.method} ${url.pathname}`];
    if (!route) { res.writeHead(404, { 'content-type': 'text/plain' }); return res.end('not found'); }
    try { await route(req, res); } catch (err) {
      if (!res.headersSent) json(res, err?.status ?? 500, { error: String(err?.message ?? err) });
      else res.end();
    }
  });
  await new Promise((resolve) => server.listen(port, '127.0.0.1', resolve));
  await session.start();

  return {
    url: `http://127.0.0.1:${server.address().port}`,
    session,
    async close() {
      for (const res of clients) res.end();
      server.close();
      await replay.close();
      await browser.close().catch(() => {});
      await servers.close().catch(() => {});
    },
  };
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const live = await startLive();
  const v = harnessVersion();
  console.log(`WebMCP live demo — control room: ${live.url}`);
  console.log(`kernel: ${v ?? `not found at ${harnessBin()}`}`);
  console.log('Claude Code: run `npm run live:setup` once, then open a session in the folder it prints.');
  const stop = async () => { await live.close(); process.exit(0); };
  process.on('SIGINT', stop);
  process.on('SIGTERM', stop);
}
