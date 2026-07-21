#!/usr/bin/env node
/**
 * hero-mcp — a focused, governable MCP capability: generate ONE house-style blog
 * hero image. The caller supplies only the *scene* (`concept` + `name`); this
 * server bakes in the blog's asset dir, palette, 1376x768 dimensions, JPG output,
 * and the no-body-text rule from hero-profile.json.
 *
 * Backend: shells out to the already-authenticated `agy` (Antigravity) CLI —
 * `agy -p "<task>"` generates the image with the user's own Google auth, so this
 * server needs NO API key of its own. Pure helpers live in ./lib.ts (tested).
 * See _tasks/2_development/hero-mcp-server.md.
 */
import { readFileSync, mkdirSync, existsSync, mkdtempSync, rmSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { execFile } from "node:child_process";
import os from "node:os";
import path from "node:path";
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import sharp from "sharp";
import { z } from "zod";
import {
  ProfileSchema,
  type Profile,
  nameSchema,
  buildPrompt,
  assetReturnPath,
} from "./lib.js";

const execFileP = promisify(execFile);

const profilePath =
  process.env.HERO_PROFILE ??
  fileURLToPath(new URL("../hero-profile.json", import.meta.url));
// Fail fast and clearly on a missing/malformed profile (#5).
const profile: Profile = (() => {
  try {
    return ProfileSchema.parse(JSON.parse(readFileSync(profilePath, "utf8")));
  } catch (err) {
    console.error(
      `hero-mcp: invalid or unreadable profile at ${profilePath}: ${(err as Error).message}`,
    );
    process.exit(1);
  }
})();

const repoRoot = process.env.HERO_REPO_ROOT ?? process.cwd();
const assetsDir = path.resolve(
  repoRoot,
  process.env.HERO_ASSETS_DIR ?? profile.assets_dir,
);
const agyBin = process.env.HERO_AGY_BIN ?? "agy";
// Permission posture: `--sandbox` fences the worst prompt-injection outcome — a crafted
// `concept` can't run arbitrary shell. `--dangerously-skip-permissions` is still needed
// so the non-interactive `-p` flow auto-approves agy's own image-gen tool (`--mode
// accept-edits` alone silently produces nothing). For an untrusted caller, run the whole
// thing inside the OS-level container sandbox (docker/). Override via HERO_AGY_FLAGS.
const agyFlags = (
  process.env.HERO_AGY_FLAGS ?? "--sandbox --dangerously-skip-permissions"
)
  .split(/\s+/)
  .filter(Boolean);
const rawTimeout = Number(process.env.HERO_AGY_TIMEOUT_MS);
const agyTimeoutMs =
  Number.isFinite(rawTimeout) && rawTimeout > 0 ? rawTimeout : 420000;
// Ask the present human before running the agent (via MCP elicitation).
//   off     — never ask
//   auto    — ask when the host can; proceed when it can't (sandbox is the backstop)
//   require — ask, and FAIL CLOSED when the host has no elicitation channel: with no
//             human to ask, ASK collapses to DENY (invariant-10 posture) — the right
//             default once this sits behind mcp-gateway for non-human callers.
const elicitMode = process.env.HERO_ELICIT ?? "auto";
const [W, H] = profile.dims;

/**
 * Drive `agy` to render the scene, then crop/resize to EXACTLY WxH JPG in the blog
 * assets dir. Returns the asset path (repo-relative if inside the repo, else absolute).
 */
async function generate(
  concept: string,
  name: string,
  labels?: string[],
): Promise<string> {
  // agy needs a workspace dir it may write to; use a throwaway temp dir.
  const workdir = mkdtempSync(path.join(os.tmpdir(), "hero-mcp-"));
  const outPng = path.join(workdir, `${name}.png`);
  try {
    const prompt = buildPrompt(profile, concept, outPng, [W, H], labels);
    // Backend: the already-authenticated agy (Antigravity) CLI. No API key here —
    // agy carries the user's Google auth. `-p` runs a single prompt non-interactively;
    // `agyFlags` set the permission posture (see above). Untrusted `concept` is a
    // prompt-injection surface, so we constrain agy rather than skip all permissions.
    try {
      await execFileP(
        agyBin,
        ["-p", prompt, ...agyFlags, "--add-dir", workdir],
        { cwd: workdir, timeout: agyTimeoutMs, maxBuffer: 64 * 1024 * 1024 },
      );
    } catch (err) {
      // agy may exit non-zero or overflow stdout yet still have written the image;
      // only treat it as a failure if the file really isn't there.
      if (!existsSync(outPng)) throw err;
    }
    if (!existsSync(outPng)) {
      throw new Error(`agy did not produce an image at ${outPng}`);
    }
    // Enforce the hard spec deterministically: exactly WxH JPG in the assets dir.
    mkdirSync(assetsDir, { recursive: true });
    const outJpg = path.join(assetsDir, `${name}.jpg`);
    await sharp(outPng)
      .resize(W, H, { fit: "cover" })
      .jpeg({ quality: profile.jpeg_quality })
      .toFile(outJpg);
    return assetReturnPath(repoRoot, outJpg);
  } finally {
    rmSync(workdir, { recursive: true, force: true });
  }
}

/**
 * Surface an ASK to the present human before running the agent, via MCP elicitation.
 * The caller's `concept` becomes the agent's prompt, so this is where untrusted input
 * gets a human glance. If the host can't elicit (or HERO_ELICIT=off), proceed — the
 * sandbox is still the backstop. This is the coarse, server-level version of `ASK`;
 * per-action forwarding from the agent is the next layer.
 */
async function confirmWithUser(
  server: McpServer,
  concept: string,
  name: string,
): Promise<boolean> {
  if (elicitMode === "off") return true;
  if (!server.server.getClientCapabilities()?.elicitation) {
    if (elicitMode === "require") {
      throw new Error(
        "HERO_ELICIT=require: the host has no elicitation channel, so there is no " +
          "human to ask — failing closed (ASK -> DENY). Use HERO_ELICIT=auto|off to proceed.",
      );
    }
    return true; // auto: no human gate available — the sandbox is the backstop
  }
  const result = await server.server.elicitInput({
    message:
      `Generate hero "${name}" by running the agy agent (sandboxed) on this concept — ` +
      `the concept becomes the agent's prompt:\n\n"${concept}"\n\nProceed?`,
    requestedSchema: {
      type: "object",
      properties: {
        proceed: {
          type: "boolean",
          title: "Proceed",
          description: "Run agy to generate this hero image?",
        },
      },
      required: ["proceed"],
    },
  });
  return result.action === "accept" && result.content?.proceed === true;
}

const server = new McpServer({ name: "hero-mcp", version: "0.3.0" });

server.registerTool(
  "generate_hero",
  {
    title: "Generate a blog hero image (house style, via agy)",
    description:
      "Generate one house-style hero image for the blog. Supply only the SCENE " +
      "(concept + a kebab-case name). Palette, 1376x768 dimensions, JPG output, the " +
      "asset directory, and the no-body-text rule are baked in. Backed by the " +
      "already-authenticated `agy` (Antigravity) CLI — no API key needed.",
    inputSchema: {
      concept: z
        .string()
        .describe("The scene to depict — subject/composition only, not palette or size."),
      name: nameSchema.describe("kebab-case theme slug, e.g. permission-taint-gate"),
      labels: z
        .array(z.string())
        .optional()
        .describe('Short, legible labels to allow, e.g. ["ALLOW","DENY"].'),
    },
  },
  async ({ concept, name, labels }) => {
    try {
      if (!(await confirmWithUser(server, concept, name))) {
        return {
          content: [
            {
              type: "text",
              text: "generate_hero: declined by the user; no image generated.",
            },
          ],
        };
      }
      const asset = await generate(concept, name, labels);
      return {
        content: [
          { type: "text", text: JSON.stringify({ asset, dims: `${W}x${H}` }, null, 2) },
        ],
      };
    } catch (err) {
      return {
        isError: true,
        content: [
          { type: "text", text: `generate_hero failed: ${(err as Error).message}` },
        ],
      };
    }
  },
);

const transport = new StdioServerTransport();
await server.connect(transport);
