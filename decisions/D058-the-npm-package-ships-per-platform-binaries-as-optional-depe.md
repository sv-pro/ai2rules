# D58 — The npm package ships per-platform binaries as optional dependencies, and runs no install script


**Date:** 2026-08-11. **Amends D56's packaging half.** Prompted by an outside signal —
Socket.dev scored `ai2rules-harness` **64% on supply chain security** — and the score was
correct.

- **The uncomfortable part first.** The package existed to distribute a governance kernel,
  and its own install did this: run a `postinstall`, `fetch()` a binary from the internet,
  unpack it with `spawnSync('tar')`, `chmod 0o755`, and leave it ready to execute. That is
  a close description of the attack class supply-chain scanners exist to catch. A scanner
  cannot tell our download from a hostile one and should not try. **The score was not a
  false positive; it was a correct reading of behaviour we had chosen.**
- **The real weakness underneath the score, which matters more than the number.** The
  checksum was fetched from the *same host, at the same moment,* as the artifact it
  verified. That proves the file arrived intact. It says nothing about whether it is the
  right file: anyone able to replace the binary could replace the digest beside it. **That
  is integrity, not provenance** — the distinction this project spends its time drawing
  everywhere else.
- **Decision.** Four platform packages — `ai2rules-harness-{linux-x64,darwin-x64,darwin-arm64,win32-x64}`
  — each containing one prebuilt binary and declaring `os`/`cpu`. The wrapper lists them in
  `optionalDependencies`; npm resolves exactly one and skips the rest. **The wrapper has no
  `scripts` block at all.** `bin/harness.js` resolves the binary at runtime via
  `require.resolve`, which works across npm/pnpm/yarn layouts.
  Consequences, in the order they matter: no install-time network, shell, or chmod; the
  binary is covered by the integrity hash npm writes into the *consumer's* lockfile; and
  publishing moves into CI with `--provenance`, signing each tarball against the workflow
  and commit that produced it. Installs also become reproducible, offline-cacheable, and
  usable behind a proxy or mirrored registry — none of which was true before.
- **A hole this restructure exposed rather than created.** With the binary in
  `node_modules`, `harness init` was baking a kernel path **inside the project it governs**.
  Measured: `Write` to that path returns ALLOW, and swapping in a no-op makes every verdict
  vanish. The same was true of `0.1.1`'s `vendor/harness`; nobody had looked. **`init` now
  refuses when the resolved binary is inside the target project** (`--force` overrides, for
  read-only mounts and immutable images). This is the third instance of one pattern —
  `gate-off`, the manifest, and now the kernel itself — and the pattern is worth stating
  once: **anything the enforcement depends on must live outside what it enforces upon.**
- **Not score-gaming, and the distinction is checkable.** Every change here removes a
  capability rather than hiding one. If the signals were suppressed but the behaviour kept,
  `npm/verify-packages.js` would still pass and the package would still fetch at install
  time. It does not, because there is no install script to fetch from.
- **Alternatives rejected.**
  - *Keep the `postinstall` and document it.* Documentation does not remove the capability,
    and the capability is the finding.
  - *Bundle all platforms' binaries in one package.* ~4 MB × 4 for every install, and every
    consumer downloads three binaries they cannot execute.
  - *Vendor the binary into the wrapper as base64.* Same size problem, plus it defeats
    npm's per-platform resolution and makes diffs unreadable.
  - *Publish under a scope for tidiness.* The `@ai2rules` org does not exist and creating
    one to hold five packages buys nothing; D56 already settled unscoped naming.
- **Known residual: five packages must version in lockstep**, and a skew resolves to
  nothing rather than failing loudly. `npm/verify-packages.js` fails CI on skew, on a
  reintroduced install script, and on a platform shipped but not built — the three failures
  that are otherwise silent and land on someone else's machine.
- **Related:** D56 (the wedge and its packaging residual), D37, D57 (the control plane —
  same pattern, one level down), `.github/workflows/release.yml` (the `npm` job),
  `npm/verify-packages.js`, and `crates/cli-harness/tests/init.rs`.
