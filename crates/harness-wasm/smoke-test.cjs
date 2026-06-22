// Headless smoke test for the E14 WASM engine: proves the REAL compiler + kernel
// run client-side from JavaScript (here, Node) and produce the same governance
// verdicts as the native harness.
//
// Prereq: build the Node-target bindings first, from the repo root:
//   wasm-pack build crates/harness-wasm --target nodejs --dev --out-dir pkg
// Then run:
//   node crates/harness-wasm/smoke-test.cjs

const assert = require('assert');
const w = require('./pkg/harness_wasm.js');

// 1) Engine is callable and reports its version.
assert.strictEqual(w.version(), '0.0.1', 'version()');

// 2) Default world seeds an editor.
const yaml = w.default_world();
assert.ok(yaml.includes('world_id'), 'default_world() returns manifest YAML');

// 3) The real preview compiles + decides in wasm.
const out = JSON.parse(w.preview(yaml));
assert.strictEqual(out.ok, true, 'preview() ok');
assert.ok(Array.isArray(out.surface) && out.surface.length > 0, 'projected surface');

// 4) THE money assertion — the monotonic taint floor, decided by the kernel in
//    wasm: a clean web fetch is allowed, a tainted one is denied.
const fetch = out.decisions.find((d) => d.action === 'fetch_web');
assert.ok(fetch, 'fetch_web is projected');
assert.strictEqual(fetch.clean.decision, 'Allow', 'clean fetch_web → Allow');
assert.strictEqual(fetch.tainted.decision, 'Deny', 'tainted fetch_web → Deny (taint floor)');
// Denied at the kernel's HARD taint invariant (the IRBuilder code-level floor),
// which pre-empts the softer no_tainted_network policy — exactly as native does.
assert.strictEqual(fetch.tainted.rule, 'taint_invariant', 'deciding rule');

// 5) Approval floor holds too.
const pty = out.decisions.find((d) => d.action === 'start_pty');
assert.strictEqual(pty.clean.decision, 'Ask', 'start_pty → Ask');

// 6) Bad input is a structured error, not an exception.
const bad = JSON.parse(w.preview('world_id: ""\nbase_actions: []'));
assert.strictEqual(bad.ok, false, 'invalid manifest → ok:false');

console.log('✅ WASM engine smoke test PASSED');
console.log('   engine version :', w.version());
console.log('   world_id       :', out.world_id, '(hash', out.manifest_hash + ')');
console.log('   projected tools:', out.surface.length);
console.log('   fetch_web      : clean=' + fetch.clean.decision + ' tainted=' + fetch.tainted.decision + ' (' + fetch.tainted.rule + ')');
