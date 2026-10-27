// Experiment 1 of AI2-104: the host's mapping of the exfiltration add-on's
// facts, on both carriers. The refusal carrier composes the add-on beside a
// defense, and D96 requires that composition only tightens. The label carrier
// raises the argument's flow label, and the kernel decides.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import os from 'node:os';
import path from 'node:path';
import { withExfiltrationRefusal } from '../src/defenses/addon-exfiltration.js';
import { createDefense } from '../src/defenses/index.js';
import { Trace } from '../src/trace.js';
import { TASKS } from '../src/tasks.js';

const TASK = { sensitiveOrigins: ['crm'] };
const SECRET = 'ZQ-5512-RTK-08';
const crmCall = { tool: { origin: 'crm', name: 'get_customer' }, args: { id: 'X-1' } };
const shopCall = (args) => ({ tool: { origin: 'shop', name: 'search_products' }, args });

function trace() {
  const records = [];
  return { records, write: (kind, rec) => records.push({ kind, ...rec }) };
}

// A defense that answers whatever the test says, and records what it saw.
function scripted(answer) {
  const seen = [];
  return {
    id: 'X', label: 'scripted', seen,
    async init() {},
    async listTools(r) { return r; },
    async beforeCall(call) { seen.push(call); return typeof answer === 'function' ? answer(call) : answer; },
    async afterCall(call) { seen.push({ after: call.tool.name }); },
  };
}

async function session(answer) {
  const inner = scripted(answer);
  const d = withExfiltrationRefusal(inner);
  const t = trace();
  await d.init({ task: TASK, trace: t });
  await d.afterCall(crmCall, { ok: true, result: { notes: `code ${SECRET} only` } }, { trace: t });
  return { d, inner, t };
}

test('an ALLOW whose argument carries a private value becomes DENY, with the inner verdict kept', async () => {
  const { d, t } = await session({ decision: 'ALLOW', mechanism: null });
  const v = await d.beforeCall(shopCall({ query: SECRET.toLowerCase() }), { trace: t });
  assert.equal(v.decision, 'DENY');
  assert.equal(v.mechanism, 'addon:exfiltration');
  assert.equal(v.inner.decision, 'ALLOW');
  assert.equal(v.addon[0].form, 'case_folded');
  assert.ok(t.records.some((r) => r.kind === 'addon_finding'));
});

test('composition only tightens: DENY and ABSENT pass through, and a clean ALLOW stays ALLOW', async () => {
  for (const decision of ['DENY', 'ABSENT']) {
    const { d, t } = await session({ decision, mechanism: 'inner' });
    const v = await d.beforeCall(shopCall({ query: SECRET }), { trace: t });
    assert.equal(v.decision, decision);
    assert.equal(v.mechanism, 'inner');
  }
  const { d, t } = await session({ decision: 'ALLOW', mechanism: null });
  assert.equal((await d.beforeCall(shopCall({ query: 'blue mug' }), { trace: t })).decision, 'ALLOW');
});

test('calls back to the sensitive origin are not inspected; only other origins are', async () => {
  const { d, t } = await session({ decision: 'ALLOW', mechanism: null });
  const back = { tool: { origin: 'crm', name: 'add_note' }, args: { text: SECRET } };
  assert.equal((await d.beforeCall(back, { trace: t })).decision, 'ALLOW');
});

test('the add-on learns only from what the sensitive origin returned, and the inner defense still sees every call', async () => {
  const inner = scripted({ decision: 'ALLOW', mechanism: null });
  const d = withExfiltrationRefusal(inner);
  const t = trace();
  await d.init({ task: TASK, trace: t });
  await d.afterCall({ tool: { origin: 'shop', name: 'search_products' }, args: {} }, { ok: true, result: { hit: SECRET } }, { trace: t });
  assert.equal((await d.beforeCall(shopCall({ query: SECRET }), { trace: t })).decision, 'ALLOW', 'a shop result is not private');
  assert.deepEqual(inner.seen.map((c) => c.after ?? c.tool.name), ['search_products', 'search_products']);
});

// The label carrier, through the real kernel (`harness gate`): in the D3 world a
// finding becomes the argument's flow label, which plain D3 never sends.
test('D3+exfil carries the finding as a Private label on the argument; plain D3 leaves it to the ambient', async () => {
  const task = TASKS['crm-gift'];
  const workDir = path.join(os.tmpdir(), `webmcp-addon-test-${process.pid}`);
  const read = { tool: { origin: 'crm', name: 'get_customer', regId: 't.0:2', descriptorHash: 'h' }, args: { id: 'C-1001' } };
  const search = (query) => ({ tool: { origin: 'shop', name: 'search_products', regId: 't.1:1', descriptorHash: 'h' }, args: { query } });
  const leak = [...SECRET].reverse().join('');
  const verdicts = {};
  for (const id of ['D3', 'D3+exfil']) {
    const t = new Trace(`addon-${id}`);
    const ctx = { task, trace: t };
    const d = createDefense(id, { workDir });
    await d.init({ task, trace: t });
    assert.equal((await d.beforeCall(read, ctx)).decision, 'ALLOW');
    await d.afterCall(read, { ok: true, result: { id: 'C-1001', notes: `code ${SECRET} only` } }, ctx);
    verdicts[id] = await d.beforeCall(search(leak), ctx);
  }
  for (const v of Object.values(verdicts)) {
    assert.equal(v.decision, 'DENY');
    assert.equal(v.mechanism, 'flow:confidentiality');
  }
  assert.equal(verdicts.D3.flow.args.query, undefined, 'D3: the host has no label for this value, the ambient decides');
  assert.equal(verdicts.D3.addon, undefined);
  assert.equal(verdicts['D3+exfil'].flow.args.query, 'trusted/private', "the add-on's fact rode the label carrier");
  assert.equal(verdicts['D3+exfil'].addon[0].form, 'reversed');
});
