#!/usr/bin/env node
// Does the committed WASM artifact still speak for the current kernel?
//
// `blog/public/vendor/harness-wasm/` is a build output that lives in git, so the
// public playground can load it as a static asset. Nothing forced it to be
// rebuilt: between 2026-06-22 and 2026-08-12 the committed artifact fell nine
// preview-affecting commits behind the Rust source — including the whole `roots`
// feature and D36 command classification — while every CI job stayed green
// (finding #18). AGENTS.md states the invariant ("no drift between native and
// WASM"); this script is what actually enforces it.
//
// The check is **semantic, not byte-level**: it loads the committed artifact and
// a freshly built one, and requires them to answer identically. Byte comparison
// would go red whenever the `stable` Rust toolchain or wasm-pack bumped codegen,
// which is drift in the build, not in the kernel.
//
//   wasm-pack build --target web --release   # inside crates/harness-wasm/
//   node scripts/check-wasm-freshness.mjs
//
// Fix a failure by copying the fresh build over the committed one:
//   cp crates/harness-wasm/pkg/harness_wasm{_bg.wasm,.js} blog/public/vendor/harness-wasm/

import { readFileSync, existsSync, statSync, readdirSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
import { join } from 'node:path';

const COMMITTED = process.argv[2] ?? 'blog/public/vendor/harness-wasm';
const FRESH = process.argv[3] ?? 'crates/harness-wasm/pkg';

/** Load a wasm-pack `--target web` artifact directory as a live module. */
async function load(dir, label) {
  const js = join(dir, 'harness_wasm.js');
  const wasm = join(dir, 'harness_wasm_bg.wasm');
  for (const f of [js, wasm]) {
    if (!existsSync(f)) {
      fail(`${label}: missing ${f}`, label === 'fresh'
        ? 'Build it first: (cd crates/harness-wasm && wasm-pack build --target web --release)'
        : 'The committed artifact is incomplete.');
    }
  }
  const mod = await import(pathToFileURL(js).href);
  await mod.default({ module_or_path: readFileSync(wasm) });
  return mod;
}

function fail(what, hint) {
  console.error(`\n✗ WASM freshness: ${what}`);
  if (hint) console.error(`\n  ${hint}`);
  process.exit(1);
}

// The manifests the two engines must agree on. Beyond the bundled default world,
// these pin the features whose absence made the drift invisible last time: a
// world with `roots` (#27) and one with `command_classes` (D36). An engine that
// predates either silently returns a different surface for them.
const CASES = {
  roots: `
world_id: freshness-roots
capabilities:
  - { trust: Trusted, actions: [Read, Write] }
base_actions:
  - { name: read_file, action_type: Read, side_effect: Read }
  - { name: write_file, action_type: Write, side_effect: FilesystemWrite }
roots:
  default: Ask
  rules:
    - { path: "/work", access: ReadWrite }
    - { path: "/work/inbox", access: Read, taint_source: true }
    - { path: "/etc/shadow", access: Deny, class: Secret }
`,
  command_classes: `
world_id: freshness-classes
capabilities:
  - { trust: Trusted, actions: [Command] }
base_actions:
  - { name: bash, action_type: Command, side_effect: Process }
  - { name: bash_network, action_type: Command, side_effect: Network }
  - { name: bash_unclassified, action_type: Command, side_effect: Network, approval_required: true }
command_classes:
  - action: bash
    arg: command
    default_to: bash_unclassified
    classes:
      - { to: bash_network, patterns: ["curl ", "wget "] }
`,
};

const committed = await load(COMMITTED, 'committed');
const fresh = await load(FRESH, 'fresh');

const REBUILD = `The committed artifact no longer matches the kernel. Rebuild and commit it:

    (cd crates/harness-wasm && wasm-pack build --target web --release)
    cp crates/harness-wasm/pkg/harness_wasm{_bg.wasm,.js} ${COMMITTED}/`;

// Anchor the "fresh" build to the source tree before trusting it as a reference.
// Without this the check compares two artifacts that can both be stale and calls
// that agreement — which is exactly what happened on the 0.2.2 version bump: the
// committed artifact and a `pkg/` left over from the previous build were both
// 0.2.1, and the check passed against a source tree that was not. CI always builds
// immediately beforehand so it never saw this; a developer running the script
// locally would have been told everything was fine.
// ...and against the clock, because the version only moves at release time. Within
// a release window a `pkg/` built before the latest kernel edit reports the same
// version as the source tree, so the version anchor alone still passes vacuously —
// which it did, on the very change that fixed finding #17. If any source the engine
// is compiled from is newer than the reference build, the reference is stale.
const ENGINE_SOURCES = [
  'crates/harness-types/src',
  'crates/compiler/src',
  'crates/compiler/assets',
  'crates/harness-preview/src',
  'crates/harness-wasm/src',
];
function newestMtime(dir) {
  const root = new URL(`../${dir}`, import.meta.url);
  let newest = 0;
  const walk = (u) => {
    for (const e of readdirSync(u, { withFileTypes: true })) {
      const child = new URL(`${e.name}${e.isDirectory() ? '/' : ''}`, u);
      if (e.isDirectory()) walk(child);
      else newest = Math.max(newest, statSync(child).mtimeMs);
    }
  };
  walk(new URL(`${dir}/`, new URL('../', import.meta.url)));
  return newest;
}
const builtAt = statSync(new URL(`../${FRESH}/harness_wasm_bg.wasm`, import.meta.url)).mtimeMs;
const newestSource = Math.max(...ENGINE_SOURCES.map(newestMtime));
if (newestSource > builtAt) {
  fail(
    `the reference build is stale — ${FRESH} predates the newest engine source by ` +
      `${Math.round((newestSource - builtAt) / 1000)}s`,
    `Rebuild it before comparing:\n\n    (cd crates/harness-wasm && wasm-pack build --target web --release)`,
  );
}

const workspaceVersion = readFileSync(
  new URL('../Cargo.toml', import.meta.url),
  'utf8',
).match(/^version = "([^"]+)"/m)?.[1];
if (!workspaceVersion) {
  fail('cannot read the workspace version from Cargo.toml');
}
if (fresh.version() !== workspaceVersion) {
  fail(
    `the reference build is stale — ${FRESH} reports ${fresh.version()}, Cargo.toml says ${workspaceVersion}`,
    `Rebuild it before comparing:\n\n    (cd crates/harness-wasm && wasm-pack build --target web --release)`,
  );
}

if (committed.version() !== fresh.version()) {
  fail(
    `version mismatch — committed says ${committed.version()}, source is ${fresh.version()}`,
    REBUILD,
  );
}

// The bundled default world is compiled into the artifact, so it drifts too.
if (committed.default_world() !== fresh.default_world()) {
  fail('the bundled default world manifest differs from the one in the source tree', REBUILD);
}

// Every case, including the default world the playground actually seeds with.
const cases = { ...CASES, default_world: fresh.default_world() };
for (const [name, yaml] of Object.entries(cases)) {
  const a = committed.preview(yaml);
  const b = fresh.preview(yaml);
  if (a !== b) {
    console.error(`\n  case "${name}" — committed vs source:`);
    console.error(`    committed: ${a.slice(0, 240)}`);
    console.error(`    source:    ${b.slice(0, 240)}`);
    fail(`the committed artifact answers "${name}" differently from the current kernel`, REBUILD);
  }
}

console.log(
  `✓ WASM freshness: committed artifact matches the kernel ` +
    `(v${fresh.version()}, ${Object.keys(cases).length} cases)`,
);
