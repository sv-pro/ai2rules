# Replace Claude's permission pile with one manifest

A Claude Code `settings.json` permission list is a flat pile of `allow`/`deny`/`ask`
strings. It grows unreviewably, and — the part that matters — it **cannot express
provenance**: it has no way to say "a `curl` the model proposed *after reading a web
page* is more dangerous than one you typed." A `WorldManifest` can.

This demo replaces the pile. Instead of maintaining `settings.json` rules, you set a
**minimal baseline** and let one PreToolUse hook — `harness cc-hook --grant` — be the
authoritative allow/deny/ask authority, driven by [`.claude/cc-world.yaml`](../../../.claude/cc-world.yaml).

## Overlay vs. replace — the one-flag difference

`cc-hook` is *additive* by default: on an ALLOW verdict it stays **silent** (exit 0) and
defers to Claude Code's normal permission flow. That's an **overlay** — it can only add
`deny`/`ask` on top of the native pile.

With `--grant` it emits an explicit `permissionDecision: "allow"`, which **grants** — Claude
Code skips its Allow/Deny prompt entirely. Combined with an emptied `settings.json`, the
manifest becomes the *whole* policy. That's **replace**.

> `allow` ≠ `defer`. Emitting `"allow"` *authorizes* (no prompt); exiting 0 silently only
> *permits-to-proceed* to the normal flow (which may still prompt). See the four upstream
> issues on this footgun: [#28812](https://github.com/anthropics/claude-code/issues/28812),
> [#52822](https://github.com/anthropics/claude-code/issues/52822),
> [#18312](https://github.com/anthropics/claude-code/issues/18312),
> [#39344](https://github.com/anthropics/claude-code/issues/39344).

## Run it (offline, no creds)

```bash
cargo build --offline                 # builds the `harness` binary
bash docs/demos/replace-permissions/demo.sh
```

Expected:

```
  clean Read               (manifest ALLOW -> grant)    -> allow
  clean curl example.com   (ALLOW -> grant; taints)     -> allow
  curl again, now TAINTED  (taint floor)                -> deny
  rm -rf /tmp/x            (destructive -> ask)          -> ask
  SomeUnknownTool          (not in manifest -> ABSENT)   -> deny
```

The headline: same `curl`, **allow then deny** — the only thing that changed is that the
session retrieved from the network in between. That is the taint floor doing what no
`settings.json` allowlist can, and **it still fires in replace mode.**

## Wire it into a real Claude Code project

Copy [`settings.baseline.json`](settings.baseline.json) to `.claude/settings.json`, replace
`/ABS/PATH`, and empty your `permissions.deny`/`ask`. The manifest is now your permission
system.

## Honest caveats (DYOR)

- **Native `deny`/`ask` still fire even when the hook grants** — so replace mode *requires*
  an emptied `settings.json` baseline, or the pile silently overrides the manifest.
- **The hook governs *calls*, not the *surface*.** It cannot make a tool `ABSENT` (remove it
  from the model's view) — only govern each call. True closed-ontology `ABSENT` still needs
  host-level tool pruning.
- **Some prompts can't be suppressed:** org-set connector `ask` and MCP tools marked
  `requiresUserInteraction` still prompt regardless of a hook `allow`.
- **Version-sensitive.** The grant/allow behavior has had bugs across Claude Code versions —
  pin a version and verify (see the issues above).
