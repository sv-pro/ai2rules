# D56 — `harness init`: the binary carries its own templates, so adoption needs no checkout


**Date:** 2026-08-09. **Executes `STRATEGY.md`'s first ranked bet** (the productization
wedge), deferred five times. Constrained by D37 (the shim holds no governance logic; the
governed project is untrusted) and by the `roots` primitive (#27, #28), which is what makes
a *generic* starter manifest worth installing at all.

- **Context.** `scripts/install-governance.sh` already governed a project in one command,
  and almost nobody could run it. It needed an **ai2rules checkout** for `--source`
  (templates), **`cargo` or a prebuilt binary** to install a kernel, and **`jq`** to merge
  `settings.json`. Those are three prerequisites in front of a pitch whose entire claim is
  *"kill one concrete fear in five minutes"*. The script is fine; the distribution was the
  product problem, and the strategy has said so since 2026-07-23.
- **Decision.** `harness init [TARGET]` is a first-class subcommand that governs a project
  using **nothing but the binary being invoked**. Three choices carry it, and each removes
  exactly one prerequisite:
  1. **The starter manifest is `include_str!`-ed into the executable.** No checkout.
  2. **The shim bakes `std::env::current_exe()`.** No separate install step — the trusted
     absolute path D37 requires is simply the binary the user just ran.
  3. **The settings merge is `serde_json`.** No `jq`.
  Flags: `--grant` (replace mode), `--force` (replace a tuned manifest), `--dry-run`.
- **The manifest is compiled before it is written, and this is not a nicety.** `init` runs
  the real compiler over the embedded template and writes nothing if it fails. A project
  whose thesis is that governance must be *checkable* does not get to install a manifest it
  never checked; shipping an unbuildable one to a stranger would be the exact failure this
  repo spends its time naming in other people's tools.
- **Idempotence is a security property here, not a convenience.** A duplicated `PreToolUse`
  entry runs the kernel twice per call and doubles latency, which is how a governance tool
  becomes the reason someone disables governance. Merging is keyed on the hook's `command`
  string, foreign hooks and unrelated settings keys are preserved untouched, and a tuned
  `cc-world.yaml` is never replaced without `--force` — losing that file is the worst thing
  this command could do, because it is the only artifact in a governed project that
  represents human judgement.
- **Alternatives rejected.**
  - *Keep improving the shell script.* It cannot remove its own prerequisites: the
    templates live in the checkout by construction, and installing a binary is a separate
    step no matter how the script is written.
  - *Fetch templates from GitHub at init time.* Rejected on the thesis. A governance tool
    that downloads its policy at install time makes the network a trust dependency of the
    trust boundary, and an offline machine is exactly where this should still work.
  - *Generate the manifest from the project (language detection, etc.).* Rejected for now:
    inference is how a deterministic tool acquires a stochastic dependency. A fixed,
    roots-confined starter that the user then tunes keeps the judgement human and visible.
  - *Have `init` install the binary onto `PATH` too.* Rejected — that is the per-machine
    half and belongs to the packaging layer (npm, brew, releases), not to a project-scoped
    command. `install-governance.sh` keeps that half.
- **✅ Residual closed 2026-08-10: the packaging half shipped.**
  [`ai2rules-harness`](https://www.npmjs.com/package/ai2rules-harness) **`0.1.1`** is on the
  public registry — unscoped, zero dependencies, with a `postinstall` that resolves a
  checksum-verified prebuilt from the `v0.1.1` GitHub release. Verified cold from the
  registry: `npx ai2rules-harness init` governs a fresh directory and returns a deny verdict
  for a write outside it. **The agent did not publish it** — a package name is a one-way
  door, so the `npm publish` was left to a human even after everything else was staged and
  the login was in place.
  **Unscoped rather than `@ai2rules/harness`, decided at publish time:** the `@ai2rules`
  org did not exist on npm, so the scoped name would have failed *after* login rather than
  before it, and creating an org to hold one package buys nothing. `npx ai2rules-harness
  init` reads the same.
- **New residual, found by running the published package: under `npx`, the shim bakes a
  path that can disappear.** `init` records the absolute path of the binary that ran it
  (that is what makes the trusted-path requirement of D37 free). Under `npx` that path is
  inside npm's transient `_npx` cache; when the cache is cleaned the binary is gone and the
  shim **fails open** — the host silently returns to its own permissions with no error.
  Fail-open is the correct behaviour for a missing kernel (D37) and this does not change it,
  but it makes `npx` the wrong *durable* install, so both READMEs now recommend
  `npm install -g` and describe `npx` as the way to try it. **The real fix, if this bites
  anyone: have the shim report a missing kernel once instead of failing silently** — but
  that trades a silent hole for a possible per-call warning, which is a design call and not
  a bug fix.
- **Related:** D24 (host-neutral gate ABI), D33/D37 (the cc-hook seam and the untrusted
  project directory), D47, `scripts/starter-world.yaml`, `docs/TUTORIAL.md`, `STRATEGY.md`
  (bet 1), and `crates/cli-harness/tests/init.rs` (14 tests — including that the embedded
  manifest cannot drift from the shipped one, and five regressions from the 2026-08-09
  review, each verified to fail against the pre-fix code).
