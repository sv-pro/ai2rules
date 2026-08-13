# Changelog

Notable changes to the `harness` binary and the `ai2rules-harness` npm packages.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions
follow [semantic versioning](https://semver.org/spec/v2.0.0.html), with the caveat
that this is pre-1.0 software and the manifest schema is still moving.

Entries name the **finding number** or **decision record** behind a change where
there is one, so anything here can be traced to the reasoning in
[`DECISIONS.md`](DECISIONS.md) or [`docs/reviews/`](docs/reviews/).

## [Unreleased]

### Changed

- **A `command_classes` classifier must declare `default_to`, and an action may have
  only one classifier** (findings #19/#20, D62). Both are now compile errors.
  Pattern matching over shell strings is always evadable; `default_to` is the
  fail-closed bucket an evasion lands in, so its optionality was the real hole — a
  manifest without one let a tainted session run arbitrary unmatched shell. A second
  classifier for the same action silently never ran. **Breaking** for any manifest
  that omitted `default_to`; those are precisely the vulnerable ones, and the error
  names the field and the consequence.

## [0.2.2] — 2026-08-13

A security release. Everything in it came out of a full review of our own
repository, which found five live defects behind a green test suite, a clean
linter, and five passing CI jobs. Full write-up:
[`docs/reviews/2026-08-12-full-codebase-review.md`](docs/reviews/2026-08-12-full-codebase-review.md).

**Upgrade if you use path scope (`roots`), or run the harness anywhere its state
directory might not be writable.** Both failures below are silent by construction:
nothing in a session tells you the governance stopped applying.

### Fixed

- **An unrecordable taint escalation is no longer allowed** (finding #16, D59).
  Both host adapters discarded write errors when persisting the monotonic taint
  marker. With an unwritable state directory the escalation was recorded nowhere
  and every later call read back `clean` — so a `WebFetch` followed by
  `curl … -d @/etc/passwd` was permitted, and in `--grant` mode explicitly
  allowed, with the taint floor never engaging. The marker is now written durably
  and its failure fails **closed**, scoped to the single call that would escalate;
  the rest of the session keeps working. Failing to *reach* a decision still fails
  open, as documented.
- **`agy-hook` canonicalizes manifest roots** (finding #15). It resolved action
  paths through the filesystem but compared root rules as text, so a `Deny` root
  reached through a symlink stopped matching and fell through to the policy
  default. The same manifest and the same target file produced `deny` on Claude
  Code and `force_ask` on Antigravity — or an explicit `allow` into a
  `Deny`/`Credential` root under a permissive default.
- **`harness gate` resolves manifest roots** (finding #26, D61). The CLI compiled
  the manifest itself and never resolved roots, so relative rules, `~` rules and
  symlinked rules silently did not bind for anyone driving the wire ABI directly.
- **The committed WASM artifact matches the kernel again** (finding #18, D60). The
  browser playground's engine was nine preview-affecting commits stale and
  reporting version `0.0.1`. It exports `preview`, not `gate`, so this
  misdescribed the kernel rather than exposing it — a fidelity bug, not an
  exploitable one.

### Added

- `SECURITY.md` — reporting channel, and an explicit statement of what the harness
  does *not* protect against: it is advisory rather than a sandbox, command
  classification is a heuristic whose fail-closed default carries the weight, and
  `source_channel` is currently pinned to `user_prompt` on live hosts (#21).
- This changelog.
- **Path-scope conformance** (D61): `docs/demos/one-kernel/roots-world.yaml` and
  `roots-cases.yaml`, fourteen cases asserting identical verdicts across the
  in-process kernel, the `harness gate` CLI, and both host adapters. The previous
  case set contained no path cases at all, which is why #15 and #26 survived. One
  rule deliberately points at a symlink; both fixes were reverted in turn to
  confirm the tests fail without them.
- **A `wasm` CI job** that rebuilds the engine and requires the committed artifact
  to answer identically — same version, same bundled default world, same `preview`
  output over a case set including `roots` and `command_classes`. Semantic rather
  than byte comparison, so a toolchain bump does not produce a false failure.

### Changed

- Test count 254 → 259. A flaky `ETXTBSY` in the `init` suite is fixed (#25): it
  reproduced on roughly two of every five runs that had just recompiled — which is
  every CI run — while being nearly invisible locally.

### Known issues

Open findings, all documented in the review and now tracked as issues on
[`agentic-execution-governance`](https://github.com/sv-pro/agentic-execution-governance/issues):
classifier class ordering is first-match-wins and a permissive class listed first
wins on a chained command (#17); `source_channel` is pinned to `user_prompt` on live
hosts (#21); argument aliasing and schema validation are mutually exclusive (#27).
(#19 and #20 are fixed in Unreleased, above.)

## [0.2.1] — 2026-08-12

No user-facing change. The entire release is the publish path, and all three fixes
came from it failing in ways nothing else would have caught.

### Fixed

- npm dist-tags are derived from the version, so a prerelease can no longer become
  `latest`.
- `setup-node` no longer writes an empty-token `.npmrc`, which made npm present a
  broken credential instead of falling back to OIDC.
- Both publish steps skip already-published versions, so a partial failure can be
  retried.

### Changed

- First release published with **no credential of any kind**: OIDC trusted
  publishing, provenance signed automatically, no tokens on the account and no
  secrets in the repository. Verified by three release candidates.

## [0.2.0] — 2026-08-11

### Changed

- **The npm package ships per-platform binaries as optional dependencies and runs
  no install script** (D58). Installing previously ran a `postinstall` that
  fetched a binary over the network, unpacked it via `tar`, and `chmod`ed it
  executable — a step-by-step description of the thing scanners exist to catch.
  The binary now arrives as an `optionalDependencies` entry npm resolves by
  `os`/`cpu`, covered by the integrity hash already in your lockfile. No network,
  no shell, no `chmod` at install time.
- The Windows platform package is named `…-windows-x64`; `win32` in an unscoped
  name is rejected by npm's spam filter.

### Fixed

- npm publishing is idempotent after a partial failure.

## [0.1.2] — 2026-08-10

Three defects found by real use, all the same shape: the enforcement depending on
something the thing being enforced upon could reach.

### Fixed

- **The control plane is read-only.** The agent could rewrite the policy binding
  it.
- **The kill-switch moved out of the governed project.** It was self-disabling.
- The hook command is cross-host: no environment variable, no relative path, no
  double quotes.

### Changed

- Published unscoped on npm as `ai2rules-harness`, license texts included in the
  tarball.

## [0.1.1] — 2026-08-09

### Fixed

- Two `harness init` defects found in review: silent under-coverage, and writes
  through symlinks.

## [0.1.0] — 2026-08-08

First version a stranger can install and use: `harness init` writes a starter
manifest, the `PreToolUse` shim and the host settings entry, with nothing but the
binary — no checkout, no `cargo`, no `jq`.

[Unreleased]: https://github.com/sv-pro/ai2rules/compare/v0.2.2...HEAD
[0.2.2]: https://github.com/sv-pro/ai2rules/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/sv-pro/ai2rules/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/sv-pro/ai2rules/compare/v0.1.2...v0.2.0
[0.1.2]: https://github.com/sv-pro/ai2rules/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/sv-pro/ai2rules/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/sv-pro/ai2rules/releases/tag/v0.1.0
