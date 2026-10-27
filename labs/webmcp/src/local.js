// What every entry point (the runner, the demo, the live stand) needs from the
// machine it runs on: the local settings file and the kernel binary's version.
import path from 'node:path';
import { existsSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { harnessBin } from './defenses/d2-ai2rules.js';

const LAB_ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), '..');

// Local settings (API key, ports, binary path) from `labs/webmcp/.env`, which is
// gitignored; `.env.example` lists the names. A variable already set in the
// environment wins over the file. WEBMCP_NO_DOTENV=1 skips the file, so tests
// stay hermetic on a machine that has a real key in it.
export function loadLabEnv(file = path.join(LAB_ROOT, '.env')) {
  if (process.env.WEBMCP_NO_DOTENV === '1' || !existsSync(file)) return;
  const before = new Set(Object.keys(process.env));
  process.loadEnvFile(file);
  // A name left blank in the template (`WEBMCP_BASE_PORT=`) means "not set",
  // not "the empty string": drop it so the default applies.
  for (const [k, v] of Object.entries(process.env)) if (v === '' && !before.has(k)) delete process.env[k];
}

// `harness --version`, or null when the binary is not there.
export function harnessVersion() {
  try { return execFileSync(harnessBin(), ['--version']).toString().trim(); } catch { return null; }
}
