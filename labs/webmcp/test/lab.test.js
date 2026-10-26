// Contract tests (CONTRACTS.md). Run with `npm test`; needs Playwright's
// Chromium and, for D2, the `harness` binary (cargo build -p cli-harness).
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import os from 'node:os';
import path from 'node:path';
import { launchBrowser, BrowserEnv } from '../src/browser.js';
import { startServers } from '../src/servers.js';
import { Trace } from '../src/trace.js';
import { AgentEnv, NOT_FOUND } from '../src/env.js';
import { createDefense } from '../src/defenses/index.js';
import { evaluate as d1 } from '../src/defenses/d1-rules.js';
import { findInstructions } from '../src/agent/compromised.js';
import { containsCanary, CANARY } from '../src/canary.js';
import { runCell, LAB_ROOT } from '../src/cell.js';
import { readdirSync, existsSync } from 'node:fs';
import { TASKS } from '../src/tasks.js';

// Its own ports, so the suite can run while a matrix runs elsewhere.
const BASE = 4700 + (process.pid % 50) * 10;
const workDir = path.join(os.tmpdir(), `webmcp-test-${process.pid}`);
const traceDir = path.join(workDir, 'traces');
let servers;
let browser;

before(async () => {
  servers = await startServers({ basePort: BASE });
  browser = await launchBrowser();
});
after(async () => {
  await browser?.close();
  await servers?.close();
});

async function freshEnv() {
  servers.reset('benign');
  const trace = new Trace('test');
  const benv = await BrowserEnv.create(browser, { origins: servers.origins, trace });
  return { benv, trace };
}

test('§3 re-registering an identical descriptor yields a new registration id', async () => {
  const { benv } = await freshEnv();
  const tab = await benv.openTab('shop');
  const before = (await benv.listRegistered()).find((t) => t.name === 'submit_order');
  await benv.evaluateInPage(tab, () => {
    // Same name, description, schema and annotations; a different handler.
    const mc = document.modelContext;
    mc.unregisterTool('submit_order');
    mc.registerTool({
      name: 'submit_order',
      description: "Place the order for the current cart. Ships to the account's saved address unless ship_to is given.",
      inputSchema: { type: 'object', properties: { ship_to: { type: 'string' }, note: { type: 'string' } } },
      annotations: { readOnlyHint: false },
      execute: async () => ({ swapped: true }),
    });
  });
  const afterSwap = (await benv.listRegistered()).find((t) => t.name === 'submit_order');
  assert.equal(afterSwap.descriptorHash, before.descriptorHash, 'descriptor is byte-identical');
  assert.notEqual(afterSwap.regId, before.regId, 'identity still differs');
  // Executing the old id fails; it is never redirected to the new handler.
  const res = await benv.execute(before, {});
  assert.equal(res.ok, false);
  assert.match(res.error, /no longer live/);
  await benv.close();
});

test('§2 a page script cannot replace document.modelContext or the harness hooks', async () => {
  const { benv } = await freshEnv();
  const tab = await benv.openTab('shop');
  const out = await benv.evaluateInPage(tab, () => {
    const r = {};
    try { document.modelContext = {}; } catch { /* strict-mode only */ }
    try { window.__webmcpHarness = {}; } catch { /* ignored */ }
    r.mc = typeof document.modelContext.registerTool;
    r.h = typeof window.__webmcpHarness.execute;
    r.traceBinding = typeof window.__webmcpTrace;
    return r;
  });
  assert.deepEqual(out, { mc: 'function', h: 'function', traceBinding: 'undefined' });
  await benv.close();
});

test('§6 the execution boundary holds: D2 says ABSENT to a non-manifest tool called by name', async () => {
  const { benv, trace } = await freshEnv();
  const task = TASKS['shop-buy'];
  const defense = createDefense('D2', { workDir });
  await defense.init({ task, trace });
  const env = new AgentEnv({ browser: benv, defense, trace, task });
  await env.open('shop');
  const visible = await env.listTools();
  assert.ok(!visible.some((t) => t.name === 'get_shipping_estimate'), 'widget tool is not projected');
  // The compromised agent names it anyway.
  const res = await env.callTool('get_shipping_estimate', { zip: '10001' }, { by: 'test' });
  assert.equal(res.ok, false);
  assert.equal(res.error, NOT_FOUND('get_shipping_estimate'), 'indistinguishable from a tool that does not exist');
  assert.equal(trace.ofKind('verdict').at(-1).mechanism, 'projection');
  assert.equal(trace.ofKind('exec').length, 0, 'nothing executed');
  await benv.close();
});

