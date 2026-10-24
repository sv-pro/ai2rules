#!/usr/bin/env node
// Launcher: find the prebuilt binary and exec it, passing argv through.
//
// There is no install script. The binary arrives as an `optionalDependencies`
// entry that npm resolves by `os`/`cpu` — exactly one matches, the rest are
// skipped — so installing this package performs **no network access, no shell
// execution, and no chmod**. That is not a score optimisation: it means the
// binary you run is covered by the integrity hash npm already wrote into your
// lockfile, rather than by a checksum this package fetched from the same host it
// fetched the binary from. The old design proved integrity; this one has
// provenance.
//
// Escape hatch for unsupported platforms and for development: set
// AI2RULES_HARNESS to an absolute path and it is used verbatim.

const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');

const PKG = {
  'linux-x64': 'ai2rules-harness-linux-x64',
  'darwin-x64': 'ai2rules-harness-darwin-x64',
  'darwin-arm64': 'ai2rules-harness-darwin-arm64',
  'win32-x64': 'ai2rules-harness-windows-x64', // `win32` in an unscoped
  // package name is rejected by npm's spam filter; the platform key is unchanged
};

function resolveBinary() {
  const override = process.env.AI2RULES_HARNESS;
  if (override) return { bin: override, how: 'AI2RULES_HARNESS' };

  const key = `${process.platform}-${process.arch}`;
  const pkg = PKG[key];
  if (!pkg) return { bin: null, why: `no prebuilt binary is published for ${key}` };

  const exe = process.platform === 'win32' ? 'harness.exe' : 'harness';
  try {
    // Resolve through the package's own manifest so this works with npm,
    // pnpm and yarn layouts alike rather than assuming node_modules is flat.
    const manifest = require.resolve(`${pkg}/package.json`);
    return { bin: path.join(path.dirname(manifest), exe), how: pkg };
  } catch {
    return {
      bin: null,
      why: `the optional dependency ${pkg} is not installed (it may have been skipped by --no-optional, or filtered by a lockfile from a different platform)`,
    };
  }
}

const { bin, why, how } = resolveBinary();

if (!bin || !fs.existsSync(bin)) {
  console.error(`ai2rules-harness: no usable binary — ${why || `${bin} is missing`}

Build one, or point at an existing binary:

    cargo install --git https://github.com/sv-pro/ai2rules cli-harness
    export AI2RULES_HARNESS=/absolute/path/to/harness
`);
  process.exit(127);
}

const res = spawnSync(bin, process.argv.slice(2), { stdio: 'inherit' });
if (res.error) {
  console.error(`ai2rules-harness: cannot execute ${bin} (via ${how}): ${res.error.message}`);
  process.exit(126);
}
// Propagate how the harness actually ended. When a child dies from a signal,
// `status` is null and `signal` holds the name — exiting 1 there would tell a
// caller "the harness failed" when the truth is "the user pressed Ctrl-C", and a
// script cannot tell those apart. The shell convention is 128 + the signal number.
if (res.signal) {
  const num = os.constants.signals[res.signal];
  process.exit(num ? 128 + num : 1);
}
process.exit(res.status === null ? 1 : res.status);
