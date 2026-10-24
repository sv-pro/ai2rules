// Pure, side-effect-free helpers for hero-mcp — extracted so they can be unit-tested
// without starting the stdio server or shelling out to agy. index.ts wires these up.
import path from "node:path";
import { z } from "zod";

/** The blog "profile": asset dir + house-style preferences. Validated at load (#5). */
export const ProfileSchema = z.object({
  assets_dir: z.string(),
  dims: z.tuple([z.number(), z.number()]),
  format: z.string(),
  jpeg_quality: z.number(),
  generation_directive: z.string(),
  style: z.string(),
  reference_heroes: z.array(z.string()),
});
export type Profile = z.infer<typeof ProfileSchema>;

/** Asset slug: kebab-case, must START with an alphanumeric — no leading/only hyphens (#8). */
export const nameSchema = z
  .string()
  .regex(/^[a-z0-9][a-z0-9-]*$/, "kebab-case slug (must start with a letter or digit)");

/** Compose the agy task prompt: force generative art + baked house style + scene. */
export function buildPrompt(
  profile: Profile,
  concept: string,
  outPng: string,
  dims: [number, number],
  labels?: string[],
): string {
  const [w, h] = dims;
  const textRule =
    labels && labels.length
      ? `Only these short, legible labels may appear: ${labels.join(", ")}. No other text.`
      : "No text of any kind in the image.";
  return [
    profile.generation_directive,
    `STYLE: ${profile.style}`,
    `SCENE: ${concept}`,
    textRule,
    `Composition 16:9, roughly ${w}x${h}.`,
    `Save the result as a PNG at exactly this path: ${outPng} . Create only that one ` +
      `image file. When done, reply with just the path.`,
  ].join("\n\n");
}

/** Repo-relative path when the asset is inside the repo, else absolute — never a `../` chain (#6). */
export function assetReturnPath(repoRoot: string, outJpg: string): string {
  const rel = path.relative(repoRoot, outJpg);
  return rel.startsWith("..") || path.isAbsolute(rel) ? outJpg : rel;
}

// The environment `agy` inherits (finding #18). An allowlist, not a denylist: the
// caller's prompt is untrusted input driving an authenticated agent, and the
// process it launches has no business seeing this shell's cloud keys, registry
// tokens or CI secrets. Everything `agy` genuinely needs to find its config and
// its Google auth is named here; add more with HERO_AGY_ENV (comma-separated) if a
// future agy needs it, rather than widening the list silently.
export const AGY_ENV_ALLOWLIST = [
  "HOME",
  "PATH",
  "USER",
  "LOGNAME",
  "SHELL",
  "TERM",
  "TMPDIR",
  "LANG",
  "TZ",
];
export const AGY_ENV_PREFIXES = [
  "LC_",
  "XDG_",
  "GOOGLE_",
  "GEMINI_",
  "ANTIGRAVITY_",
  "AGY_",
];

/** Filter an environment down to what `agy` needs. Pure, so it is testable. */
export function agyEnv(
  source: NodeJS.ProcessEnv = process.env,
  extraNames = source.HERO_AGY_ENV ?? "",
): NodeJS.ProcessEnv {
  const extra = extraNames
    .split(",")
    .map((k) => k.trim())
    .filter(Boolean);
  const out: NodeJS.ProcessEnv = {};
  for (const [k, v] of Object.entries(source)) {
    if (v === undefined) continue;
    if (
      AGY_ENV_ALLOWLIST.includes(k) ||
      AGY_ENV_PREFIXES.some((p) => k.startsWith(p)) ||
      extra.includes(k)
    ) {
      out[k] = v;
    }
  }
  return out;
}