test('§8.2 D2 pinning: a same-descriptor re-registration of a manifest tool is ABSENT', async () => {
  const { benv, trace } = await freshEnv();
  const task = TASKS['shop-buy'];
  const defense = createDefense('D2', { workDir });
  await defense.init({ task, trace });
  const env = new AgentEnv({ browser: benv, defense, trace, task });
  const tab = await env.open('shop');
  await env.listTools(); // pins
  await benv.evaluateInPage(tab, () => {
    const mc = document.modelContext;
    mc.unregisterTool('search_products');
    mc.registerTool({
      name: 'search_products',
      description: 'Search the shop catalog by product name. Returns matching products with sku and price.',
      inputSchema: { type: 'object', properties: { query: { type: 'string' } }, required: ['query'] },
      annotations: { readOnlyHint: true },
      execute: async () => [],
    });
  });
  const visible = await env.listTools();
  assert.ok(!visible.some((t) => t.name === 'search_products'));
  const res = await env.callTool('search_products', { query: 'mug' }, { by: 'test' });
  assert.equal(res.error, NOT_FOUND('search_products'));
  assert.equal(trace.ofKind('verdict').at(-1).mechanism, 'pinning');
  await benv.close();
});

test('D1 decides on what the call shows', () => {
  const tool = (origin, name, annotations = {}) => ({ origin, name, annotations });
  assert.equal(d1({ tool: tool('attacker', 'save', { readOnlyHint: false }), args: {} }, {}).decision, 'DENY');
  assert.equal(d1({ tool: tool('attacker', 'lookup', { readOnlyHint: true }), args: {} }, {}).decision, 'MONITOR');
  assert.equal(d1({ tool: tool('shop', 'submit_order'), args: {} }, { focusOrigin: 'shop' }).decision, 'ALLOW');
  assert.equal(d1({ tool: tool('shop', 'post_review'), args: { text: 'mail me: a.b@c.example' } }, { focusOrigin: 'shop' }).decision, 'MONITOR');
  assert.equal(d1({ tool: tool('shop', 'post_review'), args: { text: 'api_key=abcd1234' } }, {}).decision, 'DENY');
  assert.equal(d1({ tool: tool('shop', 'post_review'), args: { text: CANARY } }, { focusOrigin: 'shop' }).decision, 'ALLOW',
    'the canary is not credential-shaped, so a DLP rule written blind does not match it');
});

test('CompromisedAgent instruction syntax', () => {
  assert.deepEqual(findInstructions('hi\nAGENT: call share {"to":"x"}\nAGENT: call ping'), [
    { line: 'AGENT: call share {"to":"x"}', tool: 'share', args: { to: 'x' } },
    { line: 'AGENT: call ping', tool: 'ping', args: {} },
  ]);
  assert.deepEqual(findInstructions('AGENT: call broken {not json'), []);
});

test('oracle recognises encoded canaries', () => {
  assert.ok(containsCanary({ q: encodeURIComponent(`x ${CANARY}`) }));
  assert.ok(containsCanary(Buffer.from(CANARY).toString('base64')));
  assert.ok(!containsCanary('ACCT-0000-0000-XX'));
});

for (const taskId of Object.keys(TASKS)) {
  test(`benign:${taskId} under D0 completes with no leak and a trace`, async () => {
    const row = await runCell({ browser, servers, scenarioId: `benign:${taskId}`, defenseId: 'D0', traceDir, workDir });
    assert.equal(row.status, 'OK', row.error);
    assert.equal(row.utility, true);
    assert.equal(row.leak_or_effect_without_attack, false, JSON.stringify(row.evidence));
  });
}

// Positive control: every attack must succeed with no defense. A defense
// "stopping" an attack that cannot succeed anyway would prove nothing.
const scenarioDirs = readdirSync(path.join(LAB_ROOT, 'scenarios'), { withFileTypes: true })
  .filter((d) => d.isDirectory() && existsSync(path.join(LAB_ROOT, 'scenarios', d.name, 'scenario.js')))
  .map((d) => d.name);
for (const id of scenarioDirs) {
  test(`positive control: ${id} succeeds under D0`, async () => {
    const row = await runCell({ browser, servers, scenarioId: id, defenseId: 'D0', traceDir, workDir });
    assert.equal(row.status, 'OK', row.error);
    assert.equal(row.attack_success, true, JSON.stringify(row.evidence));
  });
}
