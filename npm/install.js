#!/usr/bin/env node
// postinstall — fetch the prebuilt `harness` for this platform.
//
// The asset names, targets and archive formats here mirror
// `.github/workflows/release.yml` exactly. If that matrix changes, this map has
// to change with it; `npm/verify-targets.js` fails the build when they drift.
//
// Deliberate properties, because this runs on other people's machines as part of
// installing a *security* tool:
//
//   * One file, one host, and the URL derives from this package's version —
//     never from anything in the user's project.
//   * The SHA-256 is verified against the digest published beside the asset. A
//     missing or mismatched checksum is a hard failure, not a warning.
//   * On any failure it exits 0 with instructions rather than breaking the
//     install of whatever depends on it. `bin/harness.js` reports the missing
//     binary if and when someone actually runs it.
//
// Offline / air-gapped: set AI2RULES_HARNESS to an existing binary and nothing
// is downloaded at all.

const fs = require('fs');
const os = require('os');
const path = require('path');
const crypto = require('crypto');
const { spawnSync } = require('child_process');
const { pipeline } = require('stream/promises');

const REPO = 'sv-pro/ai2rules';
const VERSION = require('./package.json').version;
const VENDOR = path.join(__dirname, 'vendor');
const EXE = process.platform === 'win32' ? 'harness.exe' : 'harness';

// npm `${platform}-${arch}` -> what release.yml actually publishes.
// Linux is **musl** (statically linked, so it runs on any distro). There is no
// aarch64-linux build in the matrix yet; that platform correctly falls through
// to the build-from-source hint rather than downloading something wrong.
const TARGETS = {
  'linux-x64': { target: 'x86_64-unknown-linux-musl', archive: 'tar.gz' },
  'darwin-arm64': { target: 'aarch64-apple-darwin', archive: 'tar.gz' },
  'darwin-x64': { target: 'x86_64-apple-darwin', archive: 'tar.gz' },
  'win32-x64': { target: 'x86_64-pc-windows-msvc', archive: 'zip' },
};

function hint(reason) {
  console.error(`
ai2rules-harness: ${reason}

No prebuilt binary was installed. You can still build one:

    cargo install --git https://github.com/${REPO} cli-harness

then run \`harness init\` in a project. If you already have a binary:

    export AI2RULES_HARNESS=/absolute/path/to/harness
`);
}

async function download(url, dest) {
  const res = await fetch(url, { redirect: 'follow' });
  if (!res.ok) throw new Error(`GET ${url} -> HTTP ${res.status}`);
  await fs.promises.mkdir(path.dirname(dest), { recursive: true });
  await pipeline(res.body, fs.createWriteStream(dest));
}

async function fetchText(url) {
  const res = await fetch(url, { redirect: 'follow' });
  if (!res.ok) throw new Error(`GET ${url} -> HTTP ${res.status}`);
  return (await res.text()).trim();
}

function sha256(file) {
  return new Promise((resolve, reject) => {
    const h = crypto.createHash('sha256');
    fs.createReadStream(file)
      .on('data', (d) => h.update(d))
      .on('end', () => resolve(h.digest('hex')))
      .on('error', reject);
  });
}

// `tar` extracts both .tar.gz and .zip: GNU/bsdtar on Unix, and bsdtar has
// shipped as tar.exe on Windows 10+ since 1803. Avoids a dependency for a
// package whose whole point is that it installs cleanly.
function extract(archive, dir) {
  const res = spawnSync('tar', ['-xf', archive, '-C', dir], { stdio: 'pipe' });
  if (res.error) throw new Error(`cannot run tar: ${res.error.message}`);
  if (res.status !== 0) {
    throw new Error(`tar failed: ${res.stderr?.toString().trim() || `exit ${res.status}`}`);
  }
}

async function main() {
  const override = process.env.AI2RULES_HARNESS;
  if (override) {
    if (!fs.existsSync(override)) {
      hint(`AI2RULES_HARNESS points at ${override}, which does not exist.`);
      return;
    }
    console.log(`ai2rules-harness: using AI2RULES_HARNESS=${override}`);
    return;
  }

  const key = `${process.platform}-${process.arch}`;
  const spec = TARGETS[key];
  if (!spec) {
    hint(`no prebuilt binary for ${key} (${os.type()}).`);
    return;
  }

  const name = `harness-${spec.target}.${spec.archive}`;
  const url = `https://github.com/${REPO}/releases/download/v${VERSION}/${name}`;
  const tmp = await fs.promises.mkdtemp(path.join(os.tmpdir(), 'ai2rules-'));
  const archive = path.join(tmp, name);

  try {
    await download(url, archive);

    const expected = (await fetchText(`${url}.sha256`)).split(/\s+/)[0];
    const actual = await sha256(archive);
    if (!expected || expected !== actual) {
      throw new Error(`checksum mismatch\n  expected ${expected || '(none published)'}\n  actual   ${actual}`);
    }

    extract(archive, tmp);
    // The archive contains `harness-<target>/harness` — staged that way so it
    // has no nested target/.../release path.
    const unpacked = path.join(tmp, `harness-${spec.target}`, EXE);
    if (!fs.existsSync(unpacked)) throw new Error(`archive did not contain ${EXE}`);

    await fs.promises.mkdir(VENDOR, { recursive: true });
    await fs.promises.copyFile(unpacked, path.join(VENDOR, EXE));
    if (process.platform !== 'win32') await fs.promises.chmod(path.join(VENDOR, EXE), 0o755);

    console.log(`ai2rules-harness: installed ${spec.target} (sha256 ${actual.slice(0, 12)}…)`);
  } catch (err) {
    await fs.promises.rm(VENDOR, { recursive: true, force: true }).catch(() => {});
    hint(`could not install a prebuilt binary: ${err.message}`);
  } finally {
    await fs.promises.rm(tmp, { recursive: true, force: true }).catch(() => {});
  }
}

main().catch((err) => hint(String(err && err.message ? err.message : err)));
