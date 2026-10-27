#!/usr/bin/env node
// MCP bridge for the live demo (AI2-95): a stdio MCP server that Claude Code
// starts from the demo folder's .mcp.json. It forwards every request to the
// live server (live/server.js), which lists what the pages registered and runs
// each call through the session's defense, in the visible browser.
//
// So Claude Code plays the browser's agent; this bridge and the live server play
// the browser that brokers the page's WebMCP tools; ai2rules decides in between.
//
// No dependencies: newline-delimited JSON-RPC over stdin/stdout, the MCP stdio
// transport. Only the methods a tools-only server needs.
import { createInterface } from 'node:readline';

const LIVE = process.env.WEBMCP_LIVE_URL ?? 'http://127.0.0.1:4290';
const send = (msg) => process.stdout.write(JSON.stringify(msg) + '\n');
const reply = (id, result) => send({ jsonrpc: '2.0', id, result });
const fail = (id, code, message) => send({ jsonrpc: '2.0', id, error: { code, message } });

async function live(pathname, payload) {
  // A stand that is down (stopped, or restarting) never received the request,
  // so nothing ran: say that, rather than a bare "fetch failed" the agent must
  // read as "the order may have been placed".
  const res = await fetch(`${LIVE}${pathname}`, payload === undefined ? {} : {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(payload),
  }).catch(() => {
    throw new Error(`The browser's tool broker (the demo stand at ${LIVE}) is not running, so this request never reached the page and nothing ran. Ask the user to start the stand (npm run live), then try again.`);
  });
  if (!res.ok) throw new Error(`live server ${res.status}: ${await res.text()}`);
  return res.json();
}

// A new session on the control room can change which tools exist (D2/D3 show
// only the assistant's). Tell the client to list again when that happens.
let seen = null;
let listing = false;
setInterval(async () => {
  if (!listing) return;
  try {
    const { session } = await live('/bridge/generation');
    if (seen !== null && session !== seen) send({ jsonrpc: '2.0', method: 'notifications/tools/list_changed' });
    seen = session;
  } catch { /* the live server is down; the next call will say so */ }
}, 1500).unref();

async function handle(msg) {
  const { id, method, params } = msg;
  if (method === 'initialize') {
    return reply(id, {
      protocolVersion: params?.protocolVersion ?? '2025-06-18',
      capabilities: { tools: { listChanged: true } },
      serverInfo: { name: 'webmcp-lab', version: '0.1.0' },
      instructions: "Tools registered by the web pages open in the user's browser (WebMCP). read_page reads a tab's text.",
    });
  }
  if (method === 'ping') return reply(id, {});
  if (method === 'tools/list') {
    const { session, tools } = await live('/bridge/tools', {});
    seen = session;
    listing = true;
    return reply(id, { tools });
  }
  if (method === 'tools/call') {
    const out = await live('/bridge/call', { name: params?.name, arguments: params?.arguments ?? {} });
    return reply(id, { content: [{ type: 'text', text: out.text }], isError: Boolean(out.isError) });
  }
  if (id !== undefined) return fail(id, -32601, `method not found: ${method}`);
  // Notifications (initialized, cancelled, …) need no answer.
}

createInterface({ input: process.stdin }).on('line', async (line) => {
  if (!line.trim()) return;
  let msg;
  try { msg = JSON.parse(line); } catch { return fail(null, -32700, 'parse error'); }
  try { await handle(msg); } catch (err) {
    if (msg.id !== undefined) fail(msg.id, -32000, String(err?.message ?? err));
  }
});
