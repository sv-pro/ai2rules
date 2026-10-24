#!/usr/bin/env node
// CI guard: no word jammed against an inline tag in the rendered pages.
//
// Astro, like JSX, drops the whitespace around a newline that sits between text
// and an element. So this, which looks correct in the editor:
//
//     no dependencies, and
//     <strong>no install script</strong>
//
// renders as "no dependencies, andno install script". It is invisible in review,
// invisible in the source, and only shows up by reading the built page — which is
// how it reached production on the front page.
//
// The fix in the source is an explicit `{' '}` at the end of the line. This checks
// the BUILT HTML, because that is where the defect exists.
//
// Run (from blog/): npm run build && npm run check:spacing

import { readFileSync, existsSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

const DIST = 'dist';
if (!existsSync(DIST)) {
  console.error("check:spacing: no dist — run 'npm run build' first");
  process.exit(1);
}

const INLINE = 'strong|em|code|a|span|b|i';
// A word character or sentence punctuation immediately before an inline tag.
// `(` and `[` are excluded: "(<a …>link</a>)" is correct.
const BEFORE = new RegExp(`([\\w,;:])(<(?:${INLINE})[\\s>])`, 'g');
// An inline tag closed straight into a word. Trailing punctuation is fine.
const AFTER = new RegExp(`(</(?:${INLINE})>)(\\w)`, 'g');

function htmlFiles(dir) {
  return readdirSync(dir).flatMap((e) => {
    const p = join(dir, e);
    return statSync(p).isDirectory() ? htmlFiles(p) : p.endsWith('.html') ? [p] : [];
  });
}

/** Only the prose region — nav, footer and head markup are generated. */
function prose(html) {
  const m = html.match(/<main[\s\S]*?<\/main>|<article[\s\S]*?<\/article>/);
  return m ? m[0] : '';
}

let failures = 0;
for (const file of htmlFiles(DIST).sort()) {
  const seg = prose(readFileSync(file, 'utf8'));
  if (!seg) continue;
  for (const [re, what] of [
    [BEFORE, 'no space before an inline tag'],
    [AFTER, 'no space after an inline tag'],
  ]) {
    re.lastIndex = 0;
    let m;
    while ((m = re.exec(seg)) !== null) {
      const ctx = seg
        .slice(Math.max(0, m.index - 50), m.index + m[0].length + 40)
        .replace(/\s+/g, ' ');
      console.error(`check:spacing: FAIL ${file} — ${what}\n    …${ctx}…`);
      failures++;
    }
  }
}

if (failures) {
  console.error(
    `\ncheck:spacing: ${failures} place(s) where words run together in the rendered page.` +
      `\nFix in the .astro source with an explicit {' '} at the end of the line before the tag.`,
  );
  process.exit(1);
}
console.log('✓ check:spacing: no words jammed against inline tags');
