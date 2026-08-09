#!/usr/bin/env node
// Guard: the platform map in install.js must match what release.yml publishes.
//
// This exists because the two halves fail silently when they drift. A wrong
// target name means `npm i` 404s on a URL nobody tested; a wrong archive
// extension means it downloads something it cannot extract. Neither shows up in
// any test that does not compare these two files, and both land on a stranger.
//
// Run: node npm/verify-targets.js

const fs = require('fs');
const path = require('path');

const root = path.join(__dirname, '..');
const workflow = fs.readFileSync(path.join(root, '.github/workflows/release.yml'), 'utf8');
const installer = fs.readFileSync(path.join(__dirname, 'install.js'), 'utf8');

// What the workflow builds: every `target:` / `archive:` pair in the matrix.
const published = new Map();
let target = null;
for (const line of workflow.split('\n')) {
  const t = line.match(/^\s*(?:-\s*)?target:\s*(\S+)/);
  if (t) target = t[1];
  const a = line.match(/^\s*archive:\s*(\S+)/);
  if (a && target) {
    published.set(target, a[1]);
    target = null;
  }
}

// What the installer expects.
const expected = new Map();
const re = /'([\w]+-[\w]+)':\s*\{\s*target:\s*'([^']+)',\s*archive:\s*'([^']+)'/g;
let m;
while ((m = re.exec(installer)) !== null) expected.set(m[2], { platform: m[1], archive: m[3] });

const problems = [];
if (published.size === 0) problems.push('parsed no targets out of release.yml — the guard itself is broken');

for (const [tgt, { platform, archive }] of expected) {
  if (!published.has(tgt)) {
    problems.push(`install.js maps ${platform} -> ${tgt}, which release.yml does not build`);
  } else if (published.get(tgt) !== archive) {
    problems.push(
      `${tgt}: install.js expects .${archive}, release.yml publishes .${published.get(tgt)}`
    );
  }
}

// Not an error: the workflow may build a target npm has no mapping for (npm
// cannot express every platform). Report it so the gap is visible rather than
// discovered by a user on that platform.
const unmapped = [...published.keys()].filter((t) => !expected.has(t));

if (problems.length) {
  console.error('npm/install.js and .github/workflows/release.yml disagree:\n');
  for (const p of problems) console.error(`  - ${p}`);
  console.error('');
  process.exit(1);
}

console.log(`npm targets OK — ${expected.size} mapped, ${published.size} published`);
if (unmapped.length) console.log(`  (built but unmapped in npm: ${unmapped.join(', ')})`);
