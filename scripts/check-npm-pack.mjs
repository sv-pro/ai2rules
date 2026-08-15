#!/usr/bin/env node
// Assert that the tarballs npm WOULD publish actually contain what they claim.
// Run from the repository root, after scripts/assemble-npm-packages.sh:
//
//   node scripts/check-npm-pack.mjs
//
// `npm/verify-packages.js` reads package.json files; this reads the packed
// output. The gap between them is real and silent: each package.json carries a
// `files` allowlist, and anything not on that list is dropped from the tarball
// without an error. A platform package whose `files` no longer names the binary
// publishes successfully, installs successfully, and fails at first use — on a
// version number that can never be republished.
//
// The .gitignore in each platform directory ignores `harness` and `LICENSE-*`
// (they are build outputs). npm falls back to .gitignore when a package declares
// neither `files` nor .npmignore — so deleting the `files` field, which looks
// like simplification, would publish four packages containing no binary at all.
// That is the specific accident this guards.

import { execFileSync } from 'node:child_process';
import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';

const repo = process.cwd();
const npmDir = path.join(repo, 'npm');

// A real `harness` build is several MB. Any plausible accident — an empty file,
// a placeholder, a text stub, a symlink that did not resolve — is far under this.
const MIN_BINARY_BYTES = 100_000;

const problems = [];

/** `npm pack --dry-run --json` reports the exact file list, without publishing. */
function packedFiles(dir) {
  const out = execFileSync('npm', ['pack', '--dry-run', '--json'], {
    cwd: dir,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  const parsed = JSON.parse(out);
  const entry = Array.isArray(parsed) ? parsed[0] : parsed;
  const files = new Map();
  for (const f of entry.files || []) files.set(f.path, f.size);
  return { name: entry.name, version: entry.version, files };
}

function require_(pkg, files, wanted, { minBytes = 0 } = {}) {
  if (!files.has(wanted)) {
    problems.push(`${pkg}: would publish WITHOUT ${wanted} (not in the packed tarball)`);
    return;
  }
  const size = files.get(wanted);
  if (size < minBytes) {
    problems.push(`${pkg}: ${wanted} is ${size} bytes, expected at least ${minBytes}`);
  }
}

// --- the four platform packages ------------------------------------------------
const platformDirs = readdirSync(path.join(npmDir, 'platforms'), { withFileTypes: true })
  .filter((e) => e.isDirectory())
  .map((e) => e.name);

for (const d of platformDirs) {
  const dir = path.join(npmDir, 'platforms', d);
  const target = readFileSync(path.join(dir, 'TARGET'), 'utf8').trim();
  const exe = target.includes('windows') ? 'harness.exe' : 'harness';
  const { name, version, files } = packedFiles(dir);

  require_(name, files, exe, { minBytes: MIN_BINARY_BYTES });
  require_(name, files, 'LICENSE-MIT');
  require_(name, files, 'LICENSE-APACHE');
  console.log(`  ${name}@${version}: ${files.size} files, ${exe} ${files.get(exe) ?? 0} bytes`);
}

// --- the wrapper ---------------------------------------------------------------
// It ships no binary — it resolves one through optionalDependencies — so what
// matters here is that the launcher and the license texts are present. A wrapper
// published without bin/harness.js installs fine and has no `harness` command.
{
  const { name, version, files } = packedFiles(npmDir);
  require_(name, files, 'bin/harness.js', { minBytes: 1 });
  require_(name, files, 'README.md', { minBytes: 1 });
  require_(name, files, 'LICENSE-MIT');
  require_(name, files, 'LICENSE-APACHE');
  console.log(`  ${name}@${version}: ${files.size} files (wrapper, no binary by design)`);
}

if (problems.length) {
  console.error('\nthe packages npm would publish are not the packages we think they are:\n');
  for (const p of problems) console.error(`  - ${p}`);
  console.error('');
  process.exit(1);
}

console.log(`\npack OK — ${platformDirs.length} platform packages + wrapper carry their payload`);
