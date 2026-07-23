/**
 * ai2rules-gate — OpenCode native-tool governance adapter (DECISIONS D34 / D35 / D36, E17.3).
 *
 * Hooks `tool.execute.before` and routes every OpenCode native tool call through the
 * REAL kernel via the `harness gate` wire ABI (D24) — the same engine that governs
 * Claude Code (`cc-hook`) and the MCP gateway. No policy, taint, or classification
 * logic lives here; this is plumbing only (the one-kernel / thin-adapter rule, D35).
 * The plugin sends the RAW OpenCode tool name — the kernel classifies bash shapes
 * from the world manifest's `command_classes` (D36).
 *
 *   ALLOW                          -> return  (OpenCode runs the tool)
 *   DENY / ABSENT / REPLAN / ASK   -> throw   (OpenCode aborts the call)
 *
 * OpenCode's `tool.execute.before` has no structured allow/deny/ask return channel, so
 * a block is a thrown error carrying the kernel's decision label ("DENY", "ABSENT", …).
 * `ASK` is surfaced as an explicit block in this slice (D35); pair with OpenCode's own
 * `permission` rules for an approval UX.
 *
 * Monotonic session taint is persisted in `.opencode/ai2rules-state.json`. Fail-open
 * (documented strategy): any adapter/process error logs a warning and ALLOWS, so a
 * broken gate never bricks a session — only an explicit kernel verdict blocks. A
 * process failure is never an outcome.
 *
 * Env:
 *   AI2RULES_WORLD    WorldManifest path (default docs/demos/opencode/opencode-world.yaml)
 *   AI2RULES_HARNESS  harness binary (default: target/release|debug/harness, then PATH)
 *   AI2RULES_MODE     "interactive" (default) | "background" -> context.mode
 *                     (the kernel collapses ASK->DENY in background)
 *   AI2RULES_DISABLE  "1" to bypass governance entirely
 */
import type { Plugin } from "@opencode-ai/plugin";
import { existsSync, readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { join, dirname } from "node:path";

export const Ai2rulesGate: Plugin = async ({ directory, $ }) => {
  const world =
    process.env.AI2RULES_WORLD ?? join(directory, "docs/demos/opencode/opencode-world.yaml");
  const statePath = join(directory, ".opencode", "ai2rules-state.json");
  const mode = process.env.AI2RULES_MODE === "background" ? "background" : "interactive";

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
        // Raw host tool name: classification is the kernel's job (D36).
        const req = {
          v: 1,
          tool: input.tool,
          arguments: output.args ?? {},
          context: {
            session_id: input.sessionID,
            mode,
            taint: tainted ? "tainted" : "clean",
            source_channel: "user_prompt",
            approval_token: null,
          },
        };

        const res = await $`${harness} gate --world ${world} < ${Buffer.from(JSON.stringify(req))}`
          .quiet()
          .nothrow();
        if (res.exitCode !== 0) return; // malformed/unreadable manifest -> fail-open
        const verdict = JSON.parse(res.stdout.toString());

        // Persist the kernel-computed monotonic taint for the next call; the
        // note carries the kernel's effective action (verdict.action, D36).
        if (verdict?.context?.taint === "tainted" && !tainted) {
          state[input.sessionID] = "tainted";
          state[`${input.sessionID}:cause`] = `tainted by ${input.tool} (${verdict?.action ?? input.tool})`;
          saveTaint(state);
        }

        if (verdict?.decision === "ALLOW") return; // let OpenCode run the tool
        throw new Error(
          `[ai2rules] ${verdict?.decision} (${verdict?.action ?? input.tool}): ${verdict?.reason ?? "blocked by governance"}`,
        );
      } catch (e: any) {
        // Re-throw our explicit governance blocks; fail-open on everything else.
        if (typeof e?.message === "string" && e.message.startsWith("[ai2rules] ")) throw e;
        console.warn(`[ai2rules] gate error (allowing): ${e?.message ?? e}`);
      }
    },
  };
};
