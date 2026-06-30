/**
 * ai2rules-gate — OpenCode native-tool governance adapter (DECISIONS D34 / D35, E17.3).
 *
 * Hooks `tool.execute.before` and routes every OpenCode native tool call through the
 * REAL kernel via the `harness gate` wire ABI (D24) — the same engine that governs
 * Claude Code (`cc-hook`) and the MCP gateway. No policy or taint logic lives here;
 * this is plumbing only (the one-kernel / thin-adapter rule, D35).
 *
 *   ALLOW                          -> return  (OpenCode runs the tool)
 *   DENY / ABSENT / REPLAN / ASK   -> throw   (OpenCode aborts the call)
 *
 * OpenCode's `tool.execute.before` has no structured allow/deny/ask return channel, so
 * a block is a thrown error. `ASK` is surfaced as an explicit block in this first slice
 * (D35); pair with OpenCode's own `permission` rules for an approval UX.
 *
 * Monotonic session taint is persisted in `.opencode/ai2rules-state.json`. Fail-open:
 * any adapter/internal error logs a warning and ALLOWS, so a broken gate never bricks a
 * session (only an explicit kernel verdict blocks).
 *
 * Env:
 *   AI2RULES_WORLD    WorldManifest path (default docs/demos/opencode/opencode-world.yaml)
 *   AI2RULES_HARNESS  harness binary (default: target/release|debug/harness, then PATH)
 *   AI2RULES_DISABLE  "1" to bypass governance entirely
 */
import type { Plugin } from "@opencode-ai/plugin";
import { existsSync, readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { join, dirname } from "node:path";

// Host-syntactic Bash classification (D25): patterns, not policy. The policy for each
// resulting action lives in the world manifest.
const EGRESS = ["curl ", "wget ", "nc ", "ncat ", "telnet ", "ssh ", "scp ", "sftp "];
const DESTRUCTIVE = ["rm -rf", "rm -fr", "sudo ", "mkfs", "dd if=", ":(){"];

// True iff `pat` occurs in `cmd` at a LEFT word boundary. The patterns carry their own
// right boundary (a trailing space or "="), so "nc " matches "; nc x" but not "jsonc ".
// Mirrors the Rust `word_match` (cc_hook.rs) and Python `cmd_matches` (world-gate.py).
function wordMatch(cmd: string, pat: string): boolean {
  if (!pat) return false;
  for (let from = 0; ; ) {
    const i = cmd.indexOf(pat, from);
    if (i < 0) return false;
    const before = i === 0 ? "" : cmd[i - 1];
    if (before === "" || !/[A-Za-z0-9_]/.test(before)) return true;
    from = i + 1;
  }
}

function classify(tool: string, args: any): string {
  if (tool !== "bash") return tool;
  const cmd = String(args?.command ?? "");
  if (EGRESS.some((p) => wordMatch(cmd, p))) return "bash_network";
  if (DESTRUCTIVE.some((p) => wordMatch(cmd, p))) return "bash_destructive";
  return "bash";
}

export const Ai2rulesGate: Plugin = async ({ directory, $ }) => {
  const world =
    process.env.AI2RULES_WORLD ?? join(directory, "docs/demos/opencode/opencode-world.yaml");
  const statePath = join(directory, ".opencode", "ai2rules-state.json");

  const harness = (() => {
    if (process.env.AI2RULES_HARNESS) return process.env.AI2RULES_HARNESS;
    for (const c of ["target/release/harness", "target/debug/harness"]) {
      const p = join(directory, c);
      if (existsSync(p)) return p;
    }
    return "harness"; // on PATH
  })();

  const loadTaint = (): Record<string, string> => {
    try {
      return JSON.parse(readFileSync(statePath, "utf8"));
    } catch {
      return {};
    }
  };
  const saveTaint = (state: Record<string, string>) => {
    try {
      mkdirSync(dirname(statePath), { recursive: true });
      writeFileSync(statePath, JSON.stringify(state));
    } catch {
      /* fail-open */
    }
  };

  return {
    "tool.execute.before": async (input, output) => {
      if (process.env.AI2RULES_DISABLE === "1") return;
      try {
        const state = loadTaint();
        const tainted = state[input.sessionID] === "tainted";
        const action = classify(input.tool, output.args);
        const req = {
          v: 1,
          tool: action,
          arguments: output.args ?? {},
          context: {
            session_id: input.sessionID,
            mode: "interactive",
            taint: tainted ? "tainted" : null,
            source_channel: null,
            approval_token: null,
          },
        };

        const res = await $`${harness} gate --world ${world} < ${Buffer.from(JSON.stringify(req))}`
          .quiet()
          .nothrow();
        if (res.exitCode !== 0) return; // malformed/unreadable manifest -> fail-open
        const verdict = JSON.parse(res.stdout.toString());

        // Persist the kernel-computed monotonic taint for the next call.
        if (verdict?.context?.taint === "tainted" && !tainted) {
          state[input.sessionID] = "tainted";
          saveTaint(state);
        }

        if (verdict?.decision === "ALLOW") return; // let OpenCode run the tool
        throw new Error(
          `[ai2rules] ${verdict?.decision} (${action}): ${verdict?.reason ?? "blocked by governance"}`,
        );
      } catch (e: any) {
        // Re-throw our explicit governance blocks; fail-open on everything else.
        if (typeof e?.message === "string" && e.message.startsWith("[ai2rules] ")) throw e;
        console.warn(`[ai2rules] gate error (allowing): ${e?.message ?? e}`);
      }
    },
  };
};
