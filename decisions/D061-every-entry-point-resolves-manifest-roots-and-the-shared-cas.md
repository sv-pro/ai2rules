# D61 — Every entry point resolves manifest roots, and the shared case set proves it


**Date:** 2026-08-13. **Completes D46/D59's path-scope story; prompted by finding #15.**

- **The gap the fix left behind.** D59 fixed `agy-hook` to canonicalize manifest roots the
  way `cc-hook` already did. What it did not fix is *why nobody noticed for as long as
  roots have existed*: `docs/demos/one-kernel/cases.yaml` had no path cases. The suite whose
  entire job is to prove the hosts agree never exercised the one feature where they
  disagreed. A parity harness only covers what you feed it, and this one was starved.
- **A second world, not roots bolted onto the first.** Enabling `roots` on
  `demo-world.yaml` changes the verdict of every path-shaped action arriving without a
  resolved target — its existing `read` case correctly becomes `missing_path` DENY, and the
  shell demo that asserts the old output breaks. One world cannot hold both stories, so
  path-scope gets `roots-world.yaml` + `roots-cases.yaml` and its own two parity tests.
- **The fixture is real and one rule is deliberately not canonical.** `{project}` is
  substituted with an actual temp directory containing actual files, because root policy is
  decided after canonicalization through the filesystem and a fixture of imaginary paths
  pins imaginary behaviour. One rule points at `{project}/link`, a symlink to
  `{project}/private`. That rule is the regression anchor: an entry point that resolves the
  action path but leaves the root lexical stops matching it, and its `Deny` degrades to the
  policy `default`. Verified by reverting each fix in turn — the adapter test fails with
  `path_scope_ask` where it expects `deny`, and the wire-ABI test fails on `manifest_hash`.
- **`harness gate` had the same hole (finding #26), so it is fixed here too.** It compiled
  the manifest itself and never resolved roots, so a caller of the documented wire ABI had
  nowhere to do it — relative rules, `~` rules and symlinked rules all silently stopped
  binding. It now performs the same resolve-then-canonicalize step as the two hooks. The
  kernel stays pure; this is the adapter half of the boundary, and it must not differ per
  entry point.
- **Schemas and cross-host portability are currently exclusive (finding #27).** The
  adapters alias host argument keys into the neutral vocabulary by *adding* keys
  (`AbsolutePath` → `path`, `CommandLine` → `command`), and schema validation rejects
  properties a schema does not declare — so an Antigravity call against a schema-bearing
  action returns `schema_violation` no matter how well-formed it is. `.agents/agy-world.yaml`
  already declares no schemas; `roots-world.yaml` matches, with the reason written down
  rather than left as folklore. Reconciling the two properly (alias-aware validation, or
  declaring the host keys) is open.
- **Alternatives rejected.**
  - *Add roots to `demo-world.yaml`.* Breaks the existing case set and the shell demo, for
    the reason above.
  - *Hardcode absolute fixture paths like `/etc` and `/etc/shadow`.* `/etc` is a symlink on
    macOS, so the fixture would itself be the thing under test. A generated temp tree is the
    only portable way to control canonicality.
  - *Make every root canonical so the world needs no resolution.* The test would pass on
    every entry point including the broken ones, which is precisely the failure being
    corrected.
  - *Expose `hostkit` as a library so the in-process leg can call it.* A crate-structure
    change to serve one test; the six-line stand-in is labelled as such instead.
- **Related:** D46, D48, D59; `docs/demos/one-kernel/roots-world.yaml`,
  `docs/demos/one-kernel/roots-cases.yaml`, and the two `*_path_scope` tests in
  `crates/cli-harness/tests/one_kernel.rs`.
