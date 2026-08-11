#!/usr/bin/env node
// Guard for the five-package layout. Run: node npm/verify-packages.js
//
// Three things fail silently here and each lands on someone else's machine:
//
//   1. **Version skew.** The main package pins exact versions of its platform
//      packages. If they drift, npm resolves nothing and the CLI reports a missing
//      binary on a platform we do publish.
//   2. **A reintroduced install script.** Removing `postinstall` is the entire
//      security argument of this layout. A future convenience script would undo it
//      without anything else noticing.
//   3. **A platform we ship but do not build**, or build but do not ship — the
//      package.json list and the release matrix have to agree.

const fs = require('fs');
const path = require('path');

const npmDir = __dirname;
const repo = path.join(npmDir, '..');
const main = JSON.parse(fs.readFileSync(path.join(npmDir, 'package.json'), 'utf8'));
const workflow = fs.readFileSync(path.join(repo, '.github/workflows/release.yml'), 'utf8');
const problems = [];

// --- 1. no install scripts, anywhere -----------------------------------------
const INSTALL_HOOKS = ['preinstall', 'install', 'postinstall', 'prepare'];
const platformDirs = fs
  .readdirSync(path.join(npmDir, 'platforms'), { withFileTypes: true })
  .filter((e) => e.isDirectory())
  .map((e) => e.name);

for (const [label, pkg] of [
  ['ai2rules-harness', main],
  ...platformDirs.map((d) => {
    const p = JSON.parse(fs.readFileSync(path.join(npmDir, 'platforms', d, 'package.json'), 'utf8'));
    return [p.name, p];
  }),
]) {
  const found = INSTALL_HOOKS.filter((h) => pkg.scripts && pkg.scripts[h]);
  if (found.length) {
    problems.push(
      `${label} declares install script(s): ${found.join(', ')} — the no-install-script property is the point of this layout`
    );
  }
  if (pkg.dependencies && Object.keys(pkg.dependencies).length) {
    problems.push(`${label} has runtime dependencies: ${Object.keys(pkg.dependencies).join(', ')}`);
  }
}

// --- 2. versions in lockstep --------------------------------------------------
const optional = main.optionalDependencies || {};
for (const d of platformDirs) {
  const p = JSON.parse(fs.readFileSync(path.join(npmDir, 'platforms', d, 'package.json'), 'utf8'));
  if (p.version !== main.version) {
    problems.push(`${p.name} is ${p.version}, main package is ${main.version}`);
  }
  if (!optional[p.name]) {
    problems.push(`${p.name} exists but is not listed in optionalDependencies`);
  } else if (optional[p.name] !== main.version) {
    problems.push(
      `optionalDependencies pins ${p.name}@${optional[p.name]}, but the package is ${main.version}`
    );
  }
}
for (const name of Object.keys(optional)) {
  if (!platformDirs.some((d) => `ai2rules-harness-${d}` === name)) {
    problems.push(`optionalDependencies lists ${name}, which has no platforms/ directory`);
  }
}

// --- 3. every shipped platform is actually built -------------------------------
const built = new Set([...workflow.matchAll(/^\s*(?:-\s*)?target:\s*(\S+)/gm)].map((m) => m[1]));
for (const d of platformDirs) {
  const target = fs.readFileSync(path.join(npmDir, 'platforms', d, 'TARGET'), 'utf8').trim();
  if (!built.has(target)) {
    problems.push(`platforms/${d} wants ${target}, which release.yml does not build`);
  }
}
if (built.size === 0) problems.push('parsed no targets from release.yml — this guard is broken');

if (problems.length) {
  console.error('npm package layout is inconsistent:\n');
  for (const p of problems) console.error(`  - ${p}`);
  console.error('');
  process.exit(1);
}

console.log(
  `npm layout OK — ${platformDirs.length} platform packages at ${main.version}, no install scripts, ${built.size} targets built`
);
