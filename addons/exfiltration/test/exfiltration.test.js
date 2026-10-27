// The add-on's own contract (AI2-101, D96). These tests use a record of their
// own, not the WebMCP lab's canary: the add-on must not be written against the
// oracle that later measures it.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createExfiltrationGuard, fingerprintsOf } from '../index.js';

const RECORD = {
  id: 'K-77',
  company: 'Blue Heron Freight',
  email: 'm.okafor@blueheron.example',
  phone: '+44 20 7946 0958',
  renewal: '2027-04-30',
  notes: 'Vault key VK-93XQ-4410-PL. Do not share.',
};

const guard = () => {
  const g = createExfiltrationGuard();
  g.observe(RECORD, { source: 'crm/get_customer' });
  return g;
};

test('fingerprints keep whole values and identifier-like tokens, not plain words or short values', () => {
  const f = fingerprintsOf(RECORD);
  assert.ok(f.includes('Blue Heron Freight'), 'a whole value');
  assert.ok(f.includes('VK-93XQ-4410-PL'), 'an identifier token inside a longer value');
  assert.ok(f.includes('2027-04-30'));
  assert.ok(!f.includes('K-77'), 'too short');
  assert.ok(!f.includes('Freight') && !f.includes('share.'), 'plain words are not fingerprints');
});

test('a private value is found in every form the add-on documents', () => {
  const g = guard();
  const secret = 'VK-93XQ-4410-PL';
  const cases = {
    exact: `note: ${secret}`,
    case_folded: secret.toLowerCase(),
    separators_removed: 'vk93xq4410pl',
    reversed: [...secret].reverse().join(''),
    base64: Buffer.from(secret).toString('base64'),
    percent_encoded: encodeURIComponent(`key=${secret} ok`).replace(/-/g, '%2D'),
  };
  for (const [form, text] of Object.entries(cases)) {
    const [finding] = g.inspect({ query: text });
    assert.ok(finding, `${form}: not found in ${text}`);
    assert.equal(finding.argument, 'query');
    assert.equal(finding.source, 'crm/get_customer');
    assert.equal(finding.form, form, `${text}`);
  }
  const b64NoPad = Buffer.from(secret).toString('base64').replace(/=+$/, '');
  assert.equal(g.inspect({ note: b64NoPad })[0]?.form, 'base64');
  // Inside an encoded argument the encoding is reported in front of the form.
  assert.equal(g.inspect({ q: secret.toLowerCase().replace(/-/g, '%2D') })[0]?.form, 'percent_encoded+case_folded');
  assert.equal(g.inspect({ q: 'to=Blue+Heron+Freight' })[0]?.form, 'form_encoded');
});

test('a derived form shorter than minLength is dropped, so a dashed value does not match stray digits', () => {
  const g = createExfiltrationGuard();
  g.observe({ code: '1-2-3-4-5-6-7' }, { source: 'crm' });
  assert.deepEqual(g.inspect({ note: 'order 1234567 please' }), [], 'the stripped form has 7 characters, under 8');
  assert.equal(g.inspect({ note: 'code 1-2-3-4-5-6-7' })[0]?.form, 'exact');
});

test('values nested in an argument are inspected, one finding per argument', () => {
  const g = guard();
  const findings = g.inspect({
    ship_to: { line1: '1 Main St', contact: 'm.okafor@blueheron.example' },
    note: 'renews 2027-04-30',
    qty: 1,
  });
  assert.deepEqual(findings.map((f) => f.argument), ['ship_to', 'note']);
});

test('ordinary arguments, and words a record merely shares, are not findings', () => {
  const g = guard();
  assert.deepEqual(g.inspect({ query: 'blue mug', note: 'freight please, do not share the box' }), []);
  assert.deepEqual(g.inspect({}), []);
  assert.deepEqual(g.inspect(undefined), []);
});

test('nothing is found before the host has observed a private value', () => {
  const g = createExfiltrationGuard();
  assert.deepEqual(g.inspect({ note: 'VK-93XQ-4410-PL' }), []);
  assert.equal(g.size(), 0);
});

test('findings are deterministic: the same observations and arguments give the same findings', () => {
  const args = { note: 'lp-0144-qx39-kv', query: 'Blue Heron Freight' };
  assert.deepEqual(guard().inspect(args), guard().inspect(args));
});

test('the add-on depends on nothing: no import from the kernel, the lab or a package', () => {
  const src = readFileSync(new URL('../index.js', import.meta.url), 'utf8');
  const imports = [...src.matchAll(/^\s*import\s.*?from\s+['"]([^'"]+)['"]/gm)].map((m) => m[1]);
  assert.deepEqual(imports.filter((m) => !m.startsWith('node:')), []);
});
