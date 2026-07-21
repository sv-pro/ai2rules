import { test } from "node:test";
import assert from "node:assert/strict";
import path from "node:path";
import { ProfileSchema, nameSchema, buildPrompt, assetReturnPath } from "./lib.js";

const profile = {
  assets_dir: "blog/src/assets",
  dims: [1376, 768] as [number, number],
  format: "jpg",
  jpeg_quality: 90,
  generation_directive: "GEN-DIRECTIVE",
  style: "STYLE-TEXT",
  reference_heroes: [],
};

test("buildPrompt embeds directive, style, concept, dims, and the save path", () => {
  const p = buildPrompt(profile, "a cyan gate", "/tmp/x/hero.png", [1376, 768]);
  assert.match(p, /GEN-DIRECTIVE/);
  assert.match(p, /STYLE: STYLE-TEXT/);
  assert.match(p, /SCENE: a cyan gate/);
  assert.match(p, /1376x768/);
  assert.match(p, /\/tmp\/x\/hero\.png/);
});

test("buildPrompt: no labels -> no-text rule; labels -> only-these rule", () => {
  assert.match(buildPrompt(profile, "s", "/o.png", [10, 10]), /No text of any kind/);
  const withLabels = buildPrompt(profile, "s", "/o.png", [10, 10], ["ALLOW", "DENY"]);
  assert.match(withLabels, /Only these short, legible labels may appear: ALLOW, DENY/);
});

test("nameSchema accepts kebab slugs that start with an alphanumeric", () => {
  for (const ok of ["permission-taint-gate", "a", "a1", "a-b-c", "9x"]) {
    assert.equal(nameSchema.safeParse(ok).success, true, ok);
  }
});

test("nameSchema rejects empty, leading hyphen, all-hyphens, uppercase, separators", () => {
  for (const bad of ["", "-foo", "---", "Foo", "a b", "a/b", "a.b"]) {
    assert.equal(nameSchema.safeParse(bad).success, false, bad);
  }
});

test("assetReturnPath: in-repo -> relative, out-of-repo -> absolute (never a ../ chain)", () => {
  assert.equal(
    assetReturnPath("/repo", "/repo/blog/src/assets/x.jpg"),
    path.join("blog", "src", "assets", "x.jpg"),
  );
  assert.equal(assetReturnPath("/repo", "/tmp/out/x.jpg"), "/tmp/out/x.jpg");
});

test("ProfileSchema validates a good profile and rejects a bad one", () => {
  assert.equal(ProfileSchema.safeParse(profile).success, true);
  const bad = { ...profile } as Record<string, unknown>;
  delete bad.dims;
  assert.equal(ProfileSchema.safeParse(bad).success, false);
});
