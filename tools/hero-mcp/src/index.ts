#!/usr/bin/env node
/**
 * hero-mcp — a focused, governable MCP capability: generate ONE house-style blog
 * hero image. The caller supplies only the *scene* (`concept` + `name`); this
 * server bakes in the blog's asset dir, palette, 1376x768 dimensions, JPG output,
 * and the no-body-text rule from hero-profile.json.
 *
 * Backend: shells out to the already-authenticated `agy` (Antigravity) CLI —
 * `agy -p "<task>"` generates the image with the user's own Google auth, so this
 * server needs NO API key of its own. See _tasks/2_development/hero-mcp-server.md.
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

const execFileP = promisify(execFile);

type Profile = {
  assets_dir: string;
  dims: [number, number];
  format: string;
  jpeg_quality: number;
  generation_directive: string;
  style: string;
  reference_heroes: string[];
};

const profilePath =
  process.env.HERO_PROFILE ??
  fileURLToPath(new URL("../hero-profile.json", import.meta.url));
const profile: Profile = JSON.parse(readFileSync(profilePath, "utf8"));

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
// Ask the present human before running the agent (via MCP elicitation) unless disabled.
const elicitEnabled = (process.env.HERO_ELICIT ?? "auto") !== "off";
const [W, H] = profile.dims;

/** Compose the agy task prompt: force generative art + baked house style + scene. */
function buildPrompt(concept: string, outPng: string, labels?: string[]): string {
  const textRule =
    labels && labels.length
      ? `Only these short, legible labels may appear: ${labels.join(", ")}. No other text.`
      : "No text of any kind in the image.";
  return [
    profile.generation_directive,
    `STYLE: ${profile.style}`,
    `SCENE: ${concept}`,
    textRule,
    `Composition 16:9, roughly ${W}x${H}.`,
    `Save the result as a PNG at exactly this path: ${outPng} . Create only that one ` +
      `image file. When done, reply with just the path.`,
  ].join("\n\n");
}

/**
 * Drive `agy` to render the scene, then crop/resize to EXACTLY WxH JPG in the blog
 * assets dir. Returns the repo-relative asset path.
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
    const prompt = buildPrompt(concept, outPng, labels);
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
    return path.relative(repoRoot, outJpg);
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
  if (!elicitEnabled) return true;
  if (!server.server.getClientCapabilities()?.elicitation) return true; // no human gate
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
      name: z
        .string()
        .regex(/^[a-z0-9-]+$/, "kebab-case slug")
        .describe("kebab-case theme slug, e.g. permission-taint-gate"),
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
