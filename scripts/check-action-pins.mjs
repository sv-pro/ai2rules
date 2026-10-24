#!/usr/bin/env node
// Two invariants about how this repository uses third-party GitHub Actions.
// Run from the repository root: node scripts/check-action-pins.mjs
//
//   1. EVERY action is pinned to a full 40-hex commit SHA. A tag is mutable —
//      `@v4` is whatever the owner last pointed it at, resolved at run time, in
//      workflows that hold `contents: write` and npm publish authority. That was
//      finding #25, and pinning fixed it; nothing stopped it coming back.
//
//   2. AN ACTION USED IN BOTH WORKFLOWS IS PINNED TO THE SAME SHA in both. This
//      is the one with teeth, and it is new (D70). `ci.yml`'s `release-dry-run`
//      job rehearses `release.yml`'s publish path, and a rehearsal that runs a
//      different version of an action than the performance is not a rehearsal.
//
//      It is not hypothetical. `release-dry-run` added four `upload-artifact`
//      occurrences and one `download-artifact` occurrence to `ci.yml`, while the
//      open Dependabot PRs bumping those actions touched `release.yml` only —
//      because they were opened first. Merging them as they stood would have left
//      the dry-run exercising v4 while the release ran v7, silently, with every
//      check green. The drift would have been invisible until a tag.
//
// Dependabot updates every occurrence in the repository when it rebases, so the
// correct response to a failure here is to rebase the PR, not to hand-edit one
// file into agreement.

import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';

const dir = path.join(process.cwd(), '.github/workflows');
const files = readdirSync(dir).filter((f) => f.endsWith('.yml') || f.endsWith('.yaml'));
if (files.length === 0) {
  console.error('no workflow files found — run from the repository root');
  process.exit(1);
}

const SHA = /^[0-9a-f]{40}$/;
const problems = [];
/** action name -> Map<sha, [where, …]> */
const pins = new Map();

for (const file of files) {
  const lines = readFileSync(path.join(dir, file), 'utf8').split('\n');
  lines.forEach((line, i) => {
    const m = line.match(/^\s*-?\s*uses:\s*([^\s@]+)@([^\s#]+)/);
    if (!m) return;
    const [, action, ref] = m;
    // A local composite action (./.github/actions/foo) has nothing to pin.
    if (action.startsWith('./')) return;
    const where = `${file}:${i + 1}`;

    if (!SHA.test(ref)) {
      problems.push(`${where}: ${action}@${ref} is not a 40-hex commit SHA — tags and branches are mutable`);
      return;
    }
    if (!pins.has(action)) pins.set(action, new Map());
    const byS = pins.get(action);
    if (!byS.has(ref)) byS.set(ref, []);
    byS.get(ref).push(where);
  });
}

for (const [action, byS] of pins) {
  if (byS.size <= 1) continue;
  const detail = [...byS.entries()]
    .map(([sha, wheres]) => `      ${sha.slice(0, 12)} at ${wheres.join(', ')}`)
    .join('\n');
  problems.push(
    `${action} is pinned to ${byS.size} different SHAs across the workflows:\n${detail}\n` +
      `      A bump must cover every occurrence. If this is a Dependabot PR, comment "@dependabot rebase".`
  );
}

if (problems.length) {
  console.error('workflow action pins are inconsistent:\n');
  for (const p of problems) console.error(`  - ${p}`);
  console.error('');
  process.exit(1);
}

const total = [...pins.values()].reduce((n, byS) => n + [...byS.values()].flat().length, 0);
console.log(`action pins OK — ${pins.size} actions, ${total} uses, all SHA-pinned and internally consistent`);
