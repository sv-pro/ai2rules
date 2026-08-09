#!/usr/bin/env node
// Thin launcher: exec the real Rust binary, pass through argv, exit with its code.
//
// No logic lives here. This file exists so `npx @ai2rules/harness init` works, and
// so a missing binary produces one clear sentence instead of a stack trace.

const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

const vendored = path.join(
  __dirname,
  '..',
  'vendor',
  process.platform === 'win32' ? 'harness.exe' : 'harness'
);
const bin = process.env.AI2RULES_HARNESS || vendored;

if (!fs.existsSync(bin)) {
  console.error(`@ai2rules/harness: no binary at ${bin}

The postinstall step did not install one for ${process.platform}-${process.arch}.
Build it, or point at an existing binary:

    cargo install --git https://github.com/sv-pro/ai2rules cli-harness
    export AI2RULES_HARNESS=/absolute/path/to/harness
`);
  process.exit(127);
}

const res = spawnSync(bin, process.argv.slice(2), { stdio: 'inherit' });
if (res.error) {
  console.error(`@ai2rules/harness: cannot execute ${bin}: ${res.error.message}`);
  process.exit(126);
}
process.exit(res.status === null ? 1 : res.status);
