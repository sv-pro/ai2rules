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

/**
 * `npm pack --dry-run --json` reports the exact file list, without publishing.
 *
 * Two output shapes are in the wild and this script has been burned by the
 * difference. npm <= 11 returns an ARRAY of entries; npm 12 returns an OBJECT
 * KEYED BY PACKAGE NAME. The release job installs `npm@latest` (trusted
 * publishing needs >= 11.5.1) while CI used whatever setup-node bundled, so the
 * rehearsal and the release parsed different shapes — and the v0.4.2-rc.1 npm
 * job failed on an npm 12 runner after every check had passed on npm 11.
 *
 * The parsing bug was the smaller half. The script reported the unreadable
 * output as "every file is missing from every package", which is a VERDICT about
 * the packages, when the truth was that it could not read the tool. Those are
 * different failures and only one of them is about the artifact. A guard that
 * cannot tell "the thing is broken" from "I could not inspect the thing" is not
 * a guard — it is the fail-open/fail-closed confusion this project wrote a whole
 * post about, pointed the other way. So an unrecognised shape now throws with
 * the npm version and the raw keys, rather than being scored as a defect.
 */
function packedFiles(dir) {
  const out = execFileSync('npm', ['pack', '--dry-run', '--json'], {
    cwd: dir,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  });

  let parsed;
  try {
    parsed = JSON.parse(out);
  } catch (e) {
    throw new Error(`cannot parse \`npm pack --json\` output from ${dir}: ${e.message}`);
  }

  const entries = Array.isArray(parsed) ? parsed : Object.values(parsed ?? {});
  const entry = entries.find((e) => e && typeof e === 'object' && Array.isArray(e.files));

  if (!entry || typeof entry.name !== 'string') {
    const npmV = execFileSync('npm', ['--version'], { encoding: 'utf8' }).trim();
    throw new Error(
      `unrecognised \`npm pack --json\` shape from ${dir} (npm ${npmV}).\n` +
        `      Expected an array of entries, or an object keyed by package name, each with name/version/files.\n` +
        `      Top-level was ${Array.isArray(parsed) ? `an array of ${parsed.length}` : `an object with keys: ${Object.keys(parsed ?? {}).join(', ') || '(none)'}`}.\n` +
        `      THIS IS AN INSTRUMENT FAILURE, NOT A VERDICT ABOUT THE PACKAGE.`
    );
  }

  const files = new Map();
  for (const f of entry.files) files.set(f.path, f.size);
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
