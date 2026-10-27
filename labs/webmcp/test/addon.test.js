// Experiment 1 of AI2-104: the host's mapping of the exfiltration add-on's
// facts. The refusal carrier composes the add-on beside a defense, and D96
// requires that composition only tightens.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withExfiltrationRefusal } from '../src/defenses/addon-exfiltration.js';

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
